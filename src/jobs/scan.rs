use std::path::Path;
use std::sync::Arc;

use crate::{
    dal::Store,
    error::AppError,
    jobs::{JobHandler, ScanPayload},
    scanner,
};

pub struct ScanJobHandler;

#[async_trait::async_trait]
impl JobHandler for ScanJobHandler {
    async fn run(
        &self,
        db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let p: ScanPayload = serde_json::from_value(payload)
            .map_err(|e| AppError::BadRequest(format!("invalid scan payload: {e}")))?;

        let library = db.get_library(p.library_id).await?
            .ok_or_else(|| AppError::NotFound(format!("library {} not found", p.library_id)))?;

        let root = Path::new(&library.root_path);
        let result = scanner::scan_library(&db, library.id, root).await?;

        tracing::info!(
            library_id = library.id,
            inserted = result.inserted,
            updated = result.updated,
            removed = result.removed,
            errors = result.errors.len(),
            "scan complete"
        );

        Ok(serde_json::json!({
            "inserted": result.inserted,
            "updated": result.updated,
            "removed": result.removed,
            "errors": result.errors,
        }))
    }
}
