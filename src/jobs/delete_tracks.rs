use std::sync::Arc;

use crate::{dal::Store, error::AppError};

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
                None => {
                    // Already gone — count as deleted.
                    deleted += 1;
                    continue;
                }
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

            // Remove from disk — ignore NotFound (file may already be gone).
            match tokio::fs::remove_file(&abs_path).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    errors.push(format!("track {track_id}: delete {abs_path}: {e}"));
                    continue;
                }
            }

            // Remove DB record.
            if let Err(e) = db.delete_track(*track_id).await {
                errors.push(format!("track {track_id}: db delete: {e}"));
                continue;
            }

            deleted += 1;
        }

        tracing::info!(
            deleted,
            error_count = errors.len(),
            "delete_tracks job complete"
        );

        Ok(serde_json::json!({
            "deleted": deleted,
            "errors": errors,
        }))
    }
}
