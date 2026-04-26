use std::collections::HashMap;
use std::path::Path;

/// Re-interpret a string that was decoded as Latin-1 but actually contains
/// Shift-JIS bytes. lofty returns ID3v2 Latin-1 text frames as a Rust String
/// where each source byte ≤ 0xFF maps to the matching Unicode scalar; this
/// function reverses that mapping, then decodes the recovered bytes as SJIS.
/// Falls back to the original string if the bytes aren't valid SJIS.
pub fn redecode_latin1_as_sjis(s: &str) -> String {
    let bytes: Vec<u8> = s.chars()
        .filter_map(|c| {
            let code = c as u32;
            if code < 256 { Some(code as u8) } else { None }
        })
        .collect();
    let (result, _, had_errors) = encoding_rs::SHIFT_JIS.decode(&bytes);
    if had_errors { s.to_string() } else { result.into_owned() }
}

use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::ItemKey,
};

/// Audio properties read from the file.
#[derive(Debug, Default)]
pub struct AudioProperties {
    pub duration_secs: Option<f64>,
    pub bitrate: Option<i64>,      // kbps
    pub sample_rate: Option<i64>,  // Hz
    pub channels: Option<i64>,
    pub bit_depth: Option<i64>,    // bits per sample (lossless formats)
    pub has_embedded_art: bool,
}

/// Read all tags from `path`. Returns (tags, properties).
/// `tags` keys use MusicBrainz/Picard standard field names (lowercase).
pub fn read_tags(path: &Path) -> anyhow::Result<(HashMap<String, String>, AudioProperties)> {
    let tagged_file = Probe::open(path)?.read()?;

    let mut props = AudioProperties::default();

    let file_props = tagged_file.properties();
    props.duration_secs = Some(file_props.duration().as_secs_f64());
    // overall_bitrate() returns Some(0) (not None) for M4A containers; filter
    // zero values before falling back to audio_bitrate().
    props.bitrate = file_props.overall_bitrate()
        .filter(|&b| b > 0)
        .or_else(|| file_props.audio_bitrate())
        .filter(|&b| b > 0)
        .map(|b| b as i64);
    props.sample_rate = file_props.sample_rate().map(|s| s as i64);
    props.channels = file_props.channels().map(|c| c as i64);
    props.bit_depth = file_props.bit_depth().map(|b| b as i64);

    let mut tags: HashMap<String, String> = HashMap::new();

    if let Some(tag) = tagged_file.primary_tag() {
        props.has_embedded_art = !tag.pictures().is_empty();

        // Standard indexed fields (lofty 0.21 named ItemKey variants)
        macro_rules! read_field {
            ($tag:expr, $tags:expr, $key:expr, $item_key:expr) => {
                if let Some(val) = $tag.get_string(&$item_key) {
                    $tags.insert($key.to_string(), val.to_string());
                }
            };
        }

        read_field!(tag, tags, "title",         ItemKey::TrackTitle);
        read_field!(tag, tags, "artist",         ItemKey::TrackArtist);
        read_field!(tag, tags, "albumartist",    ItemKey::AlbumArtist);
        read_field!(tag, tags, "album",          ItemKey::AlbumTitle);
        read_field!(tag, tags, "tracknumber",    ItemKey::TrackNumber);
        read_field!(tag, tags, "discnumber",     ItemKey::DiscNumber);
        read_field!(tag, tags, "totaldiscs",     ItemKey::DiscTotal);
        read_field!(tag, tags, "totaltracks",    ItemKey::TrackTotal);
        read_field!(tag, tags, "date",           ItemKey::Year);
        read_field!(tag, tags, "genre",          ItemKey::Genre);
        read_field!(tag, tags, "composer",       ItemKey::Composer);
        read_field!(tag, tags, "label",          ItemKey::Label);
        read_field!(tag, tags, "catalognumber",  ItemKey::CatalogNumber);
        read_field!(tag, tags, "comment",        ItemKey::Comment);
        read_field!(tag, tags, "lyrics",         ItemKey::Lyrics);
        read_field!(tag, tags, "isrc",           ItemKey::Isrc);
        read_field!(tag, tags, "barcode",        ItemKey::Barcode);
        read_field!(tag, tags, "musicbrainz_trackid",        ItemKey::MusicBrainzRecordingId);
        read_field!(tag, tags, "musicbrainz_releasetrackid", ItemKey::MusicBrainzTrackId);
        read_field!(tag, tags, "musicbrainz_releaseid",      ItemKey::MusicBrainzReleaseId);
        read_field!(tag, tags, "musicbrainz_artistid",       ItemKey::MusicBrainzArtistId);
        read_field!(tag, tags, "musicbrainz_albumartistid",  ItemKey::MusicBrainzReleaseArtistId);
        read_field!(tag, tags, "musicbrainz_releasegroupid", ItemKey::MusicBrainzReleaseGroupId);
        read_field!(tag, tags, "replaygain_track_gain",     ItemKey::ReplayGainTrackGain);
        read_field!(tag, tags, "replaygain_track_peak",     ItemKey::ReplayGainTrackPeak);
        read_field!(tag, tags, "replaygain_album_gain",     ItemKey::ReplayGainAlbumGain);
        read_field!(tag, tags, "replaygain_album_peak",     ItemKey::ReplayGainAlbumPeak);

        // Capture any remaining items not yet collected (covers ASIN, AcoustID, MusicBrainz IDs, etc.)
        for item in tag.items() {
            let key = format!("{:?}", item.key()).to_lowercase();
            if !tags.contains_key(&key) {
                if let Some(val) = item.value().text() {
                    tags.insert(key, val.to_string());
                }
            }
        }
    }

    Ok((tags, props))
}

/// Write `tags` (MusicBrainz standard field names) to `path`.
/// Overwrites existing tags of the primary tag type.
pub fn write_tags(path: &Path, tags: &HashMap<String, String>) -> anyhow::Result<()> {
    let mut tagged_file = Probe::open(path)?.read()?;

    {
        let tag = tagged_file.primary_tag_mut().ok_or_else(|| {
            anyhow::anyhow!("no primary tag found in {:?}", path)
        })?;

        macro_rules! write_field {
            ($tag:expr, $tags:expr, $key:expr, $item_key:expr) => {
                if let Some(val) = $tags.get($key) {
                    $tag.insert_text($item_key, val.clone());
                }
            };
        }

        write_field!(tag, tags, "title",         ItemKey::TrackTitle);
        write_field!(tag, tags, "artist",        ItemKey::TrackArtist);
        write_field!(tag, tags, "albumartist",   ItemKey::AlbumArtist);
        write_field!(tag, tags, "album",         ItemKey::AlbumTitle);
        write_field!(tag, tags, "tracknumber",   ItemKey::TrackNumber);
        write_field!(tag, tags, "discnumber",    ItemKey::DiscNumber);
        write_field!(tag, tags, "totaldiscs",    ItemKey::DiscTotal);
        write_field!(tag, tags, "totaltracks",   ItemKey::TrackTotal);
        write_field!(tag, tags, "date",          ItemKey::Year);
        write_field!(tag, tags, "genre",         ItemKey::Genre);
        write_field!(tag, tags, "composer",      ItemKey::Composer);
        write_field!(tag, tags, "label",         ItemKey::Label);
        write_field!(tag, tags, "catalognumber", ItemKey::CatalogNumber);
        write_field!(tag, tags, "comment",       ItemKey::Comment);
        write_field!(tag, tags, "lyrics",        ItemKey::Lyrics);
        write_field!(tag, tags, "isrc",          ItemKey::Isrc);
        write_field!(tag, tags, "barcode",       ItemKey::Barcode);
        write_field!(tag, tags, "musicbrainz_trackid",        ItemKey::MusicBrainzRecordingId);
        write_field!(tag, tags, "musicbrainz_releasetrackid", ItemKey::MusicBrainzTrackId);
        write_field!(tag, tags, "musicbrainz_releaseid",      ItemKey::MusicBrainzReleaseId);
        write_field!(tag, tags, "musicbrainz_artistid",       ItemKey::MusicBrainzArtistId);
        write_field!(tag, tags, "musicbrainz_albumartistid",  ItemKey::MusicBrainzReleaseArtistId);
        write_field!(tag, tags, "musicbrainz_releasegroupid", ItemKey::MusicBrainzReleaseGroupId);
        write_field!(tag, tags, "replaygain_track_gain",      ItemKey::ReplayGainTrackGain);
        write_field!(tag, tags, "replaygain_track_peak",      ItemKey::ReplayGainTrackPeak);
        write_field!(tag, tags, "replaygain_album_gain",      ItemKey::ReplayGainAlbumGain);
        write_field!(tag, tags, "replaygain_album_peak",      ItemKey::ReplayGainAlbumPeak);
    }

    tagged_file.save_to_path(path, WriteOptions::default())?;
    Ok(())
}
