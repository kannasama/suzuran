pub mod auth;
pub mod middleware;
pub mod settings;
pub mod themes;
pub mod totp;
pub mod webauthn;

use axum::Router;
use crate::state::AppState;

pub fn api_router(_state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/totp", totp::router())
        .nest("/webauthn", webauthn::router())
        .nest("/settings", settings::router())
        .nest("/themes", themes::router())
}
