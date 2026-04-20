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
    models::{EncodingProfile, UpsertEncodingProfile},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_profiles).post(create_profile))
        .route("/:id", get(get_profile).put(update_profile).delete(delete_profile))
}

#[derive(Deserialize)]
struct EncodingProfileBody {
    name: String,
    codec: String,
    bitrate: Option<String>,
    sample_rate: Option<i64>,
    channels: Option<i64>,
    bit_depth: Option<i64>,
    advanced_args: Option<String>,
}

impl From<EncodingProfileBody> for UpsertEncodingProfile {
    fn from(b: EncodingProfileBody) -> Self {
        UpsertEncodingProfile {
            name: b.name,
            codec: b.codec,
            bitrate: b.bitrate,
            sample_rate: b.sample_rate,
            channels: b.channels,
            bit_depth: b.bit_depth,
            advanced_args: b.advanced_args,
        }
    }
}

async fn list_profiles(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<EncodingProfile>>, AppError> {
    Ok(Json(state.db.list_encoding_profiles().await?))
}

async fn get_profile(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<EncodingProfile>, AppError> {
    state.db.get_encoding_profile(id).await.map(Json)
}

async fn create_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<EncodingProfileBody>,
) -> Result<(StatusCode, Json<EncodingProfile>), AppError> {
    let profile = state.db.create_encoding_profile(body.into()).await?;
    Ok((StatusCode::CREATED, Json(profile)))
}

async fn update_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<EncodingProfileBody>,
) -> Result<Json<EncodingProfile>, AppError> {
    state.db.update_encoding_profile(id, body.into()).await.map(Json)
}

async fn delete_profile(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_encoding_profile(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
