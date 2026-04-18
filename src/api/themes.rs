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
    models::Theme,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_themes).post(create_theme))
        .route("/:id", get(get_theme).put(update_theme).delete(delete_theme))
}

async fn list_themes(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Theme>>, AppError> {
    Ok(Json(state.db.list_themes().await?))
}

async fn get_theme(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Theme>, AppError> {
    state
        .db
        .get_theme(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("theme {id} not found")))
        .map(Json)
}

#[derive(Deserialize)]
struct ThemeRequest {
    name: String,
    css_vars: serde_json::Value,
    accent_color: Option<String>,
    background_url: Option<String>,
}

async fn create_theme(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<ThemeRequest>,
) -> Result<(StatusCode, Json<Theme>), AppError> {
    let theme = state
        .db
        .create_theme(
            &body.name,
            body.css_vars,
            body.accent_color.as_deref(),
            body.background_url.as_deref(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(theme)))
}

async fn update_theme(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<ThemeRequest>,
) -> Result<Json<Theme>, AppError> {
    state
        .db
        .update_theme(
            id,
            &body.name,
            body.css_vars,
            body.accent_color.as_deref(),
            body.background_url.as_deref(),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("theme {id} not found")))
        .map(Json)
}

async fn delete_theme(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_theme(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
