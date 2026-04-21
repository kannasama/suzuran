use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, post},
    Json, Router,
};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    services::{auth::AuthService, totp::TotpService},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", axum::routing::get(status))
        .route("/enroll", post(enroll))
        .route("/verify", post(verify_enroll))
        .route("/complete", post(complete_2fa))
        .route("/disenroll", delete(disenroll))
}

#[derive(Serialize)]
struct TotpStatusResponse {
    enrolled: bool,
}

async fn status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<TotpStatusResponse>, AppError> {
    let enrolled = state
        .db
        .find_totp_entry(auth.0.id)
        .await?
        .map(|e| e.verified)
        .unwrap_or(false);
    Ok(Json(TotpStatusResponse { enrolled }))
}

#[derive(Serialize)]
struct EnrollResponse {
    otpauth_uri: String,
}

#[derive(Deserialize)]
struct VerifyRequest {
    code: String,
}

#[derive(Deserialize)]
struct Complete2faRequest {
    token: String,
    code: String,
}

/// Start TOTP enrollment: generates secret, returns otpauth URI for QR code.
async fn enroll(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<EnrollResponse>, AppError> {
    let secret = TotpService::generate_secret();
    state.db.create_totp_entry(auth.0.id, &secret).await?;
    let uri = TotpService::otpauth_uri(&secret, &auth.0.username)
        .map_err(AppError::Internal)?;
    Ok(Json(EnrollResponse { otpauth_uri: uri }))
}

/// Confirm enrollment: verify a code from the authenticator app, mark TOTP as verified.
async fn verify_enroll(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<VerifyRequest>,
) -> Result<StatusCode, AppError> {
    let entry = state
        .db
        .find_totp_entry(auth.0.id)
        .await?
        .ok_or_else(|| AppError::BadRequest("no TOTP enrollment in progress".into()))?;

    TotpService::verify(&entry.secret, &auth.0.username, &body.code)?;
    state.db.mark_totp_verified(auth.0.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Complete 2FA login with TOTP: exchange a 2fa_pending token for a full session cookie.
async fn complete_2fa(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<Complete2faRequest>,
) -> Result<impl IntoResponse, AppError> {
    let claims = AuthService::decode_2fa_pending_token(&body.token, &state.config.jwt_secret)?;

    let user = state
        .db
        .find_user_by_id(claims.sub)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let entry = state
        .db
        .find_totp_entry(user.id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !entry.verified {
        return Err(AppError::Unauthorized);
    }

    TotpService::verify(&entry.secret, &user.username, &body.code)?;

    let token = AuthService::create_full_session(&state.db, &user, &state.config.jwt_secret).await?;
    let cookie = Cookie::build(("session", token))
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::days(30))
        .path("/")
        .build();

    Ok((jar.add(cookie), StatusCode::NO_CONTENT))
}

async fn disenroll(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<StatusCode, AppError> {
    state.db.delete_totp_entry(auth.0.id).await?;
    Ok(StatusCode::NO_CONTENT)
}
