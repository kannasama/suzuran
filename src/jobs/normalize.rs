use std::{path::Path, sync::Arc};

use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::{
    dal::Store,
    error::AppError,
    jobs::{
        cue_split::hash_file,
        transcode::{build_ffmpeg_args, codec_extension},
    },
    services::transcode_compat::is_compatible,
};

pub struct NormalizeJobHandler {
    store: Arc<dyn Store>,
}

impl NormalizeJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl super::JobHandler for NormalizeJobHandler {
    async fn run(
        &self,
        _db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"]
            .as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;

        // 1. Fetch track and library
        let track = self
            .store
            .get_track(track_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("track {track_id} not found")))?;

        let library = self
            .store
            .get_library(track.library_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("library {} not found", track.library_id)))?;

        // 2. Skip if normalize flag is off
        if !library.normalize_on_ingest {
            self.store
                .enqueue_job("mb_lookup", serde_json::json!({"track_id": track_id}), 4)
                .await?;
            return Ok(serde_json::json!({
                "status": "skipped",
                "reason": "normalize_on_ingest is false",
                "track_id": track_id,
            }));
        }

        // 3. Require encoding profile
        let ep_id = match library.encoding_profile_id {
            Some(id) => id,
            None => {
                self.store
                    .enqueue_job("mb_lookup", serde_json::json!({"track_id": track_id}), 4)
                    .await?;
                return Ok(serde_json::json!({
                    "status": "skipped",
                    "reason": "library has no encoding_profile_id",
                    "track_id": track_id,
                }));
            }
        };

        let profile = self.store.get_encoding_profile(ep_id).await?;

        // 4. Check if already in target format
        let src_ext = Path::new(&track.relative_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let target_ext = codec_extension(&profile.codec);

        if src_ext == target_ext {
            self.store
                .enqueue_job("mb_lookup", serde_json::json!({"track_id": track_id}), 4)
                .await?;
            return Ok(serde_json::json!({
                "status": "skipped",
                "reason": "track already in target format",
                "track_id": track_id,
            }));
        }

        // 5. Compatibility check
        if !is_compatible(
            &src_ext,
            track.sample_rate,
            track.bit_depth,
            track.bitrate,
            &profile,
        ) {
            // Incompatible source — still enqueue mb_lookup so the track is not orphaned
            self.store
                .enqueue_job("mb_lookup", serde_json::json!({"track_id": track_id}), 4)
                .await?;
            return Ok(serde_json::json!({
                "status": "skipped",
                "reason": "source/profile combination not compatible (quality guard)",
                "track_id": track_id,
            }));
        }

        // 6. Build source and output paths
        let src_path = format!(
            "{}/{}",
            library.root_path.trim_end_matches('/'),
            track.relative_path.trim_start_matches('/')
        );

        let src_stem = Path::new(&track.relative_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("track");

        // Place output alongside source (same directory), new extension
        let src_dir = Path::new(&track.relative_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");

        let out_rel = if src_dir.is_empty() {
            format!("{}.{}", src_stem, target_ext)
        } else {
            format!("{}/{}.{}", src_dir, src_stem, target_ext)
        };

        let out_path_str = format!(
            "{}/{}",
            library.root_path.trim_end_matches('/'),
            out_rel.trim_start_matches('/')
        );
        let out_path = Path::new(&out_path_str);

        // Create output parent directory if needed
        if let Some(parent) = out_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("create_dir_all: {e}")))?;
        }

        // 7. Run ffmpeg
        let mut args: Vec<String> = vec!["-i".into(), src_path.clone()];
        args.extend(build_ffmpeg_args(&profile));
        args.extend(["-progress".into(), "pipe:1".into(), "-y".into()]);
        args.push(out_path_str.clone());

        let mut child = Command::new("ffmpeg")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("ffmpeg spawn failed: {e}")))?;

        // Drain stdout (progress output)
        if let Some(mut stdout) = child.stdout.take() {
            let mut buf = Vec::new();
            let _ = stdout.read_to_end(&mut buf).await;
        }

        let status = child
            .wait()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("ffmpeg wait failed: {e}")))?;

        if !status.success() {
            return Err(AppError::Internal(anyhow::anyhow!(
                "ffmpeg exited with non-zero status for track {track_id}"
            )));
        }

        // 8. Verify output exists and is non-empty
        let out_meta = tokio::fs::metadata(&out_path_str).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("output file missing after ffmpeg: {e}"))
        })?;
        if out_meta.len() == 0 {
            return Err(AppError::Internal(anyhow::anyhow!(
                "ffmpeg produced empty output for track {track_id}"
            )));
        }

        // 9. Delete source file
        tokio::fs::remove_file(&src_path)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to delete source file: {e}")))?;

        // 10. Hash output and update track record
        let new_hash = hash_file(out_path)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("hash_file: {e}")))?;

        self.store
            .update_track_path(track_id, &out_rel, &new_hash)
            .await?;

        // 11. Enqueue mb_lookup
        self.store
            .enqueue_job("mb_lookup", serde_json::json!({"track_id": track_id}), 4)
            .await?;

        Ok(serde_json::json!({
            "status": "completed",
            "track_id": track_id,
            "old_path": track.relative_path,
            "new_path": out_rel,
        }))
    }
}
