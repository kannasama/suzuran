use reqwest::Client;
use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use tokio::{sync::Mutex, time::sleep};

/// gnudb.org has no published hard rate limit; 1 req/sec is safe and polite.
const FREEDB_RATE_LIMIT_MS: u64 = 1000;

pub struct FreedBService {
    client: Client,
    base_url: String,
    last_request: Arc<Mutex<Option<Instant>>>,
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
            .expect("failed to build FreeDB HTTP client");
        Self { client, base_url, last_request: Arc::new(Mutex::new(None)) }
    }

    /// Wait until at least FREEDB_RATE_LIMIT_MS has elapsed since the last request.
    /// Holds the async lock across the sleep to prevent concurrent burst.
    async fn rate_limit(&self) {
        let mut guard = self.last_request.lock().await;
        if let Some(prev) = *guard {
            let elapsed = prev.elapsed();
            if elapsed < Duration::from_millis(FREEDB_RATE_LIMIT_MS) {
                sleep(Duration::from_millis(FREEDB_RATE_LIMIT_MS) - elapsed).await;
            }
        }
        *guard = Some(Instant::now());
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
        self.rate_limit().await;
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

    /// Search by artist + album name. Returns up to 5 matching candidates.
    /// Uses gnudb.org's web search HTML endpoint (no formal API available).
    pub async fn text_search(
        &self,
        artist: &str,
        album: &str,
    ) -> anyhow::Result<Vec<FreedBCandidate>> {
        let keywords = format!("{} {}", artist, album);
        // Derive search base from cddb URL: strip the CGI path
        let search_base = self.base_url
            .split("/~cddb")
            .next()
            .unwrap_or(&self.base_url)
            .trim_end_matches('/');
        let search_url = format!("{}/search/search", search_base);

        self.rate_limit().await;
        let html = self.client
            .get(&search_url)
            .query(&[("keywords", &keywords), ("type", &"0".to_string())])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let disc_ids = parse_search_html(&html);
        let mut candidates = Vec::new();
        for (category, disc_id) in disc_ids.into_iter().take(5) {
            let read_cmd = format!("cddb read {} {}", category, disc_id);
            if let Ok(read_resp) = self.cddb_request(&read_cmd).await {
                if read_resp.starts_with("200") {
                    candidates.push(parse_xmcd(&read_resp));
                }
            }
        }
        Ok(candidates)
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

impl Default for FreedBService {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract (category, disc_id) pairs from gnudb.org search result HTML.
/// Links appear as: href="/gnudb/CATEGORY/DISCID"
fn parse_search_html(html: &str) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();
    for cap in html.split("href=\"/gnudb/") {
        let fragment = cap;
        if let Some(end) = fragment.find('"') {
            let path = &fragment[..end];
            let parts: Vec<&str> = path.splitn(2, '/').collect();
            if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                let entry = (parts[0].to_string(), parts[1].to_string());
                if seen.insert(entry.clone()) {
                    results.push(entry);
                }
            }
        }
    }
    results
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

pub fn parse_xmcd(text: &str) -> FreedBCandidate {
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
                    let part = line[eq + 1..].trim();
                    tracks
                        .entry(idx)
                        .and_modify(|e| e.push_str(part))
                        .or_insert_with(|| part.to_string());
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
