use std::path::Path;

use axum::{
    extract::{Path as AxumPath, State},
    routing::post,
    Json, Router,
};
use serde_json::json;

use crate::{
    api::middleware::admin::AdminUser,
    error::AppError,
    jobs::cue_split::hash_file,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/migrate-library-files/:library_id", post(migrate_library_files))
}

async fn migrate_library_files(
    _admin: AdminUser,
    AxumPath(library_id): AxumPath<i64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let library = state
        .db
        .get_library(library_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("library {library_id} not found")))?;

    let tracks = state
        .db
        .list_tracks_by_status(library_id, "active")
        .await?;

    // Only migrate source tracks (not derived tracks which already live in their profile dir)
    let source_tracks: Vec<_> = tracks
        .into_iter()
        .filter(|t| t.library_profile_id.is_none())
        .collect();

    let source_prefix = Path::new(&library.root_path).join("source");

    let mut moved: u64 = 0;
    let mut skipped: u64 = 0;
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for track in source_tracks {
        let old_abs = Path::new(&library.root_path).join(&track.relative_path);

        // Skip if already under source/ (path prefix check — not string prefix)
        if old_abs.starts_with(&source_prefix) {
            skipped += 1;
            continue;
        }

        let new_rel = format!("source/{}", track.relative_path);
        let new_abs = Path::new(&library.root_path).join(&new_rel);

        // Ensure parent directory exists
        if let Some(parent) = new_abs.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                errors.push(json!({
                    "track_id": track.id,
                    "message": format!("create_dir_all failed: {e}"),
                }));
                continue;
            }
        }

        // Attempt rename; fall back to copy+delete on cross-device (EXDEV = OS error 18)
        let rename_result = tokio::fs::rename(&old_abs, &new_abs).await;
        if let Err(e) = rename_result {
            let is_exdev = e.raw_os_error() == Some(18);
            if is_exdev {
                if let Err(e) = tokio::fs::copy(&old_abs, &new_abs).await {
                    errors.push(json!({
                        "track_id": track.id,
                        "message": format!("copy failed after EXDEV: {e}"),
                    }));
                    continue;
                }
                if let Err(e) = tokio::fs::remove_file(&old_abs).await {
                    // Copy landed at new_abs but original couldn't be removed.
                    // Clean up the orphaned copy so the next run can retry cleanly.
                    let _ = tokio::fs::remove_file(&new_abs).await;
                    errors.push(json!({
                        "track_id": track.id,
                        "message": format!("remove source after copy failed: {e}"),
                    }));
                    continue;
                }
            } else {
                errors.push(json!({
                    "track_id": track.id,
                    "message": format!("rename failed: {e}"),
                }));
                continue;
            }
        }

        // Rehash the file at its new location
        let new_hash = match hash_file(&new_abs).await {
            Ok(h) => h,
            Err(e) => {
                errors.push(json!({
                    "track_id": track.id,
                    "message": format!("hash_file failed: {e}"),
                }));
                continue;
            }
        };

        // Update DB with new relative path and hash
        if let Err(e) = state.db.update_track_path(track.id, &new_rel, &new_hash).await {
            errors.push(json!({
                "track_id": track.id,
                "message": format!("update_track_path failed: {e}"),
            }));
            continue;
        }

        moved += 1;
    }

    Ok(Json(json!({
        "moved": moved,
        "skipped": skipped,
        "errors": errors,
    })))
}
