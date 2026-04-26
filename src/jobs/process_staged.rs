use std::{path::Path, sync::Arc};

use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    picture::{MimeType, Picture, PictureType},
    probe::Probe,
};

use crate::{
    dal::Store,
    error::AppError,
    jobs::{cue_split::hash_file, TranscodePayload},
    tagger,
};

pub struct ProcessStagedJobHandler {
    store: Arc<dyn Store>,
}

impl ProcessStagedJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl super::JobHandler for ProcessStagedJobHandler {
    async fn run(
        &self,
        _db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        handle_process_staged(self.store.clone(), payload).await
    }
}

async fn handle_process_staged(
    store: Arc<dyn Store>,
    payload: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    // 1. Parse payload
    let staged_payload: super::ProcessStagedPayload = serde_json::from_value(payload)
        .map_err(|e| AppError::BadRequest(format!("invalid process_staged payload: {e}")))?;

    let track_id = staged_payload.track_id;

    // 2. Fetch track; assert status == "staged"
    let track = store
        .get_track(track_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("track {track_id} not found")))?;

    if track.status != "staged" {
        return Err(AppError::BadRequest(format!(
            "track {track_id} has status '{}', expected 'staged'",
            track.status
        )));
    }

    // 3. Fetch library
    let library = store
        .get_library(track.library_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("library {} not found", track.library_id)))?;

    let root_path = library.root_path.trim_end_matches('/').to_string();

    // Current absolute path of the staged file
    let src_abs = format!(
        "{}/{}",
        root_path,
        track.relative_path.trim_start_matches('/')
    );

    // 4. Resolve tag source: pending_tags (working copy) takes priority over suggestion.
    let resolved_tags: Option<std::collections::HashMap<String, String>> =
        if let Some(pending) = track.pending_tags.clone() {
            // User has an established working copy — use it directly.
            Some(serde_json::from_value(pending).unwrap_or_default())
        } else if let Some(suggestion_id) = staged_payload.tag_suggestion_id {
            let suggestion = store
                .get_tag_suggestion(suggestion_id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("tag suggestion {suggestion_id} not found")))?;
            Some(serde_json::from_value(suggestion.suggested_tags.clone()).unwrap_or_default())
        } else {
            None
        };

    if let Some(tags_map) = resolved_tags {
        // Write tags to audio file
        let src_path_for_tags = Path::new(&src_abs).to_owned();
        let tags_clone = tags_map.clone();
        tokio::task::spawn_blocking(move || tagger::write_tags(&src_path_for_tags, &tags_clone))
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking write_tags: {e}")))?
            .map_err(|e| AppError::Internal(anyhow::anyhow!("write_tags: {e}")))?;

        // Update DB track tags
        let merged_tags = serde_json::to_value(&tags_map)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize tags: {e}")))?;
        store.update_track_tags(track_id, merged_tags).await?;
    }

    // 5. Download and embed cover art if provided
    if let Some(ref art_url) = staged_payload.cover_art_url {
        let response = reqwest::get(art_url.as_str())
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("fetch art: {e}")))?;
        let art_bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("art bytes: {e}")))?
            .to_vec();

        // Determine MIME type: prefer the active art profile's format field over URL-suffix heuristic
        let mime = {
            let profiles = store.list_art_profiles().await?;
            let profile_format = profiles
                .iter()
                .find(|p| p.apply_to_library_id == Some(library.id))
                .map(|p| p.format.as_str());
            match profile_format {
                Some("png") => MimeType::Png,
                Some(_) => MimeType::Jpeg, // "jpeg" or any other value → JPEG
                None => {
                    // Fall back to URL-suffix detection
                    if art_url.ends_with(".png") {
                        MimeType::Png
                    } else {
                        MimeType::Jpeg
                    }
                }
            }
        };

        // Embed art into the ingest file
        let src_path_for_art = src_abs.clone();
        let art_bytes_clone = art_bytes.clone();
        let mime_clone = mime.clone();
        tokio::task::spawn_blocking(move || {
            embed_art_bytes_sync(&src_path_for_art, art_bytes_clone, mime_clone)
        })
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking embed art: {e}")))?
        .map_err(|e| AppError::Internal(anyhow::anyhow!("embed art: {e}")))?;

        // 5b. Write folder art if requested
        if staged_payload.write_folder_art {
            let folder_art_filename = store
                .get_setting("folder_art_filename")
                .await?
                .map(|s| s.value)
                .unwrap_or_default();

            if !folder_art_filename.is_empty() {
                // Determine the album directory for the source location:
                // After moving, file will be at source/{rest}; compute dest parent dir
                let rest = strip_ingest_prefix(&track.relative_path);
                let source_rel_parent = Path::new(rest)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let folder_art_dir = if source_rel_parent.is_empty() {
                    format!("{}/source", root_path)
                } else {
                    format!("{}/source/{}", root_path, source_rel_parent)
                };

                tokio::fs::create_dir_all(&folder_art_dir)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("mkdir folder_art dir: {e}")))?;

                let folder_art_path = format!("{}/{}", folder_art_dir, folder_art_filename);
                tokio::fs::write(&folder_art_path, &art_bytes)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("write folder art: {e}")))?;
            }
        }
    }

    // 6. Compute destination path: replace "ingest/" prefix with "source/"
    let rest = strip_ingest_prefix(&track.relative_path);
    let dest_relative = format!("source/{}", rest);
    let dest_abs = format!("{}/{}", root_path, dest_relative);

    // 7. Create destination parent directory
    if let Some(parent) = Path::new(&dest_abs).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("create_dir_all dest: {e}")))?;
    }

    // 8. Move file from ingest/ to source/
    tokio::fs::rename(&src_abs, &dest_abs)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rename ingest→source: {e}")))?;

    // 8.1 Delete companion (non-audio) files left in the ingest source directory so the
    //     empty-directory sweep below can actually remove it.
    const COMPANION_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff",
        "cue", "log", "nfo", "txt", "m3u", "m3u8",
    ];
    if let Some(ingest_dir) = Path::new(&src_abs).parent() {
        if let Ok(mut entries) = tokio::fs::read_dir(ingest_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let p = entry.path();
                if !p.is_file() { continue; }
                let ext = p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                if COMPANION_EXTS.contains(&ext.as_str()) {
                    let _ = tokio::fs::remove_file(&p).await;
                }
            }
        }
    }

    // 8.2 Remove empty ingest parent directories left behind by the move.
    {
        let ingest_root = format!("{}/ingest", root_path);
        let mut dir = Path::new(&src_abs)
            .parent()
            .map(|p| p.to_path_buf());
        while let Some(d) = dir {
            if d.to_string_lossy() == ingest_root || !d.starts_with(&ingest_root) {
                break;
            }
            match tokio::fs::remove_dir(&d).await {
                Ok(_) => { dir = d.parent().map(|p| p.to_path_buf()); }
                Err(_) => break, // not empty or other error — stop climbing
            }
        }
    }

    // 8.5 Write folder art from embedded art when no cover_art_url was provided
    if staged_payload.write_folder_art && staged_payload.cover_art_url.is_none() {
        let folder_art_filename = store
            .get_setting("folder_art_filename")
            .await?
            .map(|s| s.value)
            .unwrap_or_default();

        if !folder_art_filename.is_empty() {
            let dest_for_extract = dest_abs.clone();
            let extracted = tokio::task::spawn_blocking(move || {
                extract_embedded_art_sync(&dest_for_extract)
            })
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking extract art: {e}")))?;

            if let Ok(Some(art_bytes)) = extracted {
                let folder_art_dir = Path::new(&dest_abs)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from(&root_path));

                tokio::fs::create_dir_all(&folder_art_dir)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("mkdir for embedded folder art: {e}")))?;

                let folder_art_path = folder_art_dir.join(&folder_art_filename);
                tokio::fs::write(&folder_art_path, &art_bytes)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("write folder art from embedded: {e}")))?;
            }
            // No embedded art found — silently skip; not an error
        }
    }

    // 9. Hash file at destination
    let new_hash = hash_file(Path::new(&dest_abs))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("hash_file dest: {e}")))?;

    // 10. Update track record
    store.update_track_path(track_id, &dest_relative, &new_hash).await?;
    store.set_track_status(track_id, "active").await?;
    // Clear working copy now that the track is imported.
    let _ = store.clear_pending_tags(track_id).await;

    // 11. Handle supersede: displace the old active track if requested
    if let Some(old_track_id) = staged_payload.supersede_track_id {
        let old_track = store
            .get_track(old_track_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("supersede target {old_track_id} not found")))?;

        let old_abs = format!(
            "{}/{}",
            root_path,
            old_track.relative_path.trim_start_matches('/')
        );

        match staged_payload.supersede_profile_id {
            Some(profile_id) => {
                // Move old file into the derived directory of the matching profile.
                let profile = store.get_library_profile(profile_id).await?;

                // Strip "source/" prefix from old relative path; prepend derived_dir_name.
                let old_rest = old_track
                    .relative_path
                    .trim_start_matches("source/")
                    .trim_start_matches('/');
                let derived_relative = format!("{}/{}", profile.derived_dir_name, old_rest);
                let derived_abs = format!("{}/{}", root_path, derived_relative);

                if let Some(parent) = Path::new(&derived_abs).parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| AppError::Internal(anyhow::anyhow!("mkdir derived dir: {e}")))?;
                }

                // rename with EXDEV fallback (OS error 18 = cross-device link)
                if let Err(e) = tokio::fs::rename(&old_abs, &derived_abs).await {
                    if e.raw_os_error() == Some(18) {
                        tokio::fs::copy(&old_abs, &derived_abs)
                            .await
                            .map_err(|e| AppError::Internal(anyhow::anyhow!("copy for EXDEV: {e}")))?;
                        tokio::fs::remove_file(&old_abs)
                            .await
                            .map_err(|e| AppError::Internal(anyhow::anyhow!("remove after EXDEV copy: {e}")))?;
                    } else {
                        return Err(AppError::Internal(anyhow::anyhow!("move old file: {e}")));
                    }
                }

                let derived_hash = hash_file(Path::new(&derived_abs))
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("hash displaced file: {e}")))?;

                store.update_track_path(old_track_id, &derived_relative, &derived_hash).await?;
                store.set_track_library_profile(old_track_id, profile_id).await?;
                // The old track remains "active" — it is now a derived copy of the new source.
                store.create_track_link(track_id, old_track_id).await?;
            }
            None => {
                // Override/discard: delete the old file and mark it removed.
                let _ = tokio::fs::remove_file(&old_abs).await;
                store.set_track_status(old_track_id, "removed").await?;
            }
        }
    }

    // 13. Enqueue transcode job for each profile.
    // Skip any profile that was already satisfied by displacing the superseded source file;
    // that displaced file IS the derived copy — transcoding it again would create a duplicate.
    for profile_id in &staged_payload.profile_ids {
        if staged_payload.supersede_profile_id == Some(*profile_id) {
            continue;
        }
        let transcode_payload = serde_json::to_value(TranscodePayload {
            track_id,
            library_profile_id: *profile_id,
        })
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize transcode payload: {e}")))?;
        store.enqueue_job("transcode", transcode_payload, 4).await?;
    }

    // 14. Auto-organize: if the library has an organization rule and auto_organize_on_ingest,
    // enqueue an organize job so the source file lands in the correct path.
    if library.auto_organize_on_ingest && library.organization_rule_id.is_some() {
        let org_payload = serde_json::to_value(super::OrganizePayload {
            track_id,
            dry_run: false,
        })
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize organize payload: {e}")))?;
        store.enqueue_job("organize", org_payload, 4).await?;
    }

    Ok(serde_json::json!({
        "track_id": track_id,
        "profiles_enqueued": staged_payload.profile_ids.len(),
    }))
}

/// Strip the "ingest/" prefix from a relative path.
/// E.g. "ingest/album/track.flac" → "album/track.flac"
fn strip_ingest_prefix(rel_path: &str) -> &str {
    rel_path
        .trim_start_matches("ingest/")
        .trim_start_matches('/')
}

/// Sync helper — extract the CoverFront picture bytes from an audio file's primary tag.
/// Returns `Ok(None)` when the file has no embedded art; errors only on IO/parse failures.
fn extract_embedded_art_sync(path: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let tagged = Probe::open(path)?.read()?;
    let tag = match tagged.primary_tag() {
        Some(t) => t,
        None => return Ok(None),
    };
    let art = tag
        .pictures()
        .iter()
        .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
        .or_else(|| tag.pictures().iter().next());
    Ok(art.map(|p| p.data().to_vec()))
}

/// Sync helper — embed image bytes into audio file as cover art.
fn embed_art_bytes_sync(path: &str, bytes: Vec<u8>, mime: MimeType) -> anyhow::Result<()> {
    let mut tagged = Probe::open(path)?.read()?;
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| anyhow::anyhow!("no primary tag in {:?}", path))?;
    tag.remove_picture_type(PictureType::CoverFront);
    tag.push_picture(Picture::new_unchecked(
        PictureType::CoverFront,
        Some(mime),
        None,
        bytes,
    ));
    tagged.save_to_path(path, WriteOptions::default())?;
    Ok(())
}
