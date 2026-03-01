use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::helpers::MAX_ATTACHMENT_SIZE;
use crate::schema::{CapabilitiesResponse, HealthResponse, StatsResponse};
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Service is healthy", body = HealthResponse)),
    tag = "System"
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[utoipa::path(
    get,
    path = "/capabilities",
    responses((status = 200, description = "Server capabilities", body = CapabilitiesResponse)),
    tag = "System"
)]
pub async fn get_capabilities() -> Json<CapabilitiesResponse> {
    Json(CapabilitiesResponse {
        version: env!("CARGO_PKG_VERSION"),
        attachments: true,
        max_attachment_size: MAX_ATTACHMENT_SIZE,
    })
}

#[utoipa::path(
    get,
    path = "/stats",
    responses((status = 200, description = "Database statistics", body = StatsResponse)),
    tag = "System"
)]
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StatsResponse>, (StatusCode, String)> {
    let stats = state
        .db
        .stats()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(StatsResponse {
        item_count: stats.item_count,
        part_count: stats.part_count,
        attachment_count: stats.attachment_count,
        value_count: stats.value_count,
    }))
}
