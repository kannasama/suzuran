use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub force_password_change: bool,
    pub totp_required: bool,
    pub webauthn_required: bool,
    pub accent_color: Option<String>,
    pub base_theme: String,
    pub theme_id: Option<i64>,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TotpEntry {
    pub id: i64,
    pub user_id: i64,
    pub secret: String,   // base32-encoded TOTP secret (store encrypted in future)
    pub verified: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WebauthnCredential {
    pub id: i64,
    pub user_id: i64,
    pub credential_id: String,
    pub public_key: String,   // JSON-serialized webauthn_rs Passkey
    pub sign_count: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WebauthnChallenge {
    pub id: i64,
    pub user_id: i64,
    pub challenge: String,  // JSON-serialized PasskeyRegistration or PasskeyAuthentication state
    pub kind: String,       // "registration" or "authentication"
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    pub format: String,
    pub encoding_profile_id: Option<i64>,
    pub parent_library_id: Option<i64>,
    pub scan_enabled: bool,
    pub scan_interval_secs: i64,
    pub auto_transcode_on_ingest: bool,
    pub auto_organize_on_ingest: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Track {
    pub id: i64,
    pub library_id: i64,
    pub relative_path: String,
    pub file_hash: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub albumartist: Option<String>,
    pub album: Option<String>,
    pub tracknumber: Option<String>,
    pub discnumber: Option<String>,
    pub totaldiscs: Option<String>,
    pub totaltracks: Option<String>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub label: Option<String>,
    pub catalognumber: Option<String>,
    pub tags: serde_json::Value,
    pub duration_secs: Option<f64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub has_embedded_art: bool,
    pub acoustid_fingerprint: Option<String>,
    pub last_scanned_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Job {
    pub id: i64,
    pub job_type: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub priority: i64,
    pub attempts: i64,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Setting {
    pub key: String,
    pub value: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Theme {
    pub id: i64,
    pub name: String,
    pub css_vars: serde_json::Value,
    pub accent_color: Option<String>,
    pub background_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrganizationRule {
    pub id: i64,
    pub name: String,
    pub library_id: Option<i64>,
    pub priority: i32,
    pub conditions: Option<serde_json::Value>,
    pub path_template: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct TagSuggestion {
    pub id: i64,
    pub track_id: i64,
    pub source: String,
    pub suggested_tags: serde_json::Value,
    pub confidence: f32,
    pub mb_recording_id: Option<String>,
    pub mb_release_id: Option<String>,
    pub cover_art_url: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

pub struct UpsertTagSuggestion {
    pub track_id: i64,
    pub source: String,                       // "acoustid" | "mb_search" | "freedb"
    pub suggested_tags: serde_json::Value,
    pub confidence: f32,
    pub mb_recording_id: Option<String>,
    pub mb_release_id: Option<String>,
    pub cover_art_url: Option<String>,
}
