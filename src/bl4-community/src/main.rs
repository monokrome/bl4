//! BL4 Community API Server
//!
//! REST API for the Borderlands 4 items database, allowing community
//! contributions and item lookups.

use std::sync::Arc;

use axum::{
    extract::{Multipart, Path as AxumPath, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use bl4_idb::{
    AsyncAttachmentsRepository, AsyncItemsRepository, Confidence, ItemFilter, SqlxSqliteDb,
    ValueSource,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_scalar::{Scalar, Servable};
use uuid::Uuid;

// =============================================================================
// CLI
// =============================================================================

#[derive(Parser)]
#[command(name = "bl4-community")]
#[command(about = "Community API server for Borderlands 4 items database")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start the API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3030")]
        port: u16,

        /// Database path
        #[arg(short, long, env = "DATABASE_URL", default_value = "share/items.db")]
        database: String,

        /// Bind address
        #[arg(short, long, default_value = "0.0.0.0")]
        bind: String,
    },
}

// =============================================================================
// App State
// =============================================================================

pub struct AppState {
    pub db: SqlxSqliteDb,
}

// =============================================================================
// OpenAPI Schema
// =============================================================================

#[derive(OpenApi)]
#[openapi(
    info(
        title = "BL4 Community API",
        description = "REST API for Borderlands 4 items database",
        version = "0.1.0",
        license(name = "BSD-2-Clause"),
    ),
    paths(
        health,
        get_capabilities,
        list_items,
        get_item,
        upload_attachment,
        create_item,
        create_items_bulk,
        decode_serial,
        encode_serial,
        get_stats,
    ),
    components(schemas(
        HealthResponse,
        CapabilitiesResponse,
        ItemResponse,
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
        StatsResponse,
        AttachmentUploadResponse,
    ))
)]
struct ApiDoc;

// =============================================================================
// Types
// =============================================================================

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ListItemsQuery {
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub element: Option<String>,
    pub rarity: Option<String>,
    #[param(default = 100, maximum = 1000)]
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ItemResponse {
    pub serial: String,
    pub name: Option<String>,
    pub prefix: Option<String>,
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub item_type: Option<String>,
    pub rarity: Option<String>,
    pub level: Option<i32>,
    pub element: Option<String>,
    pub verification_status: String,
    pub source: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListItemsResponse {
    pub items: Vec<ItemResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateItemRequest {
    pub serial: String,
    pub name: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateItemResponse {
    pub serial: String,
    pub created: bool,
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkCreateRequest {
    pub items: Vec<CreateItemRequest>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkCreateResponse {
    /// Unique batch ID assigned to all items in this upload
    pub batch_id: String,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<CreateItemResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DecodeRequest {
    pub serial: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PartInfo {
    pub index: u32,
    pub category: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DecodeResponse {
    pub serial: String,
    pub item_type: String,
    pub item_type_name: String,
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub level: Option<u32>,
    pub rarity: Option<String>,
    pub element: Option<String>,
    pub parts: Vec<PartInfo>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct EncodeRequest {
    pub serial: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EncodeResponse {
    pub original: String,
    pub encoded: String,
    pub matches: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StatsResponse {
    pub item_count: i64,
    pub part_count: i64,
    pub attachment_count: i64,
    pub value_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CapabilitiesResponse {
    pub version: &'static str,
    pub attachments: bool,
    pub max_attachment_size: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AttachmentUploadResponse {
    pub id: i64,
    pub name: String,
    pub mime_type: String,
}

// =============================================================================
// Handlers
// =============================================================================

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Service is healthy", body = HealthResponse)),
    tag = "System"
)]
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Maximum attachment size: 10MB
const MAX_ATTACHMENT_SIZE: usize = 10 * 1024 * 1024;

#[utoipa::path(
    get,
    path = "/capabilities",
    responses((status = 200, description = "Server capabilities", body = CapabilitiesResponse)),
    tag = "System"
)]
async fn get_capabilities() -> Json<CapabilitiesResponse> {
    Json(CapabilitiesResponse {
        version: env!("CARGO_PKG_VERSION"),
        attachments: true,
        max_attachment_size: MAX_ATTACHMENT_SIZE,
    })
}

/// OPTIONS handler returns OpenAPI schema for API discovery
async fn options_schema() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[utoipa::path(
    get,
    path = "/stats",
    responses((status = 200, description = "Database statistics", body = StatsResponse)),
    tag = "System"
)]
async fn get_stats(
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

#[utoipa::path(
    get,
    path = "/items",
    params(ListItemsQuery),
    responses((status = 200, description = "List of items", body = ListItemsResponse)),
    tag = "Items"
)]
async fn list_items(
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
async fn get_item(
    State(state): State<Arc<AppState>>,
    AxumPath(serial): AxumPath<String>,
) -> Result<Json<ItemResponse>, (StatusCode, String)> {
    let item = state
        .db
        .get_item(&serial)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Item not found: {}", serial)))?;

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
    }))
}

#[utoipa::path(
    post,
    path = "/items/{serial}/attachments",
    request_body(content_type = "multipart/form-data"),
    params(
        ("serial" = String, Path, description = "Item serial")
    ),
    responses(
        (status = 201, description = "Attachment uploaded", body = AttachmentUploadResponse),
        (status = 400, description = "Invalid file or missing data"),
        (status = 404, description = "Item not found"),
        (status = 413, description = "File too large")
    ),
    tag = "Attachments"
)]
async fn upload_attachment(
    State(state): State<Arc<AppState>>,
    AxumPath(serial): AxumPath<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<AttachmentUploadResponse>), (StatusCode, String)> {
    // Verify item exists
    state
        .db
        .get_item(&serial)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Item not found: {}", serial)))?;

    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let mut view = "OTHER".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                mime_type = field.content_type().map(|s| s.to_string());
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

                if data.len() > MAX_ATTACHMENT_SIZE {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        format!(
                            "File too large: {} bytes (max {})",
                            data.len(),
                            MAX_ATTACHMENT_SIZE
                        ),
                    ));
                }

                file_data = Some(data.to_vec());
            }
            "view" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
                if matches!(text.as_str(), "POPUP" | "DETAIL" | "OTHER") {
                    view = text;
                }
            }
            _ => {}
        }
    }

    let data = file_data.ok_or((StatusCode::BAD_REQUEST, "No file provided".to_string()))?;
    let name = file_name.unwrap_or_else(|| "attachment".to_string());
    let mime = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());

    // Validate mime type (only images)
    if !mime.starts_with("image/") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Only image files are allowed".to_string(),
        ));
    }

    let id = state
        .db
        .add_attachment(&serial, &name, &mime, &data, &view)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(AttachmentUploadResponse {
            id,
            name,
            mime_type: mime,
        }),
    ))
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
async fn create_item(
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

    // Generate unique source ID for this upload
    let upload_id = Uuid::new_v4().to_string();
    let source = format!("community:{}", upload_id);

    state
        .db
        .set_source(&req.serial, &source)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
        .set_item_type(&req.serial, &decoded.item_type.to_string())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateItemResponse {
            serial: req.serial,
            created: true,
            message: format!("Item created with source community:{}", upload_id),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/items/bulk",
    request_body = BulkCreateRequest,
    responses((status = 200, description = "Bulk creation results", body = BulkCreateResponse)),
    tag = "Items"
)]
async fn create_items_bulk(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BulkCreateRequest>,
) -> Result<Json<BulkCreateResponse>, (StatusCode, String)> {
    // Generate a unique batch ID for this upload
    let batch_id = Uuid::new_v4().to_string();
    let batch_source = format!("community:{}", batch_id);

    let mut results = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;

    for item in req.items {
        let decoded = match bl4::ItemSerial::decode(&item.serial) {
            Ok(d) => d,
            Err(e) => {
                results.push(CreateItemResponse {
                    serial: item.serial,
                    created: false,
                    message: format!("Invalid serial: {}", e),
                });
                failed += 1;
                continue;
            }
        };

        match state.db.get_item(&item.serial).await {
            Ok(Some(_)) => {
                results.push(CreateItemResponse {
                    serial: item.serial,
                    created: false,
                    message: "Item already exists".into(),
                });
                failed += 1;
                continue;
            }
            Err(e) => {
                results.push(CreateItemResponse {
                    serial: item.serial,
                    created: false,
                    message: format!("Database error: {}", e),
                });
                failed += 1;
                continue;
            }
            Ok(None) => {}
        }

        if let Err(e) = state.db.add_item(&item.serial).await {
            results.push(CreateItemResponse {
                serial: item.serial,
                created: false,
                message: format!("Failed to create: {}", e),
            });
            failed += 1;
            continue;
        }

        // Use the batch-specific source for all items in this upload
        let _ = state.db.set_source(&item.serial, &batch_source).await;
        let _ = state
            .db
            .set_item_type(&item.serial, &decoded.item_type.to_string())
            .await;

        if let Some(name) = &item.name {
            let _ = state
                .db
                .set_value(
                    &item.serial,
                    "name",
                    name,
                    ValueSource::CommunityTool,
                    item.source.as_deref(),
                    Confidence::Uncertain,
                )
                .await;
        }

        results.push(CreateItemResponse {
            serial: item.serial,
            created: true,
            message: "Item created".into(),
        });
        succeeded += 1;
    }

    Ok(Json(BulkCreateResponse {
        batch_id,
        succeeded,
        failed,
        results,
    }))
}

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
async fn decode_serial(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DecodeRequest>,
) -> Result<Json<DecodeResponse>, (StatusCode, String)> {
    let item = bl4::ItemSerial::decode(&req.serial)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to decode: {}", e)))?;

    // Auto-insert valid serials into DB with community:decode source
    // Ignore errors - item may already exist
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
        .parts()
        .iter()
        .map(|(idx, _bits)| {
            let cat_id = bl4::parts::serial_id_to_parts_category(*idx);
            let category = bl4::parts::category_name(cat_id as i64).map(String::from);

            PartInfo {
                index: *idx as u32,
                category,
                name: None,
            }
        })
        .collect();

    Ok(Json(DecodeResponse {
        serial: req.serial,
        item_type: item.item_type.to_string(),
        item_type_name: item.item_type_description().to_string(),
        manufacturer,
        weapon_type,
        level,
        rarity,
        element,
        parts,
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
async fn encode_serial(
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

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            port,
            database,
            bind,
        } => {
            // Initialize tracing
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "bl4_community=info,tower_http=debug".into()),
                )
                .with(tracing_subscriber::fmt::layer())
                .init();

            // Build database URL - only add sqlite: prefix for local file paths
            let db_url = if database.contains("://") {
                database.clone()
            } else {
                format!("sqlite:{}?mode=rwc", database)
            };

            tracing::info!("Connecting to database: {}", db_url);
            let db = SqlxSqliteDb::connect(&db_url).await?;
            db.init().await?;
            tracing::info!("Database initialized");

            let state = Arc::new(AppState { db });

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);

            // API routes with CORS
            let api_routes = Router::new()
                .route("/health", get(health))
                .route("/capabilities", get(get_capabilities))
                .route("/items", get(list_items).post(create_item))
                .route("/items/bulk", post(create_items_bulk))
                .route("/items/{serial}", get(get_item))
                .route("/items/{serial}/attachments", post(upload_attachment))
                .route("/decode", post(decode_serial))
                .route("/encode", post(encode_serial))
                .route("/stats", get(get_stats))
                .merge(Scalar::with_url("/scalar", ApiDoc::openapi()))
                .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
                .with_state(state)
                .layer(cors);

            // Root OPTIONS returns OpenAPI schema (no CORS interception)
            let app = Router::new()
                .route("/", axum::routing::options(options_schema))
                .merge(api_routes)
                .layer(TraceLayer::new_for_http());

            let bind_addr = format!("{}:{}", bind, port);
            tracing::info!("Starting server on {}", bind_addr);
            tracing::info!("OpenAPI spec available at /openapi.json");
            tracing::info!("Interactive docs at /scalar");

            let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
