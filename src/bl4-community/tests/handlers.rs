use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bl4_community::{
    db::Database, helpers::sanitize_db_url, routes::build_router, state::AppState,
};
use bl4_idb::{AsyncItemsRepository, SqlxSqliteDb};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn test_app() -> axum::Router {
    let db = SqlxSqliteDb::connect("sqlite::memory:").await.unwrap();
    db.init().await.unwrap();
    let state = Arc::new(AppState {
        db: Database::Sqlite(db),
    });
    build_router(state)
}

async fn json_body(app: axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&body).unwrap();
    (status, value)
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

// Weapon with parts and element (Hellwalker, Fire shotgun)
const SERIAL_FIRE_SHOTGUN: &str = "@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-";
// Shield (equipment type)
const SERIAL_SHIELD: &str = "@Uge98>m/)}}!c5JeNWCvCXc7";
// Longer serial (utility/legendary)
const SERIAL_LONG: &str = "@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1";

// ---- System handlers ----

#[tokio::test]
async fn test_health() {
    let app = test_app().await;
    let req = Request::get("/health").body(Body::empty()).unwrap();
    let (status, body) = json_body(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn test_stats_empty_db() {
    let app = test_app().await;
    let req = Request::get("/stats").body(Body::empty()).unwrap();
    let (status, body) = json_body(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["item_count"], 0);
}

// ---- Decode/Encode handlers ----

#[tokio::test]
async fn test_decode_resolves_part_names() {
    let app = test_app().await;
    let req = post_json("/decode", serde_json::json!({"serial": SERIAL_FIRE_SHOTGUN}));
    let (status, body) = json_body(app, req).await;

    assert_eq!(status, StatusCode::OK);

    let parts = body["parts"].as_array().unwrap();
    assert!(!parts.is_empty());

    let named_count = parts.iter().filter(|p| !p["name"].is_null()).count();
    assert!(named_count > 0, "At least some parts should have resolved names");
}

#[tokio::test]
async fn test_decode_flags_elements() {
    let app = test_app().await;
    let req = post_json("/decode", serde_json::json!({"serial": SERIAL_FIRE_SHOTGUN}));
    let (status, body) = json_body(app, req).await;

    assert_eq!(status, StatusCode::OK);

    let parts = body["parts"].as_array().unwrap();
    let element_count = parts.iter().filter(|p| p["is_element"] == true).count();
    assert!(element_count > 0, "Fire shotgun should have element parts");
    assert_eq!(body["element"], "Fire");
}

#[tokio::test]
async fn test_decode_response_schema() {
    let app = test_app().await;
    let req = post_json("/decode", serde_json::json!({"serial": SERIAL_SHIELD}));
    let (status, body) = json_body(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["serial"], SERIAL_SHIELD);
    assert!(body["format"].is_string());
    assert!(body["category"].is_string());
    assert!(body["parts"].is_array());
    assert!(body["string_tokens"].is_array());
}

#[tokio::test]
async fn test_decode_includes_string_tokens_field() {
    let app = test_app().await;
    let req = post_json("/decode", serde_json::json!({"serial": SERIAL_LONG}));
    let (status, body) = json_body(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["string_tokens"].is_array());
    assert_eq!(body["serial"], SERIAL_LONG);
}

#[tokio::test]
async fn test_decode_invalid_serial() {
    let app = test_app().await;
    let req = post_json("/decode", serde_json::json!({"serial": "not-a-real-serial"}));

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_encode_roundtrip() {
    let app = test_app().await;
    let req = post_json("/encode", serde_json::json!({"serial": SERIAL_FIRE_SHOTGUN}));
    let (status, body) = json_body(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["original"], SERIAL_FIRE_SHOTGUN);
    assert_eq!(body["matches"], true);
}

// ---- Item CRUD handlers ----

#[tokio::test]
async fn test_create_and_get_item() {
    let app = test_app().await;

    // Create item
    let req = post_json(
        "/items",
        serde_json::json!({"serial": SERIAL_FIRE_SHOTGUN, "name": "Test Shotgun"}),
    );
    let (status, body) = json_body(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["created"], true);

    // Get it back
    let req = Request::get(format!("/items/{}", SERIAL_FIRE_SHOTGUN))
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_body(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["serial"], SERIAL_FIRE_SHOTGUN);
}

#[tokio::test]
async fn test_create_item_duplicate() {
    let app = test_app().await;

    let payload = serde_json::json!({"serial": SERIAL_SHIELD});

    let req = post_json("/items", payload.clone());
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let req = post_json("/items", payload);
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_list_items_pagination() {
    let app = test_app().await;

    for serial in [SERIAL_FIRE_SHOTGUN, SERIAL_SHIELD] {
        let req = post_json("/items", serde_json::json!({"serial": serial}));
        let _ = app.clone().oneshot(req).await.unwrap();
    }

    // List with limit=1
    let req = Request::get("/items?limit=1").body(Body::empty()).unwrap();
    let (status, body) = json_body(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["total"], 2);
    assert_eq!(body["limit"], 1);

    // List with offset=1
    let req = Request::get("/items?limit=1&offset=1")
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_body(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["offset"], 1);
}

#[tokio::test]
async fn test_get_item_not_found() {
    let app = test_app().await;
    let req = Request::get("/items/nonexistent")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---- Bulk handler ----

#[tokio::test]
async fn test_bulk_create() {
    let app = test_app().await;
    let req = post_json(
        "/items/bulk",
        serde_json::json!({
            "items": [
                {"serial": SERIAL_FIRE_SHOTGUN},
                {"serial": SERIAL_SHIELD},
                {"serial": "garbage-serial"}
            ]
        }),
    );
    let (status, body) = json_body(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["succeeded"], 2);
    assert_eq!(body["failed"], 1);
    assert!(body["batch_id"].is_string());
}

// ---- Helpers ----

#[test]
fn test_sanitize_db_url_with_password() {
    assert_eq!(
        sanitize_db_url("postgres://user:secret@localhost/db"),
        "postgres://user:***@localhost/db"
    );
}

#[test]
fn test_sanitize_db_url_without_password() {
    let url = "sqlite:items.db?mode=rwc";
    assert_eq!(sanitize_db_url(url), url);
}

#[test]
fn test_sanitize_db_url_no_credentials() {
    let url = "postgres://localhost/db";
    assert_eq!(sanitize_db_url(url), url);
}
