use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use uuid::Uuid;

use crate::{api::middleware::auth::AuthUser, error::AppError, state::AppState};

const ALLOWED_MIME: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/gif"];
const MAX_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

pub fn router() -> Router<AppState> {
    Router::new().route("/images", axum::routing::post(upload_image))
}

async fn upload_image(
    State(state): State<AppState>,
    _auth: AuthUser,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let ct = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        if !ALLOWED_MIME.contains(&ct.as_str()) {
            return Err(AppError::BadRequest(format!(
                "unsupported type: {ct}; allowed: jpeg, png, webp, gif"
            )));
        }
        let ext = match ct.as_str() {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => "bin",
        };
        let bytes = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        if bytes.len() > MAX_BYTES {
            return Err(AppError::BadRequest(format!(
                "file too large ({} bytes, max {MAX_BYTES})",
                bytes.len()
            )));
        }
        let filename = format!("{}.{ext}", Uuid::new_v4());
        tokio::fs::create_dir_all(&state.config.uploads_dir)
            .await
            .map_err(|e| anyhow::anyhow!("create uploads dir: {e}"))?;
        tokio::fs::write(state.config.uploads_dir.join(&filename), &bytes)
            .await
            .map_err(|e| anyhow::anyhow!("write: {e}"))?;
        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({ "url": format!("/uploads/{filename}") })),
        ));
    }
    Err(AppError::BadRequest(
        "no file field in multipart body".into(),
    ))
}
