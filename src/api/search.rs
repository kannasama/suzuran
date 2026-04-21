use axum::{
    extract::State,
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
        .route("/mb", post(search_mb))
        .route("/freedb", post(search_freedb))
}

#[derive(Deserialize)]
struct MbSearchBody {
    title: String,
    artist: String,
    album: String,
}

async fn search_mb(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<MbSearchBody>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let results = state.mb_service
        .search_recordings(&body.title, &body.artist, &body.album)
        .await?;
    let out: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(tags, confidence)| serde_json::json!({ "tags": tags, "confidence": confidence }))
        .collect();
    Ok(Json(out))
}

#[derive(Deserialize)]
struct FreedBSearchBody {
    disc_id: Option<String>,
    /// Accepted for forward compatibility; CDDB protocol does not support text search
    /// so artist/album are not forwarded to the service when disc_id is absent.
    artist: Option<String>,
    album: Option<String>,
}

async fn search_freedb(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<FreedBSearchBody>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let FreedBSearchBody { disc_id, artist: _, album: _ } = body;
    if let Some(disc_id) = disc_id {
        let candidate = state.freedb_service
            .disc_lookup(&disc_id)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("FreeDB disc lookup: {e}")))?;
        let out: Vec<serde_json::Value> = candidate
            .map(|c| vec![serde_json::json!({
                "artist": c.artist,
                "album": c.album,
                "year": c.year,
                "genre": c.genre,
                "tracks": c.tracks,
            })])
            .unwrap_or_default();
        Ok(Json(out))
    } else {
        // CDDB protocol only supports disc-ID lookup; artist/album text search is not available
        Ok(Json(vec![]))
    }
}
