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

/// Parse "256k", "1.5M", or bare "256" → kbps as i64.
fn parse_bitrate_kbps(s: &str) -> Option<i64> {
    let s = s.trim().to_lowercase();
    if let Some(rest) = s.strip_suffix('k') {
        rest.parse().ok()
    } else if let Some(rest) = s.strip_suffix('m') {
        rest.parse::<f64>().ok().map(|v| (v * 1000.0) as i64)
    } else {
        s.parse().ok()
    }
}
