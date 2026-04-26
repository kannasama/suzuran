use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    services::musicbrainz::MusicBrainzService,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mb", post(search_mb))
        .route("/freedb", post(search_freedb))
        .route("/mb-release", post(search_mb_release))
        .route("/mb-release/:id", get(get_mb_release))
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
    /// Used for text search when disc_id is absent. Both fields empty → returns [].
    artist: Option<String>,
    album: Option<String>,
}

async fn search_freedb(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<FreedBSearchBody>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let FreedBSearchBody { disc_id, artist, album } = body;
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
        let artist = artist.unwrap_or_default();
        let album = album.unwrap_or_default();
        if artist.is_empty() && album.is_empty() {
            return Ok(Json(vec![]));
        }
        let candidates = state.freedb_service
            .text_search(&artist, &album)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("FreeDB text search: {e}")))?;
        let out = candidates.into_iter().map(|c| serde_json::json!({
            "artist": c.artist,
            "album": c.album,
            "year": c.year,
            "genre": c.genre,
            "tracks": c.tracks,
        })).collect();
        Ok(Json(out))
    }
}

#[derive(Deserialize)]
struct MbReleaseSearchBody {
    artist: String,
    album: String,
}

async fn search_mb_release(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<MbReleaseSearchBody>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let releases = state.mb_service
        .search_releases(&body.artist, &body.album)
        .await?;
    let out = releases.into_iter().map(|r| release_to_json(&r)).collect();
    Ok(Json(out))
}

async fn get_mb_release(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let release = state.mb_service
        .get_release(&id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("MB release fetch: {e}")))?;
    Ok(Json(release_to_json_full(&release)))
}

fn release_to_json(r: &crate::services::musicbrainz::MbRelease) -> serde_json::Value {
    let albumartist = r.artist_credit.as_ref()
        .and_then(|ac| ac.first())
        .and_then(|a| a.name.as_ref().or(a.artist.as_ref().map(|ar| &ar.name)))
        .cloned()
        .unwrap_or_default();
    let label = r.label_info.as_ref().and_then(|li| li.first())
        .and_then(|li| li.label.as_ref())
        .map(|l| l.name.clone())
        .unwrap_or_default();
    let catalognumber = r.label_info.as_ref().and_then(|li| li.first())
        .and_then(|li| li.catalog_number.clone())
        .unwrap_or_default();
    let totaltracks: u32 = r.media.as_ref()
        .map(|m| m.iter().map(|d| d.track_count.unwrap_or(0)).sum())
        .unwrap_or(0);
    let totaldiscs = r.media.as_ref().map(|m| m.len()).unwrap_or(0);
    let release_type = r.release_group.as_ref()
        .and_then(|rg| rg.primary_type.as_deref())
        .unwrap_or("")
        .to_string();
    serde_json::json!({
        "mb_release_id": r.id,
        "album": r.title,
        "albumartist": albumartist,
        "date": r.date.clone().unwrap_or_default(),
        "label": label,
        "catalognumber": catalognumber,
        "totaltracks": totaltracks,
        "totaldiscs": totaldiscs,
        "status": r.status.clone().unwrap_or_default(),
        "release_type": release_type,
        "cover_art_url": MusicBrainzService::caa_url(&r.id),
    })
}

fn release_to_json_full(r: &crate::services::musicbrainz::MbRelease) -> serde_json::Value {
    let mut base = release_to_json(r);
    // Add track listing per disc
    let discs: Vec<serde_json::Value> = r.media.as_ref().map(|media| {
        media.iter().map(|disc| {
            let tracks: Vec<serde_json::Value> = disc.tracks.as_ref().map(|tracks| {
                tracks.iter().map(|t| serde_json::json!({
                    "number": t.number.clone().unwrap_or_default(),
                    "position": t.position.unwrap_or(0),
                    "recording_id": t.recording.as_ref().map(|r| r.id.clone()).unwrap_or_default(),
                })).collect()
            }).unwrap_or_default();
            serde_json::json!({
                "position": disc.position.unwrap_or(1),
                "track_count": disc.track_count.unwrap_or(0),
                "tracks": tracks,
            })
        }).collect()
    }).unwrap_or_default();
    base["discs"] = serde_json::json!(discs);
    base
}
