pub mod art_process;
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

/// Payload for the `art_process` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct ArtProcessPayload {
    pub track_id: i64,
    /// One of: "embed", "extract", "standardize"
    pub action: String,
    /// URL to download art from (required for "embed")
    pub source_url: Option<String>,
    /// Art profile ID to use for "standardize" (optional — uses defaults if absent)
    pub art_profile_id: Option<i64>,
}
