pub mod postgres;
pub mod sqlite;

use crate::error::AppError;

#[async_trait::async_trait]
pub trait Store: Send + Sync {
    /// Verify the DB connection is alive.
    async fn health_check(&self) -> Result<(), AppError>;
}
