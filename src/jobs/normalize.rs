use std::sync::Arc;

use crate::{dal::Store, error::AppError};

pub struct NormalizeJobHandler {
    store: Arc<dyn Store>,
}

impl NormalizeJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl super::JobHandler for NormalizeJobHandler {
    async fn run(
        &self,
        _db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"]
            .as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;

        // The normalize job is now a no-op legacy handler. The new architecture
        // uses process_staged for format normalization during ingest.
        // Chain directly to mb_lookup.
        self.store
            .enqueue_job("mb_lookup", serde_json::json!({"track_id": track_id}), 4)
            .await?;

        Ok(serde_json::json!({
            "status": "skipped",
            "reason": "normalize is a legacy no-op; use process_staged instead",
            "track_id": track_id,
        }))
    }
}
