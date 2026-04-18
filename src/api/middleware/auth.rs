use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};
use axum_extra::extract::CookieJar;

use crate::{
    error::AppError,
    models::User,
    services::auth::AuthService,
    state::AppState,
};

/// Authenticated user extracted from the session cookie.
/// Use as a handler parameter to require authentication.
pub struct AuthUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract token from HttpOnly cookie named "session"
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get("session")
            .map(|c| c.value().to_string())
            .ok_or(AppError::Unauthorized)?;

        let claims = AuthService::decode_token(&token, &state.config.jwt_secret)?;
        if claims.tfa {
            return Err(AppError::Unauthorized); // 2fa_pending tokens cannot access protected routes
        }
        let token_hash = AuthService::hash_token(&token);

        let session = state
            .db
            .find_session_by_token_hash(&token_hash)
            .await?
            .ok_or(AppError::Unauthorized)?;

        // Confirm JWT session_id matches DB row (extra safety check)
        if session.id != claims.sid {
            return Err(AppError::Unauthorized);
        }

        let user = state
            .db
            .find_user_by_id(claims.sub)
            .await?
            .ok_or(AppError::Unauthorized)?;

        Ok(AuthUser(user))
    }
}
