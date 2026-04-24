use std::{path::Path, sync::Arc};

use crate::{
    dal::{Store, UpsertIssue},
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
        let tracks = db.list_tracks_by_status(p.library_id, "active").await?;

        let mut missing = 0usize;
        let mut refreshed = 0usize;
        let mut errors: Vec<String> = Vec::new();

        for track in tracks {
            let abs_path = format!("{}/{}", root, track.relative_path.trim_start_matches('/'));
            let path = Path::new(&abs_path);

            if !path.exists() {
                // Mark removed and create/update a missing_file issue
                if let Err(e) = db.mark_track_removed(track.id).await {
                    errors.push(format!("{}: failed to mark removed: {e}", track.relative_path));
                } else {
                    missing += 1;
                    tracing::info!(
                        track_id = track.id,
                        path = %track.relative_path,
                        "maintenance: file missing, marked removed"
                    );
                    // Create missing_file issue (best-effort — don't abort if this fails)
                    let _ = db
                        .upsert_issue(UpsertIssue {
                            library_id: p.library_id,
                            track_id: Some(track.id),
                            issue_type: "missing_file".into(),
                            detail: Some(format!("File not found: {}", track.relative_path)),
                            severity: "high".into(),
                        })
                        .await;
                }
                continue;
            }

            // File exists: clear any prior missing_file issue
            let _ = db.resolve_issue(track.id, "missing_file").await;

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

            // Flag tracks with missing audio properties (bitrate or duration)
            let has_bad_audio = audio_props.bitrate.map_or(true, |b| b == 0)
                || audio_props.duration_secs.map_or(true, |d| d <= 0.0);

            if has_bad_audio {
                let _ = db
                    .upsert_issue(UpsertIssue {
                        library_id: p.library_id,
                        track_id: Some(track.id),
                        issue_type: "bad_audio_properties".into(),
                        detail: Some(format!(
                            "bitrate={:?} duration={:?}",
                            audio_props.bitrate, audio_props.duration_secs
                        )),
                        severity: "medium".into(),
                    })
                    .await;
            } else {
                // Clear any prior bad_audio_properties issue
                let _ = db.resolve_issue(track.id, "bad_audio_properties").await;
            }

            // Flag untagged tracks (no title, artist, or album)
            let is_untagged = track.title.as_ref().map_or(true, |s| s.is_empty())
                && track.artist.as_ref().map_or(true, |s| s.is_empty())
                && track.album.as_ref().map_or(true, |s| s.is_empty());

            if is_untagged {
                let _ = db
                    .upsert_issue(UpsertIssue {
                        library_id: p.library_id,
                        track_id: Some(track.id),
                        issue_type: "untagged".into(),
                        detail: Some("No title, artist, or album tag".into()),
                        severity: "low".into(),
                    })
                    .await;
            } else {
                let _ = db.resolve_issue(track.id, "untagged").await;
            }

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
