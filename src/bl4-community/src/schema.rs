use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

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
pub struct ItemValueRequest {
    pub field: String,
    pub value: String,
    pub source: String,
    pub source_detail: Option<String>,
    pub confidence: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateItemRequest {
    pub serial: String,
    pub name: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub values: Vec<ItemValueRequest>,
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
    pub index: u64,
    pub name: Option<String>,
    pub short_name: String,
    pub slot: String,
    pub is_element: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StringToken {
    pub asset_path: String,
    pub short_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DecodeResponse {
    pub serial: String,
    pub format: String,
    pub category: String,
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub level: Option<u32>,
    pub rarity: Option<String>,
    pub element: Option<String>,
    pub parts: Vec<PartInfo>,
    pub string_tokens: Vec<StringToken>,
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
