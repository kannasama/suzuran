use axum::{async_trait, extract::FromRequestParts, http::request::Parts};

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    models::User,
    state::AppState,
};

/// Requires authentication AND role `admin` or `library_admin`.
/// Used to gate library management features (virtual libraries, organization
/// rules) below the full-admin level, in anticipation of streaming-only users.
pub struct LibraryAdminUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for LibraryAdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        if user.role != "admin" && user.role != "library_admin" {
            return Err(AppError::Forbidden);
        }
        Ok(LibraryAdminUser(user))
    }
}
