# Library Model Redesign & Ingest Workflow — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace parent-child library DAG with `library_profiles`, introduce non-destructive ingest workflow with `process_staged` job, and add MusicBrainz text-search fallback before FreeDB.

**Architecture:** Schema migrations first; then model structs, DAL, scanner, jobs, API, UI in dependency order. Each task ends with a Docker build verification and a commit. Per lessons, all build verification via `docker buildx build` — no local cargo/npm.

**Tech Stack:** Rust/Axum, SQLx (Postgres + SQLite), React/Vite/Tailwind.

**Branch:** Work on the active phase branch (e.g. `0.3`). Do not commit to `main`.

**Design reference:** `docs/plans/2026-04-21-library-ingest-redesign.md`

---

## Build verification command (used at end of every task)

```bash
docker buildx build --progress=plain -t suzuran:dev .
```

Expected: migrations apply, Rust compiles, no errors.

---

### Task 1: DB Migration 0021 — library_profiles + library column cleanup

**Files:**
- Create: `migrations/postgres/0021_library_profiles.sql`
- Create: `migrations/sqlite/0021_library_profiles.sql`

**Current libraries columns to drop:** `parent_library_id`, `encoding_profile_id`, `auto_transcode_on_ingest`, `normalize_on_ingest` (added in 0016). Note: `ingest_dir` does not exist in the current schema — skip it.

**Step 1: Write Postgres migration**

```sql
-- migrations/postgres/0021_library_profiles.sql

-- Remove columns replaced by library_profiles join table
ALTER TABLE libraries
    DROP COLUMN IF EXISTS parent_library_id,
    DROP COLUMN IF EXISTS encoding_profile_id,
    DROP COLUMN IF EXISTS auto_transcode_on_ingest,
    DROP COLUMN IF EXISTS normalize_on_ingest;

DROP INDEX IF EXISTS libraries_parent_library_id_idx;

-- New table: one row per derived format per library
CREATE TABLE library_profiles (
    id                   BIGSERIAL PRIMARY KEY,
    library_id           BIGINT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    encoding_profile_id  BIGINT NOT NULL REFERENCES encoding_profiles(id) ON DELETE RESTRICT,
    derived_dir_name     TEXT NOT NULL,
    include_on_submit    BOOLEAN NOT NULL DEFAULT TRUE,
    auto_include_above_hz INTEGER NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (library_id, derived_dir_name)
);

-- Data migration: existing child libraries → library_profiles entries
-- (library_id = parent_library_id; derived_dir_name = last path component of child root_path)
-- If no child libraries exist this is a no-op.
-- Note: child libraries themselves are NOT deleted here; that cleanup is manual post-verification.
```

**Step 2: Write SQLite migration**

SQLite cannot drop columns with FK references via ALTER TABLE on older engines; recreate the table:

```sql
-- migrations/sqlite/0021_library_profiles.sql
PRAGMA foreign_keys=OFF;

CREATE TABLE libraries_new (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    name                    TEXT NOT NULL,
    root_path               TEXT NOT NULL UNIQUE,
    format                  TEXT NOT NULL,
    scan_enabled            INTEGER NOT NULL DEFAULT 1,
    scan_interval_secs      INTEGER NOT NULL DEFAULT 3600,
    auto_organize_on_ingest INTEGER NOT NULL DEFAULT 0,
    tag_encoding            TEXT NOT NULL DEFAULT 'utf8',
    organization_rule_id    INTEGER REFERENCES organization_rules(id),
    created_at              TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO libraries_new
    SELECT id, name, root_path, format, scan_enabled, scan_interval_secs,
           auto_organize_on_ingest, tag_encoding, organization_rule_id, created_at
    FROM libraries;

DROP TABLE libraries;
ALTER TABLE libraries_new RENAME TO libraries;

CREATE TABLE library_profiles (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id            INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    encoding_profile_id   INTEGER NOT NULL REFERENCES encoding_profiles(id) ON DELETE RESTRICT,
    derived_dir_name      TEXT NOT NULL,
    include_on_submit     INTEGER NOT NULL DEFAULT 1,
    auto_include_above_hz INTEGER NULL,
    created_at            TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (library_id, derived_dir_name)
);

PRAGMA foreign_keys=ON;
```

**Step 3: Build to verify**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```
Expected: migrations 0001–0021 apply without error (compilation errors will appear because Rust models still reference dropped columns — that is expected and will be fixed in Task 4).

**Step 4: Commit**
```bash
git add migrations/postgres/0021_library_profiles.sql migrations/sqlite/0021_library_profiles.sql
git commit -m "feat: migration 0021 — library_profiles table, drop parent-child columns from libraries"
```

---

### Task 2: DB Migration 0022 — tracks.status + tracks.library_profile_id

**Files:**
- Create: `migrations/postgres/0022_tracks_ingest_columns.sql`
- Create: `migrations/sqlite/0022_tracks_ingest_columns.sql`

**Step 1: Write Postgres migration**

```sql
-- migrations/postgres/0022_tracks_ingest_columns.sql

ALTER TABLE tracks
    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('staged', 'active', 'removed')),
    ADD COLUMN IF NOT EXISTS library_profile_id BIGINT NULL
        REFERENCES library_profiles(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_tracks_status ON tracks(status);
CREATE INDEX IF NOT EXISTS idx_tracks_library_profile_id ON tracks(library_profile_id);
```

**Step 2: Write SQLite migration**

SQLite supports `ADD COLUMN` for non-constrained columns; the CHECK constraint can be added inline:

```sql
-- migrations/sqlite/0022_tracks_ingest_columns.sql

ALTER TABLE tracks ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('staged', 'active', 'removed'));
ALTER TABLE tracks ADD COLUMN library_profile_id INTEGER NULL
    REFERENCES library_profiles(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_tracks_status ON tracks(status);
CREATE INDEX IF NOT EXISTS idx_tracks_library_profile_id ON tracks(library_profile_id);
```

**Step 3: Build, commit**
```bash
docker buildx build --progress=plain -t suzuran:dev .
git add migrations/
git commit -m "feat: migration 0022 — tracks.status (staged/active/removed) and tracks.library_profile_id"
```

---

### Task 3: DB Migration 0023 — track_links simplification, vls redesign, process_staged job type, settings seed

**Files:**
- Create: `migrations/postgres/0023_redesign_remaining.sql`
- Create: `migrations/sqlite/0023_redesign_remaining.sql`

**Step 1: Write Postgres migration**

```sql
-- migrations/postgres/0023_redesign_remaining.sql

-- 1. track_links: drop encoding_profile_id (now redundant via library_profile_id on derived track)
ALTER TABLE track_links DROP COLUMN IF EXISTS encoding_profile_id;

-- 2. virtual_library_sources: add surrogate id + library_profile_id; replace composite PK
ALTER TABLE virtual_library_sources
    ADD COLUMN id BIGSERIAL,
    ADD COLUMN library_profile_id BIGINT NULL
        REFERENCES library_profiles(id) ON DELETE CASCADE;

ALTER TABLE virtual_library_sources DROP CONSTRAINT virtual_library_sources_pkey;
ALTER TABLE virtual_library_sources ADD PRIMARY KEY (id);

DROP INDEX IF EXISTS idx_vls_priority;
CREATE UNIQUE INDEX idx_vls_unique
    ON virtual_library_sources(virtual_library_id, library_id, library_profile_id);
CREATE INDEX idx_vls_priority
    ON virtual_library_sources(virtual_library_id, priority);

-- 3. jobs: add process_staged to CHECK (follow pattern from 0019)
ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_job_type_check;
ALTER TABLE jobs ADD CONSTRAINT jobs_job_type_check CHECK (job_type IN (
    'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
    'transcode', 'art_process', 'organize', 'cue_split',
    'normalize', 'virtual_sync', 'process_staged'
));

-- 4. settings: seed folder_art_filename
INSERT INTO settings (key, value)
    VALUES ('folder_art_filename', 'folder.jpg')
    ON CONFLICT (key) DO NOTHING;
```

**Step 2: Write SQLite migration**

Follow the table-recreation pattern from `migrations/sqlite/0019_jobs_add_virtual_sync.sql`.

```sql
-- migrations/sqlite/0023_redesign_remaining.sql
PRAGMA foreign_keys=OFF;

-- track_links: drop encoding_profile_id by recreating
CREATE TABLE track_links_new (
    source_track_id  INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    derived_track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (source_track_id, derived_track_id)
);
INSERT INTO track_links_new (source_track_id, derived_track_id, created_at)
    SELECT source_track_id, derived_track_id, created_at FROM track_links;
DROP TABLE track_links;
ALTER TABLE track_links_new RENAME TO track_links;
CREATE INDEX idx_track_links_source  ON track_links(source_track_id);
CREATE INDEX idx_track_links_derived ON track_links(derived_track_id);

-- virtual_library_sources: add surrogate id + library_profile_id
CREATE TABLE virtual_library_sources_new (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    virtual_library_id INTEGER NOT NULL REFERENCES virtual_libraries(id) ON DELETE CASCADE,
    library_id         INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    library_profile_id INTEGER NULL REFERENCES library_profiles(id) ON DELETE CASCADE,
    priority           INTEGER NOT NULL DEFAULT 0,
    UNIQUE (virtual_library_id, library_id, library_profile_id)
);
INSERT INTO virtual_library_sources_new (virtual_library_id, library_id, priority)
    SELECT virtual_library_id, library_id, priority FROM virtual_library_sources;
DROP TABLE virtual_library_sources;
ALTER TABLE virtual_library_sources_new RENAME TO virtual_library_sources;
CREATE INDEX idx_vls_priority ON virtual_library_sources(virtual_library_id, priority);

-- jobs: add process_staged (copy-recreate pattern from 0019)
CREATE TABLE jobs_new (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type     TEXT NOT NULL CHECK (job_type IN (
                     'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
                     'transcode', 'art_process', 'organize', 'cue_split',
                     'normalize', 'virtual_sync', 'process_staged'
                 )),
    status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
                     'pending', 'running', 'completed', 'failed', 'cancelled'
                 )),
    payload      TEXT NOT NULL DEFAULT '{}',
    result       TEXT,
    priority     INTEGER NOT NULL DEFAULT 0,
    attempts     INTEGER NOT NULL DEFAULT 0,
    error        TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    started_at   TEXT,
    completed_at TEXT
);
INSERT INTO jobs_new SELECT * FROM jobs;
DROP TABLE jobs;
ALTER TABLE jobs_new RENAME TO jobs;
CREATE INDEX jobs_status_priority_idx ON jobs(status, priority DESC, created_at ASC);
CREATE INDEX jobs_job_type_status_idx ON jobs(job_type, status);

PRAGMA foreign_keys=ON;

-- settings: seed folder_art_filename
INSERT OR IGNORE INTO settings (key, value) VALUES ('folder_art_filename', 'folder.jpg');
```

**Step 3: Build, commit**
```bash
docker buildx build --progress=plain -t suzuran:dev .
git add migrations/
git commit -m "feat: migration 0023 — track_links simplification, vls surrogate id + library_profile_id, process_staged job type, folder_art_filename setting"
```

---

### Task 4: Rust Model Structs

**Files:**
- Modify: `src/models/mod.rs`

**Step 1: Update `Library` struct**

Remove fields: `parent_library_id: Option<i64>`, `encoding_profile_id: Option<i64>`, `auto_transcode_on_ingest: bool`, `normalize_on_ingest: bool`.

Remaining fields: `id`, `name`, `root_path`, `format`, `scan_enabled`, `scan_interval_secs`, `auto_organize_on_ingest`, `tag_encoding`, `organization_rule_id`, `created_at`.

**Step 2: Add `LibraryProfile` + `UpsertLibraryProfile` structs**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LibraryProfile {
    pub id: i64,
    pub library_id: i64,
    pub encoding_profile_id: i64,
    pub derived_dir_name: String,
    pub include_on_submit: bool,
    pub auto_include_above_hz: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertLibraryProfile {
    pub library_id: i64,
    pub encoding_profile_id: i64,
    pub derived_dir_name: String,
    pub include_on_submit: bool,
    pub auto_include_above_hz: Option<i64>,
}
```

**Step 3: Update `Track` struct** — add fields:
```rust
pub status: String,               // "staged" | "active" | "removed"
pub library_profile_id: Option<i64>,
```

**Step 4: Update `TrackLink` struct** — remove `encoding_profile_id: Option<i64>`.

**Step 5: Update `VirtualLibrarySource` struct** — add `pub id: i64`, add `pub library_profile_id: Option<i64>`.

**Step 6: Update `UpsertTrack` DTO** (in `src/dal/mod.rs`) — add:
```rust
pub status: String,               // default "active"
pub library_profile_id: Option<i64>,
```

**Step 7: Build (expect compile errors at DAL/job callsites — note them for Tasks 5–8)**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 8: Commit**
```bash
git add src/models/mod.rs src/dal/mod.rs
git commit -m "feat: Rust models — LibraryProfile struct, Track status/library_profile_id, simplified TrackLink, VirtualLibrarySource surrogate id"
```

---

### Task 5: DAL — Store Trait + Implementations

**Files:**
- Modify: `src/dal/mod.rs`
- Modify: `src/dal/postgres.rs`
- Modify: `src/dal/sqlite.rs`

**Step 1: Update `Store` trait in `src/dal/mod.rs`**

Remove methods: `list_child_libraries`, `set_library_encoding_profile`.

Remove `normalize_on_ingest` parameter from `update_library`.

Update `create_track_link` signature: remove `encoding_profile_id` param → just `(source_track_id: i64, derived_track_id: i64)`.

Update `set_virtual_library_sources` input type: `Vec<(i64, i64, Option<i64>)>` = `(virtual_library_id, library_id, library_profile_id)` + a `priority` field. Suggest a struct:
```rust
pub struct VirtualLibrarySourceInput {
    pub library_id: i64,
    pub library_profile_id: Option<i64>,
    pub priority: i32,
}
```

Add new methods:
```rust
async fn create_library_profile(&self, p: &UpsertLibraryProfile) -> Result<LibraryProfile, AppError>;
async fn get_library_profile(&self, id: i64) -> Result<LibraryProfile, AppError>;
async fn list_library_profiles(&self, library_id: i64) -> Result<Vec<LibraryProfile>, AppError>;
async fn update_library_profile(&self, id: i64, p: &UpsertLibraryProfile) -> Result<LibraryProfile, AppError>;
async fn delete_library_profile(&self, id: i64) -> Result<(), AppError>;
async fn set_track_status(&self, id: i64, status: &str) -> Result<(), AppError>;
async fn list_tracks_by_status(&self, library_id: i64, status: &str) -> Result<Vec<Track>, AppError>;
async fn list_tracks_by_profile(&self, library_id: i64, library_profile_id: Option<i64>) -> Result<Vec<Track>, AppError>;
```

**Step 2: Implement new methods in `src/dal/postgres.rs`**

- `create_library_profile`: `INSERT INTO library_profiles (...) VALUES ($1,$2,$3,$4,$5) RETURNING *`
- `get_library_profile`: `SELECT * FROM library_profiles WHERE id=$1` — return `AppError::NotFound` if missing
- `list_library_profiles`: `SELECT * FROM library_profiles WHERE library_id=$1 ORDER BY id`
- `update_library_profile`: `UPDATE library_profiles SET ... WHERE id=$1 RETURNING *`
- `delete_library_profile`: `DELETE FROM library_profiles WHERE id=$1` — error if not found
- `set_track_status`: `UPDATE tracks SET status=$2 WHERE id=$1`
- `list_tracks_by_status`: `SELECT * FROM tracks WHERE library_id=$1 AND status=$2 ORDER BY id`
- `list_tracks_by_profile`: `SELECT * FROM tracks WHERE library_id=$1 AND library_profile_id IS NOT DISTINCT FROM $2 ORDER BY id`

Update existing queries:
- `list_tracks` (for `GET /libraries/:id/tracks`): add optional `status` filter; default to `status='active'` so ingest-staged tracks don't appear in library view.
- `upsert_track`: include `status` and `library_profile_id` columns.
- `create_track_link`: remove `encoding_profile_id` column from INSERT.
- `set_virtual_library_sources`: use new `VirtualLibrarySourceInput` struct; INSERT includes `library_profile_id`.
- `list_virtual_library_sources`: SELECT includes `id` and `library_profile_id`.
- `update_library`: remove `normalize_on_ingest` from SET clause.

**Step 3: Mirror all changes in `src/dal/sqlite.rs`** (same queries; SQLite uses `?` placeholders, booleans as INTEGER).

**Step 4: Fix tests that use removed fields**

- `tests/common/mod.rs`: update library/track fixture helpers; remove `encoding_profile_id`, `parent_library_id`, `auto_transcode_on_ingest`, `normalize_on_ingest` from library setup; add `status: "active".into()` to `UpsertTrack` defaults.
- `tests/virtual_libraries_dal.rs`: update `set_virtual_library_sources` calls to use new `VirtualLibrarySourceInput`.
- `tests/transcode_job.rs`: remove `encoding_profile_id` from `create_track_link` call.
- `tests/normalize_job.rs`: if `normalize_on_ingest` is gone from Library, update setup fixture.

**Step 5: Build**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 6: Commit**
```bash
git add src/dal/ tests/common/ tests/virtual_libraries_dal.rs tests/transcode_job.rs tests/normalize_job.rs
git commit -m "feat: DAL — library_profiles CRUD, track status queries, updated signatures (remove child library, encoding_profile_id)"
```

---

### Task 6: Scanner Update

**Files:**
- Modify: `src/scanner/mod.rs`
- Modify: `tests/scanner.rs`

**Step 1: Read `src/scanner/mod.rs` in full before editing.**

**Step 2: Update scan logic**

Replace single-directory walk with two distinct walks:

1. **Walk `{library.root_path}/ingest/`** — for new/staged tracks:
   - New file → `upsert_track` with `status: "staged"`, `relative_path` relative to `ingest/`, enqueue `fingerprint`.
   - File no longer present → update status to `"removed"` (do NOT delete record).
   - Skip CUE-backed audio detection (unchanged logic — CUE detection flags file but doesn't split).

2. **Walk `{library.root_path}/source/`** — for active tracks:
   - Hash change → update hash + path.
   - File removed → set `status = "removed"`.

Remove the block that calls `list_child_libraries` and enqueues `transcode` jobs (this is now done only after user submission via `process_staged`).

Remove `normalize_on_ingest` check from scanner (no longer exists on Library model).

**Step 3: Update `tests/scanner.rs`**

- Update existing test fixtures so "active" track files live under `{root_path}/source/`.
- Add test: file placed in `{root_path}/ingest/` → track created with `status="staged"`.
- Add test: file in `source/` with changed hash → track updated.
- Verify: scanner does NOT enqueue transcode jobs.

**Step 4: Build**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 5: Commit**
```bash
git add src/scanner/mod.rs tests/scanner.rs
git commit -m "feat: scanner — ingest/ subdirectory creates staged tracks, source/ monitors active tracks, remove auto-transcode"
```

---

### Task 7: MusicBrainz Service — search_recordings

**Files:**
- Modify: `src/services/musicbrainz.rs`
- Modify: `tests/musicbrainz_service.rs`

**Step 1: Read `src/services/musicbrainz.rs` to understand rate limiter and HTTP client setup.**

**Step 2: Add `search_recordings` method**

Endpoint: `GET https://musicbrainz.org/ws/2/recording/?query=recording:"{title}" AND artist:"{artist}" AND release:"{album}"&fmt=json&limit=5`

```rust
pub async fn search_recordings(
    &self,
    title: &str,
    artist: &str,
    album: &str,
) -> Result<Vec<(HashMap<String, String>, f64)>, AppError>
```

- Acquire the existing 1.1s rate limiter permit before the HTTP call.
- Parse `recordings` array from response JSON.
- For each recording, call existing `to_tag_map` to produce a tag HashMap.
- Return `(tags, confidence)` pairs with confidence capped at `0.6`.
- Up to 5 results.

**Step 3: Add wiremock test to `tests/musicbrainz_service.rs`**

Mock `GET /ws/2/recording/` returning a JSON body with one recording entry. Assert:
- `search_recordings("title", "artist", "album")` returns 1 result.
- Confidence ≤ 0.6.
- Tags map contains expected fields.

**Step 4: Build**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 5: Commit**
```bash
git add src/services/musicbrainz.rs tests/musicbrainz_service.rs
git commit -m "feat: MusicBrainzService::search_recordings — text search with confidence cap 0.6"
```

---

### Task 8: Job Handler Updates

**Files:**
- Modify: `src/jobs/mod.rs`
- Modify: `src/jobs/fingerprint.rs`
- Modify: `src/jobs/mb_lookup.rs`
- Modify: `src/jobs/transcode.rs`
- Modify: `src/jobs/virtual_sync.rs`
- Modify: `tests/fingerprint_job.rs`
- Modify: `tests/mb_lookup_job.rs`
- Modify: `tests/transcode_job.rs`
- Modify: `tests/virtual_sync_job.rs`

**Step 1: Read all four job handler files before editing.**

**Step 2: Update `src/jobs/mod.rs`**

Update `TranscodePayload`: replace `child_library_id: i64` + `encoding_profile_id: i64` with `library_profile_id: i64`.

Add `ProcessStagedPayload` (used in Task 9):
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessStagedPayload {
    pub track_id: i64,
    pub tag_suggestion_id: Option<i64>,
    pub cover_art_url: Option<String>,
    pub write_folder_art: bool,
    pub profile_ids: Vec<i64>,
}
```

**Step 3: Update `src/jobs/fingerprint.rs`**

Remove the `normalize_on_ingest` / profile codec check block. After fingerprint update, always enqueue `mb_lookup` — no conditional `normalize` enqueue. The `Library` struct no longer has `normalize_on_ingest`.

**Step 4: Update `src/jobs/mb_lookup.rs`**

New fallback chain (replace existing logic):
```
AcoustID results ≥ 0.8 → create tag_suggestions (source="acoustid")
AcoustID returns nothing →
  read track tags for title/artist/album
  call mb_service.search_recordings(title, artist, album)
    results found → create tag_suggestions (source="mb_search", confidence from result ≤ 0.6)
    no results + track has DISCID tag → enqueue freedb_lookup
```

Use existing `store.get_track(id)` to fetch the track; `track.tags` JSONB/TEXT provides title/artist/album. Parse `track.tags` as `HashMap<String,String>`.

**Step 5: Update `src/jobs/transcode.rs`**

Change payload handling: fetch `LibraryProfile` by `library_profile_id`. Get `encoding_profile_id` from profile. Get `derived_dir_name` from profile.

Output path: `{library.root_path}/{profile.derived_dir_name}/{relative_path_within_source}`.

Set `library_profile_id` on derived track upsert.

`create_track_link`: remove `encoding_profile_id` arg (signature changed in Task 5).

**Step 6: Update `src/jobs/virtual_sync.rs`**

`VirtualLibrarySource` now has `library_profile_id`. When building the identity→track map:
- `library_profile_id IS NULL` → call `list_tracks_by_profile(library_id, None)` to get source tracks (`status='active'`).
- `library_profile_id = Some(id)` → call `list_tracks_by_profile(library_id, Some(id))` to get derived tracks.

The identity dedup logic (first match per `track_identity` wins) is unchanged.

**Step 7: Update tests**

- `tests/fingerprint_job.rs`: assert `normalize` job is NOT enqueued; assert `mb_lookup` IS always enqueued.
- `tests/mb_lookup_job.rs`: add test for text-search fallback path (AcoustID returns 0 → MB search mock returns 1 result → suggestion created with source="mb_search"). Add test: AcoustID 0 + MB search 0 + DISCID present → `freedb_lookup` enqueued.
- `tests/transcode_job.rs`: update fixture to use `library_profile_id` in `TranscodePayload`; create `library_profiles` row in setup.
- `tests/virtual_sync_job.rs`: update source setup to use `VirtualLibrarySourceInput` with `library_profile_id`.

**Step 8: Build**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 9: Commit**
```bash
git add src/jobs/ tests/fingerprint_job.rs tests/mb_lookup_job.rs tests/transcode_job.rs tests/virtual_sync_job.rs
git commit -m "feat: job handlers — fingerprint always→mb_lookup, mb_lookup text-search fallback, transcode by library_profile, virtual_sync profile-aware"
```

---

### Task 9: New `process_staged` Job Handler

**Files:**
- Create: `src/jobs/process_staged.rs`
- Modify: `src/scheduler/mod.rs`
- Create: `tests/process_staged_job.rs`

**Step 1: Implement `ProcessStagedJobHandler`**

Implement `JobHandler` for `process_staged` type. Pipeline:

1. Parse `ProcessStagedPayload`. Fetch track. Assert `track.status == "staged"`. Fetch library.
2. If `tag_suggestion_id` is set: write approved tags to file using `tagger::write_tags` (same logic as `apply_suggestion` but inline — do NOT call `apply_suggestion` as it enqueues a separate `art_process` job which would race with file move). Use `store.update_track_tags(track_id, merged_tags)`.
3. If `cover_art_url` is set:
   a. Download bytes via `reqwest`.
   b. Embed art into file at `ingest/` path using `lofty` (same approach as `ArtProcessJobHandler` embed action).
   c. If `write_folder_art` is true and `folder_art_filename` setting is non-empty: write the art bytes to `{root_path}/source/{album_dir}/{folder_art_filename}` (create dir if needed).
4. Compute destination path: `source/{relative_path_within_ingest}`.
5. `tokio::fs::create_dir_all(dest_parent)`.
6. `tokio::fs::rename(src_path, dest_path)`.
7. Compute SHA-256 hash of file at dest path.
8. `store.update_track_path(track_id, dest_relative_path, new_hash)`.
9. `store.set_track_status(track_id, "active")`.
10. For each `profile_id` in `payload.profile_ids`: `store.enqueue_job("transcode", TranscodePayload { track_id, library_profile_id: profile_id }, 0)`.

Return JSON summary `{ "track_id": N, "profiles_enqueued": M }`.

**Step 2: Register in `src/scheduler/mod.rs`**

Register `ProcessStagedJobHandler` with concurrency=2, following the same pattern as `cue_split`.

**Step 3: Write `tests/process_staged_job.rs`**

Test cases (use `TAGGED_FLAC` fixture and temp dir):

```
test_process_staged_moves_file_to_source:
  setup: library with tmp root, create ingest/ and source/ dirs, write TAGGED_FLAC to ingest/album/test.flac
         create staged track record, create library_profiles entry
  action: run ProcessStagedJobHandler with profile_ids=[profile.id]
  assert: file exists at source/album/test.flac
  assert: file NOT at ingest/album/test.flac
  assert: track.status == "active"
  assert: track.relative_path updated
  assert: one transcode job in jobs table

test_process_staged_missing_track_returns_error:
  payload.track_id = 99999
  assert: AppError::NotFound (or job fails with error)

test_process_staged_already_active_returns_error:
  track.status = "active"
  assert: job returns error (track not staged)

test_process_staged_with_folder_art:
  setup: cover_art_url set (wiremock serving 1x1.png), write_folder_art=true
         folder_art_filename setting = "folder.jpg"
  assert: art file written to source/album/folder.jpg
```

**Step 4: Build**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 5: Commit**
```bash
git add src/jobs/process_staged.rs src/scheduler/mod.rs tests/process_staged_job.rs
git commit -m "feat: process_staged job — tag write, art embed, file move ingest→source, transcode enqueue"
```

---

### Task 10: API Layer Updates

**Files:**
- Create: `src/api/library_profiles.rs`
- Create: `src/api/ingest.rs`
- Create: `src/api/search.rs`
- Modify: `src/api/libraries.rs`
- Modify: `src/api/virtual_libraries.rs`
- Modify: `src/api/tag_suggestions.rs`
- Modify: `src/api/mod.rs`
- Create: `tests/library_profiles_api.rs`

**Step 1: `src/api/library_profiles.rs`**

Routes: mount at `/library-profiles`
- `GET /` with optional `?library_id=N` — calls `list_library_profiles(library_id)` (require `library_id` param; 400 if missing)
- `POST /` (AdminUser) — create → 201 + body
- `GET /:id` (AuthUser) — get or 404
- `PUT /:id` (AdminUser) — update or 404
- `DELETE /:id` (AdminUser) — delete → 204

Body struct `LibraryProfileBody` maps to `UpsertLibraryProfile`. Follow the pattern from `src/api/encoding_profiles.rs`.

**Step 2: `src/api/ingest.rs`**

Routes: mount at `/ingest`
- `GET /staged` (AuthUser) — returns all staged tracks across all libraries. Query: `SELECT t.* FROM tracks t WHERE t.status='staged' ORDER BY t.library_id, t.tags->>'album', t.id`. Return `Vec<Track>`.
- `POST /submit` (AuthUser) — parses `ProcessStagedPayload`-shaped body, enqueues `process_staged` job → 202 + job id.

**Step 3: `src/api/search.rs`**

Routes: mount at `/search`
- `POST /mb` (AuthUser) — body `{title: str, artist: str, album: str}` → calls `mb_service.search_recordings(...)` → returns Vec of candidates synchronously.
- `POST /freedb` (AuthUser) — body `{disc_id?: str, artist?: str, album?: str}` → calls `freedb_service` appropriately → returns candidates.

**Step 4: Update `src/api/tag_suggestions.rs`**

Add `POST /` (AuthUser) — create a new tag_suggestion from body `{track_id, source, suggested_tags, confidence}`. This supports the manual search dialog creating a suggestion. Returns 201 + created row.

**Step 5: Update `src/api/libraries.rs`**

- Remove `normalize_on_ingest` from `PUT /:id` body and handler.
- Update `GET /:id/tracks`: add optional `?status=` query param. Default to `status=active` if not provided (staged tracks should NOT appear in library view). Pass to `store.list_tracks_by_status`.

**Step 6: Update `src/api/virtual_libraries.rs`**

`PUT /:id/sources` body: change from `Vec<{library_id, priority}>` to `Vec<{library_id, library_profile_id?, priority}>`. Map to `VirtualLibrarySourceInput` for the store call.

**Step 7: Mount in `src/api/mod.rs`**

```rust
.nest("/library-profiles", library_profiles::router())
.nest("/ingest", ingest::router())
.nest("/search", search::router())
```

**Step 8: Write `tests/library_profiles_api.rs`**

Follow `tests/encoding_profiles_api.rs` pattern:
- Create → 201 with body
- List by library_id → contains created entry
- Get by id → 200
- Update → 200
- Delete → 204
- Auth guards: unauthenticated → 401, non-admin POST → 403

**Step 9: Build**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 10: Commit**
```bash
git add src/api/ tests/library_profiles_api.rs
git commit -m "feat: API — library_profiles CRUD, ingest staged/submit, search MB/FreeDB, tag_suggestion create, track status filter"
```

---

### Task 11: UI — Types, API Client, Library Form Profiles Section

**Files:**
- Create: `ui/src/types/libraryProfile.ts`
- Create: `ui/src/api/libraryProfiles.ts`
- Modify: `ui/src/api/libraries.ts`
- Modify: `ui/src/types/virtualLibrary.ts`
- Modify: `ui/src/api/virtualLibraries.ts`
- Modify: `ui/src/components/LibraryFormModal.tsx`

**Step 1: Read `ui/src/components/LibraryFormModal.tsx` before editing.**

**Step 2: `ui/src/types/libraryProfile.ts`**

```typescript
export interface LibraryProfile {
  id: number;
  library_id: number;
  encoding_profile_id: number;
  derived_dir_name: string;
  include_on_submit: boolean;
  auto_include_above_hz: number | null;
  created_at: string;
}

export interface UpsertLibraryProfile {
  library_id: number;
  encoding_profile_id: number;
  derived_dir_name: string;
  include_on_submit: boolean;
  auto_include_above_hz: number | null;
}
```

**Step 3: `ui/src/api/libraryProfiles.ts`** — CRUD functions. Mirror `ui/src/api/encodingProfiles.ts` pattern:
- `listLibraryProfiles(libraryId: number)` → `GET /library-profiles?library_id={libraryId}`
- `createLibraryProfile(data: UpsertLibraryProfile)` → `POST /library-profiles`
- `updateLibraryProfile(id, data)` → `PUT /library-profiles/{id}`
- `deleteLibraryProfile(id)` → `DELETE /library-profiles/{id}`

**Step 4: Update `ui/src/api/libraries.ts`**

Remove from `Library` interface and `UpdateLibraryInput`: `parent_library_id`, `encoding_profile_id`, `auto_transcode_on_ingest`, `normalize_on_ingest`.

Remaining: `id`, `name`, `root_path`, `format`, `scan_enabled`, `scan_interval_secs`, `auto_organize_on_ingest`, `tag_encoding`, `organization_rule_id`, `created_at`.

**Step 5: Update `VirtualLibrarySource` type** in `ui/src/types/virtualLibrary.ts`:
Add `library_profile_id: number | null`.
Update `SourcePriorityList` component if it passes source data to `PUT /:id/sources` — add `library_profile_id` field.

**Step 6: Update `ui/src/components/LibraryFormModal.tsx`**

Replace the single encoding profile dropdown with a **Profiles** section:

- On mount (edit mode): fetch `listLibraryProfiles(library.id)` → display existing profiles.
- Each profile row (inline editable or separate modal):
  - `derived_dir_name` text input
  - Encoding profile select (fetch from `listEncodingProfiles()`)
  - `include_on_submit` toggle
  - `auto_include_above_hz` number input — show only when the selected encoding profile has codec `flac` (lossless-to-lossless scenario)
  - Delete button → call `deleteLibraryProfile(id)` and remove from list
- Add Profile button → creates a new empty row; on save calls `createLibraryProfile`.
- Remove `normalize_on_ingest` toggle.
- Ensure `scan_enabled` toggle and `scan_interval_secs` input are present and functional (may already exist; verify).

**Step 7: Commit**
```bash
git add ui/src/types/libraryProfile.ts ui/src/api/libraryProfiles.ts ui/src/api/libraries.ts ui/src/types/virtualLibrary.ts ui/src/api/virtualLibraries.ts ui/src/components/LibraryFormModal.tsx
git commit -m "feat(ui): library form profiles section, type cleanup for redesign"
```

---

### Task 12: UI — Library View Updates

**Files:**
- Modify: `ui/src/pages/LibraryPage.tsx`
- Possibly: `ui/src/components/TopNav.tsx` (wherever the library toolbar label lives)

**Step 1: Read `ui/src/pages/LibraryPage.tsx` before editing.**

**Step 2: Library name in toolbar**

Replace hardcoded "Library #N" label with `library.name` from fetched library object.

**Step 3: Scan button**

Add a **Scan** button to the library toolbar. On click: `POST /api/v1/jobs/scan` with body `{ library_id: library.id }`. Show a brief toast/indicator on success.

Note: verify that `ScanPayload` in `src/jobs/mod.rs` already includes `library_id`. If not, add it and update `src/api/jobs.rs` accordingly.

**Step 4: Active track list**

The `GET /libraries/:id/tracks` endpoint now defaults to `status=active` (Task 10). No client-side filtering needed. Remove any stub placeholder. Ensure grouping (None/Album/Artist/Genre/Year) and sort controls are wired up.

**Step 5: Commit**
```bash
git add ui/src/pages/LibraryPage.tsx
git commit -m "feat(ui): library view — library name in toolbar, scan button, active-only track list"
```

---

### Task 13: UI — Ingest Section (replaces Inbox)

**Files:**
- Create: `ui/src/pages/IngestPage.tsx`
- Modify: `ui/src/App.tsx` (or wherever routes are defined — update Inbox → Ingest route)
- Modify: `ui/src/components/TopNav.tsx` (rename nav link)
- Create: `ui/src/api/ingest.ts`

**Step 1: Read `ui/src/pages/InboxPage.tsx` before writing IngestPage.**

**Step 2: `ui/src/api/ingest.ts`**

```typescript
export const getStagedTracks = () => client.get<Track[]>('/ingest/staged');
export const submitTrack = (payload: ProcessStagedPayload) =>
  client.post<{ job_id: number }>('/ingest/submit', payload);

interface ProcessStagedPayload {
  track_id: number;
  tag_suggestion_id?: number;
  cover_art_url?: string;
  write_folder_art: boolean;
  profile_ids: number[];
}
```

**Step 3: `ui/src/pages/IngestPage.tsx`**

Layout: full-width, no sidebar tree pane.

Data loading:
- `GET /ingest/staged` → staged tracks
- `GET /tag-suggestions?status=pending` → suggestions by track_id
- `GET /library-profiles` (per library as needed) → for submission dialog profiles checklist

Group tracks by `track.tags.album` (or "Unknown Album" fallback).

Per-album:
- Header: album title, track count, format badge
- Art preview: first accepted suggestion's `cover_art_url` thumbnail (if any)
- Per-track row: `TagDiffTable` with current tags vs suggestion tags, confidence badge, source label
- Per-track actions: **Accept** (`POST /tag-suggestions/:id/accept`), **Edit** (inline tag edit), **Reject** (`POST /tag-suggestions/:id/reject`), **Search** (opens `IngestSearchDialog` — Task 14)

Batch action bar at top: "Accept all ≥ N%" input + button → `POST /tag-suggestions/batch-accept {min_confidence: N/100}`.

**Submit** button per album → opens pre-flight dialog:
- Tags summary (shows what will be written)
- Art panel: suggested art thumbnail | Upload (`ImageUpload` component) | Skip
- CUE split checkbox (only show if track is CUE-backed — detect via `track.tags.cue_backed` or similar flag to be determined)
- Profiles checklist: fetch `listLibraryProfiles(library_id)`, pre-select entries where:
  - `include_on_submit=true` AND (`auto_include_above_hz` is null OR track's sample_rate ≥ `auto_include_above_hz`)
  - User can check/uncheck
- Confirm → call `submitTrack(payload)` → show job queued notification

Update route in App.tsx: `/inbox` → `/ingest`. Update TopNav: "Inbox" → "Ingest".

**Step 4: Commit**
```bash
git add ui/src/pages/IngestPage.tsx ui/src/api/ingest.ts ui/src/App.tsx ui/src/components/TopNav.tsx
git commit -m "feat(ui): Ingest section — staged track list, album grouping, tag diff, submission pre-flight dialog"
```

---

### Task 14: UI — Settings General Tab + Manual Search Dialog

**Files:**
- Modify: `ui/src/pages/SettingsPage.tsx`
- Create: `ui/src/components/IngestSearchDialog.tsx`

**Step 1: Read `ui/src/pages/SettingsPage.tsx` General tab section before editing.**

**Step 2: Add `folder_art_filename` field**

In the General tab, following the existing per-field save pattern (7 settings already present), add:
- Label: "Folder art filename"
- Text input bound to `folder_art_filename` setting key
- Save button → `PUT /settings/folder_art_filename`
- Helper text: "Written alongside audio files in source/. Leave empty to disable. Default: folder.jpg"

**Step 3: `ui/src/components/IngestSearchDialog.tsx`**

Modal with two tabs: **MusicBrainz** | **FreeDB**.

MusicBrainz tab:
- Title, Artist, Album inputs (pre-populated from `track.tags`)
- Search button → `POST /search/mb {title, artist, album}` → display candidates
- Each candidate row: track title, artist, album, release year, confidence badge
- Select button → `POST /tag-suggestions {track_id, source: "mb_search", suggested_tags: candidate.tags, confidence: candidate.confidence}` → refetch suggestions → close dialog

FreeDB tab:
- Disc ID input (pre-populated from `track.tags.DISCID` if set)
- Artist + Album inputs for text search
- Search button → `POST /search/freedb {disc_id?, artist?, album?}` → display candidates
- Same select → create tag_suggestion flow

Props: `{ track: Track, libraryId: number, onClose: () => void }`.

**Step 4: Wire `IngestSearchDialog` into `IngestPage.tsx`**

When user clicks **Search** on a track row, open `IngestSearchDialog` with that track. On dialog close, refresh tag suggestions.

**Step 5: Build**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 6: Commit**
```bash
git add ui/src/pages/SettingsPage.tsx ui/src/components/IngestSearchDialog.tsx ui/src/pages/IngestPage.tsx
git commit -m "feat(ui): settings folder_art_filename field, manual search dialog (MB + FreeDB tabs)"
```

---

### Task 15: Admin — Filesystem Migration Endpoint

**Files:**
- Create: `src/api/migrate.rs`
- Modify: `src/api/mod.rs`

This provides the user-triggered migration that moves existing active source files from `{root_path}/` to `{root_path}/source/` for libraries that were using the old flat layout.

**Step 1: Implement `POST /admin/migrate-library-files/:library_id`** (AdminUser required)

```rust
// Algorithm:
// 1. Fetch library by id.
// 2. list_tracks_by_status(library_id, "active") → tracks with library_profile_id IS NULL
// 3. For each source track (library_profile_id IS NULL):
//    old_abs = library.root_path / track.relative_path
//    if old_abs starts with root_path/source/ → skip (already migrated)
//    new_rel = "source/" + track.relative_path
//    new_abs = library.root_path / new_rel
//    tokio::fs::create_dir_all(new_abs.parent())
//    tokio::fs::rename(old_abs, new_abs)  ← hard-link not rename across mounts; check same FS
//    rehash new_abs
//    store.update_track_path(track.id, new_rel, new_hash)
// 4. Return { moved: N, skipped: M, errors: [{ track_id, message }] }
```

**Step 2: Mount in `src/api/mod.rs`**
```rust
.nest("/admin", migrate::router())
```

**Step 3: Build**
```bash
docker buildx build --progress=plain -t suzuran:dev .
```

**Step 4: Commit**
```bash
git add src/api/migrate.rs src/api/mod.rs
git commit -m "feat: admin POST /admin/migrate-library-files/:id — moves source files to source/ subdirectory"
```

---

### Task 16: Codebase Filemap Update

**File:** `tasks/codebase-filemap.md`

Update every entry that changed:
- `src/models/mod.rs`: add `LibraryProfile`, `UpsertLibraryProfile`; update `Library` (remove dropped fields); update `Track` (add status, library_profile_id); update `TrackLink` (remove encoding_profile_id); update `VirtualLibrarySource` (add id, library_profile_id)
- `src/dal/mod.rs`: add library_profiles CRUD methods, `set_track_status`, `list_tracks_by_status`, `list_tracks_by_profile`; remove `list_child_libraries`, `set_library_encoding_profile`; update `create_track_link`, `set_virtual_library_sources`, `update_library`
- `src/scanner/mod.rs`: update description (ingest/ + source/ dirs, staged status, no auto-transcode)
- `src/services/musicbrainz.rs`: add `search_recordings` method
- `src/jobs/fingerprint.rs`: remove normalize_on_ingest branch
- `src/jobs/mb_lookup.rs`: add text search fallback
- `src/jobs/transcode.rs`: update to use library_profile_id
- `src/jobs/virtual_sync.rs`: update for profile-aware source selection
- `src/jobs/process_staged.rs`: new entry
- `src/api/library_profiles.rs`: new entry (CRUD)
- `src/api/ingest.rs`: new entry (staged list, submit)
- `src/api/search.rs`: new entry (MB search, FreeDB search)
- `src/api/migrate.rs`: new entry
- `src/api/libraries.rs`: update (status filter on tracks)
- `src/api/virtual_libraries.rs`: update (library_profile_id in sources)
- `src/api/tag_suggestions.rs`: add POST / create
- Migrations: add 0021, 0022, 0023 entries

**Commit:**
```bash
git add tasks/codebase-filemap.md
git commit -m "docs: update codebase filemap for library-ingest redesign (tasks 1-15)"
```

---

## Final Build Verification

```bash
docker buildx build --progress=plain -t suzuran:dev .
docker compose up --build -d
docker compose logs -f app
```

Expected: migrations 0001–0023 apply cleanly, app starts, no panics in logs.
