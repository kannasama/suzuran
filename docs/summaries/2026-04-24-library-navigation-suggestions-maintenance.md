# Session Summary — 2026-04-24: Library Navigation, Suggestion Review, Maintenance Job

Branch: `0.6`

## Tasks Completed

### Task 1: Quick fixes (context menu labels, right-click behavior, viewport clamping)
- `LibraryPage.tsx`: Context menu items renamed to "Identify via AcoustID" and "Search MusicBrainz / FreeDB…"
- Right-click no longer modifies selection or opens BulkEditPanel
- ContextMenu component clamps to viewport boundaries

### Task 2: Library browse navigation (sidebar + two-state right pane)
- `LibraryTree.tsx`: Sub-nodes under each library — All Tracks / Artist / Album Artist / Album / Genre
- `LibraryPage.tsx`: `browseMode` and `browseFilter` state; State B shows distinct value list; State C shows filtered tracks with breadcrumb bar
- `LibraryTree.tsx`: Exported `BrowseMode` type; sidebar highlights active sub-node

### Task 3: Default library + BulkEditPanel "Clear" → "Close"
- `migrations/postgres/0032_libraries_default_maintenance.sql`: Added `is_default`, `maintenance_interval_secs` columns
- `migrations/sqlite/0032_libraries_default_maintenance.sql`: Same for SQLite
- `src/models/mod.rs`, `src/dal/mod.rs`, `src/dal/postgres.rs`, `src/dal/sqlite.rs`: Updated `Library` struct and DAL
- `src/api/libraries.rs`: `UpdateLibraryRequest` with `is_default`, `maintenance_interval_secs`; `set_default_library` call
- `ui/src/api/libraries.ts`: Updated types
- `ui/src/components/LibraryFormModal.tsx`: Default library toggle + maintenance interval input
- `LibraryPage.tsx`: Auto-selects default library on load; "Clear" → "Close"

### Task 4: Ingest badge counts staged tracks, not tag suggestions
- `ui/src/components/TopNav.tsx`: Switched from `tagSuggestionsApi.count()` to `getStagedCount()`
- `ui/src/api/ingest.ts`: Added `getStagedCount()` calling `GET /ingest/count`
- `src/api/ingest.rs`: Added `GET /ingest/count` route + `count_staged` handler

### Task 5: Tabbed edit panel with field-level suggestion review
- `src/services/tagging.rs`: `apply_suggestion` takes `fields: Option<&[String]>` — `None` = apply all, `Some` = filter
- `src/api/tag_suggestions.rs`: `accept` handler takes optional `AcceptBody { fields }`, passes to `apply_suggestion`; `batch_accept` passes `None`
- `ui/src/api/tagSuggestions.ts`: `accept(id, fields?)` sends `{ fields }` only when provided
- `ui/src/pages/LibraryPage.tsx`: BulkEditPanel rewritten with Edit/Suggestion tabs; `SuggestionReviewPane` shows field-level diff table with checkboxes, Accept(N fields)/Reject buttons

### Task 6: Maintenance job — re-reads audio properties, marks missing files removed
- `src/dal/mod.rs`: Added `update_track_audio_properties` to Store trait
- `src/dal/postgres.rs`, `src/dal/sqlite.rs`: Implemented the new DAL method
- `src/jobs/maintenance.rs`: New `MaintenanceJobHandler` — iterates active tracks, checks file existence, re-reads audio props via `tagger::read_tags`, updates DB
- `src/jobs/mod.rs`: `pub mod maintenance` + `MaintenancePayload { library_id }`
- `src/scheduler/mod.rs`: Registered `maintenance` handler with semaphore of 1
- `src/api/libraries.rs`: `POST /libraries/:id/maintenance` route + handler
- `ui/src/api/libraries.ts`: `triggerMaintenance(libraryId)`
- `ui/src/pages/LibraryPage.tsx`: "Maintain" button in library toolbar alongside "Scan"

## Commits
- `d55ee6c` — pre-session state (prior session docs)
- `24d786e` — feat: tabbed edit panel (Task 5)
- `878c05e` — feat: maintenance job (Task 6)

### Task 7: Issues tab
- `migrations/postgres/0033_jobs_add_maintenance.sql`, `migrations/sqlite/0033_jobs_add_maintenance.sql`: Added `maintenance` to job_type CHECK constraint
- `migrations/postgres/0034_issues.sql`, `migrations/sqlite/0034_issues.sql`: New `issues` table with unique index on `(track_id, issue_type)`
- `src/models/mod.rs`: `Issue` + `UpsertIssue` structs
- `src/dal/mod.rs`: `upsert_issue`, `resolve_issue`, `dismiss_issue`, `list_issues`, `get_issue`, `issue_count` in Store trait
- `src/dal/postgres.rs`, `src/dal/sqlite.rs`: Implementations (ON CONFLICT upsert pattern)
- `src/jobs/maintenance.rs`: Updated to upsert `missing_file`, `bad_audio_properties`, `untagged` issues; resolves issues when conditions clear
- `src/api/issues.rs`: New router — `GET /issues`, `GET /issues/count`, `POST /issues/:id/dismiss`, `POST /issues/rescan`
- `src/api/mod.rs`: Registered `/issues` route
- `ui/src/types/issue.ts`: `Issue` type
- `ui/src/api/issues.ts`: `issuesApi.list/count/dismiss/rescan`
- `ui/src/pages/IssuesPage.tsx`: Issues table with library/type filters, show-dismissed toggle, Rescan (bad audio) and Dismiss actions
- `ui/src/App.tsx`: Added `/issues` route
- `ui/src/components/TopNav.tsx`: Issues nav item gets a yellow badge with unresolved/undismissed count

### Bug Fix — Three post-Task-7 fixes (f7392f3)

**Fix 1: M4A bitrate reads as 0k after maintenance job**
- `src/tagger/mod.rs`: `overall_bitrate()` returns `Some(0)` for M4A containers (not `None`), so the
  `or_else(|| audio_bitrate())` fallback never fired. Added `.filter(|&b| b > 0)` before the
  `or_else` to treat zero as absent and fall through to `audio_bitrate()`.

**Fix 2: Deleting a library profile must clean up derived tracks**
- `src/api/library_profiles.rs`: `delete_profile` now calls `list_tracks_by_profile(library_id, Some(id))`,
  removes each file from disk (best-effort), calls `mark_track_removed` for each, then deletes the profile.

**Fix 3: Suggestions don't show all available MusicBrainz fields**
- `src/services/musicbrainz.rs`:
  - `get_recording` `inc` extended with `+recordings` so track listings within each medium are returned
  - `MbRelease` gains `country: Option<String>`
  - `MbReleaseGroup` gains `id: Option<String>` and `secondary_types: Option<Vec<String>>`
  - `MbMedia` gains `tracks: Option<Vec<MbTrack>>`; new `MbTrack` and `MbTrackRecording` structs
  - `to_tag_map` now emits: `totaldiscs` (always), `discnumber`, `tracknumber`, `totaltracks`
    (from track position match), `releasestatus`, `releasecountry`, `musicbrainz_artistid`,
    `musicbrainz_albumartistid`, `musicbrainz_releasegroupid`
- `tests/musicbrainz_service.rs`: Added `country: None` to two `MbRelease` literals
- `ui/src/pages/LibraryPage.tsx`: `FIELD_LABELS` map derived from `BULK_EDIT_FIELDS`; `SuggestionReviewPane`
  shows human-readable field names (e.g. "MB Artist ID") instead of raw keys, with the raw key as a tooltip

## Commits
- `d55ee6c` — pre-session state (prior session docs)
- `24d786e` — feat: tabbed edit panel (Task 5)
- `878c05e` — feat: maintenance job (Task 6)
- `f31954a` — feat: issues tab (Task 7)
- `f7392f3` — fix: m4a bitrate, profile delete cleanup, suggestion field coverage
- `8f640d0` — fix: scan and maintenance buttons poll job status and refresh tracks on completion
- `f183c9d` — fix: quality column — separate lossless (bit-depth/kHz) and lossy (kbps) views

### Bug Fix — Scan/Maintain buttons not refreshing track list (8f640d0)

`ui/src/pages/LibraryPage.tsx`: Both handlers now capture the returned `job_id` and poll
`GET /jobs/:id` every 2 s. When status is `completed` or `failed`, the interval clears and
`qc.invalidateQueries({ queryKey: ['library-tracks'] })` fires. Status labels updated from
"queued" → "Scanning…" / "Maintaining…".

### Bug Fix — Quality column showing wrong values (f183c9d)

`formatBitrate(bps)` was dividing by 1000 treating kbps as bps — so a 256 kbps AAC track
displayed as "0k". Replaced with `formatQuality(bitrate, bitDepth, sampleRate)`:
- `bit_depth` present (FLAC, WAV, ALAC): shows `"{depth}-bit / {rate}kHz"`
- `bit_depth` null (AAC, MP3, Opus): shows `"{kbps}k"` — no division
Column renamed **Bitrate → Quality**, widened from `w-14` to `w-24`.

## Pending
- Field-level selection in Ingest accept flow (currently only Library BulkEditPanel)
