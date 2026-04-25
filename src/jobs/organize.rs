use std::{collections::HashMap, path::Component, sync::Arc};
use serde_json::Value;
use tokio::fs;
use crate::{
    dal::Store,
    error::AppError,
    jobs::{JobHandler, OrganizePayload},
    organizer::rules::apply_rules,
};

pub struct OrganizeJobHandler;

/// Probe file bytes for audio magic signatures.
/// Returns a bare extension string (e.g. "flac") or None if unrecognised.
async fn probe_audio_ext(path: &std::path::Path) -> Option<&'static str> {
    let buf = tokio::fs::read(path).await.ok()?;
    if buf.starts_with(b"fLaC") { return Some("flac"); }
    if buf.starts_with(b"OggS") { return Some("ogg"); }
    if buf.starts_with(b"RIFF") { return Some("wav"); }
    if buf.starts_with(b"wvpk") { return Some("wv"); }
    if buf.len() >= 8 && &buf[4..8] == b"ftyp" { return Some("m4a"); }
    if buf.starts_with(b"ID3") || (buf.len() >= 2 && buf[0] == 0xFF && buf[1] >= 0xE0) {
        return Some("mp3");
    }
    None
}

#[async_trait::async_trait]
impl JobHandler for OrganizeJobHandler {
    async fn run(&self, db: Arc<dyn Store>, payload: Value) -> Result<Value, AppError> {
        let p: OrganizePayload = serde_json::from_value(payload)
            .map_err(|e| AppError::BadRequest(format!("invalid organize payload: {e}")))?;

        let track = db.get_track(p.track_id).await?
            .ok_or_else(|| AppError::NotFound(format!("track {} not found", p.track_id)))?;

        // Skip ingest/ tracks — organize only applies to active source/ tracks
        if track.relative_path.starts_with("ingest/") {
            tracing::info!(track_id = p.track_id, path = %track.relative_path, "organize: track is in ingest/ — skipped");
            return Ok(serde_json::json!({ "skipped": true, "reason": "ingest track" }));
        }

        let library = db.get_library(track.library_id).await?
            .ok_or_else(|| AppError::NotFound(format!("library {} not found", track.library_id)))?;

        // Build tag map from the track's full tags JSON
        let tags: HashMap<String, String> = track.tags
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Load the single rule the library subscribes to (if any)
        let rule_pairs: Vec<(Option<Value>, String)> = if let Some(rule_id) = library.organization_rule_id {
            match db.get_organization_rule(rule_id).await? {
                Some(r) if r.enabled => vec![(r.conditions, r.path_template)],
                _ => vec![],
            }
        } else {
            vec![]
        };

        let rule_output = apply_rules(&rule_pairs, &tags);

        // Guard against path traversal in rule output, regardless of dry_run mode
        if let Some(ref path) = rule_output {
            if std::path::Path::new(path).components().any(|c| {
                matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))
            }) {
                return Err(AppError::BadRequest(format!(
                    "organize rule produced an unsafe path: {path}"
                )));
            }
        }

        // dry_run: always return the proposed path (null if no rule matched), never move anything
        if p.dry_run {
            let proposed = rule_output.map(|raw| format!("source/{raw}"));
            return Ok(serde_json::json!({ "dry_run": true, "proposed_path": proposed }));
        }

        // Non-dry-run: a rule must have matched; log and skip gracefully if not
        if rule_pairs.is_empty() {
            tracing::info!(track_id = p.track_id, "organize: no rule configured for library — skipped");
            return Ok(serde_json::json!({ "skipped": true, "reason": "no rule configured" }));
        }

        let rule_output = match rule_output {
            Some(p) => p,
            None => {
                tracing::info!(track_id = p.track_id, current_path = %track.relative_path, "organize: no rule matched track tags — skipped");
                return Ok(serde_json::json!({ "skipped": true, "reason": "no rule matched" }));
            }
        };

        // Determine file extension from DB path; if absent (corrupted extensionless file),
        // probe the actual file bytes.
        let old_abs = std::path::Path::new(&library.root_path).join(&track.relative_path);

        let ext = {
            let from_path = old_abs.extension().and_then(|e| e.to_str()).map(|s| s.to_string());
            match from_path {
                Some(e) if !e.is_empty() => e,
                _ => {
                    // Extension missing — try magic-byte probe
                    match probe_audio_ext(&old_abs).await {
                        Some(e) => e.to_string(),
                        None => {
                            tracing::warn!(track_id = p.track_id, path = %track.relative_path,
                                "organize: cannot determine audio format — skipped");
                            return Ok(serde_json::json!({ "skipped": true, "reason": "unknown audio format" }));
                        }
                    }
                }
            }
        };

        // Enforce source/ prefix and append extension
        let new_relative = format!("source/{rule_output}.{ext}");

        let new_abs = std::path::Path::new(&library.root_path).join(&new_relative);

        // Guard: source file must exist. If not, the DB path is stale — log and skip.
        if !old_abs.exists() {
            tracing::warn!(track_id = p.track_id, path = %track.relative_path, "organize: source file not found at DB path — skipped");
            return Ok(serde_json::json!({ "skipped": true, "reason": "source file not found", "db_path": track.relative_path }));
        }

        // If the resolved absolute paths are the same the file is already at the correct location.
        if old_abs == new_abs {
            tracing::info!(track_id = p.track_id, path = %new_relative, "organize: file already at rule-dictated location");
            return Ok(serde_json::json!({ "skipped": true, "reason": "already organized", "path": new_relative }));
        }

        tracing::info!(track_id = p.track_id, old_path = %track.relative_path, new_path = %new_relative, "organize: moving track");

        if let Some(parent) = new_abs.parent() {
            fs::create_dir_all(parent).await.map_err(|e| AppError::Internal(e.into()))?;
        }
        fs::rename(&old_abs, &new_abs).await.map_err(|e| AppError::Internal(e.into()))?;

        db.update_track_path(track.id, &new_relative, &track.file_hash).await?;

        // Move companion files (art, cue sheets, logs, etc.) from the old directory to the new one.
        // Only moves files whose extensions are known companion types; leaves any remaining audio
        // files in place (other tracks that haven't been organized yet).
        const COMPANION_EXTS: &[&str] = &[
            "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff",
            "cue", "log", "nfo", "txt", "m3u", "m3u8",
        ];
        let new_dir = new_abs.parent().map(|p| p.to_path_buf());
        if let (Some(old_dir), Some(new_dir)) = (old_abs.parent(), new_dir) {
            if let Ok(mut dir_entries) = fs::read_dir(old_dir).await {
                while let Ok(Some(entry)) = dir_entries.next_entry().await {
                    let path = entry.path();
                    if !path.is_file() { continue; }
                    let file_ext = path.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    if !COMPANION_EXTS.contains(&file_ext.as_str()) { continue; }
                    let Some(fname) = path.file_name() else { continue };
                    let dest = new_dir.join(fname);
                    if let Err(e) = fs::rename(&path, &dest).await {
                        tracing::warn!(src = %path.display(), dst = %dest.display(), error = %e, "organize: failed to move companion file");
                    } else {
                        tracing::info!(file = %fname.to_string_lossy(), "organize: moved companion file");
                    }
                }
            }
        }

        // Enqueue organize jobs for any derived tracks linked to this source track.
        let derived = db.list_derived_tracks(track.id).await.unwrap_or_default();
        for link in &derived {
            if let Err(e) = db.enqueue_job(
                "organize",
                serde_json::json!({"track_id": link.derived_track_id, "dry_run": false}),
                5,
            ).await {
                tracing::warn!(derived_track_id = link.derived_track_id, error = %e,
                    "organize: failed to enqueue re-organize for derived track");
            }
        }
        if !derived.is_empty() {
            tracing::info!(track_id = p.track_id, derived_count = derived.len(),
                "organize: enqueued re-organize for derived tracks");
        }

        Ok(serde_json::json!({
            "moved": true,
            "old_path": track.relative_path,
            "new_path": new_relative,
        }))
    }
}
