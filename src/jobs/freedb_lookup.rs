use std::sync::Arc;

use crate::{
    dal::{Store, UpsertTagSuggestion},
    error::AppError,
    services::freedb::FreedBService,
};

pub struct FreedBLookupJobHandler {
    pub freedb: Arc<FreedBService>,
}

impl FreedBLookupJobHandler {
    pub fn new(freedb: Arc<FreedBService>) -> Self {
        Self { freedb }
    }
}

#[async_trait::async_trait]
impl super::JobHandler for FreedBLookupJobHandler {
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

        // Skip if no DISCID tag
        let disc_id = match track
            .tags
            .get("DISCID")
            .or_else(|| track.tags.get("discid"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
        {
            Some(id) => id,
            None => {
                return Ok(serde_json::json!({
                    "track_id": track_id,
                    "skipped": true,
                    "reason": "no DISCID tag"
                }));
            }
        };

        let candidate = match self
            .freedb
            .disc_lookup(&disc_id)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("gnudb.org: {e}")))?
        {
            Some(c) => c,
            None => {
                return Ok(serde_json::json!({
                    "track_id": track_id,
                    "suggestions_created": 0
                }));
            }
        };

        // Derive 0-based track index from tracknumber tag
        let track_index = track
            .tags
            .get("tracknumber")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.trim().parse::<usize>().ok())
            .map(|n| n.saturating_sub(1))
            .unwrap_or(0);

        let tags = FreedBService::to_tag_map(&candidate, track_index);

        db.create_tag_suggestion(UpsertTagSuggestion {
            track_id,
            source: "freedb".into(),
            suggested_tags: serde_json::to_value(&tags)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?,
            confidence: 0.5,
            mb_recording_id: None,
            mb_release_id: None,
            cover_art_url: None,
            alternatives: None,
        })
        .await?;

        tracing::debug!(track_id, disc_id, "freedb_lookup complete");

        Ok(serde_json::json!({
            "track_id": track_id,
            "suggestions_created": 1
        }))
    }
}
