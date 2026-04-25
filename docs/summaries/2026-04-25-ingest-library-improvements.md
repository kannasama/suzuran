# Ingest & Library Improvements — 2026-04-25

## Task List

### Ingest Flow

- [ ] **T1 — Album-level alternate release picker**
  Dropdown at album header populated from `suggestion.alternatives`. Selecting an alternate updates
  tag diffs for all tracks in the album group. On accept, reject all other non-selected alternatives
  for that suggestion.
  Files: `ui/src/pages/IngestPage.tsx`, `ui/src/components/AlternativesPanel.tsx`

- [ ] **T2 — Tabular album-level edits**
  Replace `AlbumEditPanel` form (17 fields + Apply to All) with a diff-table layout matching
  `TagDiffTable` — field | current | new value (inline editable). No separate form.
  Files: `ui/src/pages/IngestPage.tsx`

- [x] **T3 — Empty folder cleanup after process_staged**
  After moving files from `ingest/` → `source/`, walk up parent dirs and `remove_dir` each level
  while empty.
  Files: `src/jobs/process_staged.rs`

- [x] **T4 — Fix duplicate derived tracks on supersede**
  Screenshot confirms: derived M4A copies appear both at library root level AND inside the
  organized folder hierarchy. Likely cause: transcode job builds output path from source track
  path at enqueue time; if source is later moved by organize, the transcoded file lands at the
  pre-organize path. Investigate and fix.
  Files: `src/jobs/process_staged.rs`, `src/jobs/transcode.rs`

- [x] **T5 — Quality display: sample rate for lossy codecs**
  MP3/M4A/AAC/OGG quality strings must include sample rate: "48kHz / 192k", "44.1kHz / 320k".
  Lossless formats keep existing display (bit depth + sample rate already shown separately).
  Affects supersede comparison row in IngestPage and quality column in LibraryPage.
  Files: `ui/src/pages/IngestPage.tsx`, `ui/src/pages/LibraryPage.tsx` (quality formatting helper)

- [x] **T8 — Show track filename in ingest review**
  Display the source filename of each track being reviewed in the ingest flow (per-track row or
  header), so the user can identify which file is being processed.
  Files: `ui/src/pages/IngestPage.tsx`

- [ ] **T9 — Clarify Save vs Accept; prevent Accept from overriding manual edits**
  In the ingest per-track edit flow, "Accept" on a suggestion was overwriting fields the user
  had manually saved via TrackEditPanel. Manual edits (confidence 1.0 suggestions) must take
  precedence over, or at least not be silently replaced by, accepting an auto-suggestion.
  Clarify button labeling and intent; ensure Accept does not clobber manually-saved fields
  unless the user explicitly chooses to override them.
  Files: `ui/src/pages/IngestPage.tsx`, `ui/src/components/TrackEditPanel.tsx`,
         `ui/src/api/tagSuggestions.ts`

- [ ] **T10 — Per-field suggestion selection in Ingest view**
  Per-field checkbox selection (choose which fields to apply from a suggestion) is already
  implemented in the Library view's SuggestionReviewPane but is missing from the Ingest view's
  per-track accept flow. The accept API already supports `{fields?: string[]}` — wire up
  field checkboxes in IngestPage so users can accept a subset of suggested tags per track.
  Files: `ui/src/pages/IngestPage.tsx`

- [x] **T11 — Group ingest tracks by scanned folder**
  Add folder-based grouping to the ingest view: tracks are grouped by the parent directory of
  their `relative_path` (i.e., the folder they were dropped into under `ingest/`). Most albums
  map 1:1 to a folder, making it easy to track which files belong to the same import batch.
  Files: `ui/src/pages/IngestPage.tsx`

### Library View

- [x] **T6 — Relative path / filename columns**
  Add `relative_path` and `filename` (basename of `relative_path`) to column picker and table.
  Files: `ui/src/hooks/useUserPrefs.ts`, `ui/src/pages/LibraryPage.tsx`

- [x] **T7 — Delete derived tracks with confirmation**
  "Delete file…" option in per-track ⋯ menu, gated on `library_profile_id != null`.
  Confirmation modal shows relative path + filename. Override checkbox skips 15-min delay
  (immediate delete). Standard path uses existing `scheduleDelete` with 15-min `run_after`.
  Files: `ui/src/pages/LibraryPage.tsx`, `ui/src/api/tracks.ts`

## Progress Log

### T11 — Group ingest tracks by scanned folder
- Added `groupMode: 'album' | 'folder'` state.
- `getIngestFolder(relativePath)` helper strips `ingest/` prefix, returns parent dir (or `(root)` for flat files).
- Group key switches based on mode; folder mode shows the ingest subdirectory path as the group header.
- Album/Folder toggle added to the batch accept bar (pill-style segmented button).
- Committed.

### T4 — Fix duplicate derived tracks on supersede
Two root causes identified and fixed in `process_staged.rs`:

1. **Duplicate derived**: When `supersede_profile_id` matches a profile in `profile_ids`, the displaced old source file is already placed in `derived_dir_name/` — that IS the derived copy. The transcode loop now skips any profile_id equal to `supersede_profile_id` to prevent a second copy being created.

2. **Source file not organized**: `process_staged` moved files `ingest/ → source/` but never enqueued an organize job. Added: if `library.auto_organize_on_ingest && library.organization_rule_id.is_some()`, enqueue `organize` for the new source track_id after the move.

User confirmed: FLAC superseding files also landed in source root without organization rules applying — now fixed by the auto-organize enqueue.
- Committed.

### T7 — Delete derived tracks with confirmation
- Backend `DeleteRequest` extended with `immediate: bool` (default false). When true, uses `enqueue_job` (no delay) instead of `enqueue_job_after`.
- `scheduleDelete(ids, immediate?)` API client updated.
- `DerivedTrackRow` gets `onDelete?` prop; actions cell renders a ✕ button.
- `DerivedDeleteModal`: shows filename + full relative path, "Delete immediately" checkbox (toggles button label and hides delay note), Cancel + Delete Now / Schedule Deletion.
- Committed.

### T6 — Relative path / filename columns
- Added `filename` (basename) and `relative_path` (full path) to `COLUMNS` in LibraryPage and `DEFAULT_COL_WIDTHS` in `useUserPrefs`.
- Both columns are opt-in (hidden by default); `DEFAULT_VISIBLE_COLS` excludes them.
- Render cells added to both `TrackRow` and `DerivedTrackRow`.
- Committed.

### T8 — Show track filename in ingest review
- Track title span converted to a flex column: title on line 1, filename (basename of `relative_path`) on line 2 in muted monospace.
- Secondary filename line only shown when a title exists; when no title, filename is already the primary text.
- Committed.

### T5 — Quality display for lossy codecs
- `LibraryPage.formatQuality`: lossy branch now returns `${khz}kHz / ${bitrate}k` when sample rate is available.
- `IngestPage.fmtQuality`: lossless path unchanged (`CODEC · kHz · N-bit`); lossy path → `CODEC · kHz / Nk`.
- Supersede badge inline spans: lossless shows kHz + bit-depth, lossy shows combined `kHz / Nk` span.
- Committed.

### 2026-04-25 — Session start
- Plan reviewed and approved by user.
- User added T5 requirement: include sample rate in lossy quality display.
- Screenshot `/tmp/suzuran-01.png` confirms T4: derived M4A files at library root, source files
  in organized subfolder — two distinct copies visible in file manager.
