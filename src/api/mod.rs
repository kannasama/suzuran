pub mod auth;
pub mod middleware;

use axum::Router;
use crate::state::AppState;

pub fn api_router(state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
}
