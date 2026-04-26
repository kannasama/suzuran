use std::{path::Path, sync::Arc};

use crate::{dal::Store, error::AppError, jobs::{remove_empty_dirs, COMPANION_EXTS}};

pub struct DeleteTracksJobHandler;

#[async_trait::async_trait]
impl super::JobHandler for DeleteTracksJobHandler {
    async fn run(
        &self,
        db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let track_ids: Vec<i64> = payload["track_ids"]
            .as_array()
            .ok_or_else(|| AppError::BadRequest("missing track_ids array".into()))?
            .iter()
            .filter_map(|v| v.as_i64())
            .collect();

        let mut deleted: usize = 0;
        let mut errors: Vec<String> = Vec::new();

        for track_id in &track_ids {
            let track = match db.get_track(*track_id).await? {
                Some(t) => t,
                None => { deleted += 1; continue; }
            };

            let library = match db.get_library(track.library_id).await? {
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
            let abs_path = Path::new(&abs_path);

            // Remove the audio file
            match tokio::fs::remove_file(abs_path).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    errors.push(format!("track {track_id}: delete {}: {e}", abs_path.display()));
                    continue;
                }
            }

            // Remove companion files from the same directory
            if let Some(dir) = abs_path.parent() {
                remove_companion_files(dir).await;

                // Sweep empty parent dirs up to the library root
                let library_root = Path::new(&library.root_path);
                remove_empty_dirs(dir.to_path_buf(), library_root).await;
            }

            if let Err(e) = db.delete_track(*track_id).await {
                errors.push(format!("track {track_id}: db delete: {e}"));
                continue;
            }

            deleted += 1;
        }

        tracing::info!(deleted, error_count = errors.len(), "delete_tracks job complete");

        Ok(serde_json::json!({ "deleted": deleted, "errors": errors }))
    }
}

/// Remove all companion files (art, logs, cue sheets, etc.) from `dir`.
async fn remove_companion_files(dir: &Path) {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return,
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_file() { continue; }
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if !COMPANION_EXTS.contains(&ext.as_str()) { continue; }
        if let Err(e) = tokio::fs::remove_file(&path).await {
            tracing::warn!(path = %path.display(), error = %e, "delete_tracks: failed to remove companion file");
        }
    }
}
