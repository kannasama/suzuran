pub mod postgres;
pub mod sqlite;

use chrono::{DateTime, Utc};

use serde_json::Value as JsonValue;

use crate::{error::AppError, models::{ArtProfile, EncodingProfile, Job, Library, OrganizationRule, Session, Setting, TagSuggestion, Theme, TotpEntry, Track, TrackLink, User, VirtualLibrary, VirtualLibrarySource, VirtualLibraryTrack, WebauthnChallenge, WebauthnCredential}};

pub use crate::models::UpsertTagSuggestion;
pub use crate::models::UpsertEncodingProfile;
pub use crate::models::UpsertArtProfile;
pub use crate::models::UpsertVirtualLibrary;

#[derive(Default)]
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
    pub bit_depth: Option<i64>,
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
        normalize_on_ingest: bool,
        tag_encoding: &str,
    ) -> Result<Option<Library>, AppError>;
    async fn delete_library(&self, id: i64) -> Result<(), AppError>;
    async fn set_library_encoding_profile(
        &self,
        library_id: i64,
        encoding_profile_id: Option<i64>,
    ) -> Result<(), AppError>;

    // ── jobs ─────────────────────────────────────────────────────
    async fn enqueue_job(
        &self,
        job_type: &str,
        payload: serde_json::Value,
        priority: i64,
    ) -> Result<Job, AppError>;
    async fn claim_next_job(&self, job_types: &[&str]) -> Result<Option<Job>, AppError>;
    async fn complete_job(&self, id: i64, result: serde_json::Value) -> Result<(), AppError>;
    async fn fail_job(&self, id: i64, error: &str) -> Result<(), AppError>;
    async fn cancel_job(&self, id: i64) -> Result<(), AppError>;
    async fn list_jobs(&self, status: Option<&str>, limit: i64) -> Result<Vec<Job>, AppError>;
    async fn get_job(&self, id: i64) -> Result<Option<Job>, AppError>;
    async fn list_jobs_by_type_and_payload_key(
        &self,
        job_type: &str,
        key: &str,
        value: &str,
    ) -> Result<Vec<Job>, AppError>;

    // ── organization rules ────────────────────────────────────────
    /// Returns all rules when library_id is None; when Some, returns global rules
    /// (library_id IS NULL) plus rules scoped to that library, ordered by priority asc.
    async fn list_organization_rules(&self, library_id: Option<i64>) -> Result<Vec<OrganizationRule>, AppError>;
    async fn get_organization_rule(&self, id: i64) -> Result<Option<OrganizationRule>, AppError>;
    async fn create_organization_rule(
        &self,
        name: &str,
        library_id: Option<i64>,
        priority: i32,
        conditions: Option<serde_json::Value>,
        path_template: &str,
        enabled: bool,
    ) -> Result<OrganizationRule, AppError>;
    async fn update_organization_rule(
        &self,
        id: i64,
        name: &str,
        priority: i32,
        conditions: Option<serde_json::Value>,
        path_template: &str,
        enabled: bool,
    ) -> Result<Option<OrganizationRule>, AppError>;
    async fn delete_organization_rule(&self, id: i64) -> Result<(), AppError>;

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
    async fn update_track_path(&self, id: i64, relative_path: &str, file_hash: &str) -> Result<(), AppError>;
    async fn update_track_fingerprint(
        &self,
        track_id: i64,
        fingerprint: &str,
        duration_secs: f64,
    ) -> Result<(), AppError>;

    // ── encoding profiles ─────────────────────────────────────────
    async fn create_encoding_profile(&self, dto: UpsertEncodingProfile) -> Result<EncodingProfile, AppError>;
    async fn get_encoding_profile(&self, id: i64) -> Result<EncodingProfile, AppError>;
    async fn list_encoding_profiles(&self) -> Result<Vec<EncodingProfile>, AppError>;
    async fn update_encoding_profile(&self, id: i64, dto: UpsertEncodingProfile) -> Result<EncodingProfile, AppError>;
    async fn delete_encoding_profile(&self, id: i64) -> Result<(), AppError>;

    // ── art profiles ──────────────────────────────────────────────
    async fn create_art_profile(&self, dto: UpsertArtProfile) -> Result<ArtProfile, AppError>;
    async fn get_art_profile(&self, id: i64) -> Result<ArtProfile, AppError>;
    async fn list_art_profiles(&self) -> Result<Vec<ArtProfile>, AppError>;
    async fn update_art_profile(&self, id: i64, dto: UpsertArtProfile) -> Result<ArtProfile, AppError>;
    async fn delete_art_profile(&self, id: i64) -> Result<(), AppError>;

    // ── track links ───────────────────────────────────────────────
    async fn create_track_link(
        &self,
        source_id: i64,
        derived_id: i64,
        encoding_profile_id: Option<i64>,
    ) -> Result<TrackLink, AppError>;
    async fn list_derived_tracks(&self, source_id: i64) -> Result<Vec<TrackLink>, AppError>;
    async fn list_source_tracks(&self, derived_id: i64) -> Result<Vec<TrackLink>, AppError>;

    // ── child libraries ───────────────────────────────────────────
    async fn list_child_libraries(&self, parent_id: i64) -> Result<Vec<Library>, AppError>;

    // ── tag suggestions ───────────────────────────────────────────
    async fn create_tag_suggestion(&self, dto: UpsertTagSuggestion) -> Result<TagSuggestion, AppError>;
    async fn list_pending_tag_suggestions(&self, track_id: Option<i64>) -> Result<Vec<TagSuggestion>, AppError>;
    async fn get_tag_suggestion(&self, id: i64) -> Result<Option<TagSuggestion>, AppError>;
    async fn set_tag_suggestion_status(&self, id: i64, status: &str) -> Result<(), AppError>;
    async fn pending_tag_suggestion_count(&self) -> Result<i64, AppError>;
    async fn update_track_tags(&self, track_id: i64, tags: serde_json::Value) -> Result<(), AppError>;
    async fn set_track_has_embedded_art(&self, track_id: i64, has_art: bool) -> Result<(), AppError>;

    // ── virtual libraries ─────────────────────────────────────────
    async fn create_virtual_library(&self, dto: UpsertVirtualLibrary) -> Result<VirtualLibrary, AppError>;
    async fn get_virtual_library(&self, id: i64) -> Result<VirtualLibrary, AppError>;
    async fn list_virtual_libraries(&self) -> Result<Vec<VirtualLibrary>, AppError>;
    async fn update_virtual_library(&self, id: i64, dto: UpsertVirtualLibrary) -> Result<VirtualLibrary, AppError>;
    async fn delete_virtual_library(&self, id: i64) -> Result<(), AppError>;

    /// Replace the full source list atomically (delete old + insert new in a transaction).
    /// `sources` contains `(library_id, priority)` tuples.
    async fn set_virtual_library_sources(&self, id: i64, sources: &[(i64, i64)]) -> Result<(), AppError>;
    async fn list_virtual_library_sources(&self, id: i64) -> Result<Vec<VirtualLibrarySource>, AppError>;

    async fn upsert_virtual_library_track(&self, vlib_id: i64, track_id: i64, link_path: &str) -> Result<(), AppError>;
    async fn list_virtual_library_tracks(&self, vlib_id: i64) -> Result<Vec<VirtualLibraryTrack>, AppError>;
    async fn clear_virtual_library_tracks(&self, vlib_id: i64) -> Result<(), AppError>;
}
