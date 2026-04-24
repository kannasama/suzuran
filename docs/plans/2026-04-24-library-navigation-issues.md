# Library Navigation, Suggestion Review, Maintenance Job & Issues Tab

**Date:** 2026-04-24
**Branch:** `0.6`

## Overview

This plan covers a batch of library view improvements, bug fixes, a scheduled maintenance job, and the initial implementation of the Issues tab. Items are grouped by concern.

---

## 1. Library Sidebar Sub-Navigation

### What

Each physical library in the left sidebar gets a fixed set of browse nodes beneath it:

```
Libraries
  ├── My FLAC Library [flac]
  │   ├── All Tracks
  │   ├── Artist
  │   ├── Album Artist
  │   ├── Album
  │   └── Genre
  └── Lossy Library [mp3]
Virtual
  └── (no sub-nodes)
```

Clicking the library name itself behaves as before (selects it, shows all active tracks). Virtual libraries get no sub-nodes — they are composite views.

### Right pane — State B: Browse list

When a browse node is selected (e.g. "Album Artist"), the right pane replaces the track list with a sorted value list:

| Album Artist                   | Tracks |
|-------------------------------|--------|
| Godspeed You! Black Emperor   | 143    |
| Radiohead                     | 89     |

Values are derived client-side from the already-fetched track list — no new API endpoints. Sorted alphabetically. Track count shown per value.

### Right pane — State C: Filtered track list

Clicking a value in the browse list filters the track list to matching tracks. A breadcrumb bar replaces the browse list heading, sized as a secondary toolbar (more than a thin strip, less prominent than the main toolbar):

```
Album Artist  ›  Radiohead  ×
```

`×` returns to the browse list. All existing track list features (sort, group, column picker, multi-select, bulk edit) work normally in this state.

### Frontend state additions (`LibraryPage`)

```ts
browseMode: 'artist' | 'albumartist' | 'album' | 'genre' | null
browseFilter: string | null
```

- `browseMode = null` → State A (all tracks)
- `browseMode` set, `browseFilter = null` → State B (value list)
- Both set → State C (filtered track list)

`LibraryTree` receives two new callbacks:
- `onSelectBrowseMode(libraryId, mode)` — sets browse mode, clears filter
- Existing `onSelectLibrary` clears browse state entirely

---

## 2. Default Library

### What

A library can be marked as the default. When the Library page loads with no prior selection, the default library is auto-selected in "All Tracks" mode.

### Data layer

- New column: `libraries.is_default BOOLEAN NOT NULL DEFAULT FALSE`
- Migrations: `0032_libraries_default.sql` for both Postgres and SQLite
- Application-level enforcement: setting a library as default clears the flag on any previously-default library (no DB unique constraint — simpler update ordering)
- DAL: `set_default_library(id: i64)` — clears all, sets the one
- Model: `Library` gains `is_default: bool`

### Frontend

- `LibraryFormModal` gains a "Set as default" checkbox
- On Library page mount: find `libraries.find(l => l.is_default)`, auto-select it
- If none is default, behavior is unchanged

---

## 3. Quick Bug Fixes

### 3a. BulkEditPanel "Clear" → "Close"

`LibraryPage.tsx` line ~940: rename the button label. The action (`onClose`) is unchanged — it collapses the panel and clears selection. "Clear" implied wiping form values; "Close" is accurate.

### 3b. Context menu label renames

Both `handleContextMenu` and `handleThreeDotsClick`:

| Before | After |
|--------|-------|
| `Lookup` | `Identify via AcoustID` |
| `Search` | `Search MusicBrainz / FreeDB…` |

### 3c. Right-click must not modify selection or open BulkEditPanel

`handleContextMenu` currently calls `setSelectedTrackIds(new Set([track.id]))` when the right-clicked track is not selected. This causes the BulkEditPanel to open. Fix: right-click shows the context menu against the current selection without altering it. If the right-clicked track is already selected, operate on the selection. If it is not selected, show the menu scoped to that track without changing `selectedTrackIds`.

### 3d. ⋯ menu viewport boundary check

`handleThreeDotsClick` positions the menu at `(rect.left, rect.bottom + 4)` with no boundary check. After computing position, clamp: if `top + estimatedMenuHeight > window.innerHeight`, flip above the button (`rect.top - estimatedMenuHeight`). If `left + menuWidth > window.innerWidth`, align to right edge.

### 3e. Ingest badge — count staged tracks only

The Ingest tab badge currently pulls from `GET /tag-suggestions/count`, which counts all pending suggestions including those for active tracks created by Library view lookups. The badge should reflect the count of tracks with `status = 'staged'` only. Options:

- Add `GET /ingest/count` returning `{ count: number }` (staged track count)
- Or extend the existing `GET /staged` response to include a count field

The Library view suggestion workflow (accept/reject via ⋯ menu) is separate from Ingest and should not bleed into the Ingest badge.

---

## 4. Tabbed Edit Panel — Suggestion Review

### What

The BulkEditPanel gains a second tab: **Suggestion**. The existing content becomes the **Edit** tab. The Suggestion tab is only visible when at least one selected track has a pending suggestion.

### Single-track selection

Shows a field-by-field diff:

| Field         | Current Value     | Suggested Value   | ✓ |
|---------------|-------------------|-------------------|---|
| Title         | untitled          | Pyramid Song      | ☑ |
| Artist        | Radiohead         | Radiohead         | ☑ |
| Album         |                   | Amnesiac          | ☑ |
| MB Release ID |                   | abc-123…          | ☑ |

- Checkbox per row (pre-checked for fields that differ or are empty)
- "Select all / Deselect all" toggle
- **Accept** (applies checked fields only) and **Reject** buttons

### Multi-track selection

Shows a list of selected tracks that have pending suggestions:

```
Pyramid Song           87%   [Review]
Knives Out             91%   [Review]
```

Clicking "Review" on a row drills into that track's single-track diff view (back button returns to the list).

### Field-level selection — all flows

The same field-selection logic applies everywhere a suggestion can be accepted:
- Library view: via the new Suggestion tab
- Ingest view: the Accept button in per-track ⋯ menu opens the same review UI (the existing quick-accept shortcut remains for users who want to accept all fields without review)

Backend: `POST /tag-suggestions/:id/accept` gains an optional `fields: string[]` body parameter. When provided, only those fields from `suggested_tags` are applied. When absent, all fields are applied (existing behavior preserved).

---

## 5. Maintenance Job

### What

A per-library scheduled job that audits the state of files on disk and updates the DB to match. Covers the 0k bitrate regression (bitrate was read at ingest time; existing tracks were never retroactively updated) and ongoing filesystem drift detection.

### Job behaviour

For each active track in the library:

1. **File existence check** — if the file is missing, set `status = 'removed'` and create an Issue (see §6)
2. **Audio property refresh** — re-read bitrate, sample rate, bit depth, duration from file using the existing `tagger::read_tags` / `AudioProperties` pipeline; update the DB row if any value differs

### Scheduling

- New column: `libraries.maintenance_interval_secs INTEGER` (nullable — null means disabled)
- Migration: included in `0032` or a separate `0033`
- Scheduler picks up maintenance jobs on the same poll loop as scan jobs
- Manual trigger: **Maintenance** button in the library toolbar, alongside the existing Scan button

### New job type

- Job type string: `"maintenance"`
- Payload: `MaintenancePayload { library_id: i64 }`
- Migrations: add `maintenance` to the `job_type` CHECK constraint (Postgres ALTER, SQLite table recreate)
- Handler: `MaintenanceJobHandler` in `src/jobs/maintenance.rs`

### DAL additions

- `list_active_tracks_by_library(library_id: i64)` — if not already available
- `update_track_audio_properties(id, bitrate, sample_rate, bit_depth, duration_secs)` — targeted update

---

## 6. Issues Tab

### What

The Issues tab (currently unused) surfaces library health problems detected by the maintenance job and queryable from the tracks table. Issues are dismissable by the user and auto-cleared when resolved by a subsequent maintenance pass.

### Issue categories

| Type | Source | Severity |
|------|--------|----------|
| `missing_file` | Maintenance job | High |
| `bad_audio_properties` | Maintenance job / query | Medium |
| `untagged` | Query (no title/artist/album) | Low |
| `duplicate_mb_id` | Query (shared `musicbrainz_recordingid`) | Low |

### Data model

New table: `issues`

```sql
CREATE TABLE issues (
  id          BIGSERIAL PRIMARY KEY,
  library_id  BIGINT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  track_id    BIGINT REFERENCES tracks(id) ON DELETE CASCADE,
  issue_type  TEXT NOT NULL CHECK (issue_type IN ('missing_file', 'bad_audio_properties', 'untagged', 'duplicate_mb_id')),
  detail      TEXT,
  severity    TEXT NOT NULL CHECK (severity IN ('high', 'medium', 'low')),
  dismissed   BOOLEAN NOT NULL DEFAULT FALSE,
  resolved    BOOLEAN NOT NULL DEFAULT FALSE,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- Migration: `0033_issues.sql` (or `0034` depending on numbering)
- Maintenance job upserts issues on each pass; sets `resolved = true` when the condition clears
- Dismissed issues: suppressed in the default view; a "Show dismissed" toggle reveals them

### API

- `GET /issues` — list; optional `?library_id=`, `?type=`, `?severity=`; excludes resolved + dismissed by default
- `POST /issues/:id/dismiss` — sets `dismissed = true`
- `POST /issues/rescan` — accepts `{ track_ids: number[] }`; re-reads audio properties for the given tracks (same logic as maintenance job, targeted); clears `bad_audio_properties` issues on success

### Issue actions in the UI

| Issue type | Actions |
|------------|---------|
| `missing_file` | **Remove from library** (deletes track DB record, confirmation required), **Dismiss** |
| `bad_audio_properties` | **Rescan** (re-reads file properties, updates DB), **Dismiss** |
| `untagged` | **Dismiss** |
| `duplicate_mb_id` | **Dismiss** |

### Issues tab UI

- Filterable by library and issue type
- Sorted by severity (high → low) then by `created_at` desc
- Each row: severity indicator, track path, issue description, library name, action buttons
- "Show dismissed" toggle
- Issues auto-clear from the list when `resolved = true` (next page refresh or polling)

---

## Migration Numbering

Check current max before creating files. Expected sequence:

| # | File | Content |
|---|------|---------|
| 0032 | `libraries_default_maintenance.sql` | `is_default`, `maintenance_interval_secs` columns |
| 0033 | `jobs_add_maintenance.sql` | Add `maintenance` to job_type CHECK |
| 0034 | `issues.sql` | `issues` table |

Both Postgres and SQLite variants required for each.

---

## Out of Scope

- Virtual library browse nodes
- Batch issue resolution (dismiss-all, rescan-all) — can follow later
- Issues for virtual library tracks
