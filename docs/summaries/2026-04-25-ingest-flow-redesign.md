# Session Summary — Ingest Flow Redesign (Working-Copy Model)

**Branch:** `fix/cue-split-ingest-flow`
**Date:** 2026-04-25

## What Was Done

Full redesign of the ingest flow to fix a series of UX and correctness bugs reported by the user.

### Root Causes Fixed

1. **Album-level propagation bug** — `AlbumEditPanel` created `tag_suggestion` rows at confidence 1.0, replacing the original MB suggestion and stripping alternatives. Fixed: panel now writes directly to `pending_tags`.
2. **Import dialog showing wrong data** — `SubmitDialog` read `suggestion.suggested_tags` from the first suggestion, bypassing all user edits. Fixed: loads `pending_tags` per track for a read-only preview.
3. **Rescan always returning the same wrong album** — `mb_lookup` re-ran every time. Fixed: early return if `pending_tags` is already set.
4. **Manual search returning ≤5 results with no track numbers** — limit was 5, no `tracknumber` injection, query too strict. Fixed: limit 20, tracknumber injected from release medium, flexible query.

### Changes by Layer

**DB migrations:**
- `0037_tracks_pending_tags.sql` (Postgres + SQLite) — adds `pending_tags` column

**Rust backend:**
- `src/models/mod.rs` — `Track` gains `pending_tags: Option<serde_json::Value>`
- `src/dal/mod.rs` — `Store` trait gains `get_pending_tags`, `set_pending_tags`, `clear_pending_tags`
- `src/dal/postgres.rs` + `src/dal/sqlite.rs` — implement the 3 new DAL methods
- `src/api/tracks.rs` — adds `GET/PUT/DELETE /:id/pending-tags` endpoints
- `src/jobs/mb_lookup.rs` — skips if `pending_tags` already set
- `src/jobs/process_staged.rs` — resolves tags from `pending_tags` first, clears after import
- `src/services/musicbrainz.rs` — `search_recordings` limit 20, tracknumber injection, flexible query

**Frontend:**
- `ui/src/api/tracks.ts` — `getPendingTags`, `setPendingTags`, `clearPendingTags`
- `ui/src/pages/IngestPage.tsx` — complete rewrite:
  - `TrackRow`: collapsed/expanded, suggestion bar, `WorkingTagsEditor` (auto-save on blur)
  - `AlbumEditPanel`: writes directly to `pending_tags` via API
  - `SubmitDialog`: read-only per-track preview, blocks Import if unready tracks
- `ui/src/components/IngestSearchDialog.tsx` — shows `#tracknumber` in MB results

### Design Spec

Written to `docs/plans/2026-04-25-ingest-flow-redesign.md` and committed.

## Commits

- `docs: ingest flow redesign spec — working-copy model`
- `feat: add pending_tags column to tracks (migrations 0037)`
- `feat: pending_tags DAL — Store trait + Postgres + SQLite impls`
- `feat: pending_tags API endpoints (GET/PUT/DELETE /:id/pending-tags)`
- `feat: mb_lookup skips if pending_tags already set`
- `feat: process_staged resolves tags from pending_tags first`
- `feat: search_recordings limit 20, tracknumber injection, flexible query`
- `feat: pending_tags API client functions`
- `feat: rewrite IngestPage with working-copy model`
- `feat: display tracknumber in MusicBrainz search results`
- `fix: remove unused imports (useCallback, Checkbox) from IngestPage`

## Build Status

Docker build passes — all Rust tests pass, TypeScript compiles clean.
