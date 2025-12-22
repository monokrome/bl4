use crate::commands::{
    connect_db_impl, get_bank_impl, get_character_impl, get_inventory_impl, get_item_detail_impl,
    get_save_info_impl, open_save_impl, save_changes_impl, set_character_impl, sync_to_bank_impl,
    BankInfo, CharacterInfo, InventoryItem, ItemDetail, SaveInfo, SetCharacterRequest,
};
use crate::state::AppState;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

#[derive(Deserialize)]
pub struct OpenSaveRequest {
    path: String,
    steam_id: String,
}

#[derive(Deserialize)]
pub struct ConnectDbRequest {
    path: String,
}

#[derive(Deserialize)]
pub struct SyncRequest {
    serials: Vec<String>,
}

#[derive(Deserialize)]
pub struct ItemDetailRequest {
    serial: String,
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data: Some(data),
            error: None,
        })
    }
}

impl ApiResponse<()> {
    fn error(msg: String) -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
    }

    fn success() -> Json<Self> {
        Json(Self {
            success: true,
            data: None,
            error: None,
        })
    }
}

type AppStateArc = Arc<AppState>;

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn open_save(
    State(state): State<AppStateArc>,
    Json(req): Json<OpenSaveRequest>,
) -> Result<Json<ApiResponse<SaveInfo>>, (StatusCode, Json<ApiResponse<()>>)> {
    open_save_impl(&state, req.path, req.steam_id)
        .map(ApiResponse::ok)
        .map_err(ApiResponse::error)
}

async fn save_changes(
    State(state): State<AppStateArc>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    save_changes_impl(&state)
        .map(|_| ApiResponse::success())
        .map_err(ApiResponse::error)
}

async fn get_save_info(
    State(state): State<AppStateArc>,
) -> Result<Json<ApiResponse<Option<SaveInfo>>>, (StatusCode, Json<ApiResponse<()>>)> {
    get_save_info_impl(&state)
        .map(ApiResponse::ok)
        .map_err(ApiResponse::error)
}

async fn get_character(
    State(state): State<AppStateArc>,
) -> Result<Json<ApiResponse<CharacterInfo>>, (StatusCode, Json<ApiResponse<()>>)> {
    get_character_impl(&state)
        .map(ApiResponse::ok)
        .map_err(ApiResponse::error)
}

async fn set_character(
    State(state): State<AppStateArc>,
    Json(req): Json<SetCharacterRequest>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    set_character_impl(&state, req)
        .map(|_| ApiResponse::success())
        .map_err(ApiResponse::error)
}

async fn get_inventory(
    State(state): State<AppStateArc>,
) -> Result<Json<ApiResponse<Vec<InventoryItem>>>, (StatusCode, Json<ApiResponse<()>>)> {
    get_inventory_impl(&state)
        .map(ApiResponse::ok)
        .map_err(ApiResponse::error)
}

async fn connect_db(
    State(state): State<AppStateArc>,
    Json(req): Json<ConnectDbRequest>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    connect_db_impl(&state, req.path)
        .map(|_| ApiResponse::success())
        .map_err(ApiResponse::error)
}

async fn sync_to_bank(
    State(state): State<AppStateArc>,
    Json(req): Json<SyncRequest>,
) -> Result<Json<ApiResponse<u32>>, (StatusCode, Json<ApiResponse<()>>)> {
    sync_to_bank_impl(&state, req.serials)
        .map(ApiResponse::ok)
        .map_err(ApiResponse::error)
}

async fn get_bank(
    State(state): State<AppStateArc>,
) -> Result<Json<ApiResponse<BankInfo>>, (StatusCode, Json<ApiResponse<()>>)> {
    get_bank_impl(&state)
        .map(ApiResponse::ok)
        .map_err(ApiResponse::error)
}

async fn get_item_detail(
    State(state): State<AppStateArc>,
    Json(req): Json<ItemDetailRequest>,
) -> Result<Json<ApiResponse<ItemDetail>>, (StatusCode, Json<ApiResponse<()>>)> {
    get_item_detail_impl(&state, &req.serial)
        .map(ApiResponse::ok)
        .map_err(ApiResponse::error)
}

pub async fn run() {
    let state = Arc::new(AppState::default());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .route("/health", get(health))
        .route("/save/open", post(open_save))
        .route("/save", post(save_changes))
        .route("/save/info", get(get_save_info))
        .route("/character", get(get_character))
        .route("/character", post(set_character))
        .route("/inventory", get(get_inventory))
        .route("/bank", get(get_bank))
        .route("/bank/sync", post(sync_to_bank))
        .route("/item/detail", post(get_item_detail))
        .route("/db/connect", post(connect_db));

    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new("ui/dist"))
        .layer(cors)
        .with_state(state);

    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    println!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
