# MBP-Style Release Matching + Ingest/Library UI Improvements

**Date:** 2026-04-21  
**Branch:** `main`  
**Commits:** `42f2c3f`, `dd90647`, `babcf42`, `72f5977`, `4665452`

---

## What Was Implemented

### Task 1 — MBP-Style Release Matching (`42f2c3f`)

Rewrote `src/jobs/mb_lookup.rs` and updated `src/services/musicbrainz.rs` to pick the best
MusicBrainz release like Picard does, rather than taking the first result blindly.

**Key changes:**

- Added `status: Option<String>` to `MbRelease` struct
- Added `score_release(release, existing_tags)` to `MusicBrainzService`:
  - Official release: +30
  - Release type: Album +40, EP +25, Single +15, Compilation +10
  - Date decay: earlier releases preferred (decay per decade, ~10pts)
  - Existing-tag seed: album match +25, albumartist +20, year match +15, totaltracks +10
- Added `AlternativeEntry` struct (`suggested_tags`, `mb_release_id`, `cover_art_url`)
- Added `pick_best_release()` — scores all releases, sorts descending, returns best + rest
- AcoustID path now creates **one suggestion per recording** with best-scored release as
  primary tags and all others stored as `alternatives: JSONB`
- Added migration `0031_tag_suggestions_alternatives.sql` for both Postgres and SQLite

**MB inc parameter fix (within Task 1):**  
The MB recording lookup was returning 400 errors due to an invalid include parameter.
Initial diagnosis blamed `release-groups` (wrong). Correct cause: `labels` — only valid on
`/release/`, not `/recording/`. Fixed inc to `releases+release-groups+artist-credits+media`.

**Struct updates required across callers:**  
After adding `alternatives` to `UpsertTagSuggestion`, all callers needed `alternatives: None`:
- `src/jobs/freedb_lookup.rs`
- `src/api/tag_suggestions.rs`
- `tests/tag_suggestions_dal.rs` (3 instantiations)
- `tests/tag_suggestions_api.rs` (1 instantiation)
- `tests/tagging_service.rs` (1 instantiation)
- `tests/musicbrainz_service.rs` (`MbRelease` structs needed `status: None`)

### Task 2 — TagDiffTable Dark Mode Fix (`dd90647`)

Added `text-text-primary` to the `<table>` element in `ui/src/components/TagDiffTable.tsx`.
Tags were rendering as black text on a dark background because the table inherited browser
default text color instead of the theme color.

### Task 3 — Expanded Tag Field Set + AlternativesPanel (`babcf42`)

**`TrackEditPanel.tsx` — expanded from 7 fields to 25:**

Five categories: Basic (title, artist, albumartist, album, tracknumber, discnumber, date,
genre), Sort (albumartistsort, artistsort), Release metadata (releasetype, releasestatus,
releasecountry, originalyear, originaldate, totaltracks, totaldiscs), Label (label,
catalognumber, barcode), MusicBrainz IDs (5 fields, all `fullWidth: true` / `col-span-2`).

All inputs use `font-mono`. 2-column grid layout, MB ID fields span full width.

**`AlternativesPanel.tsx` — new component:**

Shows alternative releases from a suggestion's `alternatives` array. Each card shows
album/artist/date + cover thumbnail. "Use this" button creates a new suggestion via
`tagSuggestionsApi.create()` using the parent suggestion's source and confidence.
Used in both Ingest and Library views.

**`ui/src/types/tagSuggestion.ts`:**  
Added `AlternativeRelease` interface and `alternatives?: AlternativeRelease[]` to `TagSuggestion`.

### Task 4 — Library Track Selection + Bulk Edit Panel (`72f5977`)

**`LibraryPage.tsx` — major additions:**

- `selectedTrackIds: Set<number>` state + `lastSelectedIdRef` for shift-click ranges
- `toggleSelectTrack(id, shiftKey, trackList)` — range selection via shift-click
- Select-all checkbox in column header (with `indeterminate` support)
- Each track row has a checkbox; selected rows get `bg-accent/10` highlight
- `BulkEditPanel` component at the bottom (visible when any tracks are selected):
  - 3-column grid of all 25 tag fields with `(unchanged)` placeholder text
  - "Apply to Selected" fans out `tagSuggestionsApi.create()` for each selected track
  - "Clear" button deselects all

### Task 5 — Ingest Album Art + Album Tag Editor (`4665452`)

**`IngestPage.tsx` — additions:**

- `ALBUM_EDIT_FIELDS` constant (17 album-scope fields excluding per-track fields like
  title, tracknumber, genre, artistsort, and MB recording/artist IDs)
- `albumArtUrls: Record<string, string>` state — persists per-album art choices
- `AlbumEditPanel` component: 3-column grid, "Apply to All" fans out one suggestion per track
- "Edit Album" toggle button in every album header → shows `AlbumEditPanel`
- "Add Art" / "Change Art" toggle in album header
- Inline art upload panel below header: `ImageUpload` component + "Remove" button; closes
  on URL commit
- Album thumbnail enlarged from `w-8 h-8` → `w-14 h-14`; uses `presetArtUrl || coverArtUrl`
- `presetArtUrl` threaded into `SubmitDialog`; `uploadedArtUrl` initialised from it so
  art chosen in the header carries through to Import without re-uploading
- "Alt…" button per track (when `suggestion.alternatives` exists) → toggles `AlternativesPanel`

---

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| One suggestion per recording (not per release) | Mirrors Picard behaviour; all releases for a recording are ranked and the best becomes primary |
| Alternatives stored as JSONB on the suggestion row | Avoids a new table; alternatives are read-only display data consumed by the UI |
| `source: 'mb_search'` for manual/album edits | Backend CHECK constraint accepts only `acoustid | mb_search | freedb`; no `manual` source exists; documented in code with comment |
| `AlbumEditPanel` excludes per-track fields | title, tracknumber, discnumber, genre, artistsort, musicbrainz_artistid, musicbrainz_trackid are meaningless to apply uniformly across all tracks of an album |
| Art URL state lifted to IngestPage | Allows preset art to flow into SubmitDialog without being reset when the dialog opens |

---

## Feedback Captured

- **MB 400 diagnosis required two iterations.** First hypothesis (remove `release-groups`) was
  wrong — the 400s persisted. Second investigation found `labels` as the culprit (`labels` is only
  a valid sub-include on `/release/`, not `/recording/`). Lesson: when an HTTP error fix doesn't
  stop the error, assume the fix was wrong; check the API docs more carefully before committing.

- **Docker cache can hide Rust compile errors.** After a commit that touched only UI files, the
  Rust layers showed as CACHED. When subsequent Rust changes appeared to produce no new build
  output, `--no-cache` was needed to confirm the code compiled cleanly. Standard practice now:
  when in doubt whether a Rust change was actually compiled, force a rebuild with `--no-cache`.

- **User confirmed: Issue 2 was about multi-file tag editing, not accept-all.** Initial
  interpretation of "no means to set tag fields across multiple files" was wrong — assistant
  thought it was about batch-accept. User clarified: it's a UI for editing shared album-level
  tags (albumartist, album, etc.) across multiple tracks simultaneously. Plan and implement
  accordingly.

- **User confirmed: Library bulk edit goes at bottom of track list pane.** When asked about
  placement, user specified: "keep the library track list as-is, but add an edit panel to the
  bottom of the track list pane when a track is (or multiple tracks are) selected." Nothing
  deferred.
