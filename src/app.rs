use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{api::api_router, error::AppError, state::AppState};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", api_router(state.clone()))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    state.db.health_check().await?;
    Ok(Json(json!({ "status": "ok" })))
}
