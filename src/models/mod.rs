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
