use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    jobs::ProcessStagedPayload,
    models::Track,
    services::transcode_compat::{
        codecs_match, format_from_path, parse_bitrate_kbps, quality_cmp, quality_rank,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/staged", get(list_staged))
        .route("/submit", post(submit))
        .route("/supersede-check", post(supersede_check))
}

async fn list_staged(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Track>>, AppError> {
    let libraries = state.db.list_libraries().await?;
    let mut all_tracks: Vec<Track> = Vec::new();
    for lib in libraries {
        let tracks = state.db.list_tracks_by_status(lib.id, "staged").await?;
        all_tracks.extend(tracks);
    }
    Ok(Json(all_tracks))
}

async fn submit(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(payload): Json<ProcessStagedPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let job = state.db.enqueue_job(
        "process_staged",
        serde_json::to_value(&payload)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize payload: {e}")))?,
        0,
    ).await?;
    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "job_id": job.id }))))
}

// ── supersede-check ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SupersedeCheckRequest {
    track_ids: Vec<i64>,
}

#[derive(Serialize)]
struct SupersedeCheckResult {
    track_id: i64,
    #[serde(rename = "match")]
    supersede_match: Option<SupersedeMatchInfo>,
}

#[derive(Serialize)]
struct SupersedeMatchInfo {
    active_track_id: i64,
    active_track_format: String,
    active_track_sample_rate: Option<i64>,
    active_track_bit_depth: Option<i64>,
    active_track_bitrate: Option<i64>,
    active_quality_rank: u64,
    staged_quality_rank: u64,
    identity_method: &'static str,
    is_upgrade: bool,
    profile_match: Option<ProfileMatchInfo>,
}

#[derive(Serialize, Clone)]
struct ProfileMatchInfo {
    library_profile_id: i64,
    profile_name: String,
    derived_dir_name: String,
}

async fn supersede_check(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(req): Json<SupersedeCheckRequest>,
) -> Result<Json<Vec<SupersedeCheckResult>>, AppError> {
    let mut results = Vec::with_capacity(req.track_ids.len());

    for track_id in req.track_ids {
        let staged = match state.db.get_track(track_id).await? {
            Some(t) => t,
            None => {
                results.push(SupersedeCheckResult { track_id, supersede_match: None });
                continue;
            }
        };

        // Only check staged tracks
        if staged.status != "staged" {
            results.push(SupersedeCheckResult { track_id, supersede_match: None });
            continue;
        }

        let (active, method) = find_active_match(&state, &staged).await?;

        let Some(active) = active else {
            results.push(SupersedeCheckResult { track_id, supersede_match: None });
            continue;
        };

        let staged_fmt = format_from_path(&staged.relative_path).to_string();
        let active_fmt = format_from_path(&active.relative_path).to_string();

        let staged_rank = quality_rank(&staged_fmt, staged.sample_rate, staged.bit_depth, staged.bitrate);
        let active_rank = quality_rank(&active_fmt, active.sample_rate, active.bit_depth, active.bitrate);

        let is_upgrade = quality_cmp(
            &staged_fmt, staged.sample_rate, staged.bit_depth, staged.bitrate,
            &active_fmt, active.sample_rate, active.bit_depth, active.bitrate,
        ) == std::cmp::Ordering::Greater;

        // Only surface if the staged track is actually an upgrade
        if !is_upgrade {
            results.push(SupersedeCheckResult { track_id, supersede_match: None });
            continue;
        }

        let profile_match = find_profile_match(&state, active.library_id, &active).await?;

        results.push(SupersedeCheckResult {
            track_id,
            supersede_match: Some(SupersedeMatchInfo {
                active_track_id: active.id,
                active_track_format: active_fmt,
                active_track_sample_rate: active.sample_rate,
                active_track_bit_depth: active.bit_depth,
                active_track_bitrate: active.bitrate,
                active_quality_rank: active_rank,
                staged_quality_rank: staged_rank,
                identity_method: method,
                is_upgrade,
                profile_match,
            }),
        });
    }

    Ok(Json(results))
}

/// Three-tier identity matching: MB recording ID → tag tuple → AcoustID fingerprint.
/// Returns the matching active track and a string naming the method used.
async fn find_active_match(
    state: &AppState,
    staged: &Track,
) -> Result<(Option<Track>, &'static str), AppError> {
    // Tier 1: MusicBrainz recording ID
    if let Some(mb_id) = staged
        .tags
        .as_object()
        .and_then(|o| o.get("musicbrainz_recordingid"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        if let Some(t) = state.db
            .find_active_source_track_by_mb_id(staged.library_id, mb_id)
            .await?
        {
            return Ok((Some(t), "mb_recording_id"));
        }
    }

    // Tier 2: normalised tag tuple
    let aa = staged
        .albumartist
        .as_deref()
        .or(staged.artist.as_deref())
        .unwrap_or("")
        .to_lowercase();
    let al = staged.album.as_deref().unwrap_or("").to_lowercase();
    let disc = staged.discnumber.as_deref().unwrap_or("1").to_string();
    let track_num = staged
        .tracknumber
        .as_deref()
        .unwrap_or("0")
        .split('/')
        .next()
        .unwrap_or("0")
        .trim()
        .to_string();

    if !aa.is_empty() && !al.is_empty() {
        if let Some(t) = state.db
            .find_active_source_track_by_tags(staged.library_id, &aa, &al, &disc, &track_num)
            .await?
        {
            return Ok((Some(t), "tag_tuple"));
        }
    }

    // Tier 3: AcoustID fingerprint
    if let Some(fp) = staged.acoustid_fingerprint.as_deref().filter(|s| !s.is_empty()) {
        if let Some(t) = state.db
            .find_active_source_track_by_fingerprint(staged.library_id, fp)
            .await?
        {
            return Ok((Some(t), "acoustid"));
        }
    }

    Ok((None, ""))
}

/// Find the library profile whose encoding profile best matches the old active track's
/// audio properties (codec, sample rate ±5%, bitrate ±20%).
async fn find_profile_match(
    state: &AppState,
    library_id: i64,
    old_track: &Track,
) -> Result<Option<ProfileMatchInfo>, AppError> {
    let profiles = state.db.list_library_profiles(library_id).await?;
    let old_fmt = format_from_path(&old_track.relative_path);
    let old_sr = old_track.sample_rate;
    let old_br = old_track.bitrate;

    let mut best: Option<(i64, ProfileMatchInfo)> = None; // (score, info)

    for lp in profiles {
        let enc = match state.db.get_encoding_profile(lp.encoding_profile_id).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !codecs_match(old_fmt, &enc.codec) {
            continue;
        }

        // Sample rate: within 5%
        let sr_ok = match (old_sr, enc.sample_rate) {
            (Some(a), Some(b)) => {
                let diff = (a - b).unsigned_abs() as f64 / b.max(1) as f64;
                diff <= 0.05
            }
            (None, None) => true,
            // If one side has a sample rate and the other doesn't, still consider it a match
            // (avoids blocking on tracks that haven't been scanned for sample rate yet)
            _ => true,
        };

        if !sr_ok {
            continue;
        }

        // Bitrate: within 20%; score = absolute diff in kbps (lower = better)
        let br_score: i64 = match (old_br, enc.bitrate.as_deref().and_then(parse_bitrate_kbps)) {
            (Some(a), Some(b)) => {
                let diff = (a - b).unsigned_abs() as f64 / b.max(1) as f64;
                if diff > 0.20 {
                    continue;
                }
                (diff * 1000.0) as i64
            }
            // One or both sides missing bitrate — treat as a weak match
            _ => 500,
        };

        if best.is_none() || br_score < best.as_ref().unwrap().0 {
            best = Some((
                br_score,
                ProfileMatchInfo {
                    library_profile_id: lp.id,
                    profile_name: enc.name.clone(),
                    derived_dir_name: lp.derived_dir_name.clone(),
                },
            ));
        }
    }

    Ok(best.map(|(_, info)| info))
}
