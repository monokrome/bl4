use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use bl4_idb::{generate_item_uuid, generate_random_uuid};
use uuid::Uuid;

use crate::db::Database;
use crate::schema::{
    BulkCreateRequest, BulkCreateResponse, CreateItemRequest, CreateItemResponse, ItemValueRequest,
};
use crate::state::AppState;

struct ValidItem {
    serial: String,
    item_type: String,
    source: String,
    uuid: Uuid,
    name: Option<String>,
    values: Vec<ItemValueRequest>,
}

struct BulkValidationResult {
    valid_items: Vec<ValidItem>,
    errors: Vec<CreateItemResponse>,
}

fn validate_bulk_items(
    items: Vec<CreateItemRequest>,
    batch_source: &str,
) -> BulkValidationResult {
    let mut valid_items = Vec::new();
    let mut errors = Vec::new();

    for item in items {
        let decoded = match bl4::ItemSerial::decode(&item.serial) {
            Ok(d) => d,
            Err(e) => {
                errors.push(CreateItemResponse {
                    serial: item.serial,
                    created: false,
                    message: format!("Invalid serial: {}", e),
                });
                continue;
            }
        };

        let (source, uuid) = match &item.source {
            Some(hashed_source) => {
                let uuid = generate_item_uuid(&item.serial, hashed_source);
                (hashed_source.clone(), uuid)
            }
            None => {
                let uuid = generate_random_uuid();
                (batch_source.to_owned(), uuid)
            }
        };

        valid_items.push(ValidItem {
            serial: item.serial,
            item_type: decoded.item_type_description().to_string(),
            source,
            uuid,
            name: item.name,
            values: item.values,
        });
    }

    BulkValidationResult { valid_items, errors }
}

async fn update_bulk_metadata(db: &Database, valid_items: &[ValidItem]) {
    let source_updates: Vec<(&str, &str)> = valid_items
        .iter()
        .map(|i| (i.serial.as_str(), i.source.as_str()))
        .collect();
    let _ = db.set_sources_bulk(&source_updates).await;

    let type_updates: Vec<(&str, &str)> = valid_items
        .iter()
        .map(|i| (i.serial.as_str(), i.item_type.as_str()))
        .collect();
    let _ = db.set_item_types_bulk(&type_updates).await;

    let uuid_strings: Vec<String> = valid_items.iter().map(|i| i.uuid.to_string()).collect();
    let uuid_values: Vec<(&str, &str, &str, &str, &str)> = valid_items
        .iter()
        .zip(uuid_strings.iter())
        .map(|(i, uuid_str)| {
            (
                i.serial.as_str(),
                "uuid",
                uuid_str.as_str(),
                "decoder",
                "verified",
            )
        })
        .collect();
    let _ = db.set_values_bulk(&uuid_values).await;

    let name_values: Vec<(&str, &str, &str, &str, &str)> = valid_items
        .iter()
        .filter_map(|i| {
            i.name.as_ref().map(|n| {
                (
                    i.serial.as_str(),
                    "name",
                    n.as_str(),
                    "community_tool",
                    "uncertain",
                )
            })
        })
        .collect();
    if !name_values.is_empty() {
        let _ = db.set_values_bulk(&name_values).await;
    }

    let all_values: Vec<(&str, &str, &str, &str, &str)> = valid_items
        .iter()
        .flat_map(|i| {
            i.values.iter().map(move |v| {
                (
                    i.serial.as_str(),
                    v.field.as_str(),
                    v.value.as_str(),
                    v.source.as_str(),
                    v.confidence.as_str(),
                )
            })
        })
        .collect();
    if !all_values.is_empty() {
        let _ = db.set_values_bulk(&all_values).await;
    }
}

#[utoipa::path(
    post,
    path = "/items/bulk",
    request_body = BulkCreateRequest,
    responses((status = 200, description = "Bulk creation results", body = BulkCreateResponse)),
    tag = "Items"
)]
pub async fn create_items_bulk(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BulkCreateRequest>,
) -> Result<Json<BulkCreateResponse>, (StatusCode, String)> {
    let batch_id = Uuid::new_v4().to_string();
    let batch_source = format!("community:{}", batch_id);

    let validation = validate_bulk_items(req.items, &batch_source);
    let mut results: Vec<CreateItemResponse> = validation.errors;
    let failed = results.len();
    let valid_items = validation.valid_items;

    let serials: Vec<&str> = valid_items.iter().map(|i| i.serial.as_str()).collect();
    let bulk_result = state
        .db
        .add_items_bulk(&serials)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let succeeded = valid_items.len();
    let new_items = bulk_result.succeeded;
    let duplicates = bulk_result.failed;

    update_bulk_metadata(&state.db, &valid_items).await;

    for item in valid_items {
        results.push(CreateItemResponse {
            serial: item.serial,
            created: true,
            message: format!("Item synced with uuid {}", item.uuid),
        });
    }

    tracing::info!(
        batch_id = %batch_id,
        total = succeeded,
        new_items = new_items,
        duplicates = duplicates,
        "Bulk publish completed"
    );

    Ok(Json(BulkCreateResponse {
        batch_id,
        succeeded,
        failed,
        results,
    }))
}
