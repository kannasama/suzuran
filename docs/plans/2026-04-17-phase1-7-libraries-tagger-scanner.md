# Phase 1.7 — Libraries + Tagger + Scanner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement library CRUD, the `lofty`-based tag read/write abstraction, and the file scanner that walks a library root, hashes files, and upserts `tracks` rows.

**Architecture:** `Tagger` is a pure module — reads/writes tags via `lofty`, returns/accepts a `HashMap<String, String>` using MusicBrainz standard field names. The `Scanner` walks `root_path` using `walkdir`, computes SHA-256 hashes, diffs against the DB, and upserts tracks. Scanner is called by scan job handlers (Phase 1.8); here it is implemented and unit-tested in isolation.

**Tech Stack:** lofty 0.21, walkdir 2, sha2 0.10 (already in Cargo.toml), tokio::fs for async file ops.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `Cargo.toml` | Modify | Add lofty, walkdir |
| `src/models/mod.rs` | Modify | Add `Library`, `Track` structs |
| `src/dal/mod.rs` | Modify | Add library + track Store methods |
| `src/dal/postgres.rs` | Modify | Library + track queries |
| `src/dal/sqlite.rs` | Modify | Library + track queries |
| `src/tagger/mod.rs` | Create | `read_tags`, `write_tags`, `AudioProperties` |
| `src/scanner/mod.rs` | Create | `Scanner` — walks root, hashes, upserts tracks |
| `src/api/libraries.rs` | Create | Library CRUD handlers + routes |
| `src/api/mod.rs` | Modify | Mount libraries routes |
| `tests/tagger.rs` | Create | Tag read/write round-trip test |
| `tests/scanner.rs` | Create | Scanner detects new, changed, removed files |

---

## Task 1: Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add lofty and walkdir**

```toml
lofty = "0.21"
walkdir = "2"
hex = "0.4"
```

---

## Task 2: Models

**Files:**
- Modify: `src/models/mod.rs`

- [ ] **Step 1: Append `Library` and `Track`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    pub format: String,
    pub encoding_profile_id: Option<i64>,
    pub parent_library_id: Option<i64>,
    pub scan_enabled: bool,
    pub scan_interval_secs: i64,
    pub auto_transcode_on_ingest: bool,
    pub auto_organize_on_ingest: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Track {
    pub id: i64,
    pub library_id: i64,
    pub relative_path: String,
    pub file_hash: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub albumartist: Option<String>,
    pub album: Option<String>,
    pub tracknumber: Option<String>,
    pub discnumber: Option<String>,
    pub totaldiscs: Option<String>,
    pub totaltracks: Option<String>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub label: Option<String>,
    pub catalognumber: Option<String>,
    pub tags: serde_json::Value,
    pub duration_secs: Option<f64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub has_embedded_art: bool,
    pub acoustid_fingerprint: Option<String>,
    pub last_scanned_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
```

---

## Task 3: Store trait — libraries + tracks

**Files:**
- Modify: `src/dal/mod.rs`

- [ ] **Step 1: Add library and track methods**

```rust
// ── libraries ────────────────────────────────────────────────
async fn list_libraries(&self) -> Result<Vec<Library>, AppError>;
async fn get_library(&self, id: i64) -> Result<Option<Library>, AppError>;
async fn create_library(
    &self,
    name: &str,
    root_path: &str,
    format: &str,
    parent_library_id: Option<i64>,
) -> Result<Library, AppError>;
async fn update_library(
    &self,
    id: i64,
    name: &str,
    scan_enabled: bool,
    scan_interval_secs: i64,
    auto_transcode_on_ingest: bool,
    auto_organize_on_ingest: bool,
) -> Result<Option<Library>, AppError>;
async fn delete_library(&self, id: i64) -> Result<(), AppError>;

// ── tracks ────────────────────────────────────────────────────
async fn list_tracks_by_library(&self, library_id: i64) -> Result<Vec<Track>, AppError>;
async fn get_track(&self, id: i64) -> Result<Option<Track>, AppError>;
async fn find_track_by_path(
    &self,
    library_id: i64,
    relative_path: &str,
) -> Result<Option<Track>, AppError>;
async fn upsert_track(&self, track: UpsertTrack) -> Result<Track, AppError>;
async fn mark_track_removed(&self, id: i64) -> Result<(), AppError>;
async fn list_track_paths_by_library(
    &self,
    library_id: i64,
) -> Result<Vec<(i64, String, String)>, AppError>; // (id, relative_path, file_hash)
```

Add this struct to `src/dal/mod.rs` (not in `models` — it's a write-specific DTO):

```rust
use serde_json::Value as JsonValue;

pub struct UpsertTrack {
    pub library_id: i64,
    pub relative_path: String,
    pub file_hash: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub albumartist: Option<String>,
    pub album: Option<String>,
    pub tracknumber: Option<String>,
    pub discnumber: Option<String>,
    pub totaldiscs: Option<String>,
    pub totaltracks: Option<String>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub label: Option<String>,
    pub catalognumber: Option<String>,
    pub tags: JsonValue,
    pub duration_secs: Option<f64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub has_embedded_art: bool,
}
```

Also add `use crate::models::{..., Library, Track};` to the imports.

---

## Task 4: Postgres — library + track implementations

**Files:**
- Modify: `src/dal/postgres.rs`

- [ ] **Step 1: Append library implementations**

```rust
async fn list_libraries(&self) -> Result<Vec<Library>, AppError> {
    sqlx::query_as::<_, Library>("SELECT * FROM libraries ORDER BY name")
        .fetch_all(&self.pool).await.map_err(AppError::Database)
}

async fn get_library(&self, id: i64) -> Result<Option<Library>, AppError> {
    sqlx::query_as::<_, Library>("SELECT * FROM libraries WHERE id = $1")
        .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn create_library(
    &self, name: &str, root_path: &str, format: &str, parent_library_id: Option<i64>,
) -> Result<Library, AppError> {
    sqlx::query_as::<_, Library>(
        "INSERT INTO libraries (name, root_path, format, parent_library_id)
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(name).bind(root_path).bind(format).bind(parent_library_id)
    .fetch_one(&self.pool).await.map_err(AppError::Database)
}

async fn update_library(
    &self, id: i64, name: &str, scan_enabled: bool, scan_interval_secs: i64,
    auto_transcode_on_ingest: bool, auto_organize_on_ingest: bool,
) -> Result<Option<Library>, AppError> {
    sqlx::query_as::<_, Library>(
        "UPDATE libraries SET name=$1, scan_enabled=$2, scan_interval_secs=$3,
         auto_transcode_on_ingest=$4, auto_organize_on_ingest=$5
         WHERE id=$6 RETURNING *",
    )
    .bind(name).bind(scan_enabled).bind(scan_interval_secs)
    .bind(auto_transcode_on_ingest).bind(auto_organize_on_ingest).bind(id)
    .fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn delete_library(&self, id: i64) -> Result<(), AppError> {
    sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(id).execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}
```

- [ ] **Step 2: Append track implementations**

```rust
async fn list_tracks_by_library(&self, library_id: i64) -> Result<Vec<Track>, AppError> {
    sqlx::query_as::<_, Track>("SELECT * FROM tracks WHERE library_id = $1 ORDER BY albumartist, album, discnumber, tracknumber")
        .bind(library_id).fetch_all(&self.pool).await.map_err(AppError::Database)
}

async fn get_track(&self, id: i64) -> Result<Option<Track>, AppError> {
    sqlx::query_as::<_, Track>("SELECT * FROM tracks WHERE id = $1")
        .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn find_track_by_path(&self, library_id: i64, relative_path: &str) -> Result<Option<Track>, AppError> {
    sqlx::query_as::<_, Track>(
        "SELECT * FROM tracks WHERE library_id = $1 AND relative_path = $2",
    )
    .bind(library_id).bind(relative_path)
    .fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn upsert_track(&self, t: UpsertTrack) -> Result<Track, AppError> {
    sqlx::query_as::<_, Track>(
        "INSERT INTO tracks (library_id, relative_path, file_hash, title, artist, albumartist,
         album, tracknumber, discnumber, totaldiscs, totaltracks, date, genre, composer,
         label, catalognumber, tags, duration_secs, bitrate, sample_rate, channels, has_embedded_art,
         last_scanned_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,NOW())
         ON CONFLICT (library_id, relative_path) DO UPDATE SET
           file_hash=$3, title=$4, artist=$5, albumartist=$6, album=$7, tracknumber=$8,
           discnumber=$9, totaldiscs=$10, totaltracks=$11, date=$12, genre=$13, composer=$14,
           label=$15, catalognumber=$16, tags=$17, duration_secs=$18, bitrate=$19,
           sample_rate=$20, channels=$21, has_embedded_art=$22, last_scanned_at=NOW()
         RETURNING *",
    )
    .bind(t.library_id).bind(&t.relative_path).bind(&t.file_hash)
    .bind(&t.title).bind(&t.artist).bind(&t.albumartist).bind(&t.album)
    .bind(&t.tracknumber).bind(&t.discnumber).bind(&t.totaldiscs).bind(&t.totaltracks)
    .bind(&t.date).bind(&t.genre).bind(&t.composer).bind(&t.label).bind(&t.catalognumber)
    .bind(&t.tags).bind(t.duration_secs).bind(t.bitrate).bind(t.sample_rate)
    .bind(t.channels).bind(t.has_embedded_art)
    .fetch_one(&self.pool).await.map_err(AppError::Database)
}

async fn mark_track_removed(&self, id: i64) -> Result<(), AppError> {
    sqlx::query("DELETE FROM tracks WHERE id = $1")
        .bind(id).execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}

async fn list_track_paths_by_library(&self, library_id: i64) -> Result<Vec<(i64, String, String)>, AppError> {
    sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, relative_path, file_hash FROM tracks WHERE library_id = $1",
    )
    .bind(library_id).fetch_all(&self.pool).await.map_err(AppError::Database)
}
```

---

## Task 5: SQLite — library + track implementations

**Files:**
- Modify: `src/dal/sqlite.rs`

- [ ] **Step 1: Append library implementations** (identical logic, `?N` placeholders)

```rust
async fn list_libraries(&self) -> Result<Vec<Library>, AppError> {
    sqlx::query_as::<_, Library>("SELECT * FROM libraries ORDER BY name")
        .fetch_all(&self.pool).await.map_err(AppError::Database)
}

async fn get_library(&self, id: i64) -> Result<Option<Library>, AppError> {
    sqlx::query_as::<_, Library>("SELECT * FROM libraries WHERE id = ?1")
        .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn create_library(
    &self, name: &str, root_path: &str, format: &str, parent_library_id: Option<i64>,
) -> Result<Library, AppError> {
    sqlx::query_as::<_, Library>(
        "INSERT INTO libraries (name, root_path, format, parent_library_id)
         VALUES (?1, ?2, ?3, ?4) RETURNING *",
    )
    .bind(name).bind(root_path).bind(format).bind(parent_library_id)
    .fetch_one(&self.pool).await.map_err(AppError::Database)
}

async fn update_library(
    &self, id: i64, name: &str, scan_enabled: bool, scan_interval_secs: i64,
    auto_transcode_on_ingest: bool, auto_organize_on_ingest: bool,
) -> Result<Option<Library>, AppError> {
    sqlx::query_as::<_, Library>(
        "UPDATE libraries SET name=?1, scan_enabled=?2, scan_interval_secs=?3,
         auto_transcode_on_ingest=?4, auto_organize_on_ingest=?5
         WHERE id=?6 RETURNING *",
    )
    .bind(name).bind(scan_enabled).bind(scan_interval_secs)
    .bind(auto_transcode_on_ingest).bind(auto_organize_on_ingest).bind(id)
    .fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn delete_library(&self, id: i64) -> Result<(), AppError> {
    sqlx::query("DELETE FROM libraries WHERE id = ?1")
        .bind(id).execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}
```

- [ ] **Step 2: Append track implementations**

```rust
async fn list_tracks_by_library(&self, library_id: i64) -> Result<Vec<Track>, AppError> {
    sqlx::query_as::<_, Track>(
        "SELECT * FROM tracks WHERE library_id = ?1 ORDER BY albumartist, album, discnumber, tracknumber",
    )
    .bind(library_id).fetch_all(&self.pool).await.map_err(AppError::Database)
}

async fn get_track(&self, id: i64) -> Result<Option<Track>, AppError> {
    sqlx::query_as::<_, Track>("SELECT * FROM tracks WHERE id = ?1")
        .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn find_track_by_path(&self, library_id: i64, relative_path: &str) -> Result<Option<Track>, AppError> {
    sqlx::query_as::<_, Track>(
        "SELECT * FROM tracks WHERE library_id = ?1 AND relative_path = ?2",
    )
    .bind(library_id).bind(relative_path)
    .fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn upsert_track(&self, t: UpsertTrack) -> Result<Track, AppError> {
    sqlx::query_as::<_, Track>(
        "INSERT INTO tracks (library_id, relative_path, file_hash, title, artist, albumartist,
         album, tracknumber, discnumber, totaldiscs, totaltracks, date, genre, composer,
         label, catalognumber, tags, duration_secs, bitrate, sample_rate, channels, has_embedded_art,
         last_scanned_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,datetime('now'))
         ON CONFLICT (library_id, relative_path) DO UPDATE SET
           file_hash=?3, title=?4, artist=?5, albumartist=?6, album=?7, tracknumber=?8,
           discnumber=?9, totaldiscs=?10, totaltracks=?11, date=?12, genre=?13, composer=?14,
           label=?15, catalognumber=?16, tags=?17, duration_secs=?18, bitrate=?19,
           sample_rate=?20, channels=?21, has_embedded_art=?22, last_scanned_at=datetime('now')
         RETURNING *",
    )
    .bind(t.library_id).bind(&t.relative_path).bind(&t.file_hash)
    .bind(&t.title).bind(&t.artist).bind(&t.albumartist).bind(&t.album)
    .bind(&t.tracknumber).bind(&t.discnumber).bind(&t.totaldiscs).bind(&t.totaltracks)
    .bind(&t.date).bind(&t.genre).bind(&t.composer).bind(&t.label).bind(&t.catalognumber)
    .bind(&t.tags).bind(t.duration_secs).bind(t.bitrate).bind(t.sample_rate)
    .bind(t.channels).bind(t.has_embedded_art)
    .fetch_one(&self.pool).await.map_err(AppError::Database)
}

async fn mark_track_removed(&self, id: i64) -> Result<(), AppError> {
    sqlx::query("DELETE FROM tracks WHERE id = ?1")
        .bind(id).execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}

async fn list_track_paths_by_library(&self, library_id: i64) -> Result<Vec<(i64, String, String)>, AppError> {
    sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, relative_path, file_hash FROM tracks WHERE library_id = ?1",
    )
    .bind(library_id).fetch_all(&self.pool).await.map_err(AppError::Database)
}
```

- [ ] **Step 3: Compile check**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add src/ Cargo.toml
git commit -m "feat: Library + Track models, Store methods, Postgres + SQLite impls"
```

---

## Task 6: Tagger

**Files:**
- Create: `src/tagger/mod.rs`

- [ ] **Step 1: Write `src/tagger/mod.rs`**

```rust
use std::collections::HashMap;
use std::path::Path;

use lofty::{
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::{Accessor, ItemKey, Tag, TagType},
};

/// Audio properties read from the file.
#[derive(Debug, Default)]
pub struct AudioProperties {
    pub duration_secs: Option<f64>,
    pub bitrate: Option<i64>,      // kbps
    pub sample_rate: Option<i64>,  // Hz
    pub channels: Option<i64>,
    pub has_embedded_art: bool,
}

/// Read all tags from `path`. Returns (tags, properties).
/// `tags` keys use MusicBrainz/Picard standard field names (lowercase).
pub fn read_tags(path: &Path) -> anyhow::Result<(HashMap<String, String>, AudioProperties)> {
    let tagged_file = Probe::open(path)?.read()?;

    let mut props = AudioProperties::default();

    let file_props = tagged_file.properties();
    props.duration_secs = Some(file_props.duration().as_secs_f64());
    props.bitrate = file_props.overall_bitrate().map(|b| b as i64);
    props.sample_rate = file_props.sample_rate().map(|s| s as i64);
    props.channels = file_props.channels().map(|c| c as i64);

    let mut tags: HashMap<String, String> = HashMap::new();

    if let Some(tag) = tagged_file.primary_tag() {
        props.has_embedded_art = tag.pictures().next().is_some();

        // Standard indexed fields
        let field_map: &[(&str, ItemKey)] = &[
            ("title",           ItemKey::TrackTitle),
            ("artist",          ItemKey::TrackArtist),
            ("albumartist",     ItemKey::AlbumArtist),
            ("album",           ItemKey::AlbumTitle),
            ("tracknumber",     ItemKey::TrackNumber),
            ("discnumber",      ItemKey::DiscNumber),
            ("totaldiscs",      ItemKey::DiscTotal),
            ("totaltracks",     ItemKey::TrackTotal),
            ("date",            ItemKey::Year),
            ("genre",           ItemKey::Genre),
            ("composer",        ItemKey::Composer),
            ("label",           ItemKey::Label),
            ("catalognumber",   ItemKey::CatalogNumber),
            ("comment",         ItemKey::Comment),
            ("lyrics",          ItemKey::Lyrics),
            ("isrc",            ItemKey::Isrc),
            ("barcode",         ItemKey::Barcode),
            ("asin",            ItemKey::Asin),
            ("musicbrainz_trackid",     ItemKey::MusicBrainzTrackId),
            ("musicbrainz_albumid",     ItemKey::MusicBrainzAlbumId),
            ("musicbrainz_artistid",    ItemKey::MusicBrainzArtistId),
            ("musicbrainz_albumartistid", ItemKey::MusicBrainzAlbumArtistId),
            ("musicbrainz_releasegroupid", ItemKey::MusicBrainzReleaseGroupId),
            ("acoustid_id",     ItemKey::AcoustidId),
            ("acoustid_fingerprint", ItemKey::AcoustidFingerprint),
            ("replaygain_track_gain", ItemKey::ReplayGainTrackGain),
            ("replaygain_track_peak", ItemKey::ReplayGainTrackPeak),
            ("replaygain_album_gain", ItemKey::ReplayGainAlbumGain),
            ("replaygain_album_peak", ItemKey::ReplayGainAlbumPeak),
        ];

        for (key, item_key) in field_map {
            if let Some(val) = tag.get_string(item_key) {
                tags.insert(key.to_string(), val.to_string());
            }
        }

        // Capture any remaining unknown items as-is
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

    let tag = tagged_file.primary_tag_mut().ok_or_else(|| {
        anyhow::anyhow!("no primary tag found in {:?}", path)
    })?;

    let field_map: &[(&str, ItemKey)] = &[
        ("title",           ItemKey::TrackTitle),
        ("artist",          ItemKey::TrackArtist),
        ("albumartist",     ItemKey::AlbumArtist),
        ("album",           ItemKey::AlbumTitle),
        ("tracknumber",     ItemKey::TrackNumber),
        ("discnumber",      ItemKey::DiscNumber),
        ("totaldiscs",      ItemKey::DiscTotal),
        ("totaltracks",     ItemKey::TrackTotal),
        ("date",            ItemKey::Year),
        ("genre",           ItemKey::Genre),
        ("composer",        ItemKey::Composer),
        ("label",           ItemKey::Label),
        ("catalognumber",   ItemKey::CatalogNumber),
        ("comment",         ItemKey::Comment),
        ("lyrics",          ItemKey::Lyrics),
        ("isrc",            ItemKey::Isrc),
        ("barcode",         ItemKey::Barcode),
        ("asin",            ItemKey::Asin),
        ("musicbrainz_trackid",     ItemKey::MusicBrainzTrackId),
        ("musicbrainz_albumid",     ItemKey::MusicBrainzAlbumId),
        ("musicbrainz_artistid",    ItemKey::MusicBrainzArtistId),
        ("musicbrainz_albumartistid", ItemKey::MusicBrainzAlbumArtistId),
        ("musicbrainz_releasegroupid", ItemKey::MusicBrainzReleaseGroupId),
        ("acoustid_id",     ItemKey::AcoustidId),
        ("acoustid_fingerprint", ItemKey::AcoustidFingerprint),
        ("replaygain_track_gain", ItemKey::ReplayGainTrackGain),
        ("replaygain_track_peak", ItemKey::ReplayGainTrackPeak),
        ("replaygain_album_gain", ItemKey::ReplayGainAlbumGain),
        ("replaygain_album_peak", ItemKey::ReplayGainAlbumPeak),
    ];

    for (key, item_key) in field_map {
        if let Some(val) = tags.get(*key) {
            tag.insert_text(item_key.clone(), val.clone());
        }
    }

    lofty::write_tag(tag, path, lofty::WriteOptions::default())?;
    Ok(())
}
```

- [ ] **Step 2: Add `pub mod tagger;` to `src/lib.rs`**

---

## Task 7: Scanner

**Files:**
- Create: `src/scanner/mod.rs`

- [ ] **Step 1: Write `src/scanner/mod.rs`**

```rust
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::{
    dal::{Store, UpsertTrack},
    error::AppError,
    tagger,
};

const AUDIO_EXTENSIONS: &[&str] = &["flac", "m4a", "mp3", "opus", "ogg", "aac", "wav", "aiff"];

pub struct ScanResult {
    pub inserted: usize,
    pub updated: usize,
    pub removed: usize,
    pub errors: Vec<String>,
}

pub async fn scan_library(
    db: &Arc<dyn Store>,
    library_id: i64,
    root_path: &Path,
) -> Result<ScanResult, AppError> {
    let mut result = ScanResult { inserted: 0, updated: 0, removed: 0, errors: vec![] };

    // Build map of existing tracks: relative_path → (id, file_hash)
    let existing: HashMap<String, (i64, String)> = db
        .list_track_paths_by_library(library_id)
        .await?
        .into_iter()
        .map(|(id, path, hash)| (path, (id, hash)))
        .collect();

    let mut seen_paths: HashSet<String> = HashSet::new();

    for entry in WalkDir::new(root_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let abs_path = entry.path().to_path_buf();
        let ext = abs_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        let rel_path = match abs_path.strip_prefix(root_path) {
            Ok(p) => p.to_string_lossy().into_owned(),
            Err(_) => continue,
        };

        seen_paths.insert(rel_path.clone());

        let hash = match hash_file(&abs_path).await {
            Ok(h) => h,
            Err(e) => {
                result.errors.push(format!("{rel_path}: hash error: {e}"));
                continue;
            }
        };

        // Check if we need to scan this file
        let needs_scan = match existing.get(&rel_path) {
            Some((_, existing_hash)) => existing_hash != &hash,
            None => true,
        };

        let is_new = !existing.contains_key(&rel_path);

        if !needs_scan {
            continue;
        }

        // Read tags — run on blocking thread pool to avoid blocking async executor
        let abs_path_clone = abs_path.clone();
        let tag_result = tokio::task::spawn_blocking(move || {
            tagger::read_tags(&abs_path_clone)
        })
        .await;

        let (tags_map, audio_props) = match tag_result {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                result.errors.push(format!("{rel_path}: tag read error: {e}"));
                // Still upsert with empty tags so the file is tracked
                (HashMap::new(), tagger::AudioProperties::default())
            }
            Err(e) => {
                result.errors.push(format!("{rel_path}: spawn error: {e}"));
                continue;
            }
        };

        let tags_json = serde_json::to_value(&tags_map).unwrap_or(serde_json::json!({}));

        let upsert = UpsertTrack {
            library_id,
            relative_path: rel_path.clone(),
            file_hash: hash,
            title: tags_map.get("title").cloned(),
            artist: tags_map.get("artist").cloned(),
            albumartist: tags_map.get("albumartist").cloned(),
            album: tags_map.get("album").cloned(),
            tracknumber: tags_map.get("tracknumber").cloned(),
            discnumber: tags_map.get("discnumber").cloned(),
            totaldiscs: tags_map.get("totaldiscs").cloned(),
            totaltracks: tags_map.get("totaltracks").cloned(),
            date: tags_map.get("date").cloned(),
            genre: tags_map.get("genre").cloned(),
            composer: tags_map.get("composer").cloned(),
            label: tags_map.get("label").cloned(),
            catalognumber: tags_map.get("catalognumber").cloned(),
            tags: tags_json,
            duration_secs: audio_props.duration_secs,
            bitrate: audio_props.bitrate,
            sample_rate: audio_props.sample_rate,
            channels: audio_props.channels,
            has_embedded_art: audio_props.has_embedded_art,
        };

        db.upsert_track(upsert).await?;

        if is_new {
            result.inserted += 1;
        } else {
            result.updated += 1;
        }
    }

    // Remove tracks for files no longer on disk
    for (rel_path, (id, _)) in &existing {
        if !seen_paths.contains(rel_path) {
            db.mark_track_removed(*id).await?;
            result.removed += 1;
        }
    }

    Ok(result)
}

async fn hash_file(path: &PathBuf) -> anyhow::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
```

- [ ] **Step 2: Add `pub mod scanner;` to `src/lib.rs`**

- [ ] **Step 3: Compile check**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add src/ Cargo.toml
git commit -m "feat: Tagger (lofty tag read/write) and Scanner (walkdir + SHA-256 upsert)"
```

---

## Task 8: Libraries API handlers

**Files:**
- Create: `src/api/libraries.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Write `src/api/libraries.rs`**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    models::{Library, Track},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_libraries).post(create_library))
        .route("/:id", get(get_library).put(update_library).delete(delete_library))
        .route("/:id/tracks", get(list_tracks))
}

async fn list_libraries(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Library>>, AppError> {
    Ok(Json(state.db.list_libraries().await?))
}

async fn get_library(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Library>, AppError> {
    state.db.get_library(id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))
        .map(Json)
}

#[derive(Deserialize)]
struct CreateLibraryRequest {
    name: String,
    root_path: String,
    format: String,
    parent_library_id: Option<i64>,
}

async fn create_library(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<Library>), AppError> {
    let lib = state.db
        .create_library(&body.name, &body.root_path, &body.format, body.parent_library_id)
        .await?;
    Ok((StatusCode::CREATED, Json(lib)))
}

#[derive(Deserialize)]
struct UpdateLibraryRequest {
    name: String,
    scan_enabled: bool,
    scan_interval_secs: i64,
    auto_transcode_on_ingest: bool,
    auto_organize_on_ingest: bool,
}

async fn update_library(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateLibraryRequest>,
) -> Result<Json<Library>, AppError> {
    state.db
        .update_library(id, &body.name, body.scan_enabled, body.scan_interval_secs,
            body.auto_transcode_on_ingest, body.auto_organize_on_ingest)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("library {id} not found")))
        .map(Json)
}

async fn delete_library(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_library(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_tracks(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Track>>, AppError> {
    Ok(Json(state.db.list_tracks_by_library(id).await?))
}
```

- [ ] **Step 2: Update `src/api/mod.rs` to add libraries**

```rust
pub mod auth;
pub mod libraries;
pub mod middleware;
pub mod settings;
pub mod themes;
pub mod totp;
pub mod webauthn;

use axum::Router;
use crate::state::AppState;

pub fn api_router(state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/totp", totp::router())
        .nest("/webauthn", webauthn::router())
        .nest("/settings", settings::router())
        .nest("/themes", themes::router())
        .nest("/libraries", libraries::router())
}
```

- [ ] **Step 3: Commit**

```bash
git add src/
git commit -m "feat: Libraries CRUD API mounted at /api/v1/libraries"
```

---

## Task 9: Integration tests

**Files:**
- Create: `tests/scanner.rs`

- [ ] **Step 1: Write `tests/scanner.rs`**

```rust
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use suzuran_server::{
    dal::{sqlite::SqliteStore, Store},
    scanner::scan_library,
};

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

/// Create a minimal valid FLAC file (just the FLAC stream marker — enough for lofty to open).
/// In practice, tests should use a real minimal FLAC fixture. Here we create a temp dir
/// with a renamed empty file — the scanner will record a tag read error but still upsert.
async fn make_temp_library() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    // Create two fake .flac files (empty — lofty will error but scanner tracks them)
    fs::write(path.join("track01.flac"), b"").await.unwrap();
    fs::write(path.join("track02.flac"), b"").await.unwrap();
    (dir, path)
}

#[tokio::test]
async fn scanner_inserts_new_files() {
    let db = make_db().await;
    let (dir, root) = make_temp_library().await;

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac", None).await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 2, "should insert 2 files");
    assert_eq!(result.removed, 0);

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 2);
    drop(dir);
}

#[tokio::test]
async fn scanner_removes_deleted_files() {
    let db = make_db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    fs::write(root.join("track01.flac"), b"").await.unwrap();
    fs::write(root.join("track02.flac"), b"").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac", None).await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    // Remove one file
    fs::remove_file(root.join("track02.flac")).await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.removed, 1);

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1);
    drop(dir);
}

#[tokio::test]
async fn scanner_skips_unchanged_files() {
    let db = make_db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    fs::write(root.join("track01.flac"), b"data").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac", None).await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    // Second scan — file unchanged
    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 0);
    assert_eq!(result.updated, 0);
    drop(dir);
}
```

- [ ] **Step 2: Add `tempfile` to dev-dependencies**

```toml
[dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
tempfile = "3"
```

- [ ] **Step 3: Run tests**

```bash
cargo test --test scanner -- --nocapture
```

Expected: all 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/scanner.rs Cargo.toml tasks/codebase-filemap.md
git commit -m "test: scanner inserts/removes/skips unchanged; update filemap"
```
