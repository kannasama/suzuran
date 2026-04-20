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
    models::{ArtProfile, UpsertArtProfile},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_profiles).post(create_profile))
        .route("/:id", get(get_profile).put(update_profile).delete(delete_profile))
}

#[derive(Deserialize)]
struct ArtProfileBody {
    name: String,
    max_width_px: i64,
    max_height_px: i64,
    max_size_bytes: Option<i64>,
    format: String,
    quality: i64,
    apply_to_library_id: Option<i64>,
}

impl From<ArtProfileBody> for UpsertArtProfile {
    fn from(b: ArtProfileBody) -> Self {
        UpsertArtProfile {
            name: b.name,
            max_width_px: b.max_width_px,
            max_height_px: b.max_height_px,
            max_size_bytes: b.max_size_bytes,
            format: b.format,
            quality: b.quality,
            apply_to_library_id: b.apply_to_library_id,
        }
    }
}

async fn list_profiles(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<ArtProfile>>, AppError> {
    Ok(Json(state.db.list_art_profiles().await?))
}

async fn get_profile(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<ArtProfile>, AppError> {
    state.db.get_art_profile(id).await.map(Json)
}

async fn create_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<ArtProfileBody>,
) -> Result<(StatusCode, Json<ArtProfile>), AppError> {
    let profile = state.db.create_art_profile(body.into()).await?;
    Ok((StatusCode::CREATED, Json(profile)))
}

async fn update_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<ArtProfileBody>,
) -> Result<Json<ArtProfile>, AppError> {
    state.db.update_art_profile(id, body.into()).await.map(Json)
}

async fn delete_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_art_profile(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
