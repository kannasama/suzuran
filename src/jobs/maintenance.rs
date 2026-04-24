use std::{path::Path, sync::Arc};

use crate::{
    dal::Store,
    error::AppError,
    jobs::{JobHandler, MaintenancePayload},
    tagger,
};

pub struct MaintenanceJobHandler;

#[async_trait::async_trait]
impl JobHandler for MaintenanceJobHandler {
    async fn run(
        &self,
        db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let p: MaintenancePayload = serde_json::from_value(payload)
            .map_err(|e| AppError::BadRequest(format!("invalid maintenance payload: {e}")))?;

        let library = db
            .get_library(p.library_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("library {} not found", p.library_id)))?;

        let root = library.root_path.trim_end_matches('/').to_string();

        // Process all active tracks in this library
        let tracks = db.list_tracks_by_status(p.library_id, "active").await?;

        let mut missing = 0usize;
        let mut refreshed = 0usize;
        let mut errors: Vec<String> = Vec::new();

        for track in tracks {
            let abs_path = format!("{}/{}", root, track.relative_path.trim_start_matches('/'));
            let path = Path::new(&abs_path);

            if !path.exists() {
                // File is gone — mark removed
                if let Err(e) = db.mark_track_removed(track.id).await {
                    errors.push(format!("{}: failed to mark removed: {e}", track.relative_path));
                } else {
                    missing += 1;
                    tracing::info!(
                        track_id = track.id,
                        path = %track.relative_path,
                        "maintenance: file missing, marked removed"
                    );
                }
                continue;
            }

            // Re-read audio properties
            let abs_path_owned = abs_path.clone();
            let read_result = tokio::task::spawn_blocking(move || {
                tagger::read_tags(Path::new(&abs_path_owned))
            })
            .await;

            let audio_props = match read_result {
                Ok(Ok((_, props))) => props,
                Ok(Err(e)) => {
                    errors.push(format!("{}: tag read error: {e}", track.relative_path));
                    continue;
                }
                Err(e) => {
                    errors.push(format!("{}: spawn error: {e}", track.relative_path));
                    continue;
                }
            };

            if let Err(e) = db
                .update_track_audio_properties(
                    track.id,
                    audio_props.duration_secs,
                    audio_props.bitrate,
                    audio_props.sample_rate,
                    audio_props.channels,
                    audio_props.bit_depth,
                    audio_props.has_embedded_art,
                )
                .await
            {
                errors.push(format!("{}: db update error: {e}", track.relative_path));
            } else {
                refreshed += 1;
            }
        }

        tracing::info!(
            library_id = p.library_id,
            missing,
            refreshed,
            errors = errors.len(),
            "maintenance complete"
        );

        Ok(serde_json::json!({
            "missing": missing,
            "refreshed": refreshed,
            "errors": errors,
        }))
    }
}
