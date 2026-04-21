# Library/Ingest Redesign — Implementation Summary

**Date:** 2026-04-21  
**Branch:** `0.5` (merged to `main`)  
**Plan:** `docs/plans/2026-04-21-library-ingest-redesign-impl.md`  
**Design spec:** `docs/plans/2026-04-21-library-ingest-redesign.md`

---

## What Was Implemented

### Batch A — DB Migrations (Tasks 1–3)

Migrations 0025–0028 (both Postgres and SQLite):

- **0025** — Drops `parent_library_id`, `encoding_profile_id`, `auto_transcode_on_ingest`, `normalize_on_ingest` from `libraries`; creates `library_profiles` join table
- **0026** — Adds `tracks.status` CHECK(staged/active/removed) and `tracks.library_profile_id` FK
- **0027** — Drops `track_links.encoding_profile_id`; adds surrogate `id` + `library_profile_id` to `virtual_library_sources`; adds `process_staged` to jobs CHECK; seeds `folder_art_filename` setting
- **0028** — Drops `libraries.ingest_dir` (gap fix: was missed in initial plan)

*Note: Plan listed migrations as 0021–0023 but existing migrations ran to 0024; corrected to 0025–0027 before writing any code.*

### Batch B — Rust Models + DAL (Tasks 4–5)

- `Library` struct: removed 5 deprecated fields
- Added `LibraryProfile` + `UpsertLibraryProfile` structs
- `Track`: added `status: String`, `library_profile_id: Option<i64>`
- `TrackLink`: removed `encoding_profile_id`
- `VirtualLibrarySource`: added `id: i64`, `library_profile_id: Option<i64>`
- `UpsertTrack`: added `status`, `library_profile_id`; explicit `Default` impl
- `Store` trait: removed `list_child_libraries`, `set_library_encoding_profile`, `set_library_ingest_dir`; added 8 new methods (library_profiles CRUD, `set_track_status`, `list_tracks_by_status`, `list_tracks_by_profile`); updated 4 existing signatures
- Implementations in `postgres.rs` and `sqlite.rs` updated throughout

### Batch C — Scanner, MusicBrainz, Jobs (Tasks 6–9)

- **Scanner**: two-pass walk — `ingest/` creates staged tracks, `source/` monitors active tracks; removed auto-transcode logic
- **MusicBrainzService**: added `search_recordings(title, artist, album)` — up to 5 results, confidence capped at 0.6, uses existing 1.1s rate limiter
- **FreedBService**: added `text_search(artist, album)` — gnudb.org HTML scraping, HashSet dedup for disc IDs
- **Fingerprint job**: always enqueues `mb_lookup`; removed `normalize_on_ingest` check
- **mb_lookup job**: three-tier fallback — AcoustID ≥0.8 → suggestions; AcoustID 0 → MB text search; text search 0 + DISCID → freedb_lookup
- **Transcode job**: uses `library_profile_id` to fetch encoding profile and derived dir
- **Virtual sync job**: profile-aware source selection via `list_tracks_by_profile`
- **process_staged job** (new): writes tags, embeds art, writes folder art, moves `ingest/` → `source/`, updates track path+status, enqueues transcode per profile
- **cue_split job**: writes output to `source/`, removes original CUE+audio after split

*Note: Implementer subagent gutted `normalize.rs` to a stub. Spec reviewer caught it. Restored full ffmpeg logic, adapted to read `encoding_profile_id` from payload instead of removed library field.*

### Batch D — API Layer (Task 10)

- `src/api/library_profiles.rs` — CRUD at `/library-profiles`
- `src/api/ingest.rs` — `GET /staged`, `POST /submit` (AdminUser)
- `src/api/search.rs` — `POST /mb`, `POST /freedb`
- `src/api/tag_suggestions.rs` — added `POST /` create endpoint with source validation
- `src/api/libraries.rs` — `GET /:id/tracks` has optional `?status=` param (default `active`)
- `src/api/virtual_libraries.rs` — `PUT /:id/sources` body includes `library_profile_id` per entry

*Note: `POST /ingest/submit` upgraded from AuthUser to AdminUser after code quality review flagged it as irreversible.*

### Gap Fixes (post-Batch D)

Four gaps found in design-spec review:

1. **`ingest_dir` not dropped** — migration 0028; removed from struct/DAL/API
2. **FreeDB text search missing** — `FreedBService::text_search()` + wired in `/search/freedb`
3. **CUE split wrote to `ingest/` not `source/`** — fixed `source_out_dir` computation; removes originals post-split
4. **Art format ignored profile** — `process_staged` now calls `list_art_profiles()`, uses `profile.format` for MIME type

*Also fixed: `dedup()` on disc ID list only caught consecutive duplicates → replaced with HashSet.*

### Batch E — UI (Tasks 11–14)

- **Types**: `LibraryProfile`, updated `Track` (status, library_profile_id, bit_depth), updated `VirtualLibrarySource` (id, library_profile_id)
- **API clients**: `libraryProfiles.ts`, `ingest.ts`, `search.ts`; `tagSuggestions.ts` gained `create()`; `libraries.ts` cleaned of deprecated fields, gained `getLibrary()` + `listLibraryTracks()`
- **LibraryFormModal**: full rewrite — Profiles section (list with ▲/▼ reorder, add/delete, `auto_include_above_hz` gated on `codec === 'flac'`); `scan_enabled` + `scan_interval_secs` in edit mode; all deprecated fields removed
- **LibraryPage**: library name in toolbar (fetched), Scan button, active track list from API
- **IngestPage** (replaces InboxPage): album-grouped staged tracks, TagDiffTable per track, Accept/Edit/Reject/Search/Submit per track, pre-flight dialog (tags summary + art upload + profiles checklist + `write_folder_art` logic), batch-accept threshold bar
- **IngestSearchDialog**: MusicBrainz + FreeDB tabs, pre-populated from track, creates tag_suggestion on Select
- **SettingsPage**: `folder_art_filename` field added
- Route: `/inbox` → `/ingest` (with redirect)

*Spec gaps fixed after review: profile reorder UI, `auto_include_above_hz` conditional display, Edit action per track, art upload panel (was display-only), `write_folder_art` hardcoded false.*  
*Quality fix: `useState` initialized from async query data → profiles checklist always opened unchecked. Fixed with `useEffect` sync.*

### Batch F — Admin endpoint + filemap (Tasks 15–16)

- `POST /admin/migrate-library-files/:library_id` — moves active source tracks from flat `root_path/` layout to `root_path/source/`; EXDEV cross-device fallback (copy+delete with orphan cleanup); per-track error collection
- Codebase filemap updated for all 16 tasks + migrations 0021–0024 (gap fix)

---

## Decisions and Rationale

| Decision | Rationale |
|----------|-----------|
| Migration numbers corrected 0021→0025 | Existing repo had migrations 0021–0024 already; detected before writing any code |
| `POST /ingest/submit` requires AdminUser | Submission is irreversible (moves files); caught in code quality review |
| FreeDB text search via HTML scraping | CDDB protocol has no text search command; gnudb.org HTML is the only option |
| `source: 'mb_search'` for manual tag edits | Backend only accepts `acoustid | mb_search | freedb`; no `manual` source exists; documented with comment |
| EXDEV detection via `raw_os_error() == Some(18)` | `std::io::ErrorKind::CrossesDevices` is unstable on stable Rust |
| `write_folder_art` reads `folder_art_filename` setting | Spec says folder art is written when setting is non-empty and art is selected |

---

## Feedback Captured During Session

- **Spec gaps matter**: After Batch D, user identified that implementation was drifting from the design spec (FreeDB text search was missing). All subsequent batches instructed to refer back to `docs/plans/2026-04-21-library-ingest-redesign.md` when inconsistencies arise.
- **Two-stage review (spec then quality) catches real issues**: normalize.rs gutting, profile checkbox async init bug, EXDEV orphan risk — all caught by reviewers, not self-review.
- **Migration number conflicts**: Always verify existing migration numbers before writing new ones.
