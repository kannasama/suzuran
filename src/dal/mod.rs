pub mod postgres;
pub mod sqlite;

use chrono::{DateTime, Utc};

use serde_json::Value as JsonValue;

use crate::{error::AppError, models::{Library, Session, Setting, Theme, TotpEntry, Track, User, WebauthnChallenge, WebauthnCredential}};

pub struct UpsertTrack {
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
    pub tags: JsonValue,
    pub duration_secs: Option<f64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub has_embedded_art: bool,
}

#[async_trait::async_trait]
pub trait Store: Send + Sync {
    // ── connectivity ──────────────────────────────────────────────
    async fn health_check(&self) -> Result<(), AppError>;

    // ── users ─────────────────────────────────────────────────────
    async fn count_users(&self) -> Result<i64, AppError>;
    async fn create_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &str,
    ) -> Result<User, AppError>;
    async fn find_user_by_username(&self, username: &str) -> Result<Option<User>, AppError>;
    async fn find_user_by_id(&self, id: i64) -> Result<Option<User>, AppError>;

    // ── sessions ──────────────────────────────────────────────────
    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Session, AppError>;
    async fn find_session_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Session>, AppError>;
    async fn delete_session(&self, id: i64) -> Result<(), AppError>;
    async fn update_session_token_hash(
        &self,
        session_id: i64,
        token_hash: &str,
    ) -> Result<(), AppError>;

    // ── totp ──────────────────────────────────────────────────────
    async fn create_totp_entry(
        &self,
        user_id: i64,
        secret: &str,
    ) -> Result<TotpEntry, AppError>;
    async fn find_totp_entry(&self, user_id: i64) -> Result<Option<TotpEntry>, AppError>;
    async fn mark_totp_verified(&self, user_id: i64) -> Result<(), AppError>;
    async fn delete_totp_entry(&self, user_id: i64) -> Result<(), AppError>;

    // ── webauthn credentials ──────────────────────────────────────
    async fn create_webauthn_credential(
        &self,
        user_id: i64,
        credential_id: &str,
        public_key: &str,
        name: &str,
    ) -> Result<WebauthnCredential, AppError>;
    async fn list_webauthn_credentials(
        &self,
        user_id: i64,
    ) -> Result<Vec<WebauthnCredential>, AppError>;
    async fn find_webauthn_credential_by_cred_id(
        &self,
        credential_id: &str,
    ) -> Result<Option<WebauthnCredential>, AppError>;
    async fn update_webauthn_sign_count(
        &self,
        id: i64,
        sign_count: i64,
    ) -> Result<(), AppError>;
    async fn delete_webauthn_credential(&self, id: i64, user_id: i64) -> Result<(), AppError>;

    // ── webauthn challenges ───────────────────────────────────────
    async fn upsert_webauthn_challenge(
        &self,
        user_id: i64,
        kind: &str,
        challenge: &str,
    ) -> Result<(), AppError>;
    async fn find_webauthn_challenge(
        &self,
        user_id: i64,
        kind: &str,
    ) -> Result<Option<WebauthnChallenge>, AppError>;
    async fn delete_webauthn_challenge(&self, user_id: i64, kind: &str) -> Result<(), AppError>;

    // ── settings ──────────────────────────────────────────────────
    async fn get_setting(&self, key: &str) -> Result<Option<Setting>, AppError>;
    async fn get_all_settings(&self) -> Result<Vec<Setting>, AppError>;
    async fn set_setting(&self, key: &str, value: &str) -> Result<Setting, AppError>;

    // ── themes ────────────────────────────────────────────────────
    async fn list_themes(&self) -> Result<Vec<Theme>, AppError>;
    async fn get_theme(&self, id: i64) -> Result<Option<Theme>, AppError>;
    async fn create_theme(
        &self,
        name: &str,
        css_vars: serde_json::Value,
        accent_color: Option<&str>,
        background_url: Option<&str>,
    ) -> Result<Theme, AppError>;
    async fn update_theme(
        &self,
        id: i64,
        name: &str,
        css_vars: serde_json::Value,
        accent_color: Option<&str>,
        background_url: Option<&str>,
    ) -> Result<Option<Theme>, AppError>;
    async fn delete_theme(&self, id: i64) -> Result<(), AppError>;

    // ── libraries ────────────────────────────────────────────────
    async fn list_libraries(&self) -> Result<Vec<Library>, AppError>;
    async fn get_library(&self, id: i64) -> Result<Option<Library>, AppError>;
    async fn create_library(
        &self,
        name: &str,
        root_path: &str,
        format: &str,
        parent_library_id: Option<i64>,
    ) -> Result<Library, AppError>;
    async fn update_library(
        &self,
        id: i64,
        name: &str,
        scan_enabled: bool,
        scan_interval_secs: i64,
        auto_transcode_on_ingest: bool,
        auto_organize_on_ingest: bool,
    ) -> Result<Option<Library>, AppError>;
    async fn delete_library(&self, id: i64) -> Result<(), AppError>;

    // ── tracks ────────────────────────────────────────────────────
    async fn list_tracks_by_library(&self, library_id: i64) -> Result<Vec<Track>, AppError>;
    async fn get_track(&self, id: i64) -> Result<Option<Track>, AppError>;
    async fn find_track_by_path(
        &self,
        library_id: i64,
        relative_path: &str,
    ) -> Result<Option<Track>, AppError>;
    async fn upsert_track(&self, track: UpsertTrack) -> Result<Track, AppError>;
    async fn mark_track_removed(&self, id: i64) -> Result<(), AppError>;
    async fn list_track_paths_by_library(
        &self,
        library_id: i64,
    ) -> Result<Vec<(i64, String, String)>, AppError>;
}
