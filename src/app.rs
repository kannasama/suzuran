use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use tower_http::services::{ServeDir, ServeFile};

use crate::{api::api_router, error::AppError, state::AppState};

pub fn build_router(state: AppState) -> Router {
    // Serve the compiled SPA — fallback to index.html for client-side routing
    let ui_service = ServeDir::new("ui/dist")
        .not_found_service(ServeFile::new("ui/dist/index.html"));

    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", api_router(state.clone()))
        .fallback_service(ui_service)
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    state.db.health_check().await?;
    Ok(Json(json!({ "status": "ok" })))
}
