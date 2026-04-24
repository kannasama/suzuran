# Library UX — Round 2

**Date:** 2026-04-23  
**Branch:** 0.6  
**Commits:** ba38525 (UI round 1), 7df32a4 (backend fixes), 474b036 (derived tracks)

## What Was Implemented

All changes in `ui/src/pages/LibraryPage.tsx` and `ui/src/components/TrackEditPanel.tsx`.

**⋯ context menu (item 1)**
- `⋯` button no longer expands an inline row; instead opens a positioned
  context menu built from a `MenuItem[]` items array
- Items: Lookup, Search — always present; Accept (n%), Reject — when a pending
  suggestion exists; Alternatives… — when suggestion has alternatives
- AlternativesPanel still expands inline, triggered from the menu via
  `altPanelTrackId` state (replaces `expandedTrackId`)
- Same generic `ContextMenu` component used for both right-click and `⋯` menus
- Button gets `getBoundingClientRect()` and passes `(rect.left, rect.bottom+4)`
  to avoid stale synthetic event issues

**Multi-level Sort By (item 2)**
- `sortLevels: SortLevel[]` replaces `sortBy/sortDir`
- Each level: key selector, asc/desc toggle (▲/▼), remove button (×)
- `+ Add level` appends a new level defaulting to `tracknumber asc`
- `sortTracks()` iterates levels sequentially; first non-zero comparison wins
- Disc # added to `SORT_OPTIONS`

**Edit panel field layout (item 6)**
- `BULK_EDIT_FIELDS` in LibraryPage and `EDIT_TAG_FIELDS` in TrackEditPanel now
  both use a 6-column grid with per-field `cols` spans
- Total Tracks (1 col) placed immediately after Track # (1 col)
- Total Discs (1 col) placed immediately after Disc # (1 col)
- MB ID fields each span the full 6 columns
- `COL_SPAN: Record<number, string>` lookup prevents Tailwind purging dynamic
  class strings

**m4a bitrate fix (item 3)**
- `src/tagger/mod.rs`: `overall_bitrate()` returns `None` for M4A containers
  (lofty doesn't synthesise bitrate from the MP4 container)
- Fix: `.or_else(|| file_props.audio_bitrate())` — falls back to the AAC stream
  bitrate stored in the MP4 container metadata

**Same-format/quality transcode skip (item 4)**
- Root cause: `is_compatible` in `transcode_compat.rs` only guards against
  upscaling; it has no "source already satisfies profile" check
- The two bugs interact: NULL bitrate causes the lossy upscale guard to be
  bypassed entirely (the `if let` doesn't match), so every m4a-192k source
  passes `is_compatible` against an m4a-192k profile and gets a transcode job
- Fix: new `is_noop_transcode()` function — returns true when codecs match AND
  source quality meets or exceeds profile target
- Wired into `transcode.rs` as a second skip check after `is_compatible`
- `is_compatible` semantics preserved unchanged (upscaling guard only); existing
  tests all pass

**Derived tracks as child rows (item 5)**
- `src/dal/`: new `list_track_links_by_library(library_id)` method (postgres +
  sqlite); single JOIN query — no N+1 per track
- `src/api/libraries.rs`: `list_tracks` now returns `Vec<TrackRow>` where
  `TrackRow = { ...Track, derived_tracks: Vec<Track> }` (flattened via serde);
  derived tracks are removed from the top-level list and nested under their
  source; sorted by bitrate desc (highest quality first)
- `ui/src/types/track.ts`: `derived_tracks?: Track[]` added
- `ui/src/pages/LibraryPage.tsx`: each `TrackRow` is wrapped in
  `React.Fragment` and followed by zero or more `DerivedTrackRow` entries
- `DerivedTrackRow`: `↳` connector in checkbox column; profile dir name
  (first path segment of `relative_path`) in title column; format/bitrate/
  duration from the derived track; no checkbox, not selectable
