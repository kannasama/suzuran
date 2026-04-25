use std::sync::Arc;

use crate::{
    dal::{Store, UpsertTagSuggestion},
    error::AppError,
    services::musicbrainz::{MbRelease, MusicBrainzService},
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

/// One alternative release entry stored alongside the primary suggestion.
#[derive(serde::Serialize)]
struct AlternativeEntry<'a> {
    suggested_tags: std::collections::HashMap<String, String>,
    mb_release_id: &'a str,
    cover_art_url: String,
}

/// Score and sort releases, returning (best_release, alternatives).
/// `existing_tags` is the track's existing tag JSON (used as seed for scoring).
fn pick_best_release<'a>(
    releases: &'a [MbRelease],
    existing_tags: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<(&'a MbRelease, Vec<&'a MbRelease>)> {
    if releases.is_empty() {
        return None;
    }
    let mut scored: Vec<(i32, &MbRelease)> = releases
        .iter()
        .map(|r| (MusicBrainzService::score_release(r, existing_tags), r))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let best = scored[0].1;
    let rest: Vec<&MbRelease> = scored[1..].iter().map(|s| s.1).collect();
    Some((best, rest))
}

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

        // Existing tag map for scoring seed (may be null/empty)
        let existing_tags = track.tags.as_object();

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

                let releases = rec.releases.as_deref().unwrap_or(&[]);
                let Some((best, rest)) = pick_best_release(releases, existing_tags) else {
                    continue;
                };

                // Fetch the full release so to_tag_map has the complete track
                // listing (requires `recordings` inc, only valid on /release/:id).
                let full_release = match self.mb_service.get_release(&best.id).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("MB release fetch failed for {}: {e}", best.id);
                        continue;
                    }
                };

                let best_tags = MusicBrainzService::to_tag_map(&rec, &full_release);
                let cover_art_url = MusicBrainzService::caa_url(&best.id);

                // Build alternatives JSON array
                let alternatives: Vec<AlternativeEntry> = rest
                    .iter()
                    .map(|r| AlternativeEntry {
                        suggested_tags: MusicBrainzService::to_tag_map(&rec, r),
                        mb_release_id: &r.id,
                        cover_art_url: MusicBrainzService::caa_url(&r.id),
                    })
                    .collect();
                let alternatives_json = if alternatives.is_empty() {
                    None
                } else {
                    serde_json::to_value(&alternatives)
                        .ok()
                };

                db.create_tag_suggestion(UpsertTagSuggestion {
                    track_id,
                    source: "acoustid".into(),
                    suggested_tags: serde_json::to_value(&best_tags)
                        .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?,
                    confidence: result.score,
                    mb_recording_id: Some(rec.id.clone()),
                    mb_release_id: Some(best.id.clone()),
                    cover_art_url: Some(cover_art_url),
                    alternatives: alternatives_json,
                })
                .await?;

                suggestions_created += 1;
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
                    alternatives: None,
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
