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
    let library_format = library.as_ref().map(|l| l.format.as_str()).unwrap_or("").to_string();
    let tag_encoding = library.as_ref().map(|l| l.tag_encoding.as_str()).unwrap_or("utf8").to_string();

    // --- Pass 1: find CUE files and their paired audio ---
    let mut cue_backed_audio: HashSet<PathBuf> = HashSet::new();
    let mut cue_files: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(root_path).follow_links(true).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path().to_path_buf();
        if p.extension().and_then(|e| e.to_str()) == Some("cue") {
            if let Ok(content) = read_cue_file(&p, &tag_encoding) {
                if let Ok(sheet) = parse_cue(&content) {
                    let audio = p.parent().unwrap_or(root_path).join(&sheet.audio_file);
                    if audio.exists() {
                        cue_backed_audio.insert(audio);
                        cue_files.push(p);
                    }
                }
            }
        }
    }

    let existing: HashMap<String, (i64, String)> = db
        .list_track_paths_by_library(library_id)
        .await?
        .into_iter()
        .map(|(id, path, hash)| (path, (id, hash)))
        .collect();

    let mut seen_paths: HashSet<String> = HashSet::new();

    for entry in WalkDir::new(root_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let abs_path = entry.path().to_path_buf();

        // Skip audio files that are backed by a CUE sheet — they will be
        // split into individual tracks by the cue_split job instead.
        if cue_backed_audio.contains(&abs_path) {
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

        let needs_scan = match existing.get(&rel_path) {
            Some((_, existing_hash)) => existing_hash != &hash,
            None => true,
        };

        let is_new = !existing.contains_key(&rel_path);

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

        // Re-decode tag values if the library uses Shift-JIS encoding.
        // Old ID3v2/Vorbis rippers often stored SJIS bytes in Latin-1 frames;
        // lofty returns them as mojibake. Re-decode to recover the original text.
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
        };

        let track = db.upsert_track(upsert).await?;

        if is_new {
            result.inserted += 1;
            // Enqueue fingerprint job for newly discovered tracks
            db.enqueue_job(
                "fingerprint",
                serde_json::json!({"track_id": track.id}),
                5,
            )
            .await?;

            // Auto-transcode to child libraries (pre-filter by compatibility)
            let children = db.list_child_libraries(library_id).await?;
            for child in children.iter().filter(|c| c.auto_transcode_on_ingest) {
                if let Some(ep_id) = child.encoding_profile_id {
                    if let Ok(profile) = db.get_encoding_profile(ep_id).await {
                        let src_ext = std::path::Path::new(&track.relative_path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let src_fmt = if src_ext.is_empty() {
                            library_format.as_str()
                        } else {
                            src_ext.as_str()
                        };
                        if !crate::services::transcode_compat::is_compatible(
                            src_fmt,
                            track.sample_rate,
                            track.bit_depth,
                            track.bitrate,
                            &profile,
                        ) {
                            continue;
                        }
                    }
                }
                db.enqueue_job(
                    "transcode",
                    serde_json::json!({
                        "source_track_id": track.id,
                        "target_library_id": child.id,
                    }),
                    4,
                )
                .await?;
            }
        } else {
            result.updated += 1;
        }
    }

    for (rel_path, (id, _)) in &existing {
        if !seen_paths.contains(rel_path) {
            db.mark_track_removed(*id).await?;
            result.removed += 1;
        }
    }

    // --- Pass 3: enqueue cue_split jobs for discovered CUE sheets ---
    for cue_path in &cue_files {
        let cue_str = cue_path.to_string_lossy().to_string();
        let existing_jobs = db
            .list_jobs_by_type_and_payload_key("cue_split", "cue_path", &cue_str)
            .await?;
        // Only enqueue if there is no pending/running/completed job for this CUE file.
        let active = existing_jobs
            .iter()
            .any(|j| j.status == "pending" || j.status == "running" || j.status == "completed");
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

    Ok(result)
}

async fn hash_file(path: &PathBuf) -> anyhow::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
