use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Json, Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::handlers;
use crate::helpers::MAX_ATTACHMENT_SIZE;
use crate::rate_limit;
use crate::schema::*;
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "BL4 Community API",
        description = "REST API for Borderlands 4 items database",
        version = "0.1.0",
        license(name = "BSD-2-Clause"),
    ),
    paths(
        handlers::system::health,
        handlers::system::get_capabilities,
        handlers::items::list_items,
        handlers::items::get_item,
        handlers::attachments::upload_attachment,
        handlers::items::create_item,
        handlers::bulk::create_items_bulk,
        handlers::serial::decode_serial,
        handlers::serial::encode_serial,
        handlers::system::get_stats,
    ),
    components(schemas(
        HealthResponse,
        CapabilitiesResponse,
        ItemResponse,
        DecodedValues,
        ListItemsQuery,
        ListItemsResponse,
        CreateItemRequest,
        CreateItemResponse,
        BulkCreateRequest,
        BulkCreateResponse,
        DecodeRequest,
        DecodeResponse,
        EncodeRequest,
        EncodeResponse,
        PartInfo,
        StringToken,
        StatsResponse,
        AttachmentUploadResponse,
    ))
)]
struct ApiDoc;

pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let write_routes = Router::new()
        .route("/items", post(handlers::items::create_item))
        .route("/items/bulk", post(handlers::bulk::create_items_bulk))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_writes,
        ));

    let attachment_routes = Router::new()
        .route(
            "/items/{serial}/attachments",
            post(handlers::attachments::upload_attachment)
                .layer(DefaultBodyLimit::max(MAX_ATTACHMENT_SIZE)),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::require_allowed_ip,
        ));

    let read_routes = Router::new()
        .route("/health", get(handlers::system::health))
        .route("/capabilities", get(handlers::system::get_capabilities))
        .route("/items", get(handlers::items::list_items))
        .route("/items/{serial}", get(handlers::items::get_item))
        .route("/decode", post(handlers::serial::decode_serial))
        .route("/encode", post(handlers::serial::encode_serial))
        .route("/stats", get(handlers::system::get_stats));

    let api_routes = read_routes
        .merge(write_routes)
        .merge(attachment_routes)
        .merge(Scalar::with_url("/scalar", ApiDoc::openapi()))
        .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
        .with_state(state)
        .layer(cors);

    Router::new()
        .route(
            "/",
            axum::routing::options(|| async { Json(ApiDoc::openapi()) }),
        )
        .merge(api_routes)
        .layer(TraceLayer::new_for_http())
}
