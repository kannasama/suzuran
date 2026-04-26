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

## Commits (pending)

- `feat: add POST /tracks/:id/apply-tags with derived track propagation`
- `fix: delete_tracks removes companion files and sweeps empty dirs`
- `fix: transcode job copies companion files to derived output dir`
- `docs: session summary`
