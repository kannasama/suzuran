// stub — full implementation in Task 7
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::{
    dal::{Store, UpsertTrack},
    error::AppError,
    tagger,
};

const AUDIO_EXTENSIONS: &[&str] = &["flac", "m4a", "mp3", "opus", "ogg", "aac", "wav", "aiff"];

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

        let (tags_map, audio_props) = match tag_result {
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

    Ok(result)
}

async fn hash_file(path: &PathBuf) -> anyhow::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
