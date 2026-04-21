use std::sync::Arc;

use crate::{
    dal::{Store, UpsertTagSuggestion},
    error::AppError,
    services::musicbrainz::MusicBrainzService,
};

pub struct MbLookupJobHandler {
    pub mb_service: Arc<MusicBrainzService>,
}

impl MbLookupJobHandler {
    pub fn new(mb_service: Arc<MusicBrainzService>) -> Self {
        Self { mb_service }
    }
}

const ACOUSTID_THRESHOLD: f32 = 0.8;

#[async_trait::async_trait]
impl super::JobHandler for MbLookupJobHandler {
    async fn run(
        &self,
        db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"]
            .as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;

        let track = db
            .get_track(track_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("track {track_id} not found")))?;

        // Prefer the dedicated column; fall back to the tags JSON blob.
        let fingerprint = track
            .acoustid_fingerprint
            .as_deref()
            .or_else(|| {
                track
                    .tags
                    .get("acoustid_fingerprint")
                    .and_then(|v| v.as_str())
            })
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("track has no acoustid_fingerprint")))?
            .to_string();

        let duration = track.duration_secs.unwrap_or(0.0);

        let acoustid_key = db
            .get_setting("acoustid_api_key")
            .await?
            .map(|s| s.value)
            .unwrap_or_default();

        if acoustid_key.is_empty() {
            tracing::warn!(track_id, "acoustid_api_key is not set — skipping AcoustID, falling back to FreeDB");
            db.enqueue_job(
                "freedb_lookup",
                serde_json::json!({"track_id": track_id}),
                4,
            )
            .await?;
            return Ok(serde_json::json!({
                "track_id": track_id,
                "suggestions_created": 0,
                "skipped": "no acoustid_api_key",
            }));
        }

        let results = self
            .mb_service
            .acoustid_lookup(&acoustid_key, &fingerprint, duration)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("AcoustID: {e}")))?;

        let acoustid_had_results = !results.is_empty();
        let mut suggestions_created: usize = 0;

        for result in results.iter().filter(|r| r.score >= ACOUSTID_THRESHOLD) {
            let Some(recordings) = &result.recordings else {
                continue;
            };
            for rec_stub in recordings {
                let rec = match self.mb_service.get_recording(&rec_stub.id).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("MB recording fetch failed for {}: {e}", rec_stub.id);
                        continue;
                    }
                };

                for release in rec.releases.as_deref().unwrap_or(&[]) {
                    let tag_map = MusicBrainzService::to_tag_map(&rec, release);
                    let cover_art_url = Some(MusicBrainzService::caa_url(&release.id));

                    db.create_tag_suggestion(UpsertTagSuggestion {
                        track_id,
                        source: "acoustid".into(),
                        suggested_tags: serde_json::to_value(&tag_map)
                            .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?,
                        confidence: result.score,
                        mb_recording_id: Some(rec.id.clone()),
                        mb_release_id: Some(release.id.clone()),
                        cover_art_url,
                    })
                    .await?;

                    suggestions_created += 1;
                }
            }
        }

        tracing::debug!(
            track_id,
            suggestions_created,
            "mb_lookup acoustid phase complete"
        );

        // If AcoustID returned no results, try MB text search using track tags
        if !acoustid_had_results {
            // Extract title/artist/album from track tags JSON
            let tags_obj = track.tags.as_object().cloned().unwrap_or_default();
            let title = tags_obj.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let artist = tags_obj.get("artist").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let album = tags_obj.get("album").and_then(|v| v.as_str()).unwrap_or("").to_string();

            let mb_results = if !title.is_empty() || !artist.is_empty() {
                self.mb_service
                    .search_recordings(&title, &artist, &album)
                    .await
                    .unwrap_or_default()
            } else {
                vec![]
            };

            let mb_had_results = !mb_results.is_empty();

            for (tag_map, confidence) in mb_results {
                db.create_tag_suggestion(UpsertTagSuggestion {
                    track_id,
                    source: "mb_search".into(),
                    suggested_tags: serde_json::to_value(&tag_map)
                        .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?,
                    confidence: confidence as f32,
                    mb_recording_id: tag_map.get("musicbrainz_trackid").cloned(),
                    mb_release_id: tag_map.get("musicbrainz_releaseid").cloned(),
                    cover_art_url: None,
                })
                .await?;
                suggestions_created += 1;
            }

            tracing::debug!(
                track_id,
                suggestions_created,
                mb_had_results,
                "mb_lookup text-search phase complete"
            );

            // If no results from text search either, check for DISCID tag and fallback to FreeDB
            if !mb_had_results {
                let has_discid = tags_obj
                    .get("DISCID")
                    .or_else(|| tags_obj.get("discid"))
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);

                if has_discid {
                    db.enqueue_job(
                        "freedb_lookup",
                        serde_json::json!({"track_id": track_id}),
                        4,
                    )
                    .await?;
                }
            }
        }

        Ok(serde_json::json!({
            "track_id": track_id,
            "suggestions_created": suggestions_created,
        }))
    }
}
