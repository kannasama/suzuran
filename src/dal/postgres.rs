use chrono::{DateTime, Utc};

use crate::{dal::Store, error::AppError, models::{Session, User}};
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
}
