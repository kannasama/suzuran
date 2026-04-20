pub mod scan;
pub mod organize;
pub mod fingerprint;
pub mod freedb_lookup;
pub mod mb_lookup;

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

/// Payload for the `organize` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizePayload {
    pub track_id: i64,
    pub dry_run: bool,
}

/// Payload for the `fingerprint` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct FingerprintPayload {
    pub track_id: i64,
}
