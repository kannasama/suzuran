use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tracks/:id/transcode", post(transcode_track))
        .route("/libraries/:id/transcode", post(transcode_library))
        .route("/libraries/:id/transcode-sync", post(transcode_library_sync))
}

#[derive(Deserialize)]
struct TranscodeRequest {
    target_library_id: i64,
}

/// `POST /tracks/:id/transcode` — enqueue a single transcode job for the track.
async fn transcode_track(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<TranscodeRequest>,
) -> Result<StatusCode, AppError> {
    // Verify the track exists
    state.db.get_track(id).await?
        .ok_or_else(|| AppError::NotFound(format!("track {id} not found")))?;

    state.db.enqueue_job(
        "transcode",
        serde_json::json!({
            "source_track_id": id,
            "target_library_id": body.target_library_id,
        }),
        4,
    ).await?;

    Ok(StatusCode::ACCEPTED)
}

/// `POST /libraries/:id/transcode` — enqueue one transcode job per track in the library.
async fn transcode_library(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<TranscodeRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    // Verify the source library exists
    state.db.get_library(id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))?;

    let tracks = state.db.list_tracks_by_library(id).await?;
    let mut enqueued: usize = 0;

    for track in &tracks {
        state.db.enqueue_job(
            "transcode",
            serde_json::json!({
                "source_track_id": track.id,
                "target_library_id": body.target_library_id,
            }),
            4,
        ).await?;
        enqueued += 1;
    }

    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "enqueued": enqueued }))))
}

/// `POST /libraries/:id/transcode-sync` — enqueue transcode only for source tracks that
/// have no existing track_link into the target library.
async fn transcode_library_sync(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<TranscodeRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    // Verify the source library exists
    state.db.get_library(id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))?;

    let tracks = state.db.list_tracks_by_library(id).await?;

    // Build set of derived tracks in the target library
    let derived = state.db.list_tracks_by_library(body.target_library_id).await?;

    // Build a set of source_track_ids already linked into the target library
    let linked_sources: HashSet<i64> = {
        let mut set = HashSet::new();
        for dt in &derived {
            for link in state.db.list_source_tracks(dt.id).await? {
                set.insert(link.source_track_id);
            }
        }
        set
    };

    let mut enqueued: usize = 0;
    for track in &tracks {
        if linked_sources.contains(&track.id) {
            continue;
        }
        state.db.enqueue_job(
            "transcode",
            serde_json::json!({
                "source_track_id": track.id,
                "target_library_id": body.target_library_id,
            }),
            4,
        ).await?;
        enqueued += 1;
    }

    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "enqueued": enqueued }))))
}
