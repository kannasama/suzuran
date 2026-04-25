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

- [ ] **T3 — Empty folder cleanup after process_staged**
  After moving files from `ingest/` → `source/`, walk up parent dirs and `remove_dir` each level
  while empty.
  Files: `src/jobs/process_staged.rs`

- [ ] **T4 — Fix duplicate derived tracks on supersede**
  Screenshot confirms: derived M4A copies appear both at library root level AND inside the
  organized folder hierarchy. Likely cause: transcode job builds output path from source track
  path at enqueue time; if source is later moved by organize, the transcoded file lands at the
  pre-organize path. Investigate and fix.
  Files: `src/jobs/process_staged.rs`, `src/jobs/transcode.rs`

- [ ] **T5 — Quality display: sample rate for lossy codecs**
  MP3/M4A/AAC/OGG quality strings must include sample rate: "48kHz / 192k", "44.1kHz / 320k".
  Lossless formats keep existing display (bit depth + sample rate already shown separately).
  Affects supersede comparison row in IngestPage and quality column in LibraryPage.
  Files: `ui/src/pages/IngestPage.tsx`, `ui/src/pages/LibraryPage.tsx` (quality formatting helper)

- [ ] **T8 — Show track filename in ingest review**
  Display the source filename of each track being reviewed in the ingest flow (per-track row or
  header), so the user can identify which file is being processed.
  Files: `ui/src/pages/IngestPage.tsx`

### Library View

- [ ] **T6 — Relative path / filename columns**
  Add `relative_path` and `filename` (basename of `relative_path`) to column picker and table.
  Files: `ui/src/hooks/useUserPrefs.ts`, `ui/src/pages/LibraryPage.tsx`

- [ ] **T7 — Delete derived tracks with confirmation**
  "Delete file…" option in per-track ⋯ menu, gated on `library_profile_id != null`.
  Confirmation modal shows relative path + filename. Override checkbox skips 15-min delay
  (immediate delete). Standard path uses existing `scheduleDelete` with 15-min `run_after`.
  Files: `ui/src/pages/LibraryPage.tsx`, `ui/src/api/tracks.ts`

## Progress Log

### 2026-04-25 — Session start
- Plan reviewed and approved by user.
- User added T5 requirement: include sample rate in lossy quality display.
- Screenshot `/tmp/suzuran-01.png` confirms T4: derived M4A files at library root, source files
  in organized subfolder — two distinct copies visible in file manager.
