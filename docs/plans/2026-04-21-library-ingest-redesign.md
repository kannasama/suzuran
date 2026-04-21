# Library Model Redesign & Ingest Workflow

**Date:** 2026-04-21
**Status:** Approved
**Scope:** Replaces the parent-child library DAG with a profiles-based model; introduces a
non-destructive ingest workflow; expands the Inbox into a full Ingest section; corrects the
MusicBrainz lookup fallback chain.

---

## Motivation

The original design modelled derived formats (AAC, MP3, etc.) as independent child libraries
linked to a parent via `parent_library_id`. This creates operational friction: each format
requires its own library record, and the relationship between source and derived tracks is
implicit via `track_links`. It also allowed silent automatic transcoding on ingest, which risks
upconverting lossy sources to lossless formats with no quality gain.

This redesign:
- Collapses the library DAG into a single library that manages its own derived versions via a
  `library_profiles` join table
- Establishes a non-destructive ingest principle: no file is modified until the user explicitly
  submits a processing batch
- Extends virtual libraries to express "best available version" fallback chains
- Corrects the automated lookup fallback chain to use MusicBrainz text search before FreeDB

---

## Design Principles

**Nothing modifies files automatically.** The system performs read-only analysis (fingerprint,
tag lookup) on ingest. Tag writes, art embeds, file moves, CUE splits, and transcodes are all
enqueued only after the user reviews and submits a processing batch.

**NFS deployment note.** The target deployment mounts media on an NFS share (single filesystem,
so hard links work). inotify is not reliable on NFS and is not used. Scheduled polling is the
primary scan mechanism.

---

## Library Model

### Folder Structure

Each library has a single `root_path` that acts as a container. Three canonical subdirectories
live beneath it:

| Path | Purpose |
|------|---------|
| `{root_path}/ingest/` | Landing zone. Scanner discovers files here and performs read-only analysis. |
| `{root_path}/source/` | Processed source files. Populated by the submission pipeline after user approval. |
| `{root_path}/{derived_dir_name}/` | One per `library_profiles` entry. Populated by transcode jobs post-submission. |

The `ingest/` subdirectory is a convention; the `ingest_dir` field is removed from `libraries`.

### Changes to `libraries`

Removed columns:

| Column | Reason |
|--------|--------|
| `parent_library_id` | DAG replaced by `library_profiles` |
| `encoding_profile_id` | Single FK replaced by join table |
| `auto_transcode_on_ingest` | Replaced by per-profile `include_on_submit` |
| `normalize_on_ingest` | Absorbed into submission workflow |
| `ingest_dir` | Replaced by `ingest/` subdirectory convention |

Remaining columns are unchanged: `id`, `name`, `root_path`, `format`, `scan_enabled`,
`scan_interval_secs`, `auto_organize_on_ingest`, `tag_encoding`, `organization_rule_id`,
`created_at`.

### `library_profiles` (new table)

```
library_profiles: id, library_id, encoding_profile_id, derived_dir_name,
                  include_on_submit, auto_include_above_hz (nullable),
                  created_at
```

| Column | Description |
|--------|-------------|
| `library_id` | FK → `libraries` |
| `encoding_profile_id` | FK → `encoding_profiles` |
| `derived_dir_name` | User-configured directory name under `root_path` (e.g. `aac-192k`, `flac-96k`) |
| `include_on_submit` | Pre-selects this profile in the submission dialog by default |
| `auto_include_above_hz` | If set, overrides `include_on_submit`: the profile is only pre-selected when the source track's sample rate meets or exceeds this value (Hz). Intended for lossless-to-lossless compatibility profiles where auto-inclusion only makes sense when the source exceeds the profile's own rate. |

The existing quality guard (`is_compatible`) continues to block upconversion regardless of
these settings — lossy-to-lossless, sample-rate inflation, and bit-depth inflation are always
rejected.

---

## Track Model

### Status

`tracks` gains a `status` column:

| Value | Meaning |
|-------|---------|
| `staged` | File is in `ingest/`. Analysis complete, awaiting user submission. Not visible in Library view. |
| `active` | File is in `source/` or a derived directory. Fully processed. Visible in Library view. |
| `removed` | File no longer found on disk. |

Existing rows default to `active`.

### `library_profile_id`

`tracks` gains a nullable `library_profile_id` FK → `library_profiles`:

- `NULL` — source track; `relative_path` is relative to `{root_path}/source/`
- Set — derived track; `relative_path` is relative to `{root_path}/{derived_dir_name}/`

All versions of a recording (source and all derived) share the same `library_id`. The
`library_profile_id` distinguishes which directory subtree holds the file.

### `track_links` simplified

`encoding_profile_id` is removed — it is now redundant since the derived track's
`library_profile_id` carries that information. The table becomes a pure source→derived
relationship record:

```
track_links: source_track_id, derived_track_id, created_at
```

---

## Virtual Libraries

### `virtual_library_sources` updated

The composite primary key `(virtual_library_id, library_id)` is replaced with a surrogate `id`.
A `library_profile_id` column is added:

```
virtual_library_sources: id, virtual_library_id, library_id,
                         library_profile_id (nullable), priority
```

- `library_profile_id = NULL` — include source tracks from this library
- `library_profile_id = X` — include derived tracks for profile X from this library

A unique index on `(virtual_library_id, library_id, library_profile_id)` enforces no
duplicate entries.

### Best-available-version fallback

Multiple source entries for the same library, each targeting a different profile, can be
configured at different priority levels. The virtual sync job's existing identity-dedup logic
(first match per `track_identity` wins) handles fallback automatically.

Example — DAP virtual library that prefers FLAC 96k, falls back to full FLAC source, then
accepts AAC if neither FLAC version exists:

```
priority 1: library_id=1, library_profile_id=flac-96k
priority 2: library_id=1, library_profile_id=NULL  (source)
priority 3: library_id=1, library_profile_id=aac-192k
```

This ensures tracks that were ingested as AAC (no lossless version available) still appear in
the DAP virtual library rather than being silently absent.

---

## Ingest Workflow

### Analysis phase (automatic, read-only)

The scanner checks `{root_path}/ingest/` on each scan run (scheduled or manual). For newly
discovered files it:

1. Creates a `staged` track record
2. Enqueues `fingerprint` → `mb_lookup` (and fallbacks — see Lookup Fallback Chain)

No file is touched. CUE sheet detection happens here: CUE-backed audio files are flagged but
not split automatically.

The scanner also continues to check `{root_path}/source/` for changes to already-active tracks
(hash changes, removals).

### Ingest section

The renamed Inbox. Full-width view, no tree pane. Staged tracks are grouped by album.

Per album:

- Header: detected art preview, album title, track count, detected source format
- Per-track row: current tags vs. suggested tags (diff highlighted), confidence badge,
  source label (AcoustID / MB search / FreeDB)

Per-track actions: **Accept** · **Edit** · **Reject** · **Search**

Batch action at the top: **Accept all ≥ N%** (threshold configurable via input, defaults to
the value from settings).

### Submission

At the album or track level, a **Submit** button opens a pre-flight dialog:

| Section | Content |
|---------|---------|
| Tags | Summary of tags to be written (from accepted suggestion, or current file tags if none accepted) |
| Art | Suggested art thumbnail (if present) · Upload/drag-drop · Skip |
| CUE split | Checkbox — shown only when a CUE sheet was detected for this file |
| Profiles | Checklist of `library_profiles` entries; pre-selection driven by `include_on_submit` and `auto_include_above_hz` vs. the source track's detected sample rate; user can add or remove |

On confirm, jobs are enqueued.

### Job pipeline

**Standard path (no CUE):**

1. `process_staged` — writes approved tags to the file in `ingest/`; embeds art if selected
   (and writes external art file to the album directory if `folder_art_filename` is configured);
   moves the file to `source/`; updates `tracks.relative_path` and `tracks.status = active`.
   On completion, enqueues one `transcode` job per selected profile.

2. `transcode` (one per profile) — reads from `source/`, writes to
   `{root_path}/{derived_dir_name}/`, creates derived `tracks` row, creates `track_links` row.

**CUE split path:**

1. `cue_split` — splits the CUE-backed audio into per-track files, writes them directly to
   `source/` with approved tags applied, creates `active` track records. On completion,
   enqueues one `transcode` job per track per selected profile.

The original CUE+audio file in `ingest/` is removed after a successful split.

---

## Album Art

### Art sources in submission

The pre-flight dialog art panel presents, in order:

1. **Suggested art** — thumbnail from `tag_suggestions.cover_art_url` (Cover Art Archive) if
   present on the accepted suggestion
2. **Upload** — drag-and-drop or file picker; uses the existing `POST /api/v1/uploads/images`
   endpoint
3. **Skip** — no art action taken

### External art file

If the `folder_art_filename` setting is non-empty, the `process_staged` job writes a copy of
the selected art to the album directory in `source/` alongside the audio files, in addition to
embedding it in the audio file. Default value: `folder.jpg`. The format (JPEG/PNG) is
determined by the active art profile's `format` field.

---

## Lookup Fallback Chain

### Automated (background jobs)

```
fingerprint → mb_lookup
                ├─ AcoustID results ≥ 0.8     → create tag_suggestions (source = "acoustid")
                ├─ AcoustID returns nothing
                │    └─ MB text search (title + artist + album from existing tags)
                │         ├─ Results found     → create tag_suggestions (source = "mb_search",
                │         │                       confidence capped at 0.6)
                │         └─ No results + DISCID present → enqueue freedb_lookup
                └─ freedb_lookup
                     └─ create tag_suggestion (source = "freedb", confidence = 0.5)
```

`MusicBrainzService` gains a `search_recordings(title, artist, album)` method. The existing
1 req/s rate limiter covers it. Results flow through the existing `to_tag_map` path.

### Manual search (Search action in Ingest section)

The Search action opens a dialog with two tabs:

**MusicBrainz tab**
- Title, Artist, Album fields — pre-populated from current tags
- User can edit before submitting
- Calls a new API endpoint that invokes `MusicBrainzService::search_recordings` directly and
  returns candidates synchronously (no job queue round-trip)

**FreeDB tab**
- Two search modes:
  - *Disc ID* — pre-populated from `DISCID` tag if present; user can also enter manually
  - *Text search* — Artist and Album title fields for cases where no disc ID is available
- Calls `FreedBService` directly; returns candidates synchronously

Returned candidates from either tab are presented as selectable suggestions in the same
tag diff view. Selecting a candidate creates a `tag_suggestion` row and returns the user to
the standard Accept/Edit/Reject flow.

---

## UI Changes

### Library view

- Toolbar label updated from "Library #N" to the library's actual name
- **Scan** button added to toolbar — enqueues a scan job for the selected library
- Track list renders `active` tracks (stub placeholder removed)
- Grouping (None / Album / Artist / Genre / Year) and multi-column sort are functional

### Library form

- Single encoding profile dropdown replaced with a **Profiles** section listing
  `library_profiles` entries
- Each entry shows: derived dir name, encoding profile, include-on-submit toggle,
  `auto_include_above_hz` input (shown only when a lossless-to-lossless profile is detected)
- Entries can be added, reordered, and removed
- `scan_enabled` toggle and `scan_interval_secs` input are exposed as editable fields

### Ingest section (renamed from Inbox)

- Album-grouped layout with per-track tag diff rows
- Accept / Edit / Reject / Search actions per track
- Submit button per album and per track
- Configurable batch-accept threshold input

### Settings — General tab

- `folder_art_filename` field added (default: `folder.jpg`, empty = disable external art write)

---

## Schema Migration Summary

| Table | Changes |
|-------|---------|
| `libraries` | Drop `parent_library_id`, `encoding_profile_id`, `auto_transcode_on_ingest`, `normalize_on_ingest`, `ingest_dir` |
| `library_profiles` | New table |
| `tracks` | Add `status TEXT NOT NULL DEFAULT 'active'`; add `library_profile_id BIGINT NULL REFERENCES library_profiles(id)` |
| `track_links` | Drop `encoding_profile_id` |
| `virtual_library_sources` | Replace composite PK with surrogate `id`; add `library_profile_id BIGINT NULL REFERENCES library_profiles(id)`; add unique index on `(virtual_library_id, library_id, library_profile_id)` |
| `jobs` (job_type CHECK) | Add `process_staged` |
| `settings` | Seed `folder_art_filename` key with default value `folder.jpg` |

**Existing data:** Current child libraries (those with `parent_library_id` set) are converted
to `library_profiles` entries on their parent library. Their `encoding_profile_id` maps
directly; their `root_path` final component becomes `derived_dir_name`. Derived tracks receive
`library_profile_id` from the converted entry; source tracks receive `library_profile_id = NULL`.

**Filesystem migration:** Existing source files at `{root_path}/…` need to move to
`{root_path}/source/…`. This is a per-library migration step triggered explicitly by the user,
not run automatically on startup. A migration UI or CLI command will be provided.

---

## Deferred Items

The following gap analysis items are acknowledged but out of scope for this phase:

- `api_tokens` management API
- `audit_log` writes
- Per-user 2FA enforcement (`totp_required`, `webauthn_required`)
- `force_password_change` enforcement
- User profile update API and Account page
- TOTP secret encryption
- Issues page
- `library_admin` role in DB
