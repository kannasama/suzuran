use std::{io::Cursor, sync::Arc};

use image::{ImageFormat, codecs::jpeg::JpegEncoder};
use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    picture::{MimeType, Picture, PictureType},
    probe::Probe,
};

use crate::{dal::Store, error::AppError};

pub struct ArtProcessJobHandler {
    store: Arc<dyn Store>,
}

impl ArtProcessJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl super::JobHandler for ArtProcessJobHandler {
    async fn run(
        &self,
        _db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"]
            .as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;

        let action = payload["action"]
            .as_str()
            .ok_or_else(|| AppError::BadRequest("missing action".into()))?
            .to_string();

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

        let path = format!(
            "{}/{}",
            library.root_path.trim_end_matches('/'),
            track.relative_path.trim_start_matches('/')
        );

        match action.as_str() {
            "embed" => {
                let url = payload["source_url"]
                    .as_str()
                    .ok_or_else(|| AppError::BadRequest("embed requires source_url".into()))?
                    .to_string();

                let response = reqwest::get(&url)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("fetch art: {e}")))?;

                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("art bytes: {e}")))?;

                let mime = if url.ends_with(".png") {
                    MimeType::Png
                } else {
                    MimeType::Jpeg
                };

                embed_art_bytes_async(&path, bytes.to_vec(), mime).await?;
                self.store.set_track_has_embedded_art(track_id, true).await?;
            }

            "extract" => {
                extract_art_async(&path).await?;
            }

            "standardize" => {
                let profile_id = payload["art_profile_id"]
                    .as_i64()
                    .ok_or_else(|| AppError::BadRequest("standardize requires art_profile_id".into()))?;

                let profile = self.store.get_art_profile(profile_id).await?;
                standardize_art_async(&path, &profile).await?;
                self.store.set_track_has_embedded_art(track_id, true).await?;
            }

            other => {
                return Err(AppError::BadRequest(format!("unknown art action: {other}")));
            }
        }

        Ok(serde_json::json!({
            "status": "completed",
            "track_id": track_id,
            "action": action,
        }))
    }
}

/// Embed raw image bytes into the audio file's primary tag as cover art.
async fn embed_art_bytes_async(audio_path: &str, bytes: Vec<u8>, mime: MimeType) -> Result<(), AppError> {
    let path = audio_path.to_string();
    tokio::task::spawn_blocking(move || embed_art_bytes_sync(&path, bytes, mime))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking panicked: {e}")))?
        .map_err(|e| AppError::Internal(anyhow::anyhow!("lofty embed: {e}")))
}

/// Sync helper — embed bytes, replacing existing cover art.
fn embed_art_bytes_sync(path: &str, bytes: Vec<u8>, mime: MimeType) -> anyhow::Result<()> {
    let mut tagged = Probe::open(path)?.read()?;
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| anyhow::anyhow!("no primary tag in {:?}", path))?;
    // Remove any existing cover front art, then push the new picture
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

/// Extract the first embedded picture and write it alongside the audio file as `{stem}.cover.{ext}`.
async fn extract_art_async(audio_path: &str) -> Result<(), AppError> {
    let path = audio_path.to_string();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let tagged = Probe::open(&path)?.read()?;
        let tag = tagged
            .primary_tag()
            .ok_or_else(|| anyhow::anyhow!("no primary tag"))?;
        let pic = tag
            .pictures()
            .first()
            .ok_or_else(|| anyhow::anyhow!("no embedded art"))?;
        let ext = match pic.mime_type() {
            Some(MimeType::Png) => "png",
            _ => "jpg",
        };
        // Build output path: strip audio extension, add `.cover.{ext}`
        let base = std::path::Path::new(&path);
        let stem = base
            .file_stem()
            .ok_or_else(|| anyhow::anyhow!("invalid path"))?
            .to_string_lossy();
        let dir = base
            .parent()
            .ok_or_else(|| anyhow::anyhow!("no parent dir"))?;
        let out_name = format!("{stem}.cover.{ext}");
        let out_path = dir.join(out_name);
        std::fs::write(&out_path, pic.data())?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking panicked: {e}")))?
    .map_err(|e| AppError::Internal(anyhow::anyhow!("extract art: {e}")))
}

/// Read embedded art, resize/recompress to fit the art profile constraints, and re-embed.
async fn standardize_art_async(
    audio_path: &str,
    profile: &crate::models::ArtProfile,
) -> Result<(), AppError> {
    let path = audio_path.to_string();
    let max_w = profile.max_width_px as u32;
    let max_h = profile.max_height_px as u32;
    let quality = profile.quality as u8;
    let format = profile.format.clone();

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        // Read existing embedded art
        let tagged = Probe::open(&path)?.read()?;
        let tag = tagged
            .primary_tag()
            .ok_or_else(|| anyhow::anyhow!("no primary tag"))?;
        let pic = tag
            .pictures()
            .first()
            .ok_or_else(|| anyhow::anyhow!("no embedded art to standardize"))?;

        // Decode and resize
        let img = image::load_from_memory(pic.data())?;
        let resized = if img.width() > max_w || img.height() > max_h {
            img.resize(max_w, max_h, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // Re-encode to target format
        let mut out_bytes: Vec<u8> = Vec::new();
        let mime = if format == "png" {
            resized.write_to(&mut Cursor::new(&mut out_bytes), ImageFormat::Png)?;
            MimeType::Png
        } else {
            let enc = JpegEncoder::new_with_quality(&mut out_bytes, quality);
            resized.write_with_encoder(enc)?;
            MimeType::Jpeg
        };

        // Re-open and re-embed (drop the read handle first)
        drop(tagged);
        embed_art_bytes_sync(&path, out_bytes, mime)?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking panicked: {e}")))?
    .map_err(|e| AppError::Internal(anyhow::anyhow!("standardize art: {e}")))
}
