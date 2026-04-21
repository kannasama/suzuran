# Ingest Supersede — Implementation Summary

**Date:** 2026-04-21  
**Branch:** 0.6

## What Was Implemented

Feature: when a higher-quality version of an already-existing album is imported via the ingest
workflow, the system detects the upgrade, surfaces it in the UI, and — on user confirmation —
replaces the old active track with the new one. The old file is moved into a derived directory
(matching library profile) rather than deleted, preserving it as a lower-quality copy.

### Backend

**`src/services/transcode_compat.rs`**
- `quality_rank(format, sr, bd, br) -> u64` — encodes lossless > lossy → sample_rate →
  bit_depth/bitrate ordering into a single comparable u64
- `quality_cmp(...)` — multi-tier comparison using the same ordering
- `format_from_path(path)` — extracts extension from a file path
- `codecs_match(file_format, profile_codec)` — handles common codec aliases (m4a → aac, etc.)
- `parse_bitrate_kbps` made `pub`

**`src/dal/mod.rs` + `postgres.rs` + `sqlite.rs`**
- `find_active_source_track_by_mb_id(library_id, mb_id)` — tier 1 identity lookup
- `find_active_source_track_by_tags(library_id, albumartist_lower, album_lower, disc, track_num)` — tier 2
- `find_active_source_track_by_fingerprint(library_id, fingerprint)` — tier 3
- `set_track_library_profile(track_id, library_profile_id)` — assigns profile to displaced track

**`src/jobs/mod.rs`**
- `ProcessStagedPayload` gains `supersede_track_id: Option<i64>` and
  `supersede_profile_id: Option<i64>` (both `#[serde(default)]`); added `#[derive(Default)]`

**`src/jobs/process_staged.rs`**
- After moving staged file to `source/`, if `supersede_track_id` is set:
  - `Some(profile_id)`: moves old file to `{derived_dir_name}/`, updates path+profile,
    creates `track_link(new_source, old_derived)`. Old track stays "active" as a derived copy.
  - `None` (discard): deletes old file, sets old track status to "removed"

**`src/api/ingest.rs`**
- `POST /supersede-check` (AuthUser): accepts `{track_ids: [i64]}`, returns per-track
  supersede candidates. Three-tier identity matching: MB recording ID → tag tuple → AcoustID.
  For each match, compares quality and finds the best matching library profile (codec exact,
  sample_rate ±5%, bitrate ±20%).

**`tests/process_staged_job.rs`**
- Updated 3 struct initializers to use `..Default::default()` for the new optional fields

### Frontend

**`ui/src/api/ingest.ts`**
- `checkSupersede(trackIds)` function + `SupersedeCheckResult` / `SupersedeMatchInfo` /
  `ProfileMatchInfo` types exported
- `ProcessStagedPayload` interface updated with `supersede_track_id` / `supersede_profile_id`

**`ui/src/pages/IngestPage.tsx`**
- `useQuery` on `['ingest-supersede', ...]` calls `checkSupersede` whenever staged tracks load
- `AlbumGroup`: album header shows "N replaces existing" count badge; per-track "Replaces
  existing" badge (sky blue = profile matched, amber = no profile); clicking badge expands
  `SupersedeDetailRow` showing old quality → derived dir destination
- `SubmitDialog`: new "Supersedes" section with per-track resolution controls:
  - Profile matched → radio: "Replace → {dir}" / "Keep existing" (default: replace)
  - No profile → radio: "Keep existing" / "Replace and discard old file"
  - Import button disabled until all amber-flagged tracks have an explicit resolution

## Key Decisions

- Old file becomes an "active" derived copy (not "removed") when a profile match exists —
  consistent with how transcoded derived tracks work, and allows the old copy to serve as a
  virtual library source
- Identity matching is Rust-side sequential (3 separate DB queries) rather than a complex
  single SQL query — cleaner and profiles per library are small (1–5)
- Profile matching uses ±20% bitrate tolerance to handle encoder rounding differences
- Discard (None profile) path sets status "removed" but doesn't create a track link —
  the old track is treated as gone

## Feedback Captured

- User specified branch `0.6` (previous phases used 0.1–0.5; do not reuse)
- User explicitly bypassed plan-document step ("Go straight into implementation — plan docs
  leave too many gaps") — brainstorm session served as the plan
