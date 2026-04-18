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
    // Axum automatically handles HEAD requests from GET routes, stripping the body
    // but preserving all headers (including Content-Length and X-* metadata).
    Router::new()
        .route("/:id/stream", get(stream))
}

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
/// Supports full file, byte-range requests, correct Content-Type.
/// HEAD is handled automatically by Axum (body stripped, headers preserved).
pub async fn stream(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(track_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let (abs_path, track) = resolve_track_path(&state, track_id).await?;

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
        .header(header::CONTENT_TYPE, &content_type)
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        // X-File-Size carries the full file size regardless of range or HEAD stripping
        .header("X-File-Size", file_size.to_string());

    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{file_size}"),
        );
    }

    // Track metadata headers — available on both GET and HEAD responses.
    if let Some(dur) = track.duration_secs {
        builder = builder.header("X-Duration-Secs", dur.to_string());
    }
    if let Some(br) = track.bitrate {
        builder = builder.header("X-Bitrate", br.to_string());
    }
    if let Some(sr) = track.sample_rate {
        builder = builder.header("X-Sample-Rate", sr.to_string());
    }

    builder.body(body).map_err(|e| AppError::Internal(anyhow::anyhow!("response build error: {e}")))
}
