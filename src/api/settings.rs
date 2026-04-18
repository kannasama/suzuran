use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    models::Setting,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_settings))
        .route("/:key", get(get_setting).put(set_setting))
}

async fn list_settings(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Setting>>, AppError> {
    let settings = state.db.get_all_settings().await?;
    Ok(Json(settings))
}

async fn get_setting(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(key): Path<String>,
) -> Result<Json<Setting>, AppError> {
    state
        .db
        .get_setting(&key)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("setting '{key}' not found")))
        .map(Json)
}

#[derive(Deserialize)]
struct SetSettingRequest {
    value: String,
}

async fn set_setting(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(key): Path<String>,
    Json(body): Json<SetSettingRequest>,
) -> Result<Json<Setting>, AppError> {
    let setting = state.db.set_setting(&key, &body.value).await?;
    Ok(Json(setting))
}
