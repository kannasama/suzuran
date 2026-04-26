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
    library_profile_id: i64,
}

/// `POST /tracks/:id/transcode` — enqueue a single transcode job for the track.
async fn transcode_track(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<TranscodeRequest>,
) -> Result<StatusCode, AppError> {
    state.db.get_track(id).await?
        .ok_or_else(|| AppError::NotFound(format!("track {id} not found")))?;

    state.db.enqueue_job(
        "transcode",
        serde_json::json!({
            "track_id": id,
            "library_profile_id": body.library_profile_id,
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
    state.db.get_library(id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))?;

    let tracks = state.db.list_tracks_by_library(id).await?;
    let mut enqueued: usize = 0;

    for track in &tracks {
        state.db.enqueue_job(
            "transcode",
            serde_json::json!({
                "track_id": track.id,
                "library_profile_id": body.library_profile_id,
            }),
            4,
        ).await?;
        enqueued += 1;
    }

    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "enqueued": enqueued }))))
}

/// `POST /libraries/:id/transcode-sync` — enqueue transcode only for source tracks that
/// have no existing track_link into the target library profile.
async fn transcode_library_sync(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<TranscodeRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    state.db.get_library(id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))?;

    let profile = state.db.get_library_profile(body.library_profile_id).await?;

    let tracks = state.db.list_tracks_by_library(id).await?;

    // Build set of source_track_ids already linked into the target profile's derived tracks
    let derived = state.db.list_tracks_by_profile(profile.library_id, Some(body.library_profile_id)).await?;
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
                "track_id": track.id,
                "library_profile_id": body.library_profile_id,
            }),
            4,
        ).await?;
        enqueued += 1;
    }

    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "enqueued": enqueued }))))
}
