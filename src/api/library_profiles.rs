use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
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
    state.db.delete_library_profile(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
