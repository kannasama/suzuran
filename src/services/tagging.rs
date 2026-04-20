use crate::{dal::Store, error::AppError, models::TagSuggestion};
use std::sync::Arc;

/// Stub — full implementation in Task 7.
/// Applies accepted tag suggestion: writes tags to audio file and updates DB.
pub async fn apply_suggestion(
    _store: &Arc<dyn Store>,
    _suggestion: &TagSuggestion,
) -> Result<(), AppError> {
    Ok(())
}
