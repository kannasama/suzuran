use axum::{
    Router,
    routing::{get, post},
    extract::{Path, Query, State},
    Json,
    http::StatusCode,
};
use std::collections::HashMap;
use crate::{
    dal::UpsertTagSuggestion,
    state::AppState,
    api::middleware::auth::AuthUser,
    error::AppError,
    models::TagSuggestion,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/",             get(list).post(create))
        .route("/count",        get(count))
        .route("/batch-accept", post(batch_accept))
        .route("/:id",          get(get_one))
        .route("/:id/accept",   post(accept))
        .route("/:id/reject",   post(reject))
}

async fn list(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<TagSuggestion>>, AppError> {
    let track_id = params.get("track_id").and_then(|s| s.parse().ok());
    Ok(Json(state.db.list_pending_tag_suggestions(track_id).await?))
}

// Intentionally public — drives the nav badge without requiring auth
async fn count(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let n = state.db.pending_tag_suggestion_count().await?;
    Ok(Json(serde_json::json!({"count": n})))
}

async fn get_one(
    _user: AuthUser,
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<TagSuggestion>, AppError> {
    state.db.get_tag_suggestion(id).await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("tag_suggestion {id}")))
}

async fn accept(
    _user: AuthUser,
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let suggestion = state.db.get_tag_suggestion(id).await?
        .ok_or_else(|| AppError::NotFound(format!("tag_suggestion {id}")))?;
    crate::services::tagging::apply_suggestion(&state.db, &suggestion).await?;
    state.db.set_tag_suggestion_status(id, "accepted").await?;
    Ok(StatusCode::OK)
}

async fn reject(
    _user: AuthUser,
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    state.db.set_tag_suggestion_status(id, "rejected").await?;
    Ok(StatusCode::OK)
}

#[derive(serde::Deserialize)]
struct BatchAcceptBody {
    min_confidence: f32,
}

async fn batch_accept(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<BatchAcceptBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let suggestions = state.db.list_pending_tag_suggestions(None).await?;
    let mut accepted = 0usize;
    for s in suggestions.iter().filter(|s| s.confidence >= body.min_confidence) {
        crate::services::tagging::apply_suggestion(&state.db, s).await?;
        state.db.set_tag_suggestion_status(s.id, "accepted").await?;
        accepted += 1;
    }
    Ok(Json(serde_json::json!({"accepted": accepted})))
}

#[derive(serde::Deserialize)]
struct CreateTagSuggestionBody {
    track_id: i64,
    source: String,
    suggested_tags: serde_json::Value,
    confidence: f64,
    cover_art_url: Option<String>,
    musicbrainz_recording_id: Option<String>,
    musicbrainz_release_id: Option<String>,
}

const VALID_SUGGESTION_SOURCES: &[&str] = &["acoustid", "mb_search", "freedb"];

async fn create(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateTagSuggestionBody>,
) -> Result<(StatusCode, Json<TagSuggestion>), AppError> {
    if !VALID_SUGGESTION_SOURCES.contains(&body.source.as_str()) {
        return Err(AppError::BadRequest(format!(
            "invalid source {:?}; must be one of: acoustid, mb_search, freedb",
            body.source
        )));
    }
    let dto = UpsertTagSuggestion {
        track_id: body.track_id,
        source: body.source,
        suggested_tags: body.suggested_tags,
        confidence: body.confidence as f32,
        mb_recording_id: body.musicbrainz_recording_id,
        mb_release_id: body.musicbrainz_release_id,
        cover_art_url: body.cover_art_url,
        alternatives: None,
    };
    let suggestion = state.db.create_tag_suggestion(dto).await?;
    Ok((StatusCode::CREATED, Json(suggestion)))
}
