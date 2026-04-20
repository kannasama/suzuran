#[derive(Debug, Clone)]
pub struct CueSheet {
    pub album_title: Option<String>,
    pub performer: Option<String>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub audio_file: String,     // filename from FILE directive (not the full path)
    pub tracks: Vec<CueTrack>,
}

#[derive(Debug, Clone)]
pub struct CueTrack {
    pub number: u32,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub index_01_secs: f64,
}

pub fn parse_cue(content: &str) -> anyhow::Result<CueSheet> {
    let mut album_title = None;
    let mut performer = None;
    let mut date = None;
    let mut genre = None;
    let mut audio_file = String::new();
    let mut tracks: Vec<CueTrack> = Vec::new();
    let mut current_track: Option<(u32, Option<String>, Option<String>)> = None;

    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = strip_quoted(line, "TITLE ") {
            if current_track.is_none() { album_title = Some(val); }
            else if let Some(t) = current_track.as_mut() { t.1 = Some(val); }
        } else if let Some(val) = strip_quoted(line, "PERFORMER ") {
            if current_track.is_none() { performer = Some(val); }
            else if let Some(t) = current_track.as_mut() { t.2 = Some(val); }
        } else if let Some(val) = line.strip_prefix("REM DATE ") {
            date = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("REM GENRE ") {
            genre = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("FILE ") {
            // FILE "name.flac" WAVE|BINARY|MP3
            audio_file = if val.starts_with('"') {
                val.trim_start_matches('"').split('"').next().unwrap_or("").to_string()
            } else {
                val.split_whitespace().next().unwrap_or("").to_string()
            };
        } else if let Some(val) = line.strip_prefix("TRACK ") {
            let num: u32 = val.split_whitespace().next()
                .and_then(|s| s.parse().ok()).unwrap_or(0);
            current_track = Some((num, None, None));
        } else if let Some(val) = line.strip_prefix("INDEX 01 ") {
            let secs = parse_index_time(val.trim());
            if let Some((num, title, perf)) = current_track.take() {
                tracks.push(CueTrack { number: num, title, performer: perf, index_01_secs: secs });
            }
        }
    }

    if audio_file.is_empty() {
        anyhow::bail!("CUE sheet has no FILE directive");
    }
    Ok(CueSheet { album_title, performer, date, genre, audio_file, tracks })
}

fn strip_quoted(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let inner = rest.trim().trim_matches('"');
    Some(inner.to_string())
}

fn parse_index_time(s: &str) -> f64 {
    let parts: Vec<f64> = s.split(':')
        .filter_map(|p| p.parse().ok())
        .collect();
    match parts.as_slice() {
        [m, s, f] => m * 60.0 + s + f / 75.0,
        [m, s]    => m * 60.0 + s,
        _          => 0.0,
    }
}
