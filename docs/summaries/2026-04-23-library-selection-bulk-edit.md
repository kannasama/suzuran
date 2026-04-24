# Library Selection & Bulk Edit Redesign

**Date:** 2026-04-23  
**Branch:** 0.6  
**Commits:** 0c48db3 (feat), bb5e2ca (docs)

## What Was Implemented

All changes in `ui/src/pages/LibraryPage.tsx`.

**Row selection (foobar2000-style)**
- Click = select only that row (clears others)
- Shift+click = range select in display order
- Ctrl/Cmd+click = toggle without clearing
- Checkbox = toggle (ctrl-click equivalent); header checkbox = select all/none
- `⋯` button uses `stopPropagation` to avoid triggering row selection
- Row div gets `cursor-pointer select-none`

**BulkEditPanel as universal edit form**
- Inline `TrackEditPanel` removed from `⋯` expand; "Edit Tags" button removed
- Panel pre-populates fields with common values; `(multiple values)` placeholder
  when values differ; empty with no placeholder when all tracks have no value
- Dirty tracking: only changed fields submitted on Apply
- `key` prop remounts panel on selection change, resetting all field state
- Dirty fields show `border-accent/60` tint
- Apply disabled when no dirty fields

**Right-click context menu**
- Items: Lookup, Search | Select All, Deselect All | Copy Path
- Right-clicking unselected track selects only it first
- Dismisses on click or scroll; fixed-positioned at cursor

**Helpers added**
- `getTrackTagValue(track, key)` — top-level fields first, `track.tags` fallback
- `TOP_LEVEL_TAG_FIELDS` set
- `flatTracks` memo (display-order flatten for range selection)
- `selectedTracks` memo

## Feedback Captured

- **Do not write a plan doc when user says "take it straight to implementation"
  after brainstorming.** The brainstorm session is the plan. Writing a plan doc
  re-processes settled decisions and introduces gaps. Correction applied and
  saved to `tasks/lessons.md`.

- **Design docs are still required** to capture decisions and intent — but they
  are distinct from implementation plans. Written after implementation as a
  record of rationale, not a step-by-step recipe.
