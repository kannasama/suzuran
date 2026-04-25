use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::{
    cue::{parse_cue, read_cue_file},
    dal::{Store, UpsertTrack},
    error::AppError,
    tagger,
};

const AUDIO_EXTENSIONS: &[&str] = &[
    "flac", "m4a", "mp3", "opus", "ogg", "aac", "wav", "aiff",
    "wv",   // WavPack (lossless)
    "ape",  // Monkey's Audio (lossless)
    "tta",  // TrueAudio (lossless)
];

pub struct ScanResult {
    pub inserted: usize,
    pub updated: usize,
    pub removed: usize,
    pub errors: Vec<String>,
}

pub async fn scan_library(
    db: &Arc<dyn Store>,
    library_id: i64,
    root_path: &Path,
) -> Result<ScanResult, AppError> {
    let mut result = ScanResult { inserted: 0, updated: 0, removed: 0, errors: vec![] };

    // Look up library metadata for format and tag encoding.
    let library = db.get_library(library_id).await?;
    let tag_encoding = library.as_ref().map(|l| l.tag_encoding.as_str()).unwrap_or("utf8").to_string();

    let ingest_dir = root_path.join("ingest");
    let source_dir = root_path.join("source");

    // Load all known tracks for this library (for dedup/status updates).
    let existing: HashMap<String, (i64, String)> = db
        .list_track_paths_by_library(library_id)
        .await?
        .into_iter()
        .map(|(id, path, hash)| (path, (id, hash)))
        .collect();

    let mut seen_paths: HashSet<String> = HashSet::new();

    // ── Walk 1: ingest/ — staged tracks ──────────────────────────────────────
    if ingest_dir.exists() {
        // Detect CUE files and their paired audio within ingest/
        let mut cue_backed_audio_ingest: HashSet<PathBuf> = HashSet::new();
        let mut cue_files_ingest: Vec<PathBuf> = Vec::new();

        for entry in WalkDir::new(&ingest_dir).follow_links(true).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path().to_path_buf();
            if p.extension().and_then(|e| e.to_str()) == Some("cue") {
                if let Ok(content) = read_cue_file(&p, &tag_encoding) {
                    if let Ok(sheet) = parse_cue(&content) {
                        let audio = p.parent().unwrap_or(&ingest_dir).join(&sheet.audio_file);
                        if audio.exists() {
                            cue_backed_audio_ingest.insert(audio);
                            cue_files_ingest.push(p);
                        }
                    }
                }
            }
        }

        for entry in WalkDir::new(&ingest_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let abs_path = entry.path().to_path_buf();

            if cue_backed_audio_ingest.contains(&abs_path) {
                continue;
            }

            let ext = abs_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }

            // relative_path is relative to root_path, so it starts with "ingest/"
            let rel_path = match abs_path.strip_prefix(root_path) {
                Ok(p) => p.to_string_lossy().into_owned(),
                Err(_) => continue,
            };

            seen_paths.insert(rel_path.clone());

            let hash = match hash_file(&abs_path).await {
                Ok(h) => h,
                Err(e) => {
                    result.errors.push(format!("{rel_path}: hash error: {e}"));
                    continue;
                }
            };

            let is_new = !existing.contains_key(&rel_path);
            let needs_scan = match existing.get(&rel_path) {
                Some((_, existing_hash)) => existing_hash != &hash,
                None => true,
            };

            if !needs_scan {
                // Hash unchanged — ensure the DB record is "staged" in case the file was
                // previously removed and dropped back into ingest/.
                if let Some((id, _)) = existing.get(&rel_path) {
                    tracing::debug!(path = %rel_path, track_id = id, "scanner/ingest: hash unchanged, re-staging existing record");
                    if let Err(e) = db.set_track_status(*id, "staged").await {
                        tracing::warn!(path = %rel_path, error = %e, "scanner/ingest: failed to re-stage track");
                    }
                } else {
                    // needs_scan=false but path not in existing — shouldn't happen, log it
                    tracing::warn!(path = %rel_path, "scanner/ingest: needs_scan=false but path absent from existing map");
                }
                continue;
            }

            tracing::debug!(path = %rel_path, is_new, "scanner/ingest: scanning file");

            let abs_path_clone = abs_path.clone();
            let tag_result = tokio::task::spawn_blocking(move || {
                tagger::read_tags(&abs_path_clone)
            })
            .await;

            let (mut tags_map, audio_props) = match tag_result {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    result.errors.push(format!("{rel_path}: tag read error: {e}"));
                    (HashMap::new(), tagger::AudioProperties::default())
                }
                Err(e) => {
                    result.errors.push(format!("{rel_path}: spawn error: {e}"));
                    continue;
                }
            };

            if tag_encoding == "sjis" {
                for val in tags_map.values_mut() {
                    *val = tagger::redecode_latin1_as_sjis(val);
                }
            }

            let tags_json = serde_json::to_value(&tags_map).unwrap_or(serde_json::json!({}));

            let upsert = UpsertTrack {
                library_id,
                relative_path: rel_path.clone(),
                file_hash: hash,
                title: tags_map.get("title").cloned(),
                artist: tags_map.get("artist").cloned(),
                albumartist: tags_map.get("albumartist").cloned(),
                album: tags_map.get("album").cloned(),
                tracknumber: tags_map.get("tracknumber").cloned(),
                discnumber: tags_map.get("discnumber").cloned(),
                totaldiscs: tags_map.get("totaldiscs").cloned(),
                totaltracks: tags_map.get("totaltracks").cloned(),
                date: tags_map.get("date").cloned(),
                genre: tags_map.get("genre").cloned(),
                composer: tags_map.get("composer").cloned(),
                label: tags_map.get("label").cloned(),
                catalognumber: tags_map.get("catalognumber").cloned(),
                tags: tags_json,
                duration_secs: audio_props.duration_secs,
                bitrate: audio_props.bitrate,
                sample_rate: audio_props.sample_rate,
                channels: audio_props.channels,
                bit_depth: audio_props.bit_depth,
                has_embedded_art: audio_props.has_embedded_art,
                status: "staged".into(),
                library_profile_id: None,
            };

            let track = db.upsert_track(upsert).await?;

            if is_new {
                result.inserted += 1;
                tracing::debug!(path = %rel_path, track_id = track.id, "scanner/ingest: inserted new staged track");
                // Enqueue fingerprint for newly staged tracks
                db.enqueue_job(
                    "fingerprint",
                    serde_json::json!({"track_id": track.id}),
                    5,
                )
                .await?;
            } else {
                result.updated += 1;
                tracing::debug!(path = %rel_path, track_id = track.id, "scanner/ingest: updated existing track");
            }
        }

        // Enqueue CUE split jobs found in ingest/
        for cue_path in &cue_files_ingest {
            let cue_str = cue_path.to_string_lossy().to_string();
            let existing_jobs = db
                .list_jobs_by_type_and_payload_key("cue_split", "cue_path", &cue_str)
                .await?;
            let active = existing_jobs
                .iter()
                .any(|j| j.status == "pending" || j.status == "running");
            if !active {
                db.enqueue_job(
                    "cue_split",
                    serde_json::json!({
                        "cue_path": cue_str,
                        "library_id": library_id,
                    }),
                    6,
                )
                .await?;
            }
        }
    }

    // ── Walk 2: source/ — active tracks ──────────────────────────────────────
    if source_dir.exists() {
        // Detect CUE files and their paired audio within source/
        let mut cue_backed_audio_source: HashSet<PathBuf> = HashSet::new();
        let mut cue_files_source: Vec<PathBuf> = Vec::new();

        for entry in WalkDir::new(&source_dir).follow_links(true).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path().to_path_buf();
            if p.extension().and_then(|e| e.to_str()) == Some("cue") {
                if let Ok(content) = read_cue_file(&p, &tag_encoding) {
                    if let Ok(sheet) = parse_cue(&content) {
                        let audio = p.parent().unwrap_or(&source_dir).join(&sheet.audio_file);
                        if audio.exists() {
                            cue_backed_audio_source.insert(audio);
                            cue_files_source.push(p);
                        }
                    }
                }
            }
        }

        for entry in WalkDir::new(&source_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let abs_path = entry.path().to_path_buf();

            if cue_backed_audio_source.contains(&abs_path) {
                continue;
            }

            let ext = abs_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }

            // relative_path is relative to root_path, so it starts with "source/"
            let rel_path = match abs_path.strip_prefix(root_path) {
                Ok(p) => p.to_string_lossy().into_owned(),
                Err(_) => continue,
            };

            seen_paths.insert(rel_path.clone());

            let hash = match hash_file(&abs_path).await {
                Ok(h) => h,
                Err(e) => {
                    result.errors.push(format!("{rel_path}: hash error: {e}"));
                    continue;
                }
            };

            let is_new = !existing.contains_key(&rel_path);
            let needs_scan = match existing.get(&rel_path) {
                Some((_, existing_hash)) => existing_hash != &hash,
                None => true,
            };

            if !needs_scan {
                continue;
            }

            let abs_path_clone = abs_path.clone();
            let tag_result = tokio::task::spawn_blocking(move || {
                tagger::read_tags(&abs_path_clone)
            })
            .await;

            let (mut tags_map, audio_props) = match tag_result {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    result.errors.push(format!("{rel_path}: tag read error: {e}"));
                    (HashMap::new(), tagger::AudioProperties::default())
                }
                Err(e) => {
                    result.errors.push(format!("{rel_path}: spawn error: {e}"));
                    continue;
                }
            };

            if tag_encoding == "sjis" {
                for val in tags_map.values_mut() {
                    *val = tagger::redecode_latin1_as_sjis(val);
                }
            }

            let tags_json = serde_json::to_value(&tags_map).unwrap_or(serde_json::json!({}));

            let upsert = UpsertTrack {
                library_id,
                relative_path: rel_path.clone(),
                file_hash: hash,
                title: tags_map.get("title").cloned(),
                artist: tags_map.get("artist").cloned(),
                albumartist: tags_map.get("albumartist").cloned(),
                album: tags_map.get("album").cloned(),
                tracknumber: tags_map.get("tracknumber").cloned(),
                discnumber: tags_map.get("discnumber").cloned(),
                totaldiscs: tags_map.get("totaldiscs").cloned(),
                totaltracks: tags_map.get("totaltracks").cloned(),
                date: tags_map.get("date").cloned(),
                genre: tags_map.get("genre").cloned(),
                composer: tags_map.get("composer").cloned(),
                label: tags_map.get("label").cloned(),
                catalognumber: tags_map.get("catalognumber").cloned(),
                tags: tags_json,
                duration_secs: audio_props.duration_secs,
                bitrate: audio_props.bitrate,
                sample_rate: audio_props.sample_rate,
                channels: audio_props.channels,
                bit_depth: audio_props.bit_depth,
                has_embedded_art: audio_props.has_embedded_art,
                status: "active".into(),
                library_profile_id: None,
            };

            let _track = db.upsert_track(upsert).await?;

            if is_new {
                result.inserted += 1;
            } else {
                result.updated += 1;
            }
        }

        // Mark source tracks as removed when no longer present
        for (rel_path, (id, _)) in &existing {
            if rel_path.starts_with("source/") && !seen_paths.contains(rel_path) {
                db.set_track_status(*id, "removed").await?;
                result.removed += 1;
            }
        }

        // Enqueue CUE split jobs found in source/
        for cue_path in &cue_files_source {
            let cue_str = cue_path.to_string_lossy().to_string();
            let existing_jobs = db
                .list_jobs_by_type_and_payload_key("cue_split", "cue_path", &cue_str)
                .await?;
            let active = existing_jobs
                .iter()
                .any(|j| j.status == "pending" || j.status == "running");
            if !active {
                db.enqueue_job(
                    "cue_split",
                    serde_json::json!({
                        "cue_path": cue_str,
                        "library_id": library_id,
                    }),
                    6,
                )
                .await?;
            }
        }
    }

    // Mark ingest tracks as removed when no longer present in ingest/
    for (rel_path, (id, _)) in &existing {
        if rel_path.starts_with("ingest/") && !seen_paths.contains(rel_path) {
            tracing::debug!(path = %rel_path, track_id = id, "scanner/ingest: marking removed — no longer on disk");
            db.set_track_status(*id, "removed").await?;
            result.removed += 1;
        }
    }

    Ok(result)
}

async fn hash_file(path: &PathBuf) -> anyhow::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
