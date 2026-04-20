use reqwest::Client;
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;

const MB_RATE_LIMIT_MS: u64 = 1100; // MusicBrainz: max 1 req/sec

#[derive(Clone)]
pub struct MusicBrainzService {
    client: Client,
    acoustid_key: String,
    mb_base: String,
    acoustid_base: String,
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
    #[serde(rename = "primary-type")]
    pub primary_type: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbMedia {
    pub position: Option<u32>,
    #[serde(rename = "track-count")]
    pub track_count: Option<u32>,
}

impl MusicBrainzService {
    pub fn new(acoustid_key: String) -> Self {
        Self::with_base_urls(
            acoustid_key,
            "https://musicbrainz.org/ws/2".into(),
            "https://api.acoustid.org".into(),
        )
    }

    pub fn with_base_urls(acoustid_key: String, mb_base: String, acoustid_base: String) -> Self {
        let client = Client::builder()
            .user_agent("suzuran/0.3 ( https://github.com/user/suzuran )")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client build");
        Self { client, acoustid_key, mb_base, acoustid_base }
    }

    pub async fn acoustid_lookup(
        &self,
        fingerprint: &str,
        duration: f64,
    ) -> anyhow::Result<Vec<AcoustIdResult>> {
        let url = format!("{}/v2/lookup", self.acoustid_base);
        let resp: serde_json::Value = self.client
            .get(&url)
            .query(&[
                ("client", self.acoustid_key.as_str()),
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
        sleep(Duration::from_millis(MB_RATE_LIMIT_MS)).await;
        let url = format!("{}/recording/{}", self.mb_base, recording_id);
        let rec = self.client
            .get(&url)
            .query(&[
                ("inc", "releases+artist-credits+labels+release-groups+media"),
                ("fmt", "json"),
            ])
            .send().await?
            .error_for_status()?
            .json::<MbRecording>().await?;
        Ok(rec)
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

        // Artist from recording-level artist-credit
        let artist_name = rec.artist_credit.as_ref()
            .and_then(|ac| ac.first())
            .and_then(|a| a.name.as_ref().or(a.artist.as_ref().map(|ar| &ar.name)))
            .cloned()
            .unwrap_or_default();
        if !artist_name.is_empty() {
            tags.insert("artist".into(), artist_name.clone());
            tags.insert("albumartist".into(), artist_name);
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

        // Disc count
        if let Some(media) = &release.media {
            let disc_count = media.len();
            if disc_count > 1 {
                tags.insert("totaldiscs".into(), disc_count.to_string());
            }
        }

        // Release group type
        if let Some(rg) = &release.release_group {
            if let Some(pt) = &rg.primary_type {
                tags.insert("releasetype".into(), pt.to_lowercase());
            }
        }

        tags
    }

    /// Cover Art Archive URL for a release (front image, 500px).
    pub fn caa_url(release_id: &str) -> String {
        format!("https://coverartarchive.org/release/{}/front-500", release_id)
    }
}
