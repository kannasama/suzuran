use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use tokio::io::AsyncSeekExt;

use crate::{api::middleware::auth::AuthUser, error::AppError, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:id/stream", get(stream).head(stream_head))
}

/// Resolve a track's absolute path from its library root + relative_path.
async fn resolve_track_path(
    state: &AppState,
    track_id: i64,
) -> Result<(PathBuf, crate::models::Track), AppError> {
    let track = state.db.get_track(track_id).await?
        .ok_or_else(|| AppError::NotFound(format!("track {track_id} not found")))?;

    let library = state.db.get_library(track.library_id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {} not found", track.library_id)))?;

    let abs_path = PathBuf::from(&library.root_path).join(&track.relative_path);
    Ok((abs_path, track))
}

fn content_type_for(path: &std::path::Path) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string()
}

/// Parse a Range header value like "bytes=0-1023" or "bytes=512-".
fn parse_range(range_header: &str, file_size: u64) -> Option<(u64, u64)> {
    let s = range_header.strip_prefix("bytes=")?;
    let (start_str, end_str) = s.split_once('-')?;
    let start: u64 = start_str.parse().ok()?;
    let end: u64 = if end_str.is_empty() {
        file_size.saturating_sub(1)
    } else {
        end_str.parse().ok()?
    };
    if start > end || end >= file_size {
        return None;
    }
    Some((start, end))
}

/// GET /api/v1/tracks/:id/stream
/// Supports: full file, byte-range requests (Range header), correct Content-Type.
pub async fn stream(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(track_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let (abs_path, _track) = resolve_track_path(&state, track_id).await?;

    let metadata = tokio::fs::metadata(&abs_path).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("file metadata error for {:?}: {e}", abs_path))
    })?;
    let file_size = metadata.len();
    let content_type = content_type_for(&abs_path);

    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| parse_range(s, file_size));

    let (start, end, status) = match range {
        Some((s, e)) => (s, e, StatusCode::PARTIAL_CONTENT),
        None => (0, file_size.saturating_sub(1), StatusCode::OK),
    };

    let length = end - start + 1;

    let mut file = tokio::fs::File::open(&abs_path).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("file open error: {e}"))
    })?;

    if start > 0 {
        file.seek(std::io::SeekFrom::Start(start)).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("seek error: {e}"))
        })?;
    }

    let limited = tokio::io::AsyncReadExt::take(file, length);
    let stream = tokio_util::io::ReaderStream::new(limited);
    let body = Body::from_stream(stream);

    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(header::ACCEPT_RANGES, "bytes");

    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{file_size}"),
        );
    }

    builder.body(body).map_err(|e| AppError::Internal(anyhow::anyhow!("response build error: {e}")))
}

/// HEAD /api/v1/tracks/:id/stream
/// Returns headers without body — clients use this to get Content-Length, Content-Type, duration.
pub async fn stream_head(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(track_id): Path<i64>,
) -> Result<Response, AppError> {
    let (abs_path, track) = resolve_track_path(&state, track_id).await?;

    let metadata = tokio::fs::metadata(&abs_path).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("file metadata error: {e}"))
    })?;

    let content_type = content_type_for(&abs_path);

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, metadata.len().to_string())
        .header(header::ACCEPT_RANGES, "bytes");

    if let Some(dur) = track.duration_secs {
        builder = builder.header("X-Duration-Secs", dur.to_string());
    }
    if let Some(br) = track.bitrate {
        builder = builder.header("X-Bitrate", br.to_string());
    }
    if let Some(sr) = track.sample_rate {
        builder = builder.header("X-Sample-Rate", sr.to_string());
    }

    builder
        .body(Body::empty())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("response build error: {e}")))
}
