use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    jobs::ProcessStagedPayload,
    models::Track,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/staged", get(list_staged))
        .route("/submit", post(submit))
}

async fn list_staged(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Track>>, AppError> {
    let libraries = state.db.list_libraries().await?;
    let mut all_tracks: Vec<Track> = Vec::new();
    for lib in libraries {
        let tracks = state.db.list_tracks_by_status(lib.id, "staged").await?;
        all_tracks.extend(tracks);
    }
    Ok(Json(all_tracks))
}

async fn submit(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(payload): Json<ProcessStagedPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let job = state.db.enqueue_job(
        "process_staged",
        serde_json::to_value(&payload)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize payload: {e}")))?,
        0,
    ).await?;
    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "job_id": job.id }))))
}
