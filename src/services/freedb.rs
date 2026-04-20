use reqwest::Client;
use std::{collections::HashMap, time::Duration};

pub struct FreedBService {
    client: Client,
    base_url: String,
}

#[derive(Debug)]
pub struct FreedBCandidate {
    pub artist: String,
    pub album: String,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub tracks: Vec<String>, // indexed 0..N-1
}

impl FreedBService {
    pub fn new() -> Self {
        Self::with_base_url("http://gnudb.org/~cddb/cddb.cgi".into())
    }

    pub fn with_base_url(base_url: String) -> Self {
        let client = Client::builder()
            .user_agent("suzuran/0.3 ( music-library-manager )")
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        Self { client, base_url }
    }

    /// Look up a disc by CDDB disc ID. Returns first matching candidate, or None.
    pub async fn disc_lookup(&self, disc_id: &str) -> anyhow::Result<Option<FreedBCandidate>> {
        // Step 1: query
        let query_cmd = format!("cddb query {} 1 0 60", disc_id);
        let query_resp = self.cddb_request(&query_cmd).await?;

        let status = query_resp
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().next())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        // 202 = no match, 0 = parse failure
        if status == 202 || status == 0 {
            return Ok(None);
        }

        // Extract category + disc ID from the first result line
        let (category, found_id) = parse_query_first_result(&query_resp)
            .ok_or_else(|| anyhow::anyhow!("could not parse CDDB query response"))?;

        // Step 2: read the full entry
        let read_cmd = format!("cddb read {} {}", category, found_id);
        let read_resp = self.cddb_request(&read_cmd).await?;

        if !read_resp.starts_with("200") {
            return Ok(None);
        }

        Ok(Some(parse_xmcd(&read_resp)))
    }

    async fn cddb_request(&self, cmd: &str) -> anyhow::Result<String> {
        let text = self
            .client
            .get(&self.base_url)
            .query(&[
                ("cmd", cmd),
                ("hello", "user localhost suzuran 0.3"),
                ("proto", "6"),
            ])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(text)
    }

    /// Convert a FreedBCandidate to a tag map.
    pub fn to_tag_map(
        candidate: &FreedBCandidate,
        zero_based_track_index: usize,
    ) -> HashMap<String, String> {
        let mut tags = HashMap::new();
        tags.insert("artist".into(), candidate.artist.clone());
        tags.insert("albumartist".into(), candidate.artist.clone());
        tags.insert("album".into(), candidate.album.clone());
        if let Some(year) = &candidate.year {
            tags.insert("date".into(), year.clone());
        }
        if let Some(genre) = &candidate.genre {
            tags.insert("genre".into(), genre.clone());
        }
        if let Some(title) = candidate.tracks.get(zero_based_track_index) {
            tags.insert("title".into(), title.clone());
        }
        tags.insert("totaltracks".into(), candidate.tracks.len().to_string());
        tags
    }
}

fn parse_query_first_result(text: &str) -> Option<(String, String)> {
    // After the status line, result lines look like: "rock a50e1d13 Artist / Album"
    let line = text
        .lines()
        .skip(1)
        .find(|l| !l.starts_with('.') && !l.trim().is_empty())?;
    let mut parts = line.splitn(3, ' ');
    let category = parts.next()?.to_string();
    let disc_id = parts.next()?.to_string();
    Some((category, disc_id))
}

fn parse_xmcd(text: &str) -> FreedBCandidate {
    let mut artist = String::new();
    let mut album = String::new();
    let mut year = None;
    let mut genre = None;
    let mut tracks: std::collections::BTreeMap<usize, String> = Default::default();

    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.starts_with('#') || line == "." {
            continue;
        }

        if let Some(val) = line.strip_prefix("DTITLE=") {
            if let Some((a, b)) = val.split_once(" / ") {
                artist = a.trim().into();
                album = b.trim().into();
            } else {
                album = val.trim().into();
            }
        } else if let Some(val) = line.strip_prefix("DYEAR=") {
            year = Some(val.trim().into());
        } else if let Some(val) = line.strip_prefix("DGENRE=") {
            genre = Some(val.trim().into());
        } else if line.starts_with("TTITLE") {
            if let Some(eq) = line.find('=') {
                let idx_str = &line[6..eq];
                if let Ok(idx) = idx_str.parse::<usize>() {
                    tracks.insert(idx, line[eq + 1..].trim().into());
                }
            }
        }
    }

    FreedBCandidate {
        artist,
        album,
        year,
        genre,
        tracks: tracks.into_values().collect(),
    }
}
