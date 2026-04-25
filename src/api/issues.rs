use axum::{
    Router,
    routing::{get, post},
    extract::{Path, Query, State},
    Json,
    http::StatusCode,
};
use std::{collections::HashMap, path::Path as FsPath};

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    models::Issue,
    state::AppState,
    tagger,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/",              get(list))
        .route("/count",         get(count))
        .route("/:id/dismiss",   post(dismiss))
        .route("/rescan",        post(rescan))
}

async fn list(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Issue>>, AppError> {
    let library_id = params.get("library_id").and_then(|s| s.parse::<i64>().ok());
    let issue_type = params.get("type").map(|s| s.as_str());
    let include_dismissed = params.get("include_dismissed")
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);
    Ok(Json(
        state.db.list_issues(library_id, issue_type, include_dismissed).await?,
    ))
}

// Intentionally public — drives nav badge without auth
async fn count(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let n = state.db.issue_count().await?;
    Ok(Json(serde_json::json!({ "count": n })))
}

async fn dismiss(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.dismiss_issue(id).await?;
    Ok(StatusCode::OK)
}

#[derive(serde::Deserialize)]
struct RescanBody {
    track_ids: Vec<i64>,
}

async fn rescan(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<RescanBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut refreshed = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for track_id in body.track_ids {
        let track = match state.db.get_track(track_id).await? {
            Some(t) => t,
            None => {
                errors.push(format!("track {track_id}: not found"));
                continue;
            }
        };

        let library = match state.db.get_library(track.library_id).await? {
            Some(l) => l,
            None => {
                errors.push(format!("track {track_id}: library {} not found", track.library_id));
                continue;
            }
        };

        let abs_path = format!(
            "{}/{}",
            library.root_path.trim_end_matches('/'),
            track.relative_path.trim_start_matches('/')
        );

        let read_result = tokio::task::spawn_blocking(move || {
            tagger::read_tags(FsPath::new(&abs_path))
        })
        .await;

        let audio_props = match read_result {
            Ok(Ok((_, props))) => props,
            Ok(Err(e)) => {
                errors.push(format!("track {track_id}: read error: {e}"));
                continue;
            }
            Err(e) => {
                errors.push(format!("track {track_id}: spawn error: {e}"));
                continue;
            }
        };

        if let Err(e) = state.db
            .update_track_audio_properties(
                track_id,
                audio_props.duration_secs,
                audio_props.bitrate,
                audio_props.sample_rate,
                audio_props.channels,
                audio_props.bit_depth,
                audio_props.has_embedded_art,
            )
            .await
        {
            errors.push(format!("track {track_id}: db error: {e}"));
            continue;
        }

        // If audio properties now look good, resolve the issue
        let has_bad = audio_props.bitrate.map_or(true, |b| b == 0)
            || audio_props.duration_secs.map_or(true, |d| d <= 0.0);
        if !has_bad {
            let _ = state.db.resolve_issue(track_id, "bad_audio_properties").await;
        }

        refreshed += 1;
    }

    Ok(Json(serde_json::json!({
        "refreshed": refreshed,
        "errors": errors,
    })))
}
