pub mod cue_split;
pub mod fingerprint;
pub mod freedb_lookup;
pub mod mb_lookup;
pub mod organize;
pub mod scan;
pub mod transcode;

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

/// Payload for the `cue_split` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct CueSplitPayload {
    pub cue_path: String,
    pub library_id: i64,
}

/// Payload for the `transcode` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscodePayload {
    pub source_track_id: i64,
    pub target_library_id: i64,
}
