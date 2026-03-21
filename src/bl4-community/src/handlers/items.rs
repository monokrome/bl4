use std::sync::Arc;

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use bl4_idb::{generate_item_uuid, generate_random_uuid, Confidence, ItemFilter, ValueSource};

use crate::schema::{
    CreateItemRequest, CreateItemResponse, DecodedValues, ItemResponse, ListItemsQuery,
    ListItemsResponse,
};
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/items",
    params(ListItemsQuery),
    responses((status = 200, description = "List of items", body = ListItemsResponse)),
    tag = "Items"
)]
pub async fn list_items(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListItemsQuery>,
) -> Result<Json<ListItemsResponse>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let filter = ItemFilter {
        manufacturer: query.manufacturer.clone(),
        weapon_type: query.weapon_type.clone(),
        element: query.element.clone(),
        rarity: query.rarity.clone(),
        limit: Some(limit),
        offset: Some(offset),
    };

    let count_filter = ItemFilter {
        manufacturer: query.manufacturer,
        weapon_type: query.weapon_type,
        element: query.element,
        rarity: query.rarity,
        limit: None,
        offset: None,
    };

    let total = state
        .db
        .count_items(&count_filter)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items = state
        .db
        .list_items(&filter)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let item_responses: Vec<ItemResponse> = items
        .into_iter()
        .map(|item| ItemResponse {
            serial: item.serial,
            name: item.name,
            prefix: item.prefix,
            manufacturer: item.manufacturer,
            weapon_type: item.weapon_type,
            item_type: item.item_type,
            rarity: item.rarity,
            level: item.level,
            element: item.element,
            verification_status: item.verification_status.to_string(),
            source: item.source,
            decoded: None,
        })
        .collect();

    Ok(Json(ListItemsResponse {
        items: item_responses,
        total: total as usize,
        limit,
        offset,
    }))
}

#[utoipa::path(
    get,
    path = "/items/{serial}",
    params(("serial" = String, Path, description = "Item serial code")),
    responses(
        (status = 200, description = "Item found", body = ItemResponse),
        (status = 404, description = "Item not found")
    ),
    tag = "Items"
)]
pub async fn get_item(
    State(state): State<Arc<AppState>>,
    AxumPath(serial): AxumPath<String>,
) -> Result<Json<ItemResponse>, (StatusCode, String)> {
    let item = state
        .db
        .get_item(&serial)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Item not found: {}", serial)))?;

    let best_values = state
        .db
        .get_best_values(&item.serial)
        .await
        .unwrap_or_default();

    let decoded = if best_values.is_empty() {
        None
    } else {
        Some(DecodedValues {
            name: best_values.get("name").cloned(),
            parts: best_values.get("parts").cloned(),
            confidence: best_values.get("confidence").cloned(),
        })
    };

    Ok(Json(ItemResponse {
        serial: item.serial,
        name: item.name,
        prefix: item.prefix,
        manufacturer: item.manufacturer,
        weapon_type: item.weapon_type,
        item_type: item.item_type,
        rarity: item.rarity,
        level: item.level,
        element: item.element,
        verification_status: item.verification_status.to_string(),
        source: item.source,
        decoded,
    }))
}

#[utoipa::path(
    post,
    path = "/items",
    request_body = CreateItemRequest,
    responses(
        (status = 201, description = "Item created", body = CreateItemResponse),
        (status = 400, description = "Invalid serial"),
        (status = 409, description = "Item already exists")
    ),
    tag = "Items"
)]
pub async fn create_item(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateItemRequest>,
) -> Result<(StatusCode, Json<CreateItemResponse>), (StatusCode, String)> {
    let decoded = bl4::ItemSerial::decode(&req.serial)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid serial: {}", e)))?;

    if state
        .db
        .get_item(&req.serial)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some()
    {
        return Err((StatusCode::CONFLICT, "Item already exists".into()));
    }

    state
        .db
        .add_item(&req.serial)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (source, item_uuid) = match &req.source {
        Some(hashed_source) => {
            let uuid = generate_item_uuid(&req.serial, hashed_source);
            (hashed_source.clone(), uuid)
        }
        None => {
            let uuid = generate_random_uuid();
            (format!("community:{}", uuid), uuid)
        }
    };

    state
        .db
        .set_source(&req.serial, &source)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let _ = state
        .db
        .set_value(
            &req.serial,
            "uuid",
            &item_uuid.to_string(),
            ValueSource::Decoder,
            None,
            Confidence::Verified,
        )
        .await;

    if let Some(name) = &req.name {
        state
            .db
            .set_value(
                &req.serial,
                "name",
                name,
                ValueSource::CommunityTool,
                req.source.as_deref(),
                Confidence::Uncertain,
            )
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    state
        .db
        .set_item_type(&req.serial, decoded.item_type_description())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateItemResponse {
            serial: req.serial,
            created: true,
            message: format!("Item created with uuid {}", item_uuid),
        }),
    ))
}
