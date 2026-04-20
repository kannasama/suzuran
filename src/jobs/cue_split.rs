use std::{collections::HashMap, path::Path, sync::Arc};

use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::{
    cue::parse_cue,
    dal::{Store, UpsertTrack},
    error::AppError,
    tagger,
};

pub struct CueSplitJobHandler {
    db: Arc<dyn Store>,
}

impl CueSplitJobHandler {
    pub fn new(db: Arc<dyn Store>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl super::JobHandler for CueSplitJobHandler {
    async fn run(
        &self,
        _db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        handle_cue_split(self.db.clone(), payload).await
    }
}

async fn handle_cue_split(
    db: Arc<dyn Store>,
    payload: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    let cue_path_str = payload["cue_path"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("missing cue_path".into()))?;
    let library_id = payload["library_id"]
        .as_i64()
        .ok_or_else(|| AppError::BadRequest("missing library_id".into()))?;

    let cue_path = Path::new(cue_path_str);
    let cue_dir = cue_path
        .parent()
        .ok_or_else(|| AppError::BadRequest("cue_path has no parent directory".into()))?;

    let content = tokio::fs::read_to_string(cue_path)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("read CUE file: {e}")))?;

    let sheet = parse_cue(&content)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("parse CUE: {e}")))?;

    let audio_path = cue_dir.join(&sheet.audio_file);
    if !audio_path.exists() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "audio file not found: {}",
            audio_path.display()
        )));
    }

    let ext = audio_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("flac")
        .to_lowercase();

    let num_tracks = sheet.tracks.len();
    let mut tracks_created: i64 = 0;

    for (i, track) in sheet.tracks.iter().enumerate() {
        let title = track.title.clone().unwrap_or_else(|| format!("Track {}", track.number));
        let out_filename = format!("{:02} - {}.{}", track.number, sanitize_filename(&title), ext);
        let out_path = cue_dir.join(&out_filename);

        // Idempotency: skip if output file already exists
        if out_path.exists() {
            continue;
        }

        let start_secs = track.index_01_secs;
        let end_secs = if i + 1 < num_tracks {
            Some(sheet.tracks[i + 1].index_01_secs)
        } else {
            None
        };

        // Build ffmpeg args: -ss START [-to END] -c:a copy -y output
        let mut args: Vec<String> = vec![
            "-i".into(),
            audio_path.to_string_lossy().to_string(),
            "-ss".into(),
            format!("{:.6}", start_secs),
        ];
        if let Some(end) = end_secs {
            args.push("-to".into());
            args.push(format!("{:.6}", end));
        }
        args.extend_from_slice(&[
            "-c:a".into(),
            "copy".into(),
            "-y".into(),
            out_path.to_string_lossy().to_string(),
        ]);

        let out = Command::new("ffmpeg")
            .args(&args)
            .output()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("ffmpeg spawn failed: {e}")))?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(AppError::Internal(anyhow::anyhow!(
                "ffmpeg failed for track {}: {stderr}",
                track.number
            )));
        }

        // Write CUE metadata to the split file via lofty
        let mut tags: HashMap<String, String> = HashMap::new();
        tags.insert("tracknumber".into(), track.number.to_string());
        if let Some(t) = &track.title {
            tags.insert("title".into(), t.clone());
        }
        let artist = track
            .performer
            .clone()
            .or_else(|| sheet.performer.clone());
        if let Some(a) = artist {
            tags.insert("artist".into(), a.clone());
            tags.insert("albumartist".into(), a);
        }
        if let Some(album) = &sheet.album_title {
            tags.insert("album".into(), album.clone());
        }
        if let Some(date) = &sheet.date {
            tags.insert("date".into(), date.clone());
        }
        if let Some(genre) = &sheet.genre {
            tags.insert("genre".into(), genre.clone());
        }
        tags.insert("totaltracks".into(), num_tracks.to_string());

        let out_path_clone = out_path.clone();
        let tags_clone = tags.clone();
        tokio::task::spawn_blocking(move || tagger::write_tags(&out_path_clone, &tags_clone))
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking write_tags: {e}")))?
            .map_err(|e| AppError::Internal(anyhow::anyhow!("write_tags: {e}")))?;

        // Hash the output file
        let file_hash = hash_file(&out_path)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("hash_file: {e}")))?;

        // Read audio properties from the split file
        let out_path_clone2 = out_path.clone();
        let (tags_from_file, audio_props) = tokio::task::spawn_blocking(move || {
            tagger::read_tags(&out_path_clone2)
        })
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking read_tags: {e}")))?
        .unwrap_or_else(|_| (tags.clone(), tagger::AudioProperties::default()));

        // Build relative path from cue_dir
        let library = db
            .get_library(library_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("library {library_id} not found")))?;
        let library_root = Path::new(&library.root_path);
        let relative_path = out_path
            .strip_prefix(library_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| out_filename.clone());

        let tags_json = serde_json::to_value(&tags_from_file).unwrap_or(serde_json::json!({}));

        let upsert = UpsertTrack {
            library_id,
            relative_path,
            file_hash,
            title: tags_from_file.get("title").cloned().or_else(|| track.title.clone()),
            artist: tags_from_file.get("artist").cloned().or_else(|| {
                track.performer.clone().or_else(|| sheet.performer.clone())
            }),
            albumartist: tags_from_file
                .get("albumartist")
                .cloned()
                .or_else(|| sheet.performer.clone()),
            album: tags_from_file
                .get("album")
                .cloned()
                .or_else(|| sheet.album_title.clone()),
            tracknumber: tags_from_file
                .get("tracknumber")
                .cloned()
                .or_else(|| Some(track.number.to_string())),
            discnumber: tags_from_file.get("discnumber").cloned(),
            totaldiscs: tags_from_file.get("totaldiscs").cloned(),
            totaltracks: tags_from_file
                .get("totaltracks")
                .cloned()
                .or_else(|| Some(num_tracks.to_string())),
            date: tags_from_file
                .get("date")
                .cloned()
                .or_else(|| sheet.date.clone()),
            genre: tags_from_file
                .get("genre")
                .cloned()
                .or_else(|| sheet.genre.clone()),
            composer: tags_from_file.get("composer").cloned(),
            label: tags_from_file.get("label").cloned(),
            catalognumber: tags_from_file.get("catalognumber").cloned(),
            tags: tags_json,
            duration_secs: audio_props.duration_secs,
            bitrate: audio_props.bitrate,
            sample_rate: audio_props.sample_rate,
            channels: audio_props.channels,
            bit_depth: audio_props.bit_depth,
            has_embedded_art: audio_props.has_embedded_art,
        };

        let new_track = db.upsert_track(upsert).await?;

        // Enqueue fingerprint job for the new track
        db.enqueue_job(
            "fingerprint",
            serde_json::json!({"track_id": new_track.id}),
            5,
        )
        .await?;

        tracks_created += 1;
    }

    Ok(serde_json::json!({ "tracks_created": tracks_created }))
}

pub async fn hash_file(path: &Path) -> anyhow::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}
