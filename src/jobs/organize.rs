use std::{collections::HashMap, path::{Component, Path}, sync::Arc};
use serde_json::Value;
use tokio::fs;
use crate::{
    dal::Store,
    error::AppError,
    jobs::{copy_companions, remove_empty_dirs, JobHandler, OrganizePayload, COMPANION_EXTS},
    organizer::rules::apply_rules,
};

pub struct OrganizeJobHandler;

/// Probe file bytes for audio magic signatures.
/// Returns a bare extension string (e.g. "flac") or None if unrecognised.
async fn probe_audio_ext(path: &Path) -> Option<&'static str> {
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

/// Replace characters that are illegal or problematic on NFS/NTFS/exFAT
/// in a single path component (not a full path — no slashes expected).
fn sanitize_path_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            // Colon → modifier letter colon (U+A789); widely used by beets/Picard
            ':' => '꞉',
            // Other NTFS-illegal chars → safe replacements
            '\\' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        // Strip leading/trailing spaces and dots (illegal as NTFS trailing chars)
        .collect::<String>()
        .trim_matches(|c: char| c == ' ' || c == '.')
        .to_string()
}

/// Sanitize every component of a relative path produced by an org rule.
fn sanitize_rule_path(path: &str) -> String {
    path.split('/')
        .map(sanitize_path_component)
        .collect::<Vec<_>>()
        .join("/")
}



#[async_trait::async_trait]
impl JobHandler for OrganizeJobHandler {
    async fn run(&self, db: Arc<dyn Store>, payload: Value) -> Result<Value, AppError> {
        let p: OrganizePayload = serde_json::from_value(payload)
            .map_err(|e| AppError::BadRequest(format!("invalid organize payload: {e}")))?;

        let track = db.get_track(p.track_id).await?
            .ok_or_else(|| AppError::NotFound(format!("track {} not found", p.track_id)))?;

        // Skip ingest/ tracks — organize only applies to source/ and derived tracks
        if track.relative_path.starts_with("ingest/") {
            tracing::info!(track_id = p.track_id, path = %track.relative_path, "organize: track is in ingest/ — skipped");
            return Ok(serde_json::json!({ "skipped": true, "reason": "ingest track" }));
        }

        let library = db.get_library(track.library_id).await?
            .ok_or_else(|| AppError::NotFound(format!("library {} not found", track.library_id)))?;

        // ── Derived track path: mirror source track's organized path ─────────
        // Derived tracks have library_profile_id set and live under derived_dir_name/.
        // Rather than re-applying the org rule, we mirror the source track's current
        // (already-organized) relative path under the derived dir, preserving extension.
        if let Some(profile_id) = track.library_profile_id {
            let profile = db.get_library_profile(profile_id).await?;
            let source_links = db.list_source_tracks(track.id).await.unwrap_or_default();

            let Some(link) = source_links.first() else {
                tracing::warn!(track_id = p.track_id, "organize: derived track has no source link — skipped");
                return Ok(serde_json::json!({ "skipped": true, "reason": "no source link" }));
            };

            let source = db.get_track(link.source_track_id).await?
                .ok_or_else(|| AppError::NotFound(format!("source track {} not found", link.source_track_id)))?;

            // Strip source/ prefix and extension from source path, then reattach derived ext
            let source_stem_path = Path::new(&source.relative_path)
                .with_extension("")
                .to_string_lossy()
                .trim_start_matches("source/")
                .to_string();

            let derived_ext = Path::new(&track.relative_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();

            if derived_ext.is_empty() {
                tracing::warn!(track_id = p.track_id, path = %track.relative_path,
                    "organize: derived track has no extension — skipped");
                return Ok(serde_json::json!({ "skipped": true, "reason": "unknown audio format" }));
            }

            let new_relative = format!("{}/{}.{}", profile.derived_dir_name, source_stem_path, derived_ext);
            let old_abs = Path::new(&library.root_path).join(&track.relative_path);
            let new_abs = Path::new(&library.root_path).join(&new_relative);

            if !old_abs.exists() {
                tracing::warn!(track_id = p.track_id, path = %track.relative_path,
                    "organize: derived file not found at DB path — skipped");
                return Ok(serde_json::json!({ "skipped": true, "reason": "source file not found" }));
            }

            if old_abs == new_abs {
                tracing::info!(track_id = p.track_id, path = %new_relative, "organize: derived file already at correct location");
                return Ok(serde_json::json!({ "skipped": true, "reason": "already organized", "path": new_relative }));
            }

            if p.dry_run {
                return Ok(serde_json::json!({ "dry_run": true, "proposed_path": new_relative }));
            }

            tracing::info!(track_id = p.track_id, old_path = %track.relative_path, new_path = %new_relative, "organize: moving derived track");

            if let Some(parent) = new_abs.parent() {
                fs::create_dir_all(parent).await.map_err(|e| AppError::Internal(e.into()))?;
            }
            fs::rename(&old_abs, &new_abs).await.map_err(|e| AppError::Internal(e.into()))?;
            db.update_track_path(track.id, &new_relative, &track.file_hash).await?;

            // Move companion files from old derived dir, sweep empty dirs
            move_companions(&old_abs, &new_abs).await;
            if let Some(old_dir) = old_abs.parent() {
                let derived_root = Path::new(&library.root_path).join(&profile.derived_dir_name);
                remove_empty_dirs(old_dir.to_path_buf(), &derived_root).await;
            }

            // Copy companion files from the source track's directory into the derived dir
            let source_dir = Path::new(&library.root_path)
                .join(&source.relative_path)
                .parent()
                .map(|p| p.to_path_buf());
            if let (Some(src_dir), Some(dest_dir)) = (source_dir, new_abs.parent()) {
                copy_companions(&src_dir, dest_dir).await;
            }

            return Ok(serde_json::json!({
                "moved": true,
                "old_path": track.relative_path,
                "new_path": new_relative,
            }));
        }

        // ── Source track path: apply org rule ────────────────────────────────

        let tags: HashMap<String, String> = track.tags
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let rule_pairs: Vec<(Option<Value>, String)> = if let Some(rule_id) = library.organization_rule_id {
            match db.get_organization_rule(rule_id).await? {
                Some(r) if r.enabled => vec![(r.conditions, r.path_template)],
                _ => vec![],
            }
        } else {
            vec![]
        };

        let rule_output = apply_rules(&rule_pairs, &tags).map(|raw| sanitize_rule_path(&raw));

        // Guard against path traversal in rule output
        if let Some(ref path) = rule_output {
            if Path::new(path).components().any(|c| {
                matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))
            }) {
                return Err(AppError::BadRequest(format!(
                    "organize rule produced an unsafe path: {path}"
                )));
            }
        }

        if p.dry_run {
            let proposed = rule_output.map(|raw| format!("source/{raw}"));
            return Ok(serde_json::json!({ "dry_run": true, "proposed_path": proposed }));
        }

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

        let old_abs = Path::new(&library.root_path).join(&track.relative_path);

        let ext = {
            let from_path = old_abs.extension().and_then(|e| e.to_str()).map(|s| s.to_string());
            match from_path {
                Some(e) if !e.is_empty() => e,
                _ => match probe_audio_ext(&old_abs).await {
                    Some(e) => e.to_string(),
                    None => {
                        tracing::warn!(track_id = p.track_id, path = %track.relative_path,
                            "organize: cannot determine audio format — skipped");
                        return Ok(serde_json::json!({ "skipped": true, "reason": "unknown audio format" }));
                    }
                },
            }
        };

        let new_relative = format!("source/{rule_output}.{ext}");
        let new_abs = Path::new(&library.root_path).join(&new_relative);

        if !old_abs.exists() {
            tracing::warn!(track_id = p.track_id, path = %track.relative_path, "organize: source file not found at DB path — skipped");
            return Ok(serde_json::json!({ "skipped": true, "reason": "source file not found", "db_path": track.relative_path }));
        }

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

        // Move companion files then sweep empty dirs up to source/ root
        move_companions(&old_abs, &new_abs).await;
        if let Some(old_dir) = old_abs.parent() {
            let source_root = Path::new(&library.root_path).join("source");
            remove_empty_dirs(old_dir.to_path_buf(), &source_root).await;
        }

        // Enqueue organize jobs for derived tracks linked to this source track
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

/// Move companion files (art, cue sheets, logs, etc.) from old_abs's directory
/// to new_abs's directory. Leaves audio files and other types in place.
async fn move_companions(old_abs: &Path, new_abs: &Path) {
    let new_dir = match new_abs.parent() {
        Some(d) => d.to_path_buf(),
        None => return,
    };
    let old_dir = match old_abs.parent() {
        Some(d) => d,
        None => return,
    };
    let mut entries = match fs::read_dir(old_dir).await {
        Ok(e) => e,
        Err(_) => return,
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_file() { continue; }
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if !COMPANION_EXTS.contains(&ext.as_str()) { continue; }
        let Some(fname) = path.file_name() else { continue };
        let dest = new_dir.join(fname);
        if let Err(e) = fs::rename(&path, &dest).await {
            tracing::warn!(src = %path.display(), dst = %dest.display(), error = %e, "organize: failed to move companion file");
        } else {
            tracing::info!(file = %fname.to_string_lossy(), "organize: moved companion file");
        }
    }
}
