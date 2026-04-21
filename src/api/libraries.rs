use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    models::{Library, Track},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_libraries).post(create_library))
        .route("/:id", get(get_library).put(update_library).delete(delete_library))
        .route("/:id/tracks", get(list_tracks))
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
    parent_library_id: Option<i64>,
    ingest_dir: Option<String>,
    encoding_profile_id: Option<i64>,
    organization_rule_id: Option<i64>,
}

async fn create_library(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<Library>), AppError> {
    let lib = state.db
        .create_library(&body.name, &body.root_path, &body.format, body.parent_library_id)
        .await?;
    state.db.set_library_ingest_dir(lib.id, body.ingest_dir.as_deref()).await?;
    state.db.set_library_encoding_profile(lib.id, body.encoding_profile_id).await?;
    state.db.set_library_org_rule(lib.id, body.organization_rule_id).await?;
    Ok((StatusCode::CREATED, Json(Library {
        ingest_dir: body.ingest_dir,
        encoding_profile_id: body.encoding_profile_id,
        organization_rule_id: body.organization_rule_id,
        ..lib
    })))
}

#[derive(Deserialize)]
struct UpdateLibraryRequest {
    name: String,
    scan_enabled: bool,
    scan_interval_secs: i64,
    auto_transcode_on_ingest: bool,
    auto_organize_on_ingest: bool,
    #[serde(default)]
    normalize_on_ingest: bool,
    #[serde(default = "default_utf8")]
    tag_encoding: String,
    ingest_dir: Option<String>,
    encoding_profile_id: Option<i64>,
    organization_rule_id: Option<i64>,
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
            body.auto_transcode_on_ingest, body.auto_organize_on_ingest,
            body.normalize_on_ingest, &body.tag_encoding)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))?;
    state.db.set_library_ingest_dir(id, body.ingest_dir.as_deref()).await?;
    state.db.set_library_encoding_profile(id, body.encoding_profile_id).await?;
    state.db.set_library_org_rule(id, body.organization_rule_id).await?;
    Ok(Json(Library {
        ingest_dir: body.ingest_dir,
        encoding_profile_id: body.encoding_profile_id,
        organization_rule_id: body.organization_rule_id,
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

async fn list_tracks(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Track>>, AppError> {
    Ok(Json(state.db.list_tracks_by_library(id).await?))
}
