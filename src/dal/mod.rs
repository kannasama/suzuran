pub mod postgres;
pub mod sqlite;

use chrono::{DateTime, Utc};

use crate::{error::AppError, models::{Session, TotpEntry, User, WebauthnChallenge, WebauthnCredential}};

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
}
