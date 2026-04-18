use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    services::{auth::AuthService, webauthn::WebauthnService},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register/challenge", post(registration_challenge))
        .route("/register/complete", post(registration_complete))
        .route("/authenticate/challenge", post(authentication_challenge))
        .route("/authenticate/complete", post(authentication_complete))
        .route("/credentials", get(list_credentials))
        .route("/credentials/:id", delete(delete_credential))
}

#[derive(Deserialize)]
struct RegisterCompleteRequest {
    name: String,
    response: serde_json::Value,
}

#[derive(Deserialize)]
struct AuthChallengeRequest {
    token: String,
}

#[derive(Deserialize)]
struct AuthCompleteRequest {
    token: String,
    response: serde_json::Value,
}

#[derive(Serialize)]
struct CredentialInfo {
    id: i64,
    name: String,
    created_at: String,
    last_used_at: Option<String>,
}

/// Start WebAuthn registration (requires full session auth).
async fn registration_challenge(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<serde_json::Value>, AppError> {
    let challenge = WebauthnService::start_registration(
        &state.webauthn,
        &state.db,
        auth.0.id,
        &auth.0.username,
    )
    .await?;
    Ok(Json(challenge))
}

/// Complete WebAuthn registration.
async fn registration_complete(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<RegisterCompleteRequest>,
) -> Result<StatusCode, AppError> {
    WebauthnService::finish_registration(
        &state.webauthn,
        &state.db,
        auth.0.id,
        &body.name,
        body.response,
    )
    .await?;
    Ok(StatusCode::CREATED)
}

/// Start WebAuthn authentication (accepts 2fa_pending token, no full session required).
async fn authentication_challenge(
    State(state): State<AppState>,
    Json(body): Json<AuthChallengeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let claims = AuthService::decode_2fa_pending_token(&body.token, &state.config.jwt_secret)?;
    let challenge =
        WebauthnService::start_authentication(&state.webauthn, &state.db, claims.sub).await?;
    Ok(Json(challenge))
}

/// Complete WebAuthn authentication: exchange 2fa_pending token for a full session.
async fn authentication_complete(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<AuthCompleteRequest>,
) -> Result<impl IntoResponse, AppError> {
    let claims = AuthService::decode_2fa_pending_token(&body.token, &state.config.jwt_secret)?;

    let user = state
        .db
        .find_user_by_id(claims.sub)
        .await?
        .ok_or(AppError::Unauthorized)?;

    WebauthnService::finish_authentication(&state.webauthn, &state.db, user.id, body.response)
        .await?;

    let token = AuthService::create_full_session(&state.db, &user, &state.config.jwt_secret).await?;
    let cookie = Cookie::build(("session", token))
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::days(30))
        .path("/")
        .build();

    Ok((jar.add(cookie), StatusCode::NO_CONTENT))
}

async fn list_credentials(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<CredentialInfo>>, AppError> {
    let creds = state.db.list_webauthn_credentials(auth.0.id).await?;
    let infos = creds
        .into_iter()
        .map(|c| CredentialInfo {
            id: c.id,
            name: c.name,
            created_at: c.created_at.to_rfc3339(),
            last_used_at: c.last_used_at.map(|t| t.to_rfc3339()),
        })
        .collect();
    Ok(Json(infos))
}

async fn delete_credential(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_webauthn_credential(id, auth.0.id).await?;
    Ok(StatusCode::NO_CONTENT)
}
