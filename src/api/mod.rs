pub mod art;
pub mod art_profiles;
pub mod auth;
pub mod encoding_profiles;
pub mod ingest;
pub mod issues;
pub mod jobs;
pub mod libraries;
pub mod library_profiles;
pub mod middleware;
pub mod migrate;
pub mod organization_rules;
pub mod search;
pub mod settings;
pub mod tag_suggestions;
pub mod user_prefs;
pub mod themes;
pub mod totp;
pub mod transcode;
pub mod tracks;
pub mod uploads;
pub mod virtual_libraries;
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
        .nest("/virtual-libraries", virtual_libraries::router())
        .nest("/uploads", uploads::router())
        .nest("/library-profiles", library_profiles::router())
        .nest("/ingest", ingest::router())
        .nest("/issues", issues::router())
        .nest("/user/prefs", user_prefs::router())
        .nest("/search", search::router())
        .nest("/admin", migrate::router())
        .merge(transcode::router())
        .merge(art::router())
}
