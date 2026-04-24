pub mod art_process;
pub mod cue_split;
pub mod fingerprint;
pub mod freedb_lookup;
pub mod maintenance;
pub mod mb_lookup;
pub mod normalize;
pub mod organize;
pub mod process_staged;
pub mod scan;
pub mod transcode;
pub mod virtual_sync;

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
    pub track_id: i64,
    pub library_profile_id: i64,
}

/// Payload for the `process_staged` job type.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProcessStagedPayload {
    pub track_id: i64,
    pub tag_suggestion_id: Option<i64>,
    pub cover_art_url: Option<String>,
    pub write_folder_art: bool,
    pub profile_ids: Vec<i64>,
    /// If set, the active track with this ID is being superseded by the staged track.
    #[serde(default)]
    pub supersede_track_id: Option<i64>,
    /// Library profile whose derived directory the displaced file moves into.
    /// None = discard the old file (user override).
    #[serde(default)]
    pub supersede_profile_id: Option<i64>,
}

/// Payload for the `normalize` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct NormalizePayload {
    pub track_id: i64,
    /// Encoding profile to convert to. If absent, the job skips and chains to mb_lookup.
    pub encoding_profile_id: Option<i64>,
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

/// Payload for the `virtual_sync` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualSyncPayload {
    pub virtual_library_id: i64,
}

/// Payload for the `maintenance` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct MaintenancePayload {
    pub library_id: i64,
}
