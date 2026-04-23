# Library UX — Round 2

**Date:** 2026-04-23  
**Branch:** 0.6  
**Commit:** ba38525

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

## Root Causes Identified (not yet fixed)

**Item 3 — m4a bitrate shows as 0k:**  
`src/tagger/mod.rs` calls `overall_bitrate()` which returns `None` for M4A
containers. No fallback. Fix: use `audio_bitrate()` from lofty, or compute from
file size / duration.

**Item 4 — unnecessary transcode jobs for same-quality m4a:**  
`src/services/transcode_compat.rs::is_compatible` has no same-codec/same-quality
passthrough rule. Bitrate guard uses `src < prof` (not `src == prof`). When
bitrate is NULL (item 3 bug), the guard is bypassed entirely — causing every
m4a track to generate a transcode job even to its own profile quality.
