use chrono::{DateTime, Utc};

use crate::{dal::Store, error::AppError, models::{Session, TotpEntry, User, WebauthnChallenge, WebauthnCredential}};
use sqlx::PgPool;

pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("migrations/postgres").run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl Store for PgStore {
    async fn health_check(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn count_users(&self) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    async fn create_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &str,
    ) -> Result<User, AppError> {
        sqlx::query_as::<_, User>(
            "INSERT INTO users (username, email, password_hash, role)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(role)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db) if db.constraint() == Some("users_username_key") => {
                AppError::BadRequest("username already taken".into())
            }
            sqlx::Error::Database(ref db) if db.constraint() == Some("users_email_key") => {
                AppError::BadRequest("email already registered".into())
            }
            other => AppError::Database(other),
        })
    }

    async fn find_user_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_user_by_id(&self, id: i64) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Session, AppError> {
        sqlx::query_as::<_, Session>(
            "INSERT INTO sessions (user_id, token_hash, expires_at)
             VALUES ($1, $2, $3)
             RETURNING *",
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_session_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Session>, AppError> {
        sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn delete_session(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn update_session_token_hash(
        &self,
        session_id: i64,
        token_hash: &str,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET token_hash = $1 WHERE id = $2")
            .bind(token_hash)
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn create_totp_entry(&self, user_id: i64, secret: &str) -> Result<TotpEntry, AppError> {
        sqlx::query_as::<_, TotpEntry>(
            "INSERT INTO totp_entries (user_id, secret) VALUES ($1, $2)
             ON CONFLICT (user_id) DO UPDATE SET secret = $2, verified = FALSE
             RETURNING *",
        )
        .bind(user_id)
        .bind(secret)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_totp_entry(&self, user_id: i64) -> Result<Option<TotpEntry>, AppError> {
        sqlx::query_as::<_, TotpEntry>("SELECT * FROM totp_entries WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn mark_totp_verified(&self, user_id: i64) -> Result<(), AppError> {
        sqlx::query("UPDATE totp_entries SET verified = TRUE WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn delete_totp_entry(&self, user_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM totp_entries WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn create_webauthn_credential(
        &self,
        user_id: i64,
        credential_id: &str,
        public_key: &str,
        name: &str,
    ) -> Result<WebauthnCredential, AppError> {
        sqlx::query_as::<_, WebauthnCredential>(
            "INSERT INTO webauthn_credentials (user_id, credential_id, public_key, name)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(user_id)
        .bind(credential_id)
        .bind(public_key)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn list_webauthn_credentials(&self, user_id: i64) -> Result<Vec<WebauthnCredential>, AppError> {
        sqlx::query_as::<_, WebauthnCredential>(
            "SELECT * FROM webauthn_credentials WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_webauthn_credential_by_cred_id(
        &self,
        credential_id: &str,
    ) -> Result<Option<WebauthnCredential>, AppError> {
        sqlx::query_as::<_, WebauthnCredential>(
            "SELECT * FROM webauthn_credentials WHERE credential_id = $1",
        )
        .bind(credential_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn update_webauthn_sign_count(&self, id: i64, sign_count: i64) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE webauthn_credentials SET sign_count = $1, last_used_at = NOW() WHERE id = $2",
        )
        .bind(sign_count)
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
    }

    async fn delete_webauthn_credential(&self, id: i64, user_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM webauthn_credentials WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn upsert_webauthn_challenge(
        &self,
        user_id: i64,
        kind: &str,
        challenge: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO webauthn_challenges (user_id, kind, challenge)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, kind)
             DO UPDATE SET challenge = $3, created_at = NOW()",
        )
        .bind(user_id)
        .bind(kind)
        .bind(challenge)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
    }

    async fn find_webauthn_challenge(
        &self,
        user_id: i64,
        kind: &str,
    ) -> Result<Option<WebauthnChallenge>, AppError> {
        sqlx::query_as::<_, WebauthnChallenge>(
            "SELECT * FROM webauthn_challenges WHERE user_id = $1 AND kind = $2",
        )
        .bind(user_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn delete_webauthn_challenge(&self, user_id: i64, kind: &str) -> Result<(), AppError> {
        sqlx::query(
            "DELETE FROM webauthn_challenges WHERE user_id = $1 AND kind = $2",
        )
        .bind(user_id)
        .bind(kind)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
    }
}
