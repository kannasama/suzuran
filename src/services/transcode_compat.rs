use std::cmp::Ordering;

use crate::models::EncodingProfile;

/// Returns true if transcoding a source with the given properties into `profile` is acceptable.
/// Any rule violation returns false — the job should be skipped, not failed.
pub fn is_compatible(
    src_format: &str,           // file extension or library.format ("flac", "aac", "wv", …)
    src_sample_rate: Option<i64>,
    src_bit_depth: Option<i64>,
    src_bitrate: Option<i64>,   // kbps as stored in tracks.bitrate
    profile: &EncodingProfile,
) -> bool {
    let src_lossless  = is_lossless(src_format);
    let dst_lossless  = is_lossless(&profile.codec);

    // Rule 1: no lossy → lossless upconversion
    if !src_lossless && dst_lossless {
        return false;
    }

    // Rule 2: no sample-rate upsampling
    if let (Some(src_sr), Some(prof_sr)) = (src_sample_rate, profile.sample_rate) {
        if src_sr < prof_sr {
            return false;
        }
    }

    // Rule 3 (lossless → lossless): no bit-depth inflation
    if src_lossless && dst_lossless {
        if let (Some(src_bd), Some(prof_bd)) = (src_bit_depth, profile.bit_depth) {
            if src_bd < prof_bd {
                return false;
            }
        }
    }

    // Rule 4 (lossy → lossy): no bitrate upscaling
    if !src_lossless && !dst_lossless {
        if let (Some(src_br), Some(prof_br)) =
            (src_bitrate, profile.bitrate.as_deref().and_then(parse_bitrate_kbps))
        {
            if src_br < prof_br {
                return false;
            }
        }
    }

    true
}

pub fn is_lossless(format_or_codec: &str) -> bool {
    matches!(
        format_or_codec.to_lowercase().trim_start_matches('.'),
        "flac" | "alac" | "wavpack" | "wv" | "ape" | "tta" | "wav" | "aiff" | "aif"
    )
}

/// Extract the lowercase extension from a file path (without leading dot).
/// "source/Artist/Album/track.m4a" → "m4a"
pub fn format_from_path(path: &str) -> &str {
    path.rsplit('.').next().map(str::trim).unwrap_or("")
}

/// Returns true if `file_format` (a file extension) corresponds to `profile_codec`.
/// Handles common aliases: m4a/mp4 → aac, ogg/oga → libvorbis, etc.
pub fn codecs_match(file_format: &str, profile_codec: &str) -> bool {
    let fmt = file_format.to_lowercase();
    let codec = profile_codec.to_lowercase();
    match fmt.as_str() {
        "m4a" | "aac" | "mp4" => matches!(codec.as_str(), "aac" | "libfdk_aac" | "libfdk-aac"),
        "mp3"                  => matches!(codec.as_str(), "mp3" | "libmp3lame"),
        "ogg" | "oga"          => matches!(codec.as_str(), "vorbis" | "libvorbis"),
        "opus"                 => matches!(codec.as_str(), "opus" | "libopus"),
        "flac"                 => codec == "flac",
        "alac"                 => codec == "alac",
        "wav"                  => matches!(codec.as_str(), "wav" | "pcm_s16le" | "pcm_s24le" | "pcm_f32le"),
        "wv"                   => matches!(codec.as_str(), "wv" | "wavpack"),
        "ape"                  => matches!(codec.as_str(), "ape" | "monkey's audio"),
        "tta"                  => codec == "tta",
        _                      => fmt == codec,
    }
}

/// Compute a quality score for a track's audio properties.
///
/// Priority (highest to lowest):
///   1. Lossless > lossy
///   2. Sample rate (higher = better)
///   3. Bit depth (for lossless) or bitrate in kbps (for lossy)
///
/// The score is a plain u64 suitable for comparison or serialisation.
pub fn quality_rank(
    format: &str,
    sample_rate: Option<i64>,
    bit_depth: Option<i64>,
    bitrate: Option<i64>,
) -> u64 {
    let lossless_flag: u64 = if is_lossless(format) { 1_000_000_000_000 } else { 0 };
    // sample_rate in Hz (up to ~192 000) — scaled to avoid overlap with lower tiers
    let sr: u64 = sample_rate.unwrap_or(0).max(0) as u64 * 1_000_000;
    // bit_depth: 0–32 → multiplied so it outweighs bitrate
    let bd: u64 = bit_depth.unwrap_or(0).max(0) as u64 * 10_000;
    // bitrate in kbps: 0–2 000 (direct)
    let br: u64 = bitrate.unwrap_or(0).max(0) as u64;
    lossless_flag + sr + bd + br
}

/// Compare two tracks' quality using the same tier ordering as `quality_rank`.
pub fn quality_cmp(
    a_format: &str,
    a_sr: Option<i64>,
    a_bd: Option<i64>,
    a_br: Option<i64>,
    b_format: &str,
    b_sr: Option<i64>,
    b_bd: Option<i64>,
    b_br: Option<i64>,
) -> Ordering {
    let a_lossless = is_lossless(a_format);
    let b_lossless = is_lossless(b_format);

    // Tier 1: lossless beats lossy
    if a_lossless != b_lossless {
        return a_lossless.cmp(&b_lossless);
    }

    // Tier 2: higher sample rate wins
    let sr_cmp = a_sr.unwrap_or(0).cmp(&b_sr.unwrap_or(0));
    if sr_cmp != Ordering::Equal {
        return sr_cmp;
    }

    // Tier 3: bit depth (lossless) or bitrate (lossy)
    if a_lossless {
        a_bd.unwrap_or(0).cmp(&b_bd.unwrap_or(0))
    } else {
        a_br.unwrap_or(0).cmp(&b_br.unwrap_or(0))
    }
}

/// Returns true when the source file is already in the target format at equal or
/// better quality — i.e. transcoding would produce no improvement.  Used to skip
/// job creation rather than to gate quality.
///
/// Distinct from `is_compatible` (which only checks for upscaling violations):
/// a source can be compatible without being a no-op if the codec differs.
pub fn is_noop_transcode(
    src_format: &str,
    src_sample_rate: Option<i64>,
    src_bit_depth: Option<i64>,
    src_bitrate: Option<i64>,
    profile: &EncodingProfile,
) -> bool {
    if !codecs_match(src_format, &profile.codec) {
        return false;
    }
    let src_lossless = is_lossless(src_format);
    match src_lossless {
        false => {
            // lossy → same lossy: skip when src bitrate meets or exceeds profile target
            match (src_bitrate, profile.bitrate.as_deref().and_then(parse_bitrate_kbps)) {
                (Some(src_br), Some(prof_br)) => src_br >= prof_br,
                _ => false, // unknown bitrate — don't assume equal
            }
        }
        true => {
            // lossless → same lossless: skip when src doesn't need any conversion
            let sr_ok = match (src_sample_rate, profile.sample_rate) {
                (Some(src_sr), Some(prof_sr)) => src_sr >= prof_sr,
                _ => true,
            };
            let bd_ok = match (src_bit_depth, profile.bit_depth) {
                (Some(src_bd), Some(prof_bd)) => src_bd >= prof_bd,
                _ => true,
            };
            sr_ok && bd_ok
        }
    }
}

/// Parse "256k", "1.5M", or bare "256" → kbps as i64.
pub fn parse_bitrate_kbps(s: &str) -> Option<i64> {
    let s = s.trim().to_lowercase();
    if let Some(rest) = s.strip_suffix('k') {
        rest.parse().ok()
    } else if let Some(rest) = s.strip_suffix('m') {
        rest.parse::<f64>().ok().map(|v| (v * 1000.0) as i64)
    } else {
        s.parse().ok()
    }
}
