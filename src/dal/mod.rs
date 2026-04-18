pub mod postgres;
pub mod sqlite;

use chrono::{DateTime, Utc};

use crate::{error::AppError, models::{Session, User}};

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
}
