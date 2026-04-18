use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{dal::Store, error::AppError, models::{Session, User}};

/// Session duration: 30 days.
const SESSION_DURATION_DAYS: i64 = 30;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,   // user_id
    pub sid: i64,   // session_id (set after session row created)
    pub exp: i64,   // unix timestamp
}

pub struct AuthService;

impl AuthService {
    pub fn hash_password(password: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        Ok(argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))?
            .to_string())
    }

    pub fn verify_password(password: &str, hash: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }

    pub fn create_token(
        user_id: i64,
        session_id: i64,
        jwt_secret: &str,
    ) -> anyhow::Result<String> {
        let exp = (Utc::now() + Duration::days(SESSION_DURATION_DAYS)).timestamp();
        let claims = Claims {
            sub: user_id,
            sid: session_id,
            exp,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(jwt_secret.as_bytes()),
        )
        .map_err(|e| anyhow::anyhow!("JWT encoding failed: {e}"))
    }

    pub fn decode_token(token: &str, jwt_secret: &str) -> Result<Claims, AppError> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map(|td| td.claims)
        .map_err(|_| AppError::Unauthorized)
    }

    /// SHA-256 hex digest of a raw token — used as the DB lookup key.
    pub fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn session_expires_at() -> chrono::DateTime<chrono::Utc> {
        Utc::now() + Duration::days(SESSION_DURATION_DAYS)
    }

    /// Full login flow: verify password, create session, return signed JWT.
    pub async fn login(
        db: &Arc<dyn Store>,
        username: &str,
        password: &str,
        jwt_secret: &str,
    ) -> Result<String, AppError> {
        let user = db
            .find_user_by_username(username)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if !Self::verify_password(password, &user.password_hash) {
            return Err(AppError::Unauthorized);
        }

        // Two-step: insert with temp hash to get session id, re-sign JWT with session_id, update hash.
        let expires_at = Self::session_expires_at();
        let temp_hash = Self::hash_token(&format!("temp-{}-{}", user.id, Utc::now().timestamp()));
        let session = db.create_session(user.id, &temp_hash, expires_at).await?;

        let token = Self::create_token(user.id, session.id, jwt_secret)
            .map_err(|e| AppError::Internal(e))?;
        let final_hash = Self::hash_token(&token);

        db.update_session_token_hash(session.id, &final_hash).await?;

        Ok(token)
    }
}
