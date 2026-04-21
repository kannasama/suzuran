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
    jobs::ScanPayload,
    models::Job,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_jobs))
        .route("/scan", post(enqueue_scan))
        .route("/:id", get(get_job))
        .route("/:id/cancel", post(cancel_job))
}

#[derive(Deserialize)]
struct ListJobsQuery {
    status: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 { 50 }

async fn list_jobs(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<ListJobsQuery>,
) -> Result<Json<Vec<Job>>, AppError> {
    Ok(Json(state.db.list_jobs(q.status.as_deref(), q.limit).await?))
}

async fn get_job(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Job>, AppError> {
    state.db.get_job(id).await?
        .ok_or_else(|| AppError::NotFound(format!("job {id} not found")))
        .map(Json)
}

async fn cancel_job(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.cancel_job(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct EnqueueScanRequest {
    library_id: i64,
}

async fn enqueue_scan(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<EnqueueScanRequest>,
) -> Result<(StatusCode, Json<Job>), AppError> {
    state.db.get_library(body.library_id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {} not found", body.library_id)))?;

    let job = state.db.enqueue_job(
        "scan",
        serde_json::to_value(ScanPayload { library_id: body.library_id })
            .map_err(|e| AppError::Internal(e.into()))?,
        0,
    ).await?;

    Ok((StatusCode::CREATED, Json(job)))
}
