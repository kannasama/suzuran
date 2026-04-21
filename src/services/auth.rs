use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{dal::Store, error::AppError, models::User};

/// Session duration: 30 days.
const SESSION_DURATION_DAYS: i64 = 30;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,       // user_id
    pub sid: i64,       // session_id (0 for 2fa_pending tokens)
    pub exp: i64,       // unix timestamp
    #[serde(default)]
    pub tfa: bool,      // true = 2fa_pending (not a full session)
}

pub enum LoginResult {
    Session { token: String },
    TwoFactorRequired { token: String },
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
            tfa: false,
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
            &Validation::new(Algorithm::HS256),
        )
        .map(|td| td.claims)
        .map_err(|_| AppError::Unauthorized)
    }

    /// Issue a short-lived token for 2FA completion. Not a full session.
    pub fn issue_2fa_pending_token(user_id: i64, jwt_secret: &str) -> anyhow::Result<String> {
        let exp = (Utc::now() + Duration::minutes(5)).timestamp();
        let claims = Claims { sub: user_id, sid: 0, exp, tfa: true };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(jwt_secret.as_bytes()),
        )
        .map_err(|e| anyhow::anyhow!("JWT encoding failed: {e}"))
    }

    /// Decode a 2fa_pending token. Returns Unauthorized if it's a full session token.
    pub fn decode_2fa_pending_token(token: &str, jwt_secret: &str) -> Result<Claims, AppError> {
        let claims = Self::decode_token(token, jwt_secret)?;
        if !claims.tfa {
            return Err(AppError::Unauthorized);
        }
        Ok(claims)
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

    /// Creates a full DB session and returns the signed JWT.
    pub async fn create_full_session(
        db: &Arc<dyn Store>,
        user: &User,
        jwt_secret: &str,
    ) -> Result<String, AppError> {
        let expires_at = Self::session_expires_at();
        let temp_hash = Self::hash_token(&format!(
            "temp-{}-{}",
            user.id,
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let session = db.create_session(user.id, &temp_hash, expires_at).await?;
        let token = Self::create_token(user.id, session.id, jwt_secret)
            .map_err(AppError::Internal)?;
        let final_hash = Self::hash_token(&token);
        db.update_session_token_hash(session.id, &final_hash).await?;
        Ok(token)
    }

    /// Full login flow: verify password, check 2FA status, return LoginResult.
    pub async fn login(
        db: &Arc<dyn Store>,
        username: &str,
        password: &str,
        jwt_secret: &str,
    ) -> Result<LoginResult, AppError> {
        let user = db
            .find_user_by_username(username)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if !Self::verify_password(password, &user.password_hash) {
            return Err(AppError::Unauthorized);
        }

        let has_totp = db.find_totp_entry(user.id).await?.map(|e| e.verified).unwrap_or(false);
        let has_webauthn = !db.list_webauthn_credentials(user.id).await?.is_empty();

        // Trigger 2FA if the user has any verified second factor enrolled.
        // The totp_required / webauthn_required flags are legacy — enrollment implies enforcement.
        if has_totp || has_webauthn {
            let token = Self::issue_2fa_pending_token(user.id, jwt_secret)
                .map_err(AppError::Internal)?;
            return Ok(LoginResult::TwoFactorRequired { token });
        }

        let token = Self::create_full_session(db, &user, jwt_secret).await?;
        Ok(LoginResult::Session { token })
    }
}
