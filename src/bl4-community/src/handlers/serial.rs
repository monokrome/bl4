use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::schema::{
    DecodeRequest, DecodeResponse, EncodeRequest, EncodeResponse, PartInfo, StringToken,
};
use crate::state::AppState;

#[utoipa::path(
    post,
    path = "/decode",
    request_body = DecodeRequest,
    responses(
        (status = 200, description = "Successfully decoded", body = DecodeResponse),
        (status = 400, description = "Invalid serial format")
    ),
    tag = "Serial"
)]
pub async fn decode_serial(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DecodeRequest>,
) -> Result<Json<DecodeResponse>, (StatusCode, String)> {
    let item = bl4::ItemSerial::decode(&req.serial)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to decode: {}", e)))?;

    if state.db.add_item(&req.serial).await.is_ok() {
        let _ = state.db.set_source(&req.serial, "community:decode").await;
    }

    let (manufacturer, weapon_type) = if let Some(mfg_id) = item.manufacturer {
        bl4::parts::weapon_info_from_first_varint(mfg_id)
            .map(|(m, w)| (Some(m.to_string()), Some(w.to_string())))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    let level = item
        .level
        .and_then(bl4::parts::level_from_code)
        .map(|(capped, _)| capped as u32);

    let rarity = item.rarity_name().map(String::from);
    let element = item.element_names();

    let parts: Vec<PartInfo> = item
        .resolved_parts()
        .into_iter()
        .map(|p| PartInfo {
            index: p.index,
            name: p.name.map(String::from),
            short_name: p.short_name,
            slot: p.slot.to_string(),
            is_element: p.is_element,
        })
        .collect();

    let string_tokens: Vec<StringToken> = item
        .string_tokens()
        .into_iter()
        .map(|t| StringToken {
            asset_path: t.asset_path,
            short_name: t.short_name,
        })
        .collect();

    Ok(Json(DecodeResponse {
        serial: req.serial,
        format: item.format.to_string(),
        category: item.item_type_description().to_string(),
        manufacturer,
        weapon_type,
        level,
        rarity,
        element,
        parts,
        string_tokens,
    }))
}

#[utoipa::path(
    post,
    path = "/encode",
    request_body = EncodeRequest,
    responses(
        (status = 200, description = "Successfully encoded", body = EncodeResponse),
        (status = 400, description = "Invalid serial format")
    ),
    tag = "Serial"
)]
pub async fn encode_serial(
    Json(req): Json<EncodeRequest>,
) -> Result<Json<EncodeResponse>, (StatusCode, String)> {
    let item = bl4::ItemSerial::decode(&req.serial)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to decode: {}", e)))?;

    let encoded = item.encode();
    let matches = encoded == req.serial;

    Ok(Json(EncodeResponse {
        original: req.serial,
        encoded,
        matches,
    }))
}
