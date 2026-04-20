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
        .route("/tracks/:id/art/embed", post(art_embed))
        .route("/tracks/:id/art/extract", post(art_extract))
        .route("/tracks/:id/art/standardize", post(art_standardize))
        .route("/libraries/:id/art/standardize", post(art_standardize_library))
}

#[derive(Deserialize)]
struct ArtEmbedRequest {
    source_url: String,
}

#[derive(Deserialize)]
struct ArtStandardizeRequest {
    art_profile_id: i64,
}

/// `POST /tracks/:id/art/embed` — enqueue an art_process job with action=embed.
async fn art_embed(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<ArtEmbedRequest>,
) -> Result<StatusCode, AppError> {
    state.db.get_track(id).await?
        .ok_or_else(|| AppError::NotFound(format!("track {id} not found")))?;

    state.db.enqueue_job(
        "art_process",
        serde_json::json!({
            "track_id": id,
            "action": "embed",
            "source_url": body.source_url,
        }),
        4,
    ).await?;

    Ok(StatusCode::ACCEPTED)
}

/// `POST /tracks/:id/art/extract` — enqueue an art_process job with action=extract.
async fn art_extract(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.get_track(id).await?
        .ok_or_else(|| AppError::NotFound(format!("track {id} not found")))?;

    state.db.enqueue_job(
        "art_process",
        serde_json::json!({
            "track_id": id,
            "action": "extract",
        }),
        4,
    ).await?;

    Ok(StatusCode::ACCEPTED)
}

/// `POST /tracks/:id/art/standardize` — enqueue an art_process job with action=standardize.
async fn art_standardize(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<ArtStandardizeRequest>,
) -> Result<StatusCode, AppError> {
    state.db.get_track(id).await?
        .ok_or_else(|| AppError::NotFound(format!("track {id} not found")))?;

    state.db.enqueue_job(
        "art_process",
        serde_json::json!({
            "track_id": id,
            "action": "standardize",
            "art_profile_id": body.art_profile_id,
        }),
        4,
    ).await?;

    Ok(StatusCode::ACCEPTED)
}

/// `POST /libraries/:id/art/standardize` — enqueue art standardize for all tracks in
/// the library that have embedded art.
async fn art_standardize_library(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<ArtStandardizeRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    state.db.get_library(id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))?;

    let tracks = state.db.list_tracks_by_library(id).await?;
    let mut enqueued: usize = 0;

    for track in tracks.iter().filter(|t| t.has_embedded_art) {
        state.db.enqueue_job(
            "art_process",
            serde_json::json!({
                "track_id": track.id,
                "action": "standardize",
                "art_profile_id": body.art_profile_id,
            }),
            4,
        ).await?;
        enqueued += 1;
    }

    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "enqueued": enqueued }))))
}
