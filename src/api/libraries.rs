use std::collections::{HashMap, HashSet};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    models::{Library, Track},
    state::AppState,
};

async fn create_library_dirs(root_path: &str) -> Result<(), AppError> {
    for subdir in &["source", "ingest"] {
        tokio::fs::create_dir_all(format!("{}/{}", root_path, subdir))
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!(
                "failed to create library directory {}/{}: {}", root_path, subdir, e
            )))?;
    }
    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_libraries).post(create_library))
        .route("/:id", get(get_library).put(update_library).delete(delete_library))
        .route("/:id/tracks", get(list_tracks))
        .route("/:id/maintenance", axum::routing::post(trigger_maintenance))
}

async fn list_libraries(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Library>>, AppError> {
    Ok(Json(state.db.list_libraries().await?))
}

async fn get_library(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Library>, AppError> {
    state.db.get_library(id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))
        .map(Json)
}

#[derive(Deserialize)]
struct CreateLibraryRequest {
    name: String,
    root_path: String,
    format: String,
    organization_rule_id: Option<i64>,
}

async fn create_library(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<Library>), AppError> {
    create_library_dirs(&body.root_path).await?;
    let lib = state.db
        .create_library(&body.name, &body.root_path, &body.format)
        .await?;
    state.db.set_library_org_rule(lib.id, body.organization_rule_id).await?;
    Ok((StatusCode::CREATED, Json(Library {
        organization_rule_id: body.organization_rule_id,
        ..lib
    })))
}

#[derive(Deserialize)]
struct UpdateLibraryRequest {
    name: String,
    scan_enabled: bool,
    scan_interval_secs: i64,
    auto_organize_on_ingest: bool,
    #[serde(default = "default_utf8")]
    tag_encoding: String,
    organization_rule_id: Option<i64>,
    #[serde(default)]
    is_default: bool,
    maintenance_interval_secs: Option<i64>,
}

fn default_utf8() -> String { "utf8".into() }

async fn update_library(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateLibraryRequest>,
) -> Result<Json<Library>, AppError> {
    let lib = state.db
        .update_library(id, &body.name, body.scan_enabled, body.scan_interval_secs,
            body.auto_organize_on_ingest, &body.tag_encoding, body.maintenance_interval_secs)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))?;
    state.db.set_library_org_rule(id, body.organization_rule_id).await?;
    if body.is_default {
        state.db.set_default_library(id).await?;
    }
    Ok(Json(Library {
        organization_rule_id: body.organization_rule_id,
        is_default: body.is_default,
        ..lib
    }))
}

async fn delete_library(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_library(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct TrackListQuery {
    status: Option<String>,
}

/// Track as returned by the library listing — includes any derived variants
/// nested under the source track. Serialises flat (all Track fields at the
/// top level) so existing consumers don't need to change for the base fields.
#[derive(Serialize)]
struct TrackRow {
    #[serde(flatten)]
    track: Track,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    derived_tracks: Vec<Track>,
}

async fn list_tracks(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Query(q): Query<TrackListQuery>,
) -> Result<Json<Vec<TrackRow>>, AppError> {
    let status = q.status.unwrap_or_else(|| "active".into());
    let tracks = state.db.list_tracks_by_status(id, &status).await?;
    let links  = state.db.list_track_links_by_library(id).await?;

    // Build lookup: track_id → Track
    let mut by_id: HashMap<i64, Track> = tracks.into_iter().map(|t| (t.id, t)).collect();

    // Collect which track IDs are derived (appear as derived_track_id in any link)
    let derived_ids: HashSet<i64> = links.iter().map(|l| l.derived_track_id).collect();

    // Build source_id → Vec<Track> mapping (consume derived tracks from by_id)
    let mut children: HashMap<i64, Vec<Track>> = HashMap::new();
    for link in &links {
        if let Some(derived) = by_id.remove(&link.derived_track_id) {
            children.entry(link.source_track_id).or_default().push(derived);
        }
    }

    // Remaining tracks in by_id are source (or unlinked) tracks
    let rows: Vec<TrackRow> = by_id
        .into_values()
        .filter(|t| !derived_ids.contains(&t.id))
        .map(|t| {
            let mut derived = children.remove(&t.id).unwrap_or_default();
            // Sort derived tracks by bitrate descending (highest quality first)
            derived.sort_by(|a, b| b.bitrate.unwrap_or(0).cmp(&a.bitrate.unwrap_or(0)));
            TrackRow { track: t, derived_tracks: derived }
        })
        .collect();

    Ok(Json(rows))
}

async fn trigger_maintenance(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    // Verify library exists
    state.db
        .get_library(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))?;

    let job = state.db
        .enqueue_job("maintenance", serde_json::json!({ "library_id": id }), 2)
        .await?;

    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "job_id": job.id }))))
}
