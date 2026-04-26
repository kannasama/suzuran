# Ingest Flow Redesign

**Date:** 2026-04-25
**Branch:** fix/cue-split-ingest-flow (or new phase branch)
**Status:** Design approved

## Problem

The current ingest flow is convoluted and inconsistent:

- Album-level alternative selection does not reliably propagate to track-level fields
- `AlbumEditPanel` creates new `tag_suggestion` rows at confidence 1.0, silently replacing the original MB suggestion and stripping alternatives
- The Import dialog reads raw `suggestion.suggested_tags` for the first track — bypassing all edits the user made, presenting stale/incorrect data
- After rescanning, `mb_lookup` re-runs and overwrites user work with the same wrong fingerprint match
- Manual search returns at most 5 results and omits track number information
- The "Apply" button in `IngestDiffPanel` is ambiguous — it conflates "apply suggestion to file" with "accept this as correct"
- The "Save" button in `TrackEditPanel` is ambiguous — unclear what is being persisted
- No multi-machine persistence: all working state is in React component state

## Design

### Mental model

Suggestions are candidate sources — they provide a baseline, not a final answer. Manual review is always required before import. The user's working copy is the authoritative tag state; suggestions are tools to populate it.

A per-track **working copy** (`pending_tags`) stores the user's finalized tag values. Suggestions, album edits, and manual edits all write into this buffer. Import reads from it. The suggestion system becomes a source, not the destination.

---

### Section 1: Data model

**New column:**

```sql
-- Postgres
ALTER TABLE tracks ADD COLUMN pending_tags JSONB;

-- SQLite
ALTER TABLE tracks ADD COLUMN pending_tags TEXT;
```

`pending_tags` starts `NULL`. It is populated when the user first edits or applies a suggestion. It is cleared back to `NULL` after a successful `process_staged` import.

**Three new API endpoints (AuthUser required):**

| Method | Path | Behaviour |
|--------|------|-----------|
| `GET` | `/tracks/:id/pending-tags` | Returns `{ tags: {...} }` or `{ tags: {} }` if null |
| `PUT` | `/tracks/:id/pending-tags` | Body `{ tags: {...} }` — upserts the full pending_tags object |
| `DELETE` | `/tracks/:id/pending-tags` | Sets pending_tags to NULL (Reset action) |

**`process_staged` change:** when `pending_tags` is set, use those tags as the write payload instead of the best `tag_suggestion`. `tag_suggestion_id` in `ProcessStagedPayload` becomes optional. After a successful import, `pending_tags` is set to NULL.

---

### Section 2: Track row UI

#### Collapsed state

Single-line row per track:
- Track number
- Title (falls back to filename)
- Filename (secondary, muted)
- Confidence badge — blue ≥ 80%, amber < 80%, grey = none
- Status pill:
  - `ready` — `pending_tags` has all required fields (`title`, `tracknumber`, `album`, `albumartist`, `date`)
  - `review` — low confidence or one or more required fields missing
  - `no match` — no suggestion and `pending_tags` is null
- Compact tag preview: `title · artist · track#` — reads from `pending_tags` if set, else current track tags
- Chevron toggle

#### Expanded state

**Suggestion bar** — bordered box at top of expanded panel, color-coded:
- Blue border/tint: confidence ≥ 80%
- Amber border/tint: confidence < 80%
- Grey/neutral: no suggestion

Contents: source label · confidence % · one-line summary (album · artist · year · track N of M)

Actions in suggestion bar:
- `Apply Suggested` — writes the suggestion's full tag set to `pending_tags` via `PUT /tracks/:id/pending-tags`, then refreshes the fields below. Does not auto-apply on alternative selection.
- `Alternatives ▾` — dropdown to switch the displayed suggestion to a different release for this track only. Selecting an alternative updates the suggestion bar display only; the user still clicks `Apply Suggested` to apply. (Distinct from the album-level alternative dropdown in the album header, which targets all tracks.)
- `Search` — opens `IngestSearchDialog`
- `Lookup` — enqueues `fingerprint` job

If no suggestion exists: grey bar showing only `Search` and `Lookup`.

**Edit fields** — 2-column grid of all tag fields:
- Pre-populated from `pending_tags` if set, otherwise from the track's current scanned tags
- Every field blur fires `PUT /tracks/:id/pending-tags` with the full current field state
- Fields with missing values highlight amber
- Fields modified from original scanned values highlight accent (blue)

**Footer:**
- `Reset` — fires `DELETE /tracks/:id/pending-tags`, reverts all fields to original scanned tag values
- Autosave note: "Working copy — edits saved automatically"

---

### Section 3: Album-level editing

**"Edit Album" panel** — behaviour change:

Previously created new `tag_suggestion` rows. Now writes directly to `pending_tags` for all tracks in the group via `PUT /tracks/:id/pending-tags` per track, merging album-scope fields into existing `pending_tags` (or current scanned tags if null). Track-specific fields (`title`, `tracknumber`, `artist`) are preserved.

Album-scope fields managed by "Edit Album":
`album`, `albumartist`, `albumartistsort`, `date`, `originalyear`, `originaldate`, `releasetype`, `releasestatus`, `releasecountry`, `totaltracks`, `totaldiscs`, `label`, `catalognumber`, `barcode`, `musicbrainz_albumartistid`, `musicbrainz_releasegroupid`, `musicbrainz_releaseid`

**New: "Apply Suggested" button in Edit Album panel** — pulls album-scope fields from the best available suggestion (or currently selected alternative) into the Edit Album form fields. User reviews and adjusts before clicking "Apply to All."

**Album-level alternative dropdown** (in album header):

Selecting an alternative updates the suggestion bar for all tracks in the group — display only, does not auto-apply. The user clicks `Apply Suggested` per track, or uses Edit Album → Apply to All to push the alternative's album-scope fields to all working copies.

This eliminates the current propagation bug where the confidence-1.0 suggestion overwrite silently discarded album-level alternative selections.

---

### Section 4: Import dialog

The dialog is "confirm and configure" only — no tag editing inside it.

**Per-track summary** (replaces the current `suggestion.suggested_tags.slice(0,10)` dump):
- One row per track: track number · title · status indicator
- Required fields missing are flagged inline (field name in amber)
- If any track is in `review` or `no match` status: warning banner + Import button disabled
- User closes dialog, fixes the issue in the track editor, re-opens Import

**Data source**: reads from `pending_tags` if set, falls back to best `tag_suggestion`. This ensures the dialog always shows what will actually be written — not stale suggestion data.

**Art section** — unchanged.

**Profiles section** — unchanged.

**Supersedes section** — unchanged. Supersede identity check uses the working copy tags (MB recording ID → tag tuple → fingerprint).

---

### Section 5: Backend job behaviour

**`mb_lookup`:** if `pending_tags IS NOT NULL` on the target track, return early without creating a new suggestion. This prevents rescans from polluting a finalized working copy with the same wrong fingerprint match.

**`process_staged`:**
- `tag_suggestion_id` in `ProcessStagedPayload` becomes `Option<i64>`
- When `pending_tags` is set: use those tags as the write payload, ignore `tag_suggestion_id`
- After successful import: set `pending_tags = NULL`

**Search (`IngestSearchDialog`):**
- Raise `search_recordings()` result limit from 5 → 20
- Add `tracknumber` (track position within the matched release) to each search result row in the dialog
- When a search result is selected, creates a `tag_suggestion` as today — which appears in the suggestion bar; user still clicks "Apply Suggested" to pull into working copy

**`tag_suggestions`:** no schema changes. Suggestions remain as candidate sources. `reject` endpoint remains but is less critical — if you don't want a suggestion, simply don't apply it.

---

## Migration notes

- Two new migrations required (one Postgres, one SQLite) for `pending_tags` column
- New migration number: check `migrations/` for current max before creating
- Existing `tag_suggestion` rows and the accept/reject flow remain intact for the batch-accept workflow (Library page suggestion review)
- The `AlbumEditPanel` source label `'mb_search'` with `confidence: 1.0` pattern is retired; no new suggestions created from album edits

## Files touched

| File | Change |
|------|--------|
| `migrations/postgres/00NN_tracks_pending_tags.sql` | ADD COLUMN pending_tags JSONB |
| `migrations/sqlite/00NN_tracks_pending_tags.sql` | ADD COLUMN pending_tags TEXT |
| `src/models/mod.rs` | Add `pending_tags: Option<serde_json::Value>` to `Track` |
| `src/dal/mod.rs` | Add `get_pending_tags`, `set_pending_tags`, `clear_pending_tags` to Store trait |
| `src/dal/postgres.rs` | Implement new DAL methods |
| `src/dal/sqlite.rs` | Implement new DAL methods |
| `src/api/tracks.rs` | Add `GET/PUT/DELETE /:id/pending-tags` handlers |
| `src/api/mod.rs` | Wire new routes |
| `src/jobs/mb_lookup.rs` | Early-return if pending_tags IS NOT NULL |
| `src/jobs/process_staged.rs` | Use pending_tags when present; clear after success |
| `src/jobs/mod.rs` | `tag_suggestion_id` in ProcessStagedPayload → `Option<i64>` |
| `src/services/musicbrainz.rs` | Raise search result limit to 20 |
| `ui/src/api/tracks.ts` | Add `getPendingTags`, `setPendingTags`, `deletePendingTags` |
| `ui/src/api/ingest.ts` | Update `submitTrack` — tag_suggestion_id optional |
| `ui/src/pages/IngestPage.tsx` | Full track row UI redesign (collapsed/expanded/suggestion bar/working copy fields) |
| `ui/src/components/IngestSearchDialog.tsx` | Display tracknumber in results; raise limit |
