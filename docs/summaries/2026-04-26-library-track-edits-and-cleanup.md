# Session Summary — Library Track Edits, Sidecar Cleanup, Derived Propagation

**Branch:** `fix/cue-split-ingest-flow`
**Date:** 2026-04-26 (in progress)

## Work In Progress

### Bug 1 — Bulk/single track edit not updating records or files

**Root cause:** `BulkEditPanel.handleApply()` (LibraryPage.tsx) called
`tagSuggestionsApi.create()` but never accepted the suggestion. Creating a suggestion
does not write to the audio file or DB record — acceptance is the step that does.

**Agreed fix:** Adapt the ingest "working copy" (pending_tags) model:
- `handleApply` calls `setPendingTags(track_id, dirtyFields)` to store edits
- Then calls new `POST /tracks/:id/apply-tags` to flush: merge pending_tags into
  existing tags, write to audio file via `tagger::write_tags`, update DB record
  (tags JSONB + indexed columns), clear pending_tags
- After updating source track, propagate tag changes to all derived tracks
  (via `list_derived_tracks` → `write_tags` + `update_track_tags` per derived)

**Files:** `src/api/tracks.rs`, `ui/src/api/tracks.ts`, `ui/src/pages/LibraryPage.tsx`

---

### Bug 2 — Folder cleanup not removing sidecar files after track deletion

**Root cause:** `delete_tracks.rs` removes only the audio file. Companion files
(folder.jpg, logs, cue sheets, etc.) are left orphaned. No directory cleanup runs.

**Agreed fix:** After removing the audio file, delete companion files from the same
directory, then sweep empty parent dirs up to the library root.

**Files:** `src/jobs/delete_tracks.rs`

---

### Bug 3 — Derived tracks not getting copies of sidecar files

**Root cause:** The transcode job creates the derived audio file and writes tags but
never copies companion files. The organize job's `copy_companions` only runs when
a derived track is moved — newly-transcoded tracks are already at the correct path,
so organize skips them ("already organized") and companions are never propagated.

**Agreed fix:** After the transcode job creates the output file, copy companion files
from the source track's directory into the derived output directory.

**Files:** `src/jobs/transcode.rs`

---

### Shared utility — companion file helpers

`COMPANION_EXTS`, `copy_companions`, and `remove_empty_dirs` are currently private
in `organize.rs`. Moving them to `src/jobs/mod.rs` as `pub` items so they can be
reused by `transcode.rs` and `delete_tracks.rs` without duplication.

---

## Commits

- `docs: in-progress session summary — library track edits and cleanup`
- `fix: library track tag apply, companion cleanup, derived propagation`
- `docs: update filemap and session summary for library tag fixes`

## Build Status

Docker build passes — all Rust tests pass, TypeScript compiles clean.

---

## Follow-up — MB ID Fields and Indexed Column Gaps

### Bug A — Wrong and missing MusicBrainz field mappings in tagger

**Root cause:** lofty's `ItemKey` naming is counterintuitive:
- `ItemKey::MusicBrainzRecordingId` → writes `MUSICBRAINZ_TRACKID` (the standard recording ID tag)
- `ItemKey::MusicBrainzTrackId` → writes `MUSICBRAINZ_RELEASETRACKID` (per-release track ID)
- `ItemKey::MusicBrainzReleaseId` → writes `MUSICBRAINZ_ALBUMID` (the release ID)

Current `tagger/mod.rs` maps `"musicbrainz_trackid"` → `ItemKey::MusicBrainzTrackId`, which
writes `MUSICBRAINZ_RELEASETRACKID` — wrong. The MB service stores the recording ID under
`"musicbrainz_trackid"` and it should write to `MUSICBRAINZ_TRACKID` via `MusicBrainzRecordingId`.

`"musicbrainz_releaseid"` is entirely absent from both `read_tags` and `write_tags`.
`"musicbrainz_albumartistid"` is also absent.

**Agreed fix:** `src/tagger/mod.rs` — fix `musicbrainz_trackid` → `MusicBrainzRecordingId`;
add `musicbrainz_releaseid` → `MusicBrainzReleaseId`; add `musicbrainz_albumartistid` →
`MusicBrainzReleaseArtistId` in both `read_tags` and `write_tags`.

---

### Bug B — `update_track_tags` missing indexed columns

**Root cause:** `update_track_tags` in both DAL implementations omits `totaldiscs`,
`totaltracks`, and `composer` from the `UPDATE` statement. The `tags` JSONB blob and
audio file are written correctly; only the indexed columns are stale.

**Agreed fix:** `src/dal/sqlite.rs` + `src/dal/postgres.rs` — add the three missing
columns to the `UPDATE` statement in `update_track_tags`.

**Files:** `src/tagger/mod.rs`, `src/dal/sqlite.rs`, `src/dal/postgres.rs`

## Pending Commits

- `fix: correct MB field mappings in tagger and add missing indexed columns`
