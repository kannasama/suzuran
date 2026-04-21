use std::sync::Arc;

use tokio::process::Command;

use crate::{dal::Store, error::AppError};

pub struct FingerprintJobHandler;

#[async_trait::async_trait]
impl super::JobHandler for FingerprintJobHandler {
    async fn run(
        &self,
        db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"]
            .as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;

        let track = db
            .get_track(track_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("track {track_id} not found")))?;

        let library = db
            .get_library(track.library_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("library {} not found", track.library_id)))?;

        let full_path = format!(
            "{}/{}",
            library.root_path.trim_end_matches('/'),
            track.relative_path.trim_start_matches('/')
        );

        let out = Command::new("fpcalc")
            .args(["-json", &full_path])
            .output()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("fpcalc spawn failed: {e}")))?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(AppError::Internal(anyhow::anyhow!("fpcalc failed: {stderr}")));
        }

        let json: serde_json::Value = serde_json::from_slice(&out.stdout)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("fpcalc json parse: {e}")))?;

        let fingerprint = json["fingerprint"]
            .as_str()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no fingerprint in fpcalc output")))?;
        let duration = json["duration"].as_f64().unwrap_or(0.0);

        db.update_track_fingerprint(track_id, fingerprint, duration).await?;

        // Always enqueue mb_lookup after fingerprinting
        db.enqueue_job("mb_lookup", serde_json::json!({"track_id": track_id}), 4).await?;

        Ok(serde_json::json!({
            "track_id": track_id,
            "fingerprint": fingerprint,
            "duration_secs": duration,
        }))
    }
}
