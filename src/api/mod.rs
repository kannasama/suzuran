pub mod art_profiles;
pub mod auth;
pub mod encoding_profiles;
pub mod jobs;
pub mod libraries;
pub mod middleware;
pub mod organization_rules;
pub mod settings;
pub mod tag_suggestions;
pub mod themes;
pub mod totp;
pub mod tracks;
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
        .nest("/libraries", libraries::router())
        .nest("/jobs", jobs::router())
        .nest("/tracks", tracks::router())
        .nest("/organization-rules", organization_rules::router())
        .nest("/tag-suggestions", tag_suggestions::router())
        .nest("/encoding-profiles", encoding_profiles::router())
        .nest("/art-profiles", art_profiles::router())
}
