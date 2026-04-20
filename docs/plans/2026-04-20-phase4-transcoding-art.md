# Phase 4 — Transcoding, Album Art, CUE Splitting & Extended Ingest Formats

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add ffmpeg-based transcode pipeline, album art standardization, CUE+FLAC sheet splitting, and extended lossless ingest format support (WavPack, APE, TrueAudio).

**Architecture:** Three new job handlers (`transcode`, `art_process`, `cue_split`) plus two new config tables (`encoding_profiles`, `art_profiles`) and a `track_links` relationship table. Transcode jobs build ffmpeg commands from encoding profiles and write derived `track_links` records. Art-process jobs use `lofty` for embed/extract and the `image` crate for resize/recompress. CUE+FLAC pairs are detected during scan, deferred from normal ingestion, and split into individual tracks via ffmpeg. Extended formats (WavPack `.wv`, APE `.ape`, TrueAudio `.tta`) are already tag-readable by lofty's default features — only the scanner's extension list needs updating.

**Tech Stack:** Rust/Axum + `lofty 0.21` (existing) + `ffmpeg` subprocess + `image = "0.25"` (new) + React/TanStack Query (existing).

**Branch:**
```bash
git checkout main && git checkout -b 0.4
```

---

## Phase 4 Notes

### CUE+FLAC splitting scope

A `.cue` file references one audio file (`FILE` directive) and defines track boundaries as `INDEX 01 MM:SS:FF` timestamps. The scanner detects these pairs and skips the whole-file audio from normal ingestion, enqueueing a `cue_split` job instead. The handler splits with `ffmpeg -c copy` (no re-encode), writes CUE metadata to each output file via lofty, upserts individual tracks into DB, and enqueues fingerprint per track. Output files are placed alongside the CUE file (`{NN} - {title}.flac`); the organization engine handles final placement on accept.

The split is idempotent: if output files already exist on disk, the job skips re-splitting but still ensures tracks are in DB.

### Extended ingest formats

`lofty 0.21` default features include `wavpack`, `ape`, and `riff` (WAV/AIFF). WavPack (`.wv`), Monkey's Audio (`.ape`), and TrueAudio (`.tta`) tag reading/writing work via `Probe::open` without any Cargo.toml changes. Musepack (`.mpc`) has limited lofty support and is excluded from this phase.

### Meaningful output checkpoints

| After task | What you can see |
|-----------|-----------------|
| Task 1 | WavPack/APE files ingested by scanner and visible in library |
| Task 4 | Track links visible in DB after manual SQL query |
| Task 6 | CUE+FLAC pairs split into individual tracks on next scan |
| Task 7 | Transcode jobs complete and derived tracks appear in library |
| Task 9 | Encoding/art profiles creatable via API (curl or Insomnia) |
| Task 11 | Encoding/art profiles manageable in Settings UI |
| Task 12 | Transcode + art actions wired into Library view |

---

## Task 1: Extended ingest formats

**Files:**
- Modify: `src/scanner/mod.rs` — expand `AUDIO_EXTENSIONS`
- Create: `tests/scanner_extended_formats.rs`

**Step 1: Write the failing test**

```rust
// tests/scanner_extended_formats.rs
mod common;

#[tokio::test]
async fn test_wavpack_file_ingested() {
    // common::setup_library_with_file copies a test fixture into a temp dir
    // and returns (store, library_id, root_path)
    let (store, library_id, root) = common::setup_library_with_file("fixtures/silence.wv").await;
    suzuran_server::scanner::scan_library(&store, library_id, &root).await.unwrap();
    let tracks = store.list_tracks_by_library(library_id).await.unwrap();
    assert_eq!(tracks.len(), 1, "WavPack file should be ingested");
}

#[tokio::test]
async fn test_ape_file_ingested() {
    let (store, library_id, root) = common::setup_library_with_file("fixtures/silence.ape").await;
    suzuran_server::scanner::scan_library(&store, library_id, &root).await.unwrap();
    let tracks = store.list_tracks_by_library(library_id).await.unwrap();
    assert_eq!(tracks.len(), 1, "APE file should be ingested");
}
```

Add small silence fixtures (`tests/fixtures/silence.wv`, `tests/fixtures/silence.ape`, `tests/fixtures/silence.tta`) — generate with ffmpeg during build setup or commit pre-built 1-second silent files. Check `tests/fixtures/` for existing audio fixtures and follow the same pattern.

**Step 2: Verify fail**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```
Expected: test compile error or test failure (extensions not in list).

**Step 3: Update `src/scanner/mod.rs`**

```rust
const AUDIO_EXTENSIONS: &[&str] = &[
    "flac", "m4a", "mp3", "opus", "ogg", "aac", "wav", "aiff",
    "wv",   // WavPack (lossless)
    "ape",  // Monkey's Audio (lossless)
    "tta",  // TrueAudio (lossless)
];
```

No other scanner changes needed — `tagger::read_tags` calls `lofty::probe::Probe::open(path)` which auto-detects format.

**Step 4: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 5: Update codebase filemap** — note extended AUDIO_EXTENSIONS; add test file entry.

**Step 6: Commit**
```bash
git add src/scanner/mod.rs tests/scanner_extended_formats.rs tests/fixtures/ tasks/codebase-filemap.md
git commit -m "feat(4.1): extended ingest formats — WavPack, APE, TrueAudio"
```

---

## Task 2: Encoding profiles — DB + DAL

**Files:**
- Create: `migrations/postgres/0011_encoding_profiles.sql`
- Create: `migrations/sqlite/0011_encoding_profiles.sql`
- Modify: `src/models/mod.rs` — add `EncodingProfile`, `UpsertEncodingProfile`
- Modify: `src/dal/mod.rs` — add 5 Store trait methods
- Modify: `src/dal/postgres.rs`, `src/dal/sqlite.rs`
- Create: `tests/encoding_profiles_dal.rs`

**Step 1: Write the failing test**

```rust
// tests/encoding_profiles_dal.rs
mod common;
use suzuran_server::dal::UpsertEncodingProfile;

#[tokio::test]
async fn test_encoding_profile_crud() {
    let store = common::setup_store().await;

    let ep = store.create_encoding_profile(UpsertEncodingProfile {
        name: "AAC 256k".into(),
        codec: "aac".into(),
        bitrate: Some("256k".into()),
        sample_rate: Some(44100),
        channels: Some(2),
        advanced_args: None,
    }).await.unwrap();

    assert_eq!(ep.codec, "aac");

    let all = store.list_encoding_profiles().await.unwrap();
    assert_eq!(all.len(), 1);

    let fetched = store.get_encoding_profile(ep.id).await.unwrap();
    assert_eq!(fetched.name, "AAC 256k");

    let updated = store.update_encoding_profile(ep.id, UpsertEncodingProfile {
        name: "AAC 320k".into(),
        codec: "aac".into(),
        bitrate: Some("320k".into()),
        sample_rate: Some(44100),
        channels: Some(2),
        advanced_args: None,
    }).await.unwrap();
    assert_eq!(updated.bitrate.as_deref(), Some("320k"));

    store.delete_encoding_profile(ep.id).await.unwrap();
    assert!(store.list_encoding_profiles().await.unwrap().is_empty());
}
```

**Step 2: Verify fail**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 3: Write migrations**

`migrations/postgres/0011_encoding_profiles.sql`:
```sql
CREATE TABLE encoding_profiles (
    id            BIGSERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    codec         TEXT NOT NULL,
    bitrate       TEXT,
    sample_rate   INTEGER,
    channels      INTEGER,
    advanced_args TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

`migrations/sqlite/0011_encoding_profiles.sql`: same with `INTEGER PRIMARY KEY AUTOINCREMENT` and `TEXT NOT NULL DEFAULT (strftime(...))` for `created_at`.

**Step 4: Add model to `src/models/mod.rs`**

```rust
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct EncodingProfile {
    pub id: i64,
    pub name: String,
    pub codec: String,             // "aac", "mp3", "opus", "flac", …
    pub bitrate: Option<String>,   // "256k" — None for lossless codecs
    pub sample_rate: Option<i64>,  // None = preserve source
    pub channels: Option<i64>,     // None = preserve source
    pub advanced_args: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct UpsertEncodingProfile {
    pub name: String,
    pub codec: String,
    pub bitrate: Option<String>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub advanced_args: Option<String>,
}
```

**Step 5: Add to `Store` trait in `src/dal/mod.rs`**

```rust
async fn create_encoding_profile(&self, dto: UpsertEncodingProfile) -> Result<EncodingProfile, AppError>;
async fn get_encoding_profile(&self, id: i64) -> Result<EncodingProfile, AppError>;
async fn list_encoding_profiles(&self) -> Result<Vec<EncodingProfile>, AppError>;
async fn update_encoding_profile(&self, id: i64, dto: UpsertEncodingProfile) -> Result<EncodingProfile, AppError>;
async fn delete_encoding_profile(&self, id: i64) -> Result<(), AppError>;
```

**Step 6: Implement in `src/dal/postgres.rs`**

Standard `sqlx::query_as!` patterns matching the `tag_suggestions` implementations from Phase 3. Use `RETURNING *` for create/update, `SELECT *` for get/list, `DELETE` for delete.

**Step 7: Implement in `src/dal/sqlite.rs`**

Follow PgStore pattern with `?` placeholders and `TEXT` for `created_at`. For update, use `RETURNING *` (SQLite 3.35+) or `INSERT OR REPLACE` — check how existing updates are done in the sqlite store and match that pattern.

**Step 8: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 9: Update codebase filemap** — add migration entries, model entry, test entry.

**Step 10: Commit**
```bash
git add migrations/ src/models/mod.rs src/dal/ tests/encoding_profiles_dal.rs tasks/codebase-filemap.md
git commit -m "feat(4.2): encoding_profiles migration, model, DAL"
```

---

## Task 3: Art profiles — DB + DAL

**Files:**
- Create: `migrations/postgres/0012_art_profiles.sql`
- Create: `migrations/sqlite/0012_art_profiles.sql`
- Modify: `src/models/mod.rs` — add `ArtProfile`, `UpsertArtProfile`
- Modify: `src/dal/mod.rs`, `src/dal/postgres.rs`, `src/dal/sqlite.rs`
- Create: `tests/art_profiles_dal.rs`

**Step 1: Write the failing test**

```rust
// tests/art_profiles_dal.rs
mod common;
use suzuran_server::dal::UpsertArtProfile;

#[tokio::test]
async fn test_art_profile_crud() {
    let store = common::setup_store().await;

    let ap = store.create_art_profile(UpsertArtProfile {
        name: "Standard 500px".into(),
        max_width_px: 500,
        max_height_px: 500,
        max_size_bytes: Some(200_000),
        format: "jpeg".into(),
        quality: 90,
        apply_to_library_id: None,
    }).await.unwrap();

    assert_eq!(ap.format, "jpeg");
    assert_eq!(ap.quality, 90);

    let all = store.list_art_profiles().await.unwrap();
    assert_eq!(all.len(), 1);

    store.delete_art_profile(ap.id).await.unwrap();
    assert!(store.list_art_profiles().await.unwrap().is_empty());
}
```

**Step 2: Verify fail** (compile error — types not defined)

**Step 3: Write migrations**

`migrations/postgres/0012_art_profiles.sql`:
```sql
CREATE TABLE art_profiles (
    id                  BIGSERIAL PRIMARY KEY,
    name                TEXT NOT NULL,
    max_width_px        INTEGER NOT NULL DEFAULT 500,
    max_height_px       INTEGER NOT NULL DEFAULT 500,
    max_size_bytes      INTEGER,
    format              TEXT NOT NULL DEFAULT 'jpeg'
                            CHECK (format IN ('jpeg', 'png')),
    quality             INTEGER NOT NULL DEFAULT 90
                            CHECK (quality BETWEEN 1 AND 100),
    apply_to_library_id BIGINT REFERENCES libraries(id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

SQLite migration: same structure with SQLite types.

**Step 4: Add to `src/models/mod.rs`**

```rust
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct ArtProfile {
    pub id: i64,
    pub name: String,
    pub max_width_px: i64,
    pub max_height_px: i64,
    pub max_size_bytes: Option<i64>,
    pub format: String,     // "jpeg" | "png"
    pub quality: i64,       // 1–100
    pub apply_to_library_id: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct UpsertArtProfile {
    pub name: String,
    pub max_width_px: i64,
    pub max_height_px: i64,
    pub max_size_bytes: Option<i64>,
    pub format: String,
    pub quality: i64,
    pub apply_to_library_id: Option<i64>,
}
```

**Step 5: Add to `Store` trait** — same five methods as encoding_profiles (`create_art_profile`, `get_art_profile`, `list_art_profiles`, `update_art_profile`, `delete_art_profile`).

**Step 6–7: Implement in PgStore + SqliteStore** — same pattern as encoding_profiles.

**Step 8: Verify pass**

**Step 9: Update codebase filemap** — add migration + model + test entries.

**Step 10: Commit**
```bash
git add migrations/ src/models/mod.rs src/dal/ tests/art_profiles_dal.rs tasks/codebase-filemap.md
git commit -m "feat(4.3): art_profiles migration, model, DAL"
```

---

## Task 4: Track links — DB + DAL

**Files:**
- Create: `migrations/postgres/0013_track_links.sql`
- Create: `migrations/sqlite/0013_track_links.sql`
- Modify: `src/models/mod.rs` — add `TrackLink`
- Modify: `src/dal/mod.rs`, `src/dal/postgres.rs`, `src/dal/sqlite.rs`
- Create: `tests/track_links_dal.rs`

**Step 1: Write the failing test**

```rust
// tests/track_links_dal.rs
mod common;

#[tokio::test]
async fn test_create_and_query_track_link() {
    let (store, src_id, derived_id, ep_id) =
        common::setup_two_tracks_with_encoding_profile().await;

    store.create_track_link(src_id, derived_id, Some(ep_id)).await.unwrap();

    let derived = store.list_derived_tracks(src_id).await.unwrap();
    assert_eq!(derived.len(), 1);
    assert_eq!(derived[0].derived_track_id, derived_id);

    let sources = store.list_source_tracks(derived_id).await.unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_track_id, src_id);
}
```

**Step 2: Verify fail**

**Step 3: Write migrations**

`migrations/postgres/0013_track_links.sql`:
```sql
CREATE TABLE track_links (
    source_track_id     BIGINT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    derived_track_id    BIGINT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    encoding_profile_id BIGINT REFERENCES encoding_profiles(id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (source_track_id, derived_track_id)
);

CREATE INDEX idx_track_links_source  ON track_links(source_track_id);
CREATE INDEX idx_track_links_derived ON track_links(derived_track_id);
```

SQLite: same structure.

**Step 4: Add to `src/models/mod.rs`**

```rust
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct TrackLink {
    pub source_track_id: i64,
    pub derived_track_id: i64,
    pub encoding_profile_id: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

**Step 5: Add to `Store` trait**

```rust
async fn create_track_link(
    &self, source_id: i64, derived_id: i64, encoding_profile_id: Option<i64>
) -> Result<TrackLink, AppError>;
async fn list_derived_tracks(&self, source_id: i64) -> Result<Vec<TrackLink>, AppError>;
async fn list_source_tracks(&self, derived_id: i64) -> Result<Vec<TrackLink>, AppError>;
```

**Step 6–7: Implement in PgStore + SqliteStore** — straightforward INSERT + SELECT queries.

**Step 8: Verify pass**

**Step 9: Update codebase filemap**

**Step 10: Commit**
```bash
git add migrations/ src/models/mod.rs src/dal/ tests/track_links_dal.rs tasks/codebase-filemap.md
git commit -m "feat(4.4): track_links migration, model, DAL"
```

---

## Task 5: CUE sheet parser

**Files:**
- Create: `src/cue/mod.rs`
- Modify: `src/lib.rs` — `pub mod cue;`
- Create: `tests/cue_parser.rs`

**Step 1: Write the failing test**

```rust
// tests/cue_parser.rs
use suzuran_server::cue::{parse_cue, CueSheet};

const SAMPLE_CUE: &str = r#"
REM GENRE Rock
REM DATE 1979
PERFORMER "Pink Floyd"
TITLE "The Wall (Disc 2)"
FILE "disc2.flac" WAVE

  TRACK 01 AUDIO
    TITLE "Hey You"
    PERFORMER "Pink Floyd"
    INDEX 01 00:00:00

  TRACK 02 AUDIO
    TITLE "Is There Anybody Out There?"
    PERFORMER "Pink Floyd"
    INDEX 01 04:42:00

  TRACK 03 AUDIO
    TITLE "Nobody Home"
    INDEX 01 07:19:00
"#;

#[test]
fn test_parse_cue_sheet() {
    let sheet = parse_cue(SAMPLE_CUE).unwrap();
    assert_eq!(sheet.album_title.as_deref(), Some("The Wall (Disc 2)"));
    assert_eq!(sheet.performer.as_deref(), Some("Pink Floyd"));
    assert_eq!(sheet.date.as_deref(), Some("1979"));
    assert_eq!(sheet.audio_file, "disc2.flac");
    assert_eq!(sheet.tracks.len(), 3);

    assert_eq!(sheet.tracks[0].number, 1);
    assert_eq!(sheet.tracks[0].title.as_deref(), Some("Hey You"));
    assert!((sheet.tracks[0].index_01_secs - 0.0).abs() < 0.01);

    // INDEX 01 04:42:00 → 4*60 + 42 + 0/75 = 282.0 seconds
    assert!((sheet.tracks[1].index_01_secs - 282.0).abs() < 0.01);

    // INDEX 01 07:19:00 → 7*60 + 19 + 0/75 = 439.0 seconds
    assert!((sheet.tracks[2].index_01_secs - 439.0).abs() < 0.01);
}

#[test]
fn test_track_duration_calc() {
    let sheet = parse_cue(SAMPLE_CUE).unwrap();
    // track 1 ends at track 2 start (282.0), track 3 has no end (None = EOF)
    let end_0 = sheet.tracks.get(1).map(|t| t.index_01_secs);
    assert_eq!(end_0, Some(282.0));
    assert!(sheet.tracks.get(3).is_none());
}
```

**Step 2: Verify fail**

**Step 3: Implement `src/cue/mod.rs`**

```rust
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
            audio_file = strip_quoted(val, "").unwrap_or_else(|| {
                val.split_whitespace().next().unwrap_or("").to_string()
            });
        } else if let Some(val) = line.strip_prefix("TRACK ") {
            // Flush previous track (no INDEX yet — stored when INDEX 01 seen)
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

/// Strip surrounding quotes and prefix: e.g. strip_quoted(`TITLE "Foo"`, `TITLE `) → Some("Foo")
fn strip_quoted(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let inner = rest.trim().trim_matches('"');
    Some(inner.to_string())
}

/// Parse "MM:SS:FF" (CD frames, 75/sec) → seconds as f64.
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
```

**Step 4: Verify pass**

**Step 5: Update codebase filemap** — add `src/cue/mod.rs`, `tests/cue_parser.rs`.

**Step 6: Commit**
```bash
git add src/cue/mod.rs src/lib.rs tests/cue_parser.rs tasks/codebase-filemap.md
git commit -m "feat(4.5): CUE sheet parser — FILE, TRACK, TITLE, PERFORMER, INDEX 01"
```

---

## Task 6: CUE split job — scanner detection + handler

**Files:**
- Create: `migrations/postgres/0014_jobs_add_cue_split.sql`
- Create: `migrations/sqlite/0014_jobs_add_cue_split.sql`
- Modify: `src/scanner/mod.rs` — detect CUE+audio pairs, skip whole-file, enqueue `cue_split`
- Create: `src/jobs/cue_split.rs`
- Modify: `src/jobs/mod.rs` — add `CueSplitPayload`; export module
- Modify: `src/scheduler/mod.rs` — add semaphore + handler registration
- Create: `tests/cue_split_job.rs`

**Step 1: Write the failing test**

```rust
// tests/cue_split_job.rs
use suzuran_server::jobs::cue_split::CueSplitJobHandler;
use suzuran_server::jobs::JobHandler;
mod common;

#[tokio::test]
async fn test_cue_split_creates_individual_tracks() {
    // common::setup_cue_library returns (store, library_id, root)
    // with a "album.flac" + "album.cue" (3-track CUE sheet) in a temp dir
    let (store, library_id, _root) = common::setup_cue_library().await;

    let handler = CueSplitJobHandler::new(store.clone());
    let cue_path = _root.join("album.cue").to_string_lossy().to_string();
    let result = handler.handle(serde_json::json!({
        "cue_path": cue_path,
        "library_id": library_id
    })).await.unwrap();

    assert_eq!(result["tracks_created"].as_i64(), Some(3));

    let tracks = store.list_tracks_by_library(library_id).await.unwrap();
    assert_eq!(tracks.len(), 3, "3 individual tracks should be in DB");
    assert_eq!(tracks[0].tracknumber.as_deref(), Some("1"));
}

#[tokio::test]
async fn test_cue_split_is_idempotent() {
    let (store, library_id, root) = common::setup_cue_library().await;
    let handler = CueSplitJobHandler::new(store.clone());
    let cue_path = root.join("album.cue").to_string_lossy().to_string();

    handler.handle(serde_json::json!({"cue_path": cue_path, "library_id": library_id})).await.unwrap();
    let result2 = handler.handle(serde_json::json!({"cue_path": cue_path, "library_id": library_id})).await.unwrap();

    // Second run: files already exist, no re-split, tracks already in DB
    assert_eq!(result2["tracks_created"].as_i64(), Some(0));
    let tracks = store.list_tracks_by_library(library_id).await.unwrap();
    assert_eq!(tracks.len(), 3, "no duplicate tracks");
}

#[tokio::test]
async fn test_scanner_skips_cue_backed_audio() {
    let (store, library_id, root) = common::setup_cue_library().await;
    suzuran_server::scanner::scan_library(&store, library_id, &root).await.unwrap();

    // The whole-file "album.flac" must NOT be in the DB
    // Only the cue_split job should be enqueued
    let tracks = store.list_tracks_by_library(library_id).await.unwrap();
    assert_eq!(tracks.len(), 0, "whole-file flac must not be ingested before split");

    let jobs = store.list_jobs(None, Some("pending")).await.unwrap();
    assert!(jobs.iter().any(|j| j.job_type == "cue_split"),
        "cue_split job should be queued");
}
```

**Step 2: Verify fail**

**Step 3: Write migrations**

`migrations/postgres/0014_jobs_add_cue_split.sql`:
```sql
ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_job_type_check;
ALTER TABLE jobs
    ADD CONSTRAINT jobs_job_type_check CHECK (job_type IN (
        'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
        'transcode', 'art_process', 'organize', 'cue_split'
    ));
```

SQLite: `CREATE TABLE jobs_new ... DROP TABLE jobs ... ALTER TABLE jobs_new RENAME TO jobs` (SQLite lacks `ALTER CONSTRAINT` — use the same approach as migration 0010 for SQLite if it uses a different strategy; check `migrations/sqlite/0010_jobs_add_freedb_lookup.sql` for the exact pattern).

**Step 4: Modify `src/scanner/mod.rs`**

Add a two-pass approach before the main walk loop:

```rust
use crate::cue::parse_cue;
use std::collections::HashSet;

// --- Pass 1: find CUE files and their paired audio ---
let mut cue_backed_audio: HashSet<PathBuf> = HashSet::new();
let mut cue_files: Vec<PathBuf> = Vec::new();

for entry in WalkDir::new(root_path).follow_links(true).into_iter().filter_map(|e| e.ok()) {
    let p = entry.path().to_path_buf();
    if p.extension().and_then(|e| e.to_str()) == Some("cue") {
        if let Ok(content) = std::fs::read_to_string(&p) {
            if let Ok(sheet) = parse_cue(&content) {
                let audio = p.parent().unwrap_or(root_path).join(&sheet.audio_file);
                if audio.exists() {
                    cue_backed_audio.insert(audio);
                    cue_files.push(p);
                }
            }
        }
    }
}
```

Then in the main walk loop, after computing `abs_path`, skip cue-backed files and avoid re-enqueueing:

```rust
// Skip audio files that are CUE-backed (whole-file → handled by cue_split)
if cue_backed_audio.contains(&abs_path) {
    continue;
}
```

After the main walk loop, for each CUE file, enqueue a `cue_split` job if one isn't already pending/running:

```rust
for cue_path in &cue_files {
    let cue_str = cue_path.to_string_lossy().to_string();
    let existing = db.list_jobs_by_type_and_payload_key("cue_split", "cue_path", &cue_str).await?;
    if existing.iter().all(|j| j.status == "failed" || j.status == "cancelled") {
        db.enqueue_job("cue_split", serde_json::json!({
            "cue_path": cue_str,
            "library_id": library_id,
        }), 6).await?;
    }
}
```

Add `list_jobs_by_type_and_payload_key` to the Store trait (postgres: `WHERE job_type = $1 AND payload->>$2 = $3`; sqlite: `WHERE job_type = ? AND json_extract(payload, '$.' || ?) = ?`).

**Step 5: Implement `src/jobs/cue_split.rs`**

Key logic (full struct + impl):

```rust
pub struct CueSplitJobHandler { store: Arc<dyn Store> }

impl CueSplitJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self { Self { store } }
}

#[async_trait::async_trait]
impl super::JobHandler for CueSplitJobHandler {
    async fn handle(&self, payload: serde_json::Value) -> Result<serde_json::Value, AppError> {
        let cue_path_str = payload["cue_path"].as_str()
            .ok_or_else(|| AppError::BadRequest("missing cue_path".into()))?;
        let library_id = payload["library_id"].as_i64()
            .ok_or_else(|| AppError::BadRequest("missing library_id".into()))?;

        let cue_path = std::path::Path::new(cue_path_str);
        let cue_dir  = cue_path.parent().unwrap_or(cue_path);

        let content = tokio::fs::read_to_string(cue_path).await
            .map_err(|e| AppError::Internal(format!("CUE read: {e}")))?;
        let sheet = crate::cue::parse_cue(&content)
            .map_err(|e| AppError::Internal(format!("CUE parse: {e}")))?;

        let audio_path = cue_dir.join(&sheet.audio_file);
        if !audio_path.exists() {
            return Err(AppError::Internal(format!("CUE audio file not found: {}", audio_path.display())));
        }

        let library = self.store.get_library(library_id).await?;
        let library_root = std::path::Path::new(&library.root_path);

        let mut tracks_created: usize = 0;

        for (i, track) in sheet.tracks.iter().enumerate() {
            let title = track.title.as_deref().unwrap_or("Track");
            let safe_title = sanitize_filename(title);
            let output_name = format!("{:02} - {}.flac", track.number, safe_title);
            let output_path = cue_dir.join(&output_name);

            // Idempotency: skip if output already exists
            if output_path.exists() {
                // Ensure track is in DB even if file pre-exists
                let rel = output_path.strip_prefix(library_root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(output_name.clone());
                let _ = self.store.get_track_by_library_and_path(library_id, &rel).await;
                continue;
            }

            let start_secs = track.index_01_secs;
            let end_secs   = sheet.tracks.get(i + 1).map(|t| t.index_01_secs);

            // Build ffmpeg args
            let mut args = vec![
                "-i".to_string(), audio_path.to_string_lossy().to_string(),
                "-ss".to_string(), format!("{:.3}", start_secs),
            ];
            if let Some(end) = end_secs {
                args.extend(["-to".to_string(), format!("{:.3}", end)]);
            }
            args.extend(["-c:a".to_string(), "copy".to_string(),
                          "-y".to_string(), output_path.to_string_lossy().to_string()]);

            let status = tokio::process::Command::new("ffmpeg")
                .args(&args)
                .stderr(std::process::Stdio::null())
                .status().await
                .map_err(|e| AppError::Internal(format!("ffmpeg spawn: {e}")))?;

            if !status.success() {
                return Err(AppError::Internal(format!("ffmpeg failed for track {}", track.number)));
            }

            // Write CUE metadata to split file
            let mut tags: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            if let Some(t) = &track.title    { tags.insert("title".into(), t.clone()); }
            let performer = track.performer.as_ref().or(sheet.performer.as_ref());
            if let Some(p) = performer       { tags.insert("artist".into(), p.clone());
                                               tags.insert("albumartist".into(), p.clone()); }
            if let Some(a) = &sheet.album_title { tags.insert("album".into(), a.clone()); }
            if let Some(d) = &sheet.date     { tags.insert("date".into(), d.clone()); }
            if let Some(g) = &sheet.genre    { tags.insert("genre".into(), g.clone()); }
            tags.insert("tracknumber".into(), track.number.to_string());
            tags.insert("totaltracks".into(), sheet.tracks.len().to_string());

            let output_path_clone = output_path.clone();
            let tags_clone = tags.clone();
            tokio::task::spawn_blocking(move || crate::tagger::write_tags(&output_path_clone, &tags_clone))
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .map_err(|e| AppError::Internal(format!("lofty write: {e}")))?;

            // Hash + upsert track
            let hash = hash_file(&output_path).await
                .map_err(|e| AppError::Internal(format!("hash: {e}")))?;
            let rel = output_path.strip_prefix(library_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(output_name);
            let tags_json = serde_json::to_value(&tags).unwrap_or(serde_json::json!({}));

            let new_track = self.store.upsert_track(crate::dal::UpsertTrack {
                library_id,
                relative_path: rel,
                file_hash: hash,
                title: tags.get("title").cloned(),
                artist: tags.get("artist").cloned(),
                albumartist: tags.get("albumartist").cloned(),
                album: tags.get("album").cloned(),
                tracknumber: tags.get("tracknumber").cloned(),
                totaltracks: tags.get("totaltracks").cloned(),
                date: tags.get("date").cloned(),
                genre: tags.get("genre").cloned(),
                tags: tags_json,
                ..Default::default()  // remaining audio props filled on fingerprint
            }).await?;

            self.store.enqueue_job("fingerprint",
                serde_json::json!({"track_id": new_track.id}), 5).await?;

            tracks_created += 1;
        }

        Ok(serde_json::json!({"tracks_created": tracks_created}))
    }
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if r#"/\:*?"<>|"#.contains(c) { '_' } else { c })
        .collect()
}

async fn hash_file(path: &std::path::Path) -> anyhow::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    use sha2::{Digest, Sha256};
    Ok(hex::encode(Sha256::digest(&bytes)))
}
```

Add `pub struct CueSplitPayload { pub cue_path: String, pub library_id: i64 }` and `pub mod cue_split;` to `src/jobs/mod.rs`.

**Step 6: Register in `src/scheduler/mod.rs`**

Add semaphore for `cue_split` (2 concurrent, since it runs ffmpeg). Add dispatch arm:
```rust
"cue_split" => CueSplitJobHandler::new(state.store.clone()).handle(payload).await,
```

**Step 7: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 8: Update codebase filemap**

**Step 9: Commit**
```bash
git add migrations/ src/scanner/mod.rs src/jobs/cue_split.rs src/jobs/mod.rs src/scheduler/mod.rs src/dal/ tests/cue_split_job.rs tasks/codebase-filemap.md
git commit -m "feat(4.6): CUE split — scanner detection, ffmpeg split, track upsert"
```

---

## Task 7: Transcode job handler

**Files:**
- Modify: `src/jobs/mod.rs` — add `TranscodePayload`; export module
- Create: `src/jobs/transcode.rs`
- Modify: `src/scheduler/mod.rs` — register handler (semaphore already pre-allocated)
- Modify: `src/dal/mod.rs` — add `get_track_by_library_and_path` if not present
- Create: `tests/transcode_job.rs`

**Step 1: Write the failing test**

```rust
// tests/transcode_job.rs
use suzuran_server::jobs::transcode::TranscodeJobHandler;
use suzuran_server::jobs::JobHandler;
use suzuran_server::dal::UpsertEncodingProfile;
mod common;

#[tokio::test]
async fn test_transcode_creates_derived_track_and_link() {
    // setup: source library (FLAC), derived library with encoding_profile_id,
    // one source FLAC track
    let (store, src_track_id, target_lib_id) =
        common::setup_transcode_scenario().await;

    let handler = TranscodeJobHandler::new(store.clone());
    let result = handler.handle(serde_json::json!({
        "source_track_id": src_track_id,
        "target_library_id": target_lib_id,
    })).await.unwrap();

    assert_eq!(result["status"].as_str(), Some("completed"));

    let derived_id = result["derived_track_id"].as_i64().expect("derived_track_id in result");
    let links = store.list_derived_tracks(src_track_id).await.unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].derived_track_id, derived_id);
}

#[tokio::test]
async fn test_transcode_fails_without_encoding_profile() {
    let (store, src_track_id, lib_id_no_profile) =
        common::setup_transcode_scenario_no_profile().await;
    let handler = TranscodeJobHandler::new(store.clone());
    let result = handler.handle(serde_json::json!({
        "source_track_id": src_track_id,
        "target_library_id": lib_id_no_profile,
    })).await;
    assert!(result.is_err(), "should fail when target library has no encoding profile");
}
```

**Step 2: Verify fail**

**Step 3: Implement `src/jobs/transcode.rs`**

```rust
pub struct TranscodeJobHandler { store: Arc<dyn Store> }

impl TranscodeJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self { Self { store } }
}

fn codec_extension(codec: &str) -> &str {
    match codec {
        "aac"                   => "m4a",
        "mp3" | "libmp3lame"    => "mp3",
        "opus" | "libopus"      => "opus",
        "flac"                  => "flac",
        "vorbis" | "libvorbis"  => "ogg",
        other                   => other,
    }
}

fn build_ffmpeg_args(profile: &EncodingProfile) -> Vec<String> {
    let mut args = vec!["-vn".to_string()];         // drop video/art streams
    args.extend(["-c:a".to_string(), profile.codec.clone()]);
    if let Some(b) = &profile.bitrate {
        args.extend(["-b:a".to_string(), b.clone()]);
    }
    if let Some(sr) = profile.sample_rate {
        args.extend(["-ar".to_string(), sr.to_string()]);
    }
    if let Some(ch) = profile.channels {
        args.extend(["-ac".to_string(), ch.to_string()]);
    }
    if let Some(adv) = &profile.advanced_args {
        args.extend(adv.split_whitespace().map(str::to_string));
    }
    args
}

#[async_trait::async_trait]
impl super::JobHandler for TranscodeJobHandler {
    async fn handle(&self, payload: serde_json::Value) -> Result<serde_json::Value, AppError> {
        let src_track_id  = payload["source_track_id"].as_i64()
            .ok_or_else(|| AppError::BadRequest("missing source_track_id".into()))?;
        let tgt_library_id = payload["target_library_id"].as_i64()
            .ok_or_else(|| AppError::BadRequest("missing target_library_id".into()))?;

        let src_track = self.store.get_track(src_track_id).await?;
        let src_lib   = self.store.get_library(src_track.library_id).await?;
        let tgt_lib   = self.store.get_library(tgt_library_id).await?;

        let ep_id = tgt_lib.encoding_profile_id
            .ok_or_else(|| AppError::BadRequest("target library has no encoding_profile_id".into()))?;
        let profile = self.store.get_encoding_profile(ep_id).await?;

        let src_path = format!("{}/{}", src_lib.root_path.trim_end_matches('/'),
                                         src_track.relative_path.trim_start_matches('/'));

        // Determine output path: same relative path with codec extension
        let ext = codec_extension(&profile.codec);
        let out_rel = std::path::Path::new(&src_track.relative_path)
            .with_extension(ext)
            .to_string_lossy()
            .to_string();
        let out_path = format!("{}/{}", tgt_lib.root_path.trim_end_matches('/'),
                                        out_rel.trim_start_matches('/'));

        // Create output directory
        if let Some(parent) = std::path::Path::new(&out_path).parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| AppError::Internal(format!("mkdir: {e}")))?;
        }

        // Run ffmpeg
        let mut args: Vec<String> = vec!["-i".to_string(), src_path];
        args.extend(build_ffmpeg_args(&profile));
        args.extend(["-progress".to_string(), "pipe:1".to_string(),
                      "-y".to_string(), out_path.clone()]);

        let mut child = tokio::process::Command::new("ffmpeg")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| AppError::Internal(format!("ffmpeg spawn: {e}")))?;

        // Drain progress output (parse out_time_ms for future polling)
        let stdout = child.stdout.take();
        let _progress_task = tokio::spawn(async move {
            if let Some(mut out) = stdout {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(_line)) = lines.next_line().await {
                    // Future: parse "out_time_ms=N" and update job progress
                }
            }
        });

        let status = child.wait().await
            .map_err(|e| AppError::Internal(format!("ffmpeg wait: {e}")))?;
        if !status.success() {
            return Err(AppError::Internal("ffmpeg transcode failed".into()));
        }

        // Write source tags to output file
        let tags: std::collections::HashMap<String, String> = src_track.tags
            .as_object().cloned().unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
            .collect();
        let out_path_buf = std::path::PathBuf::from(&out_path);
        let tags_clone = tags.clone();
        tokio::task::spawn_blocking(move || crate::tagger::write_tags(&out_path_buf, &tags_clone))
            .await.map_err(|e| AppError::Internal(e.to_string()))?
            .map_err(|e| AppError::Internal(format!("lofty write: {e}")))?;

        // Compute hash + upsert derived track
        let hash = cue_split::hash_file(std::path::Path::new(&out_path)).await
            .map_err(|e| AppError::Internal(format!("hash: {e}")))?;
        let tags_json = serde_json::to_value(&tags).unwrap_or(serde_json::json!({}));

        let derived_track = self.store.upsert_track(crate::dal::UpsertTrack {
            library_id: tgt_library_id,
            relative_path: out_rel,
            file_hash: hash,
            title: src_track.title.clone(),
            artist: src_track.artist.clone(),
            albumartist: src_track.albumartist.clone(),
            album: src_track.album.clone(),
            tracknumber: src_track.tracknumber.clone(),
            discnumber: src_track.discnumber.clone(),
            totaldiscs: src_track.totaldiscs.clone(),
            totaltracks: src_track.totaltracks.clone(),
            date: src_track.date.clone(),
            genre: src_track.genre.clone(),
            composer: src_track.composer.clone(),
            label: src_track.label.clone(),
            catalognumber: src_track.catalognumber.clone(),
            tags: tags_json,
            duration_secs: src_track.duration_secs,
            bitrate: None,          // will be populated on next scan/fingerprint
            sample_rate: None,
            channels: None,
            has_embedded_art: src_track.has_embedded_art,
        }).await?;

        self.store.create_track_link(src_track_id, derived_track.id, Some(ep_id)).await?;

        Ok(serde_json::json!({
            "status": "completed",
            "source_track_id": src_track_id,
            "derived_track_id": derived_track.id,
        }))
    }
}
```

Add `pub mod transcode;` and `pub struct TranscodePayload { pub source_track_id: i64, pub target_library_id: i64 }` to `src/jobs/mod.rs`.

Note: `UpsertTrack` may need `..Default::default()` if it doesn't impl `Default` yet — add `#[derive(Default)]` to `UpsertTrack` in `src/dal/mod.rs` if missing.

**Step 4: Register in `src/scheduler/mod.rs`**

```rust
"transcode" => TranscodeJobHandler::new(state.store.clone()).handle(payload).await,
```

The scheduler already pre-allocates a semaphore for `transcode` (concurrency 2) — verify this is wired and add it if not.

**Step 5: Verify pass**

**Step 6: Update codebase filemap**

**Step 7: Commit**
```bash
git add src/jobs/transcode.rs src/jobs/mod.rs src/scheduler/mod.rs src/dal/ tests/transcode_job.rs tasks/codebase-filemap.md
git commit -m "feat(4.7): transcode job — ffmpeg pipeline, encoding profiles, track_links"
```

---

## Task 8: Art process job handler + auto-embed on suggestion accept

**Files:**
- Modify: `Cargo.toml` — add `image = "0.25"`
- Create: `src/jobs/art_process.rs`
- Modify: `src/jobs/mod.rs` — add `ArtProcessPayload`; export module
- Modify: `src/services/tagging.rs` — enqueue `art_process` after accept
- Modify: `src/scheduler/mod.rs` — register handler
- Create: `tests/art_process_job.rs`

**Step 1: Write the failing test**

```rust
// tests/art_process_job.rs
use suzuran_server::jobs::art_process::ArtProcessJobHandler;
use suzuran_server::jobs::JobHandler;
use suzuran_server::dal::UpsertArtProfile;
mod common;

#[tokio::test]
async fn test_embed_art_from_url() {
    // wiremock serves a minimal 1x1 JPEG at the URL
    let (server, store, track_id, _root) = common::setup_art_process_scenario().await;
    let art_url = format!("{}/cover.jpg", server.uri());

    let handler = ArtProcessJobHandler::new(store.clone());
    let result = handler.handle(serde_json::json!({
        "track_id": track_id,
        "action": "embed",
        "source_url": art_url
    })).await.unwrap();

    assert_eq!(result["status"].as_str(), Some("completed"));
    let track = store.get_track(track_id).await.unwrap();
    assert!(track.has_embedded_art.unwrap_or(false), "art should be embedded");
}

#[tokio::test]
async fn test_standardize_resizes_to_profile() {
    let (store, track_id, art_profile_id, _root) =
        common::setup_standardize_scenario().await; // track has 1000x1000 embedded art, profile max 500px
    let handler = ArtProcessJobHandler::new(store.clone());
    handler.handle(serde_json::json!({
        "track_id": track_id,
        "action": "standardize",
        "art_profile_id": art_profile_id
    })).await.unwrap();

    // Read art back from file and check dimensions
    // (common helper reads embedded art bytes → image::load_from_memory → dimensions)
    let (w, h) = common::read_embedded_art_dimensions(_root.join("track.flac")).await;
    assert!(w <= 500 && h <= 500);
}
```

**Step 2: Verify fail**

**Step 3: Update `Cargo.toml`**

Under `[dependencies]`:
```toml
image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }
```

**Step 4: Implement `src/jobs/art_process.rs`**

```rust
use image::{DynamicImage, ImageFormat, codecs::jpeg::JpegEncoder};
use lofty::{
    file::TaggedFileExt,
    picture::{MimeType, Picture, PictureType},
    probe::Probe,
    tag::Accessor,
};
use std::{io::Cursor, sync::Arc};
use crate::{dal::Store, error::AppError};

pub struct ArtProcessJobHandler { store: Arc<dyn Store> }
impl ArtProcessJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self { Self { store } }
}

#[async_trait::async_trait]
impl super::JobHandler for ArtProcessJobHandler {
    async fn handle(&self, payload: serde_json::Value) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"].as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;
        let action = payload["action"].as_str()
            .ok_or_else(|| AppError::BadRequest("missing action".into()))?;

        let track   = self.store.get_track(track_id).await?;
        let library = self.store.get_library(track.library_id).await?;
        let path = format!("{}/{}", library.root_path.trim_end_matches('/'),
                                    track.relative_path.trim_start_matches('/'));

        match action {
            "embed" => {
                let url = payload["source_url"].as_str()
                    .ok_or_else(|| AppError::BadRequest("embed requires source_url".into()))?;
                let bytes = reqwest::get(url).await
                    .map_err(|e| AppError::Internal(format!("fetch art: {e}")))?
                    .bytes().await
                    .map_err(|e| AppError::Internal(format!("art bytes: {e}")))?;
                let mime = if url.ends_with(".png") { MimeType::Png } else { MimeType::Jpeg };
                embed_art_bytes(&path, bytes.to_vec(), mime).await?;
                self.store.set_track_has_embedded_art(track_id, true).await?;
            }
            "extract" => {
                extract_art(&path).await?;
            }
            "standardize" => {
                let profile_id = payload["art_profile_id"].as_i64()
                    .ok_or_else(|| AppError::BadRequest("standardize requires art_profile_id".into()))?;
                let profile = self.store.get_art_profile(profile_id).await?;
                standardize_art(&path, &profile).await?;
                self.store.set_track_has_embedded_art(track_id, true).await?;
            }
            other => return Err(AppError::BadRequest(format!("unknown art action: {other}"))),
        }

        Ok(serde_json::json!({"status": "completed", "track_id": track_id, "action": action}))
    }
}

async fn embed_art_bytes(audio_path: &str, bytes: Vec<u8>, mime: MimeType) -> Result<(), AppError> {
    let path = audio_path.to_string();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mut tagged = Probe::open(&path)?.read()?;
        let tag = tagged.primary_tag_mut()
            .ok_or_else(|| anyhow::anyhow!("no primary tag"))?;
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            Some(mime),
            None,
            bytes,
        ));
        tagged.save_to_path(&path, lofty::config::WriteOptions::default())?;
        Ok(())
    }).await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map_err(|e| AppError::Internal(format!("lofty embed: {e}")))
}

async fn extract_art(audio_path: &str) -> Result<(), AppError> {
    let path = audio_path.to_string();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let tagged = Probe::open(&path)?.read()?;
        let tag = tagged.primary_tag()
            .ok_or_else(|| anyhow::anyhow!("no primary tag"))?;
        let pic = tag.pictures().first()
            .ok_or_else(|| anyhow::anyhow!("no embedded art"))?;
        let ext = match pic.mime_type() {
            Some(MimeType::Png) => "png",
            _                   => "jpg",
        };
        let out = std::path::Path::new(&path)
            .with_extension(format!("cover.{ext}"));
        std::fs::write(out, pic.data())?;
        Ok(())
    }).await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map_err(|e| AppError::Internal(format!("extract art: {e}")))
}

async fn standardize_art(audio_path: &str, profile: &crate::models::ArtProfile) -> Result<(), AppError> {
    let path   = audio_path.to_string();
    let max_w  = profile.max_width_px as u32;
    let max_h  = profile.max_height_px as u32;
    let quality = profile.quality as u8;
    let format  = profile.format.clone();

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        // Read existing art
        let tagged = Probe::open(&path)?.read()?;
        let tag = tagged.primary_tag()
            .ok_or_else(|| anyhow::anyhow!("no primary tag"))?;
        let pic = tag.pictures().first()
            .ok_or_else(|| anyhow::anyhow!("no embedded art to standardize"))?;

        // Resize via image crate
        let img = image::load_from_memory(pic.data())?;
        let resized = if img.width() > max_w || img.height() > max_h {
            img.resize(max_w, max_h, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // Re-encode to target format
        let mut out_bytes: Vec<u8> = Vec::new();
        let mime = if format == "png" {
            resized.write_to(&mut Cursor::new(&mut out_bytes), ImageFormat::Png)?;
            MimeType::Png
        } else {
            let mut enc = JpegEncoder::new_with_quality(&mut out_bytes, quality);
            resized.write_with_encoder(enc)?;
            MimeType::Jpeg
        };

        drop(tagged); // release read lock before re-opening for write
        embed_art_bytes_sync(&path, out_bytes, mime)?;
        Ok(())
    }).await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map_err(|e| AppError::Internal(format!("standardize art: {e}")))
}

/// Sync version of embed (called from spawn_blocking context).
fn embed_art_bytes_sync(path: &str, bytes: Vec<u8>, mime: MimeType) -> anyhow::Result<()> {
    let mut tagged = Probe::open(path)?.read()?;
    let tag = tagged.primary_tag_mut()
        .ok_or_else(|| anyhow::anyhow!("no primary tag"))?;
    tag.clear_pictures();
    tag.push_picture(Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, bytes));
    tagged.save_to_path(path, lofty::config::WriteOptions::default())?;
    Ok(())
}
```

Add `set_track_has_embedded_art` to Store trait + implementations (`UPDATE tracks SET has_embedded_art = $1 WHERE id = $2`).

**Step 5: Wire auto-embed in `src/services/tagging.rs`**

At the end of `apply_suggestion`, after the DB update:
```rust
if let Some(url) = &suggestion.cover_art_url {
    store.enqueue_job(
        "art_process",
        serde_json::json!({
            "track_id": suggestion.track_id,
            "action": "embed",
            "source_url": url,
        }),
        3,
    ).await?;
}
```

**Step 6: Add to `src/jobs/mod.rs`**

```rust
pub struct ArtProcessPayload {
    pub track_id: i64,
    pub action: String,              // "embed" | "extract" | "standardize"
    pub source_url: Option<String>,
    pub art_profile_id: Option<i64>,
}
pub mod art_process;
```

**Step 7: Register in scheduler**

```rust
"art_process" => ArtProcessJobHandler::new(state.store.clone()).handle(payload).await,
```

**Step 8: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 9: Update codebase filemap** — add `src/jobs/art_process.rs`, `tests/art_process_job.rs`; note Cargo.toml `image` dependency.

**Step 10: Commit**
```bash
git add Cargo.toml Cargo.lock src/jobs/art_process.rs src/jobs/mod.rs src/services/tagging.rs src/scheduler/mod.rs src/dal/ tests/art_process_job.rs tasks/codebase-filemap.md
git commit -m "feat(4.8): art_process job — embed/extract/standardize, auto-embed on accept"
```

---

## Task 9: Encoding profiles + art profiles API

**Files:**
- Create: `src/api/encoding_profiles.rs`
- Create: `src/api/art_profiles.rs`
- Modify: `src/api/mod.rs` — mount both routers
- Create: `tests/encoding_profiles_api.rs`
- Create: `tests/art_profiles_api.rs`

**Step 1: Write the failing tests**

```rust
// tests/encoding_profiles_api.rs
mod common;
use common::TestApp;

#[tokio::test]
async fn test_encoding_profiles_crud() {
    let app = TestApp::spawn().await;
    let token = app.seed_admin_user().await;

    // Create
    let resp = app.authed_post(&token, "/api/v1/encoding-profiles", serde_json::json!({
        "name": "AAC 256k",
        "codec": "aac",
        "bitrate": "256k",
        "sample_rate": 44100,
        "channels": 2,
        "advanced_args": null
    })).await;
    assert_eq!(resp.status(), 201);
    let ep: serde_json::Value = resp.json().await.unwrap();
    let ep_id = ep["id"].as_i64().unwrap();

    // List
    let resp = app.authed_get(&token, "/api/v1/encoding-profiles").await;
    let list: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(list.len(), 1);

    // Update
    let resp = app.authed_put(&token, &format!("/api/v1/encoding-profiles/{ep_id}"),
        serde_json::json!({"name": "AAC 320k", "codec": "aac", "bitrate": "320k"})).await;
    assert_eq!(resp.status(), 200);

    // Delete
    let resp = app.authed_delete(&token, &format!("/api/v1/encoding-profiles/{ep_id}")).await;
    assert_eq!(resp.status(), 204);
}
```

Write a parallel test for art profiles in `tests/art_profiles_api.rs` with the same pattern.

**Step 2: Verify fail**

**Step 3: Implement `src/api/encoding_profiles.rs`**

Follow the exact same router structure as `src/api/organization_rules.rs` (which has CRUD). Endpoints:

| Method | Path | Auth | Status |
|--------|------|------|--------|
| `GET` | `/` | required | 200 + `Vec<EncodingProfile>` |
| `POST` | `/` | admin | 201 + `EncodingProfile` |
| `GET` | `/:id` | required | 200 + `EncodingProfile` |
| `PUT` | `/:id` | admin | 200 + `EncodingProfile` |
| `DELETE` | `/:id` | admin | 204 |

Request body for create/update:
```rust
#[derive(serde::Deserialize)]
struct EncodingProfileBody {
    name: String,
    codec: String,
    bitrate: Option<String>,
    sample_rate: Option<i64>,
    channels: Option<i64>,
    advanced_args: Option<String>,
}
```

Convert `EncodingProfileBody` → `UpsertEncodingProfile` in handlers.

**Step 4: Implement `src/api/art_profiles.rs`** — same structure, same admin guard for write ops, `ArtProfileBody` → `UpsertArtProfile`.

**Step 5: Mount in `src/api/mod.rs`**

```rust
.nest("/encoding-profiles", encoding_profiles::router())
.nest("/art-profiles", art_profiles::router())
```

**Step 6: Verify pass**

**Step 7: Update codebase filemap**

**Step 8: Commit**
```bash
git add src/api/encoding_profiles.rs src/api/art_profiles.rs src/api/mod.rs tests/encoding_profiles_api.rs tests/art_profiles_api.rs tasks/codebase-filemap.md
git commit -m "feat(4.9): encoding profiles + art profiles REST API"
```

---

## Task 10: Transcode API + art API + auto-transcode wiring

**Files:**
- Create: `src/api/transcode.rs`
- Create: `src/api/art.rs`
- Modify: `src/api/mod.rs` — mount
- Modify: `src/scanner/mod.rs` — auto-transcode on ingest
- Create: `tests/transcode_api.rs`
- Create: `tests/art_api.rs`

**Step 1: Write the failing tests**

```rust
// tests/transcode_api.rs
mod common;
use common::TestApp;

#[tokio::test]
async fn test_manual_transcode_enqueues_job() {
    let app = TestApp::spawn().await;
    let (token, src_track_id, tgt_lib_id) = app.seed_transcode_scenario().await;

    let resp = app.authed_post(&token,
        &format!("/api/v1/tracks/{src_track_id}/transcode"),
        serde_json::json!({"target_library_id": tgt_lib_id}),
    ).await;
    assert_eq!(resp.status(), 202);

    let jobs = app.store.list_jobs(None, Some("pending")).await.unwrap();
    assert!(jobs.iter().any(|j| j.job_type == "transcode"));
}

#[tokio::test]
async fn test_library_transcode_sync_enqueues_missing() {
    let app = TestApp::spawn().await;
    let (token, src_lib_id, tgt_lib_id) = app.seed_library_sync_scenario().await;
    // src has 3 tracks; tgt has 1 derived track already

    let resp = app.authed_post(&token,
        &format!("/api/v1/libraries/{src_lib_id}/transcode-sync"),
        serde_json::json!({"target_library_id": tgt_lib_id}),
    ).await;
    assert_eq!(resp.status(), 202);

    let jobs = app.store.list_jobs(None, Some("pending")).await.unwrap();
    let transcode_jobs: Vec<_> = jobs.iter().filter(|j| j.job_type == "transcode").collect();
    assert_eq!(transcode_jobs.len(), 2, "2 missing tracks need transcode");
}
```

**Step 2: Verify fail**

**Step 3: Implement `src/api/transcode.rs`**

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tracks/:id/transcode", post(transcode_track))
        .route("/libraries/:id/transcode",      post(transcode_library))
        .route("/libraries/:id/transcode-sync", post(transcode_library_sync))
}

// POST /tracks/:id/transcode  { target_library_id: i64 }
async fn transcode_track(_user: AuthUser, Path(id): Path<i64>,
    State(s): State<AppState>, Json(body): Json<TranscodeBody>
) -> Result<StatusCode, AppError> {
    s.store.enqueue_job("transcode", serde_json::json!({
        "source_track_id": id,
        "target_library_id": body.target_library_id,
    }), 5).await?;
    Ok(StatusCode::ACCEPTED)
}

// POST /libraries/:id/transcode  { target_library_id: i64 }
// Enqueues transcode for every track in source library
async fn transcode_library(_user: AuthUser, Path(src_lib_id): Path<i64>,
    State(s): State<AppState>, Json(body): Json<TranscodeBody>
) -> Result<Json<serde_json::Value>, AppError> {
    let tracks = s.store.list_tracks_by_library(src_lib_id).await?;
    let count = tracks.len();
    for t in tracks {
        s.store.enqueue_job("transcode", serde_json::json!({
            "source_track_id": t.id,
            "target_library_id": body.target_library_id,
        }), 5).await?;
    }
    Ok(Json(serde_json::json!({"enqueued": count})))
}

// POST /libraries/:id/transcode-sync  { target_library_id: i64 }
// Only enqueues transcode for source tracks with no existing track_link to target library
async fn transcode_library_sync(_user: AuthUser, Path(src_lib_id): Path<i64>,
    State(s): State<AppState>, Json(body): Json<TranscodeBody>
) -> Result<Json<serde_json::Value>, AppError> {
    let src_tracks = s.store.list_tracks_by_library(src_lib_id).await?;
    let derived    = s.store.list_tracks_by_library(body.target_library_id).await?;

    // Build set of source_track_ids that already have a link into target library
    let linked_sources: std::collections::HashSet<i64> = {
        let mut set = std::collections::HashSet::new();
        for dt in &derived {
            for link in s.store.list_source_tracks(dt.id).await? {
                set.insert(link.source_track_id);
            }
        }
        set
    };

    let mut enqueued = 0usize;
    for t in src_tracks.iter().filter(|t| !linked_sources.contains(&t.id)) {
        s.store.enqueue_job("transcode", serde_json::json!({
            "source_track_id": t.id,
            "target_library_id": body.target_library_id,
        }), 5).await?;
        enqueued += 1;
    }
    Ok(Json(serde_json::json!({"enqueued": enqueued})))
}

#[derive(serde::Deserialize)]
struct TranscodeBody { target_library_id: i64 }
```

**Step 4: Implement `src/api/art.rs`**

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tracks/:id/art/embed",        post(art_embed))
        .route("/tracks/:id/art/extract",      post(art_extract))
        .route("/tracks/:id/art/standardize",  post(art_standardize))
        .route("/libraries/:id/art/standardize", post(art_standardize_library))
}
```

- `art_embed`: expects `{ source_url: String }` body → enqueue `art_process` action=embed
- `art_extract`: no body → enqueue `art_process` action=extract
- `art_standardize`: expects `{ art_profile_id: i64 }` → enqueue `art_process` action=standardize
- `art_standardize_library`: enqueue `art_process` action=standardize for every track in the library that `has_embedded_art = true`

All return `202 Accepted` with `{ "enqueued": N }`.

**Step 5: Auto-transcode on ingest**

In `src/scanner/mod.rs`, after successfully upserting a new track (where `is_new` is true), check if any child libraries of this library have `auto_transcode_on_ingest = true`:

```rust
if is_new {
    result.inserted += 1;
    db.enqueue_job("fingerprint", serde_json::json!({"track_id": track.id}), 5).await?;

    // Auto-transcode to child libraries
    let children = db.list_child_libraries(library_id).await?;
    for child in children.iter().filter(|c| c.auto_transcode_on_ingest) {
        db.enqueue_job("transcode", serde_json::json!({
            "source_track_id": track.id,
            "target_library_id": child.id,
        }), 4).await?;
    }
}
```

Add `list_child_libraries(parent_id: i64) -> Result<Vec<Library>, AppError>` to the Store trait + both implementations (`WHERE parent_library_id = $1`).

**Step 6: Mount in `src/api/mod.rs`**

```rust
// Merge transcode and art routes at the /api/v1 level (not nested under a prefix,
// since they use /tracks/:id and /libraries/:id paths directly)
.merge(transcode::router())
.merge(art::router())
```

**Step 7: Verify pass**

**Step 8: Update codebase filemap**

**Step 9: Commit**
```bash
git add src/api/transcode.rs src/api/art.rs src/api/mod.rs src/scanner/mod.rs src/dal/ tests/transcode_api.rs tests/art_api.rs tasks/codebase-filemap.md
git commit -m "feat(4.10): transcode + art APIs, auto-transcode on ingest wiring"
```

---

## Task 11: UI — encoding profiles & art profiles settings

**Files:**
- Create: `ui/src/types/encodingProfile.ts`
- Create: `ui/src/types/artProfile.ts`
- Create: `ui/src/api/encodingProfiles.ts`
- Create: `ui/src/api/artProfiles.ts`
- Create: `ui/src/components/EncodingProfileForm.tsx`
- Create: `ui/src/components/ArtProfileForm.tsx`
- Modify: `ui/src/pages/SettingsPage.tsx` — add tabs/sections for both profile types

**Step 1: Add types**

`ui/src/types/encodingProfile.ts`:
```typescript
export interface EncodingProfile {
  id: number;
  name: string;
  codec: string;
  bitrate?: string;
  sample_rate?: number;
  channels?: number;
  advanced_args?: string;
  created_at: string;
}
export interface UpsertEncodingProfile {
  name: string;
  codec: string;
  bitrate?: string;
  sample_rate?: number;
  channels?: number;
  advanced_args?: string;
}
```

`ui/src/types/artProfile.ts`:
```typescript
export interface ArtProfile {
  id: number;
  name: string;
  max_width_px: number;
  max_height_px: number;
  max_size_bytes?: number;
  format: 'jpeg' | 'png';
  quality: number;
  apply_to_library_id?: number;
  created_at: string;
}
export interface UpsertArtProfile {
  name: string;
  max_width_px: number;
  max_height_px: number;
  max_size_bytes?: number;
  format: 'jpeg' | 'png';
  quality: number;
  apply_to_library_id?: number;
}
```

**Step 2: Add API clients**

`ui/src/api/encodingProfiles.ts` and `ui/src/api/artProfiles.ts` — follow the exact pattern of `ui/src/api/tagSuggestions.ts`. Each needs `list()`, `create(body)`, `update(id, body)`, `delete(id)`.

**Step 3: Implement `EncodingProfileForm`**

```tsx
// ui/src/components/EncodingProfileForm.tsx
const CODECS = ['aac', 'mp3', 'opus', 'flac', 'vorbis'] as const;

interface Props {
  initial?: UpsertEncodingProfile;
  onSave: (data: UpsertEncodingProfile) => Promise<void>;
  onCancel: () => void;
  isPending: boolean;
}

export function EncodingProfileForm({ initial, onSave, onCancel, isPending }: Props) {
  const [form, setForm] = useState<UpsertEncodingProfile>(initial ?? {
    name: '', codec: 'aac', bitrate: '256k', sample_rate: 44100, channels: 2,
  });
  const [showAdvanced, setShowAdvanced] = useState(!!form.advanced_args);

  const isLossless = form.codec === 'flac';

  return (
    <form onSubmit={e => { e.preventDefault(); onSave(form); }}
          className="space-y-3">
      {/* name, codec (dropdown), bitrate (hidden for FLAC), sample_rate, channels */}
      {/* Advanced section: collapsible textarea for advanced_args */}
      <div>
        <button type="button" onClick={() => setShowAdvanced(v => !v)}
                className="text-xs text-muted-foreground underline">
          {showAdvanced ? 'Hide' : 'Show'} advanced ffmpeg args
        </button>
        {showAdvanced && (
          <textarea
            className="mt-1 w-full font-mono text-xs bg-input border border-border rounded px-2 py-1"
            rows={2}
            placeholder="-af 'aresample=resampler=soxr'"
            value={form.advanced_args ?? ''}
            onChange={e => setForm(f => ({ ...f, advanced_args: e.target.value || undefined }))}
          />
        )}
      </div>
      <div className="flex gap-2 pt-1">
        <button type="submit" disabled={isPending}
                className="px-3 py-1 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50">
          Save
        </button>
        <button type="button" onClick={onCancel}
                className="px-3 py-1 text-sm border border-border rounded hover:bg-muted">
          Cancel
        </button>
      </div>
    </form>
  );
}
```

**Step 4: Implement `ArtProfileForm`** — similar structure: name, format (jpeg/png dropdown), quality (1–100 slider), max_width_px, max_height_px, max_size_bytes (optional).

**Step 5: Update `SettingsPage`**

Check the existing `SettingsPage` structure. It likely has tabs or sections for system settings and themes. Add two new sections (tabs or collapsible panels): **Encoding Profiles** and **Art Profiles**.

Each section shows a list of existing profiles (name, codec/format summary) with Edit and Delete buttons, plus a "New Profile" button that reveals the form inline.

Use `useQuery` to fetch lists and `useMutation` for create/update/delete, invalidating the list query on success.

**Step 6: Build and verify in browser**
```bash
docker compose up --build -d
# Navigate to /settings
# Encoding Profiles section should render
# Create a profile "AAC 256k" — it should appear in the list
# Edit it — name should update
# Delete it — list becomes empty
```

**Step 7: Update codebase filemap**

**Step 8: Commit**
```bash
git add ui/src/types/ ui/src/api/encodingProfiles.ts ui/src/api/artProfiles.ts ui/src/components/EncodingProfileForm.tsx ui/src/components/ArtProfileForm.tsx ui/src/pages/SettingsPage.tsx tasks/codebase-filemap.md
git commit -m "feat(4.11): Settings UI — encoding profiles + art profiles management"
```

---

## Task 12: UI — transcode & art operations + phase complete

**Files:**
- Create: `ui/src/api/transcode.ts`
- Create: `ui/src/api/art.ts`
- Create: `ui/src/components/TranscodeDialog.tsx`
- Modify: `ui/src/pages/LibraryPage.tsx` — add transcode + art action buttons
- Modify: `CHANGELOG.md` — add v0.4.0 entry

**Step 1: Add API clients**

`ui/src/api/transcode.ts`:
```typescript
import { client } from './client';

export const transcodeApi = {
  transcodeTrack(trackId: number, targetLibraryId: number) {
    return client.post(`/tracks/${trackId}/transcode`, { target_library_id: targetLibraryId });
  },
  transcodeLibrary(srcLibId: number, targetLibraryId: number) {
    return client.post(`/libraries/${srcLibId}/transcode`, { target_library_id: targetLibraryId });
  },
  transcodeSync(srcLibId: number, targetLibraryId: number) {
    return client.post(`/libraries/${srcLibId}/transcode-sync`, { target_library_id: targetLibraryId });
  },
};
```

`ui/src/api/art.ts`:
```typescript
import { client } from './client';

export const artApi = {
  embedFromUrl(trackId: number, sourceUrl: string) {
    return client.post(`/tracks/${trackId}/art/embed`, { source_url: sourceUrl });
  },
  extract(trackId: number) {
    return client.post(`/tracks/${trackId}/art/extract`);
  },
  standardize(trackId: number, artProfileId: number) {
    return client.post(`/tracks/${trackId}/art/standardize`, { art_profile_id: artProfileId });
  },
  standardizeLibrary(libraryId: number, artProfileId: number) {
    return client.post(`/libraries/${libraryId}/art/standardize`, { art_profile_id: artProfileId });
  },
};
```

**Step 2: Implement `TranscodeDialog`**

A small modal/popover that:
1. Fetches the list of libraries (`GET /api/v1/libraries`)
2. Filters to libraries that have an `encoding_profile_id` (i.e., derived libraries)
3. Lets the user pick a target library from a dropdown
4. Has "Transcode" and "Sync" buttons (sync only shows when operating on a whole library, not a single track)
5. On confirm: calls `transcodeApi.transcodeTrack` or `transcodeApi.transcodeSync`

```tsx
// ui/src/components/TranscodeDialog.tsx
interface Props {
  mode: 'track' | 'library';
  sourceId: number;           // track id or library id depending on mode
  onClose: () => void;
}

export function TranscodeDialog({ mode, sourceId, onClose }: Props) {
  const qc = useQueryClient();
  const { data: libraries = [] } = useQuery({
    queryKey: ['libraries'],
    queryFn: () => librariesApi.list(),
  });
  const derivedLibs = libraries.filter(l => l.encoding_profile_id != null);
  const [targetId, setTargetId] = useState<number | null>(derivedLibs[0]?.id ?? null);

  const transcode = useMutation({
    mutationFn: () => mode === 'track'
      ? transcodeApi.transcodeTrack(sourceId, targetId!)
      : transcodeApi.transcodeLibrary(sourceId, targetId!),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['jobs'] }); onClose(); },
  });
  const sync = useMutation({
    mutationFn: () => transcodeApi.transcodeSync(sourceId, targetId!),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['jobs'] }); onClose(); },
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
         onClick={onClose}>
      <div className="bg-card border border-border rounded p-4 w-80 space-y-3"
           onClick={e => e.stopPropagation()}>
        <h2 className="text-sm font-medium">Transcode to library</h2>
        <select value={targetId ?? ''} onChange={e => setTargetId(Number(e.target.value))}
                className="w-full bg-input border border-border rounded px-2 py-1 text-sm">
          {derivedLibs.map(l => <option key={l.id} value={l.id}>{l.name}</option>)}
        </select>
        {derivedLibs.length === 0 && (
          <p className="text-xs text-muted-foreground">
            No libraries with encoding profiles found. Create one in Settings first.
          </p>
        )}
        <div className="flex gap-2">
          <button onClick={() => transcode.mutate()} disabled={!targetId || transcode.isPending}
                  className="flex-1 px-3 py-1 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50">
            Transcode all
          </button>
          {mode === 'library' && (
            <button onClick={() => sync.mutate()} disabled={!targetId || sync.isPending}
                    className="flex-1 px-3 py-1 text-sm border border-border rounded hover:bg-muted disabled:opacity-50">
              Sync missing
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
```

**Step 3: Wire into `LibraryPage`**

In the track list row and album header row, add action buttons. These should match the existing action button style (check how "Edit tags" buttons look in the current library page).

Track row: add a `...` overflow menu or inline icon buttons:
- **Transcode** → opens `TranscodeDialog` with `mode="track"` and `sourceId=track.id`
- **Standardize art** → if art profiles exist, opens a mini-popover to pick a profile, then calls `artApi.standardize(track.id, profileId)`
- **Extract art** → directly calls `artApi.extract(track.id)`, shows a toast on success

Album header row (when grouping by Album): add:
- **Transcode album** → opens `TranscodeDialog` with `mode="library"` for the album's library

Library toolbar (top of track list):
- **Transcode library** → opens `TranscodeDialog` with `mode="library"` for current library
- **Standardize all art** → calls `artApi.standardizeLibrary(libraryId, profileId)` after profile selection

**Step 4: Build and verify in browser**
```bash
docker compose up --build -d
# Navigate to /library
# Add a library with an encoding profile set as target
# Click Transcode on a track → dialog opens, pick target library, click Transcode all
# Navigate to /jobs → transcode job should appear as pending
# Check logs: docker compose logs -f app → ffmpeg should run when scheduler picks up job
```

**Step 5: Update `CHANGELOG.md`**

```markdown
## [v0.4.0] — 2026-04-20

### Added
- Extended ingest format support: WavPack (.wv), Monkey's Audio (.ape), TrueAudio (.tta)
- CUE+FLAC sheet splitting — scanner detects paired CUE+audio files, splits into individual
  tracks via ffmpeg -c copy, writes CUE metadata via lofty; idempotent on re-scan
- Encoding profiles — configurable codec, bitrate, sample rate, channels, advanced ffmpeg args
- Art profiles — max dimensions, size limit, JPEG/PNG format, quality setting
- Track links — records source→derived relationships for transcoded tracks
- Transcode job — ffmpeg pipeline from encoding profile, tag copy, track_links row creation
- Art process job — embed (from URL), extract, standardize (resize/recompress via image crate)
- Auto-transcode on ingest — child libraries with auto_transcode_on_ingest=true receive jobs
- Auto-embed art on suggestion accept — art_process job enqueued when suggestion has cover_art_url
- Transcode API: per-track, per-library bulk, and sync-missing modes
- Art API: per-track embed/extract/standardize; per-library standardize
- Settings UI: encoding profiles and art profiles management with inline forms
- Library UI: transcode dialog (all / sync) and art standardize actions on tracks, albums, library
```

**Step 6: Commit**
```bash
git add ui/src/api/transcode.ts ui/src/api/art.ts ui/src/components/TranscodeDialog.tsx ui/src/pages/LibraryPage.tsx CHANGELOG.md tasks/codebase-filemap.md
git commit -m "feat(4.12): transcode + art UI, TranscodeDialog, library actions + CHANGELOG"
```

**Step 7: Tag the release**
```bash
git tag v0.4.0
```

---

## Summary

| Task | Output | Commit |
|------|--------|--------|
| 1 | Extended ingest formats (WavPack, APE, TrueAudio) | `feat(4.1)` |
| 2 | Encoding profiles — migration, model, DAL | `feat(4.2)` |
| 3 | Art profiles — migration, model, DAL | `feat(4.3)` |
| 4 | Track links — migration, model, DAL | `feat(4.4)` |
| 5 | CUE sheet parser | `feat(4.5)` |
| 6 | CUE split — scanner detection + job handler | `feat(4.6)` |
| 7 | Transcode job — ffmpeg pipeline + track_links | `feat(4.7)` |
| 8 | Art process job — embed/extract/standardize + auto-embed | `feat(4.8)` |
| 9 | Encoding profiles + art profiles REST API | `feat(4.9)` |
| 10 | Transcode + art APIs + auto-transcode wiring | `feat(4.10)` |
| 11 | Settings UI — encoding + art profile management | `feat(4.11)` |
| 12 | Library UI — transcode + art actions + release | `feat(4.12)` |
