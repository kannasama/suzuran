use crate::{dal::Store, error::AppError, models::TagSuggestion, tagger};
use std::sync::Arc;

/// Apply an accepted tag suggestion: merge tags, write to audio file via lofty, update DB.
pub async fn apply_suggestion(
    store: &Arc<dyn Store>,
    suggestion: &TagSuggestion,
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

    // Merge: start with existing tags, overlay suggestion tags
    let mut merged = track
        .tags
        .as_object()
        .cloned()
        .unwrap_or_default();

    if let Some(suggested_obj) = suggestion.suggested_tags.as_object() {
        for (k, v) in suggested_obj {
            merged.insert(k.clone(), v.clone());
        }
    }

    // Write to audio file
    let string_map: std::collections::HashMap<String, String> = merged
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();

    tagger::write_tags(std::path::Path::new(&full_path), &string_map)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("lofty write failed: {e}")))?;

    // Update DB
    store
        .update_track_tags(suggestion.track_id, serde_json::Value::Object(merged))
        .await?;

    Ok(())
}
