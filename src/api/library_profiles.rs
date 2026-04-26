use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    models::{LibraryProfile, UpsertLibraryProfile},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_profiles).post(create_profile))
        .route("/:id", get(get_profile).put(update_profile).delete(delete_profile))
        .route("/:id/enqueue-transcode", post(enqueue_transcode))
}

#[derive(Deserialize)]
struct ListQuery {
    library_id: Option<i64>,
}

#[derive(Deserialize)]
struct LibraryProfileBody {
    library_id: i64,
    encoding_profile_id: i64,
    derived_dir_name: String,
    include_on_submit: bool,
    auto_include_above_hz: Option<i64>,
}

impl From<LibraryProfileBody> for UpsertLibraryProfile {
    fn from(b: LibraryProfileBody) -> Self {
        UpsertLibraryProfile {
            library_id: b.library_id,
            encoding_profile_id: b.encoding_profile_id,
            derived_dir_name: b.derived_dir_name,
            include_on_submit: b.include_on_submit,
            auto_include_above_hz: b.auto_include_above_hz,
        }
    }
}

async fn list_profiles(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<LibraryProfile>>, AppError> {
    let library_id = q.library_id.ok_or_else(|| {
        AppError::BadRequest("missing required query parameter: library_id".into())
    })?;
    Ok(Json(state.db.list_library_profiles(library_id).await?))
}

async fn get_profile(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<LibraryProfile>, AppError> {
    state.db.get_library_profile(id).await.map(Json)
}

async fn create_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<LibraryProfileBody>,
) -> Result<(StatusCode, Json<LibraryProfile>), AppError> {
    let profile = state.db.create_library_profile(&body.into()).await?;
    Ok((StatusCode::CREATED, Json(profile)))
}

async fn update_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<LibraryProfileBody>,
) -> Result<Json<LibraryProfile>, AppError> {
    state.db.update_library_profile(id, &body.into()).await.map(Json)
}

async fn delete_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let profile = state.db.get_library_profile(id).await?;

    let library = state.db.get_library(profile.library_id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {} not found", profile.library_id)))?;
    let root = library.root_path.trim_end_matches('/').to_string();

    // Remove all derived tracks that belong to this profile
    let derived_tracks = state.db
        .list_tracks_by_profile(profile.library_id, Some(id))
        .await?;

    for track in derived_tracks {
        // Delete the file on disk (best-effort — don't abort if it fails)
        let abs_path = format!("{}/{}", root, track.relative_path.trim_start_matches('/'));
        if let Err(e) = tokio::fs::remove_file(&abs_path).await {
            tracing::warn!(
                track_id = track.id,
                path = %abs_path,
                "delete_profile: failed to remove file: {e}"
            );
        }
        // Remove the track record (deletes associated track_links via CASCADE)
        if let Err(e) = state.db.mark_track_removed(track.id).await {
            tracing::warn!(track_id = track.id, "delete_profile: failed to remove track: {e}");
        }
    }

    state.db.delete_library_profile(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /library-profiles/:id/enqueue-transcode` — enqueue one transcode job per source
/// track in the profile's library.
async fn enqueue_transcode(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let profile = state.db.get_library_profile(id).await?;
    let tracks = state.db.list_tracks_by_library(profile.library_id).await?;
    let mut enqueued: usize = 0;

    for track in &tracks {
        state.db.enqueue_job(
            "transcode",
            serde_json::json!({
                "track_id": track.id,
                "library_profile_id": id,
            }),
            4,
        ).await?;
        enqueued += 1;
    }

    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "enqueued": enqueued }))))
}
