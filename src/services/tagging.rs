use crate::{dal::Store, error::AppError, models::TagSuggestion, tagger};
use std::sync::Arc;

/// Apply an accepted tag suggestion: merge tags, write to audio file via lofty, update DB.
///
/// When `fields` is `Some`, only the named fields from the suggestion are applied.
/// When `fields` is `None`, all suggested fields are applied.
pub async fn apply_suggestion(
    store: &Arc<dyn Store>,
    suggestion: &TagSuggestion,
    fields: Option<&[String]>,
    apply_art: bool,
) -> Result<(), AppError> {
    let track = store
        .get_track(suggestion.track_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("track {} not found", suggestion.track_id)))?;

    let library = store
        .get_library(track.library_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("library {} not found", track.library_id))
        })?;

    let full_path = format!(
        "{}/{}",
        library.root_path.trim_end_matches('/'),
        track.relative_path.trim_start_matches('/')
    );

    // Merge: start with existing tags, overlay suggestion tags (filtered by fields if provided)
    let mut merged = track
        .tags
        .as_object()
        .cloned()
        .unwrap_or_default();

    if let Some(suggested_obj) = suggestion.suggested_tags.as_object() {
        for (k, v) in suggested_obj {
            let include = fields.map_or(true, |f| f.iter().any(|field| field == k));
            if include {
                merged.insert(k.clone(), v.clone());
            }
        }
    }

    // Update DB first — more recoverable than a file write
    store
        .update_track_tags(suggestion.track_id, serde_json::Value::Object(merged.clone()))
        .await?;

    // Write to audio file only if DB succeeded
    let string_map: std::collections::HashMap<String, String> = merged
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();

    tagger::write_tags(std::path::Path::new(&full_path), &string_map)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("lofty write failed: {e}")))?;

    // If the suggestion includes a cover art URL and the caller opted in, enqueue embed job
    if apply_art {
        if let Some(url) = &suggestion.cover_art_url {
            store
                .enqueue_job(
                    "art_process",
                    serde_json::json!({
                        "track_id": suggestion.track_id,
                        "action": "embed",
                        "source_url": url,
                    }),
                    3,
                )
                .await?;
        }
    }

    Ok(())
}
