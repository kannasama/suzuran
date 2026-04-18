use axum::{async_trait, extract::FromRequestParts, http::request::Parts};

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    models::User,
    state::AppState,
};

/// Requires authentication AND admin role.
pub struct AdminUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        if user.role != "admin" {
            return Err(AppError::Forbidden);
        }
        Ok(AdminUser(user))
    }
}
