use std::{path::Path, sync::Arc};

use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::{
    dal::{Store, UpsertTrack},
    error::AppError,
    jobs::cue_split::hash_file,
    models::EncodingProfile,
    services::transcode_compat::{is_compatible, is_noop_transcode},
    tagger,
};

pub struct TranscodeJobHandler {
    store: Arc<dyn Store>,
}

impl TranscodeJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl super::JobHandler for TranscodeJobHandler {
    async fn run(
        &self,
        _db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        handle_transcode(self.store.clone(), payload).await
    }
}

/// Map a codec name to its canonical file extension.
pub fn codec_extension(codec: &str) -> &str {
    match codec {
        "aac" => "m4a",
        "mp3" | "libmp3lame" => "mp3",
        "opus" | "libopus" => "opus",
        "flac" => "flac",
        "vorbis" | "libvorbis" => "ogg",
        other => other,
    }
}

/// Build the codec/quality ffmpeg args for the given encoding profile.
/// Does NOT include `-i` or the output path.
pub fn build_ffmpeg_args(profile: &EncodingProfile) -> Vec<String> {
    let mut args = vec!["-vn".to_string()];
    args.extend(["-c:a".to_string(), profile.codec.clone()]);
    if let Some(b) = &profile.bitrate {
        args.extend(["-b:a".to_string(), b.clone()]);
    }
    if let Some(sr) = profile.sample_rate {
        args.extend(["-ar".to_string(), sr.to_string()]);
    }
    if let Some(ch) = profile.channels {
        args.extend(["-ac".to_string(), ch.to_string()]);
    }
    if profile.codec == "flac" {
        match profile.bit_depth {
            Some(16) => args.extend(["-sample_fmt".to_string(), "s16".to_string()]),
            Some(24) => args.extend([
                "-sample_fmt".to_string(), "s32".to_string(),
                "-bits_per_raw_sample".to_string(), "24".to_string(),
            ]),
            Some(32) => args.extend(["-sample_fmt".to_string(), "s32".to_string()]),
            _ => {}
        }
    }
    if let Some(adv) = &profile.advanced_args {
        args.extend(adv.split_whitespace().map(str::to_string));
    }
    args
}

async fn handle_transcode(
    store: Arc<dyn Store>,
    payload: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    // 1. Extract payload fields
    let track_id = payload["track_id"]
        .as_i64()
        .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;
    let library_profile_id = payload["library_profile_id"]
        .as_i64()
        .ok_or_else(|| AppError::BadRequest("missing library_profile_id".into()))?;

    // 2. Get source track
    let track = store
        .get_track(track_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("track {track_id} not found")))?;

    // 3. Get source library
    let src_lib = store
        .get_library(track.library_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("source library {} not found", track.library_id)))?;

    // 4. Get library profile
    let lib_profile = store.get_library_profile(library_profile_id).await?;

    // 5. Get encoding profile from library profile
    let ep_id = lib_profile.encoding_profile_id;
    let profile = store.get_encoding_profile(ep_id).await?;

    // 6. Determine source format from track's relative_path extension, falling back to library format
    let src_format = Path::new(&track.relative_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| src_lib.format.clone());

    // 7a. Compatibility check — skip instead of fail for quality violations
    if !is_compatible(
        &src_format,
        track.sample_rate,
        track.bit_depth,
        track.bitrate,
        &profile,
    ) {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "source/profile combination not compatible (quality guard)",
            "track_id": track_id,
        }));
    }

    // 7b. No-op check — skip when source is already this format at this quality
    if is_noop_transcode(
        &src_format,
        track.sample_rate,
        track.bit_depth,
        track.bitrate,
        &profile,
    ) {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "source already satisfies profile format and quality",
            "track_id": track_id,
        }));
    }

    // 8. Build source absolute path
    let src_path = format!(
        "{}/{}",
        src_lib.root_path.trim_end_matches('/'),
        track.relative_path.trim_start_matches('/')
    );

    // 9. Compute output path:
    //    source relative_path is like "source/album/track.flac"
    //    output goes to "{root_path}/{derived_dir_name}/album/track.{ext}"
    //    Strip the "source/" prefix from relative_path.
    let path_without_source_prefix = track
        .relative_path
        .trim_start_matches("source/")
        .trim_start_matches('/');

    let ext = codec_extension(&profile.codec);
    let src_stem = Path::new(path_without_source_prefix)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");
    let src_dir_within = Path::new(path_without_source_prefix)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");

    let out_rel_within_derived = if src_dir_within.is_empty() {
        format!("{}.{}", src_stem, ext)
    } else {
        format!("{}/{}.{}", src_dir_within, src_stem, ext)
    };

    // out_rel is relative to library root: "{derived_dir_name}/{path}"
    let out_rel = format!("{}/{}", lib_profile.derived_dir_name, out_rel_within_derived);
    let out_path_str = format!(
        "{}/{}",
        src_lib.root_path.trim_end_matches('/'),
        out_rel
    );
    let out_path = Path::new(&out_path_str);

    // 10. Create output parent directory
    if let Some(parent) = out_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("create_dir_all: {e}")))?;
    }

    // 11. Build ffmpeg args and run
    let mut args: Vec<String> = vec!["-i".into(), src_path];
    args.extend(build_ffmpeg_args(&profile));
    args.extend(["-progress".into(), "pipe:1".into(), "-y".into()]);
    args.push(out_path_str.clone());

    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("ffmpeg spawn failed: {e}")))?;

    // 12. Drain stdout (progress output) asynchronously
    if let Some(mut stdout) = child.stdout.take() {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf).await;
    }

    // 13. Wait for ffmpeg to exit
    let status = child
        .wait()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("ffmpeg wait failed: {e}")))?;

    if !status.success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "ffmpeg exited with non-zero status for track {track_id}"
        )));
    }

    // 14. Write source tags to output file
    let mut tags_map = std::collections::HashMap::new();
    if let Some(t) = &track.title {
        tags_map.insert("title".into(), t.clone());
    }
    if let Some(a) = &track.artist {
        tags_map.insert("artist".into(), a.clone());
    }
    if let Some(aa) = &track.albumartist {
        tags_map.insert("albumartist".into(), aa.clone());
    }
    if let Some(al) = &track.album {
        tags_map.insert("album".into(), al.clone());
    }
    if let Some(tn) = &track.tracknumber {
        tags_map.insert("tracknumber".into(), tn.clone());
    }
    if let Some(dn) = &track.discnumber {
        tags_map.insert("discnumber".into(), dn.clone());
    }
    if let Some(tt) = &track.totaltracks {
        tags_map.insert("totaltracks".into(), tt.clone());
    }
    if let Some(td) = &track.totaldiscs {
        tags_map.insert("totaldiscs".into(), td.clone());
    }
    if let Some(d) = &track.date {
        tags_map.insert("date".into(), d.clone());
    }
    if let Some(g) = &track.genre {
        tags_map.insert("genre".into(), g.clone());
    }
    if let Some(c) = &track.composer {
        tags_map.insert("composer".into(), c.clone());
    }
    if let Some(l) = &track.label {
        tags_map.insert("label".into(), l.clone());
    }
    if let Some(cn) = &track.catalognumber {
        tags_map.insert("catalognumber".into(), cn.clone());
    }

    let out_path_for_tags = out_path.to_owned();
    let tags_clone = tags_map.clone();
    tokio::task::spawn_blocking(move || tagger::write_tags(&out_path_for_tags, &tags_clone))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking write_tags: {e}")))?
        .map_err(|e| AppError::Internal(anyhow::anyhow!("write_tags: {e}")))?;

    // 15. Hash the output file
    let file_hash = hash_file(out_path)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("hash_file: {e}")))?;

    // Read audio properties from the output
    let out_path_for_props = out_path.to_owned();
    let (tags_from_file, audio_props) =
        tokio::task::spawn_blocking(move || tagger::read_tags(&out_path_for_props))
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking read_tags: {e}")))?
            .unwrap_or_else(|_| (tags_map.clone(), tagger::AudioProperties::default()));

    let tags_json = serde_json::to_value(&tags_from_file).unwrap_or(serde_json::json!({}));

    // 16. Upsert derived track (same library, different profile)
    let derived_track = store
        .upsert_track(UpsertTrack {
            library_id: track.library_id,
            relative_path: out_rel,
            file_hash,
            title: tags_from_file.get("title").cloned().or_else(|| track.title.clone()),
            artist: tags_from_file.get("artist").cloned().or_else(|| track.artist.clone()),
            albumartist: tags_from_file
                .get("albumartist")
                .cloned()
                .or_else(|| track.albumartist.clone()),
            album: tags_from_file.get("album").cloned().or_else(|| track.album.clone()),
            tracknumber: tags_from_file
                .get("tracknumber")
                .cloned()
                .or_else(|| track.tracknumber.clone()),
            discnumber: tags_from_file
                .get("discnumber")
                .cloned()
                .or_else(|| track.discnumber.clone()),
            totaldiscs: tags_from_file
                .get("totaldiscs")
                .cloned()
                .or_else(|| track.totaldiscs.clone()),
            totaltracks: tags_from_file
                .get("totaltracks")
                .cloned()
                .or_else(|| track.totaltracks.clone()),
            date: tags_from_file.get("date").cloned().or_else(|| track.date.clone()),
            genre: tags_from_file.get("genre").cloned().or_else(|| track.genre.clone()),
            composer: tags_from_file.get("composer").cloned().or_else(|| track.composer.clone()),
            label: tags_from_file.get("label").cloned().or_else(|| track.label.clone()),
            catalognumber: tags_from_file
                .get("catalognumber")
                .cloned()
                .or_else(|| track.catalognumber.clone()),
            tags: tags_json,
            duration_secs: audio_props.duration_secs,
            bitrate: audio_props.bitrate,
            sample_rate: audio_props.sample_rate,
            channels: audio_props.channels,
            bit_depth: audio_props.bit_depth,
            has_embedded_art: audio_props.has_embedded_art,
            status: "active".into(),
            library_profile_id: Some(lib_profile.id),
        })
        .await?;

    // 17. Create track link between source and derived track
    store
        .create_track_link(track_id, derived_track.id)
        .await?;

    // 18. Return success result
    Ok(serde_json::json!({
        "status": "completed",
        "track_id": track_id,
        "derived_track_id": derived_track.id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn profile(codec: &str, bit_depth: Option<i64>) -> EncodingProfile {
        EncodingProfile {
            id: 1,
            name: "test".into(),
            codec: codec.into(),
            bitrate: None,
            sample_rate: None,
            channels: None,
            bit_depth,
            advanced_args: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn flac_16bit() {
        let args = build_ffmpeg_args(&profile("flac", Some(16)));
        assert!(args.contains(&"-sample_fmt".to_string()));
        assert!(args.contains(&"s16".to_string()));
        assert!(!args.contains(&"-bits_per_raw_sample".to_string()));
    }

    #[test]
    fn flac_24bit() {
        let args = build_ffmpeg_args(&profile("flac", Some(24)));
        assert!(args.contains(&"-sample_fmt".to_string()));
        assert!(args.contains(&"s32".to_string()));
        assert!(args.contains(&"-bits_per_raw_sample".to_string()));
        assert!(args.contains(&"24".to_string()));
    }

    #[test]
    fn flac_32bit() {
        let args = build_ffmpeg_args(&profile("flac", Some(32)));
        assert!(args.contains(&"-sample_fmt".to_string()));
        assert!(args.contains(&"s32".to_string()));
        assert!(!args.contains(&"-bits_per_raw_sample".to_string()));
    }

    #[test]
    fn flac_no_bit_depth_leaves_sample_fmt_unset() {
        let args = build_ffmpeg_args(&profile("flac", None));
        assert!(!args.contains(&"-sample_fmt".to_string()));
    }

    #[test]
    fn non_flac_ignores_bit_depth() {
        let args = build_ffmpeg_args(&profile("aac", Some(24)));
        assert!(!args.contains(&"-sample_fmt".to_string()));
        assert!(!args.contains(&"-bits_per_raw_sample".to_string()));
    }
}
