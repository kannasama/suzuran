use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    models::User,
    services::auth::AuthService,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup-status", get(setup_status))
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

#[derive(Serialize)]
struct SetupStatusResponse {
    needs_setup: bool,
}

async fn setup_status(
    State(state): State<AppState>,
) -> Result<Json<SetupStatusResponse>, AppError> {
    let needs_setup = state.db.count_users().await? == 0;
    Ok(Json(SetupStatusResponse { needs_setup }))
}

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct UserResponse {
    id: i64,
    username: String,
    email: String,
    role: String,
    display_name: Option<String>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            role: u.role,
            display_name: u.display_name,
        }
    }
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Registration is first-run only; reject once any user exists
    if state.db.count_users().await? > 0 {
        return Err(AppError::Forbidden);
    }
    let role = "admin";

    if body.password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }

    let hash = AuthService::hash_password(&body.password).map_err(AppError::Internal)?;

    let user = state
        .db
        .create_user(&body.username, &body.email, &hash, role)
        .await?;

    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    use crate::services::auth::LoginResult;

    match AuthService::login(&state.db, &body.username, &body.password, &state.config.jwt_secret)
        .await?
    {
        LoginResult::Session { token } => {
            let cookie = Cookie::build(("session", token))
                .http_only(true)
                .same_site(SameSite::Strict)
                .max_age(Duration::days(30))
                .path("/")
                .build();
            Ok((jar.add(cookie), StatusCode::NO_CONTENT).into_response())
        }
        LoginResult::TwoFactorRequired { token } => Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "two_factor_required": true,
                "token": token
            })),
        )
            .into_response()),
    }
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    _auth: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let token = jar
        .get("session")
        .map(|c| c.value().to_string())
        .unwrap_or_default();
    let token_hash = AuthService::hash_token(&token);
    if let Some(session) = state.db.find_session_by_token_hash(&token_hash).await? {
        state.db.delete_session(session.id).await?;
    }

    let removed = jar.remove(Cookie::from("session"));
    Ok((removed, StatusCode::NO_CONTENT))
}

async fn me(auth: AuthUser) -> Json<UserResponse> {
    Json(UserResponse::from(auth.0))
}
