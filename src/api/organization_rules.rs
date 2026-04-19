use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    jobs::OrganizePayload,
    models::OrganizationRule,
    organizer::rules::apply_rules,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_rules).post(create_rule))
        // preview and apply must be registered before /:id so Axum does not
        // treat the literal strings "preview" / "apply" as an id path segment.
        .route("/preview", post(preview))
        .route("/apply", post(apply))
        .route("/:id", get(get_rule).put(update_rule).delete(delete_rule))
}

#[derive(Deserialize)]
struct ListQuery {
    library_id: Option<i64>,
}

async fn list_rules(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<OrganizationRule>>, AppError> {
    Ok(Json(state.db.list_organization_rules(q.library_id).await?))
}

async fn get_rule(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<OrganizationRule>, AppError> {
    state
        .db
        .get_organization_rule(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("rule {id} not found")))
        .map(Json)
}

#[derive(Deserialize)]
struct CreateRuleRequest {
    name: String,
    library_id: Option<i64>,
    priority: i32,
    conditions: Option<serde_json::Value>,
    path_template: String,
    enabled: bool,
}

async fn create_rule(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<CreateRuleRequest>,
) -> Result<(StatusCode, Json<OrganizationRule>), AppError> {
    let rule = state
        .db
        .create_organization_rule(
            &body.name,
            body.library_id,
            body.priority,
            body.conditions,
            &body.path_template,
            body.enabled,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(rule)))
}

#[derive(Deserialize)]
struct UpdateRuleRequest {
    name: String,
    priority: i32,
    conditions: Option<serde_json::Value>,
    path_template: String,
    enabled: bool,
}

async fn update_rule(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateRuleRequest>,
) -> Result<Json<OrganizationRule>, AppError> {
    state
        .db
        .update_organization_rule(
            id,
            &body.name,
            body.priority,
            body.conditions,
            &body.path_template,
            body.enabled,
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("rule {id} not found")))
        .map(Json)
}

async fn delete_rule(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_organization_rule(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct PreviewApplyRequest {
    library_id: i64,
    track_ids: Vec<i64>,
}

async fn preview(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<PreviewApplyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let rules = state
        .db
        .list_organization_rules(Some(body.library_id))
        .await?;
    let rule_pairs: Vec<(Option<serde_json::Value>, String)> = rules
        .into_iter()
        .filter(|r| r.enabled)
        .map(|r| (r.conditions, r.path_template))
        .collect();

    let mut results = Vec::new();
    for track_id in &body.track_ids {
        if let Some(track) = state.db.get_track(*track_id).await? {
            let tags: HashMap<String, String> = track
                .tags
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            let proposed = apply_rules(&rule_pairs, &tags);
            results.push(serde_json::json!({
                "track_id": track_id,
                "current_path": track.relative_path,
                "proposed_path": proposed,
                "rule_matched": proposed.is_some(),
            }));
        }
    }
    Ok(Json(serde_json::Value::Array(results)))
}

async fn apply(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<PreviewApplyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut enqueued = 0i64;
    for track_id in &body.track_ids {
        state
            .db
            .enqueue_job(
                "organize",
                serde_json::to_value(OrganizePayload {
                    track_id: *track_id,
                    dry_run: false,
                })
                .unwrap(),
                0,
            )
            .await?;
        enqueued += 1;
    }
    Ok(Json(serde_json::json!({ "enqueued": enqueued })))
}
