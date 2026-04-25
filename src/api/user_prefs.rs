use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    models::UserPref,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_user_prefs))
        .route("/:key", axum::routing::put(set_user_pref))
}

async fn get_user_prefs(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<UserPref>>, AppError> {
    let prefs = state.db.get_user_prefs(auth.0.id).await?;
    Ok(Json(prefs))
}

#[derive(Deserialize)]
struct SetPrefRequest {
    value: String,
}

async fn set_user_pref(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(key): Path<String>,
    Json(body): Json<SetPrefRequest>,
) -> Result<Json<UserPref>, AppError> {
    let pref = state.db.set_user_pref(auth.0.id, &key, &body.value).await?;
    Ok(Json(pref))
}
