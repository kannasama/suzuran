use reqwest::Client;
use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use tokio::{sync::Mutex, time::sleep};

use crate::error::AppError;

/// MusicBrainz: max 1 req/sec (we use 1.1 s to give a small margin).
const MB_RATE_LIMIT_MS: u64 = 1100;
/// AcoustID: no published hard limit; 350 ms (~2.8 req/s) is safe and polite.
const ACOUSTID_RATE_LIMIT_MS: u64 = 350;

#[derive(Clone)]
pub struct MusicBrainzService {
    client: Client,
    mb_base: String,
    acoustid_base: String,
    /// Tokio async mutex so the guard can be held across the sleep, preventing burst.
    last_mb_request: Arc<Mutex<Option<Instant>>>,
    last_acoustid_request: Arc<Mutex<Option<Instant>>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AcoustIdResult {
    pub id: String,
    pub score: f32,
    pub recordings: Option<Vec<AcoustIdRecording>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AcoustIdRecording {
    pub id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbRecording {
    pub id: String,
    pub title: String,
    pub length: Option<u64>,
    pub releases: Option<Vec<MbRelease>>,
    #[serde(rename = "artist-credit")]
    pub artist_credit: Option<Vec<MbArtistCredit>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbRelease {
    pub id: String,
    pub title: String,
    pub date: Option<String>,
    pub status: Option<String>,
    pub country: Option<String>,
    #[serde(rename = "artist-credit")]
    pub artist_credit: Option<Vec<MbArtistCredit>>,
    #[serde(rename = "label-info")]
    pub label_info: Option<Vec<MbLabelInfo>>,
    #[serde(rename = "release-group")]
    pub release_group: Option<MbReleaseGroup>,
    pub media: Option<Vec<MbMedia>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbArtistCredit {
    pub name: Option<String>,
    pub artist: Option<MbArtist>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbArtist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbLabelInfo {
    pub label: Option<MbLabel>,
    #[serde(rename = "catalog-number")]
    pub catalog_number: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbLabel {
    pub name: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbReleaseGroup {
    pub id: Option<String>,
    #[serde(rename = "primary-type")]
    pub primary_type: Option<String>,
    #[serde(rename = "secondary-types")]
    pub secondary_types: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbTrack {
    pub position: Option<u32>,
    pub number: Option<String>,
    pub recording: Option<MbTrackRecording>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbMedia {
    pub position: Option<u32>,
    #[serde(rename = "track-count")]
    pub track_count: Option<u32>,
    pub tracks: Option<Vec<MbTrack>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbTrackRecording {
    pub id: String,
}

impl Default for MusicBrainzService {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicBrainzService {
    pub fn new() -> Self {
        Self::with_base_urls(
            "https://musicbrainz.org/ws/2".into(),
            "https://api.acoustid.org".into(),
        )
    }

    pub fn with_base_urls(mb_base: String, acoustid_base: String) -> Self {
        let client = Client::builder()
            .user_agent("suzuran/0.3 ( music-library-manager )")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build MusicBrainz HTTP client");
        Self {
            client,
            mb_base,
            acoustid_base,
            last_mb_request: Arc::new(Mutex::new(None)),
            last_acoustid_request: Arc::new(Mutex::new(None)),
        }
    }

    /// Wait until at least MB_RATE_LIMIT_MS has elapsed since the last MB request.
    /// Holds the async lock across the sleep to prevent concurrent burst.
    async fn mb_rate_limit(&self) {
        let mut guard = self.last_mb_request.lock().await;
        if let Some(prev) = *guard {
            let elapsed = prev.elapsed();
            if elapsed < Duration::from_millis(MB_RATE_LIMIT_MS) {
                sleep(Duration::from_millis(MB_RATE_LIMIT_MS) - elapsed).await;
            }
        }
        *guard = Some(Instant::now());
    }

    /// Wait until at least ACOUSTID_RATE_LIMIT_MS has elapsed since the last AcoustID request.
    async fn acoustid_rate_limit(&self) {
        let mut guard = self.last_acoustid_request.lock().await;
        if let Some(prev) = *guard {
            let elapsed = prev.elapsed();
            if elapsed < Duration::from_millis(ACOUSTID_RATE_LIMIT_MS) {
                sleep(Duration::from_millis(ACOUSTID_RATE_LIMIT_MS) - elapsed).await;
            }
        }
        *guard = Some(Instant::now());
    }

    pub async fn acoustid_lookup(
        &self,
        key: &str,
        fingerprint: &str,
        duration: f64,
    ) -> anyhow::Result<Vec<AcoustIdResult>> {
        self.acoustid_rate_limit().await;
        let url = format!("{}/v2/lookup", self.acoustid_base);
        let resp: serde_json::Value = self.client
            .get(&url)
            .query(&[
                ("client", key),
                ("fingerprint", fingerprint),
                ("duration", &duration.round().to_string()),
                ("meta", "recordings"),
            ])
            .send().await?
            .error_for_status()?
            .json().await?;

        let results = resp["results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        Ok(results)
    }

    pub async fn get_recording(&self, recording_id: &str) -> anyhow::Result<MbRecording> {
        self.mb_rate_limit().await;
        let url = format!("{}/recording/{}", self.mb_base, recording_id);
        let rec = self.client
            .get(&url)
            .query(&[
                ("inc", "releases+release-groups+artist-credits"),
                ("fmt", "json"),
            ])
            .send().await?
            .error_for_status()?
            .json::<MbRecording>().await?;
        Ok(rec)
    }

    /// Fetch a full release by ID, including track listings (via `recordings` inc),
    /// artist credits, media, label info, and release group.
    ///
    /// This is the second step of the two-step lookup: once we have picked the
    /// best release from the recording response, we fetch the release directly
    /// so that `to_tag_map` can resolve disc/track position from the full track list.
    pub async fn get_release(&self, release_id: &str) -> anyhow::Result<MbRelease> {
        self.mb_rate_limit().await;
        let url = format!("{}/release/{}", self.mb_base, release_id);
        let release = self.client
            .get(&url)
            .query(&[
                ("inc", "recordings+artist-credits+labels+release-groups"),
                ("fmt", "json"),
            ])
            .send().await?
            .error_for_status()?
            .json::<MbRelease>().await?;
        Ok(release)
    }

    /// Build a MusicBrainz-keyed tag map from a recording + chosen release.
    pub fn to_tag_map(rec: &MbRecording, release: &MbRelease) -> HashMap<String, String> {
        let mut tags = HashMap::new();

        tags.insert("title".into(), rec.title.clone());
        tags.insert("musicbrainz_trackid".into(), rec.id.clone());
        tags.insert("musicbrainz_releaseid".into(), release.id.clone());
        tags.insert("album".into(), release.title.clone());

        if let Some(date) = &release.date {
            tags.insert("date".into(), date.clone());
        }

        // Recording-level artist → "artist" tag
        let artist_name = rec.artist_credit.as_ref()
            .and_then(|ac| ac.first())
            .and_then(|a| a.name.as_ref().or(a.artist.as_ref().map(|ar| &ar.name)))
            .cloned()
            .unwrap_or_default();

        // Release-level artist → "albumartist" tag (falls back to recording artist if absent)
        let albumartist_name = release.artist_credit.as_ref()
            .and_then(|ac| ac.first())
            .and_then(|a| a.name.as_ref().or(a.artist.as_ref().map(|ar| &ar.name)))
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| artist_name.clone());

        if !artist_name.is_empty() {
            tags.insert("artist".into(), artist_name);
        }
        if !albumartist_name.is_empty() {
            tags.insert("albumartist".into(), albumartist_name);
        }

        // Label + catalog number
        if let Some(li) = release.label_info.as_ref().and_then(|l| l.first()) {
            if let Some(label) = &li.label {
                tags.insert("label".into(), label.name.clone());
            }
            if let Some(cat) = &li.catalog_number {
                tags.insert("catalognumber".into(), cat.clone());
            }
        }

        // Disc/track position from media; also yields totaldiscs, discnumber, tracknumber, totaltracks
        if let Some(media) = &release.media {
            tags.insert("totaldiscs".into(), media.len().to_string());
            'outer: for medium in media {
                if let Some(tracks) = &medium.tracks {
                    for track in tracks {
                        if !track.recording.as_ref().map(|r| r.id == rec.id).unwrap_or(false) {
                            continue;
                        }
                        let disc_num = medium.position.unwrap_or(1);
                        tags.insert("discnumber".into(), disc_num.to_string());
                        if let Some(pos) = track.position {
                            tags.insert("tracknumber".into(), pos.to_string());
                        } else if let Some(num) = &track.number {
                            tags.insert("tracknumber".into(), num.clone());
                        }
                        if let Some(tc) = medium.track_count {
                            tags.insert("totaltracks".into(), tc.to_string());
                        }
                        break 'outer;
                    }
                }
            }
        }

        // Release group type
        if let Some(rg) = &release.release_group {
            if let Some(pt) = &rg.primary_type {
                tags.insert("releasetype".into(), pt.to_lowercase());
            }
            if let Some(rg_id) = &rg.id {
                tags.insert("musicbrainz_releasegroupid".into(), rg_id.clone());
            }
        }

        // Release status and country
        if let Some(status) = &release.status {
            tags.insert("releasestatus".into(), status.to_lowercase());
        }
        if let Some(country) = &release.country {
            tags.insert("releasecountry".into(), country.clone());
        }

        // MusicBrainz artist IDs
        if let Some(artist_id) = rec.artist_credit.as_ref()
            .and_then(|ac| ac.first())
            .and_then(|a| a.artist.as_ref())
            .map(|ar| ar.id.clone())
        {
            tags.insert("musicbrainz_artistid".into(), artist_id);
        }
        if let Some(albumartist_id) = release.artist_credit.as_ref()
            .and_then(|ac| ac.first())
            .and_then(|a| a.artist.as_ref())
            .map(|ar| ar.id.clone())
        {
            tags.insert("musicbrainz_albumartistid".into(), albumartist_id);
        }

        tags
    }

    /// Text-search MusicBrainz for recordings matching title/artist/album.
    ///
    /// Returns up to 5 `(tag_map, confidence)` pairs where confidence is
    /// capped at 0.6 (MB text-search is inherently fuzzier than AcoustID).
    pub async fn search_recordings(
        &self,
        title: &str,
        artist: &str,
        album: &str,
    ) -> Result<Vec<(HashMap<String, String>, f64)>, AppError> {
        let query = format!(
            r#"recording:"{title}" AND artist:"{artist}" AND release:"{album}""#
        );

        self.mb_rate_limit().await;
        let url = format!("{}/recording/", self.mb_base);
        let resp: serde_json::Value = self
            .client
            .get(&url)
            .query(&[
                ("query", query.as_str()),
                ("fmt", "json"),
                ("limit", "5"),
            ])
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("MB text search HTTP: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("MB text search status: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("MB text search parse: {e}")))?;

        let recordings = match resp["recordings"].as_array() {
            Some(arr) if !arr.is_empty() => arr,
            _ => return Ok(vec![]),
        };

        let mut out = Vec::new();
        for rec_val in recordings.iter().take(5) {
            let score_raw = rec_val["score"]
                .as_f64()
                .unwrap_or(0.0);
            let confidence = (score_raw / 100.0).min(0.6);

            // Parse enough structure to call to_tag_map
            let rec: MbRecording = match serde_json::from_value(rec_val.clone()) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Use first release if present, or construct a minimal one from the recording
            if let Some(releases) = &rec.releases {
                if let Some(release) = releases.first() {
                    let tags = Self::to_tag_map(&rec, release);
                    out.push((tags, confidence));
                    continue;
                }
            }

            // No release — build a minimal fallback tag map
            let mut tags = HashMap::new();
            tags.insert("title".into(), rec.title.clone());
            tags.insert("musicbrainz_trackid".into(), rec.id.clone());
            if let Some(ac) = &rec.artist_credit {
                if let Some(first) = ac.first() {
                    if let Some(name) = first.name.as_ref().or(first.artist.as_ref().map(|a| &a.name)) {
                        tags.insert("artist".into(), name.clone());
                        tags.insert("albumartist".into(), name.clone());
                    }
                }
            }
            out.push((tags, confidence));
        }

        Ok(out)
    }

    /// Score a release for MBP-style best-match selection.
    ///
    /// Higher is better. Factors (cumulative):
    /// - Official status:      +30
    /// - Release type:         Album +40, EP +25, Single +15, Compilation +10
    /// - Date (earlier pref):  up to +20 (1960→20, 1990→10, 2020+→0)
    /// - Existing tag seeds:   album match +25, albumartist +20, date year +15, totaltracks +10
    pub fn score_release(
        release: &MbRelease,
        existing_tags: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> i32 {
        let mut score = 0i32;

        if release.status.as_deref() == Some("Official") {
            score += 30;
        }

        if let Some(rg) = &release.release_group {
            match rg.primary_type.as_deref() {
                Some("Album")       => score += 40,
                Some("EP")          => score += 25,
                Some("Single")      => score += 15,
                Some("Compilation") => score += 10,
                _                   => {}
            }
        }

        if let Some(date) = &release.date {
            if let Ok(year) = date[..date.len().min(4)].parse::<i32>() {
                let year_score = (20i32 - ((year - 1960).max(0) / 3)).max(0);
                score += year_score;
            }
        }

        if let Some(tags) = existing_tags {
            let get_tag = |key: &str| -> String {
                tags.get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_lowercase()
            };

            // Album title match
            let existing_album = get_tag("album");
            if !existing_album.is_empty() && release.title.trim().to_lowercase() == existing_album {
                score += 25;
            }

            // Album artist match
            let existing_albumartist = get_tag("albumartist");
            if !existing_albumartist.is_empty() {
                let release_artist = release.artist_credit.as_ref()
                    .and_then(|ac| ac.first())
                    .and_then(|a| a.name.as_ref().or(a.artist.as_ref().map(|ar| &ar.name)))
                    .map(|s| s.trim().to_lowercase())
                    .unwrap_or_default();
                if !release_artist.is_empty() && release_artist == existing_albumartist {
                    score += 20;
                }
            }

            // Date year match
            let existing_date = get_tag("date");
            if !existing_date.is_empty() {
                if let Some(release_date) = &release.date {
                    let ex_year = existing_date.get(..4).unwrap_or("").trim();
                    let rel_year = release_date.get(..4).unwrap_or("").trim();
                    if !ex_year.is_empty() && ex_year == rel_year {
                        score += 15;
                    }
                }
            }

            // Total tracks match
            let existing_totaltracks = get_tag("totaltracks");
            if !existing_totaltracks.is_empty() {
                if let Some(media) = &release.media {
                    if media.iter().any(|m| {
                        m.track_count.map(|tc| tc.to_string()) == Some(existing_totaltracks.clone())
                    }) {
                        score += 10;
                    }
                }
            }
        }

        score
    }

    /// Cover Art Archive URL for a release (front image, 500px).
    pub fn caa_url(release_id: &str) -> String {
        format!("https://coverartarchive.org/release/{}/front-500", release_id)
    }
}
