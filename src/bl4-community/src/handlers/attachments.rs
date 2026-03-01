use std::sync::Arc;

use axum::{
    extract::{Multipart, Path as AxumPath, State},
    http::StatusCode,
    Json,
};

use crate::helpers::{resize_image_if_needed, MAX_ATTACHMENT_SIZE};
use crate::schema::AttachmentUploadResponse;
use crate::state::AppState;

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
pub async fn upload_attachment(
    State(state): State<Arc<AppState>>,
    AxumPath(serial): AxumPath<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<AttachmentUploadResponse>), (StatusCode, String)> {
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

    if !mime.starts_with("image/") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Only image files are allowed".to_string(),
        ));
    }

    let (final_data, final_mime) = resize_image_if_needed(&data, &mime).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to process image: {}", e),
        )
    })?;

    let id = state
        .db
        .add_attachment(&serial, &name, &final_mime, &final_data, &view)
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
