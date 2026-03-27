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
    let resolved = bl4::resolve::full_resolve(&req.serial)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to decode: {}", e)))?;

    if state.db.add_item(&req.serial).await.is_ok() {
        let _ = state.db.set_source(&req.serial, "community:decode").await;
    }

    let parts: Vec<PartInfo> = resolved
        .parts
        .iter()
        .map(|p| PartInfo {
            index: p.index,
            name: p.name.map(String::from),
            short_name: p.short_name.clone(),
            slot: p.slot.to_string(),
            is_element: p.is_element,
        })
        .collect();

    let string_tokens: Vec<StringToken> = resolved
        .strings
        .iter()
        .map(|t| StringToken {
            asset_path: t.asset_path.clone(),
            short_name: t.short_name.clone(),
        })
        .collect();

    Ok(Json(DecodeResponse {
        serial: req.serial,
        format: resolved.serial.format.to_string(),
        category: resolved.serial.item_type_description().to_string(),
        manufacturer: resolved.manufacturer.clone(),
        weapon_type: resolved.weapon_type.clone(),
        level: resolved.level.map(|l| l as u32),
        rarity: resolved.serial.rarity_name().map(String::from),
        element: resolved.serial.element_names(),
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
