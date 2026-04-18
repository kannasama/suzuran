pub mod scan;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{dal::Store, error::AppError};

#[async_trait::async_trait]
pub trait JobHandler: Send + Sync {
    async fn run(&self, db: Arc<dyn Store>, payload: serde_json::Value) -> Result<serde_json::Value, AppError>;
}

/// Payload for the `scan` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScanPayload {
    pub library_id: i64,
}
