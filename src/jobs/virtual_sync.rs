use std::{collections::HashMap, sync::Arc};

use crate::{
    dal::Store,
    error::AppError,
    models::{Library, Track},
};

pub struct VirtualSyncJobHandler {
    store: Arc<dyn Store>,
}

impl VirtualSyncJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

/// Compute a stable identity string for a track.
///
/// Prefers `musicbrainz_recordingid` from the `tags` JSON. Falls back to a
/// normalized `(albumartist, album, discnumber, tracknumber)` tuple so that
/// the same recording in two different libraries de-duplicates correctly.
fn track_identity(track: &Track) -> String {
    if let Some(mb) = track
        .tags
        .as_object()
        .and_then(|obj| obj.get("musicbrainz_recordingid"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        return format!("mb:{mb}");
    }

    let aa = track
        .albumartist
        .as_deref()
        .or(track.artist.as_deref())
        .unwrap_or("")
        .to_lowercase();
    let al = track.album.as_deref().unwrap_or("").to_lowercase();
    let dn = track.discnumber.as_deref().unwrap_or("1");
    let tn = track
        .tracknumber
        .as_deref()
        .unwrap_or("0")
        .split('/')
        .next()
        .unwrap_or("0")
        .trim();

    format!("tag:{aa}\x00{al}\x00{dn}\x00{tn}")
}

#[async_trait::async_trait]
impl super::JobHandler for VirtualSyncJobHandler {
    async fn run(
        &self,
        _db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let vlib_id = payload["virtual_library_id"]
            .as_i64()
            .ok_or_else(|| AppError::BadRequest("missing virtual_library_id".into()))?;

        let vlib = self.store.get_virtual_library(vlib_id).await?;
        let sources = self.store.list_virtual_library_sources(vlib_id).await?;

        // Build identity → (Library, Track) map — sources are already ordered by priority ASC,
        // so the first entry for each identity wins (lowest priority number = highest precedence).
        let mut identity_map: HashMap<String, (Library, Track)> = HashMap::new();
        for source in &sources {
            let lib = self
                .store
                .get_library(source.library_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("library {} not found", source.library_id))
                })?;
            let tracks = self.store.list_tracks_by_library(source.library_id).await?;
            for track in tracks {
                let id = track_identity(&track);
                identity_map.entry(id).or_insert((lib.clone(), track));
            }
        }

        // Clear existing filesystem links
        let existing = self.store.list_virtual_library_tracks(vlib_id).await?;
        for vt in &existing {
            let link =
                std::path::Path::new(&vlib.root_path).join(vt.link_path.trim_start_matches('/'));
            let _ = tokio::fs::remove_file(&link).await;
        }
        self.store.clear_virtual_library_tracks(vlib_id).await?;

        // Materialize new links
        let mut linked: usize = 0;
        for (_, (src_lib, track)) in &identity_map {
            let src_path = format!(
                "{}/{}",
                src_lib.root_path.trim_end_matches('/'),
                track.relative_path.trim_start_matches('/')
            );
            let link_rel = &track.relative_path;
            let link_abs = std::path::Path::new(&vlib.root_path)
                .join(link_rel.trim_start_matches('/'));

            if let Some(parent) = link_abs.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("mkdir: {e}")))?;
            }

            // Remove any stale entry that may have been left from a previous partial run
            let _ = tokio::fs::remove_file(&link_abs).await;

            match vlib.link_type.as_str() {
                "symlink" => {
                    tokio::fs::symlink(&src_path, &link_abs)
                        .await
                        .map_err(|e| AppError::Internal(anyhow::anyhow!("symlink: {e}")))?;
                }
                "hardlink" => {
                    tokio::fs::hard_link(&src_path, &link_abs)
                        .await
                        .map_err(|e| AppError::Internal(anyhow::anyhow!(
                            "hardlink (ensure same filesystem): {e}"
                        )))?;
                }
                other => {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "unknown link_type: {other}"
                    )));
                }
            }

            self.store
                .upsert_virtual_library_track(vlib_id, track.id, link_rel)
                .await?;
            linked += 1;
        }

        Ok(serde_json::json!({
            "status": "completed",
            "virtual_library_id": vlib_id,
            "tracks_linked": linked,
        }))
    }
}
