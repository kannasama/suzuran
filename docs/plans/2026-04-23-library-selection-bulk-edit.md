# Library View — Row Selection & Bulk Edit Redesign

**Date:** 2026-04-23  
**Branch:** 0.6  
**Status:** Implemented

## Problem

The library view had two separate editing surfaces:
- A per-row inline `TrackEditPanel` opened by an "Edit Tags" button in the `⋯` expand
- A `BulkEditPanel` that only appeared for multi-track selections and always started
  with blank fields regardless of current tag values

Selection was checkbox-only. There was no right-click context menu.

## Design Decisions

### Row selection — foobar2000 model

Plain click selects only the clicked row (clears other selections). Shift+click
extends the range from the last-clicked row. Ctrl/Cmd+click toggles a single row
without clearing others. This matches the mental model of power users who come
from media players like foobar2000.

The checkbox column is retained as a visual indicator of selected state. Clicking
the checkbox is equivalent to ctrl+click — it toggles that row's selection without
clearing others. This avoids removing visual affordance while keeping the row as
the primary selection target.

Range selection uses the display order (after group/sort), not the API order. This
is the only correct behaviour — the visible sequence is the user's frame of
reference.

### BulkEditPanel as universal edit form

The inline `TrackEditPanel` (opened per-track from the `⋯` expand) is removed.
The `BulkEditPanel` becomes the sole editing surface for both single-track and
multi-track selections.

**Why remove the inline panel:** Two editing surfaces created confusion about
which one was "the" way to edit. The bulk panel is strictly more capable — it
handles single-track editing correctly while also supporting multi-track.

**Field pre-population:** When the panel opens, each field is pre-populated with
the common value across all selected tracks. If values differ, the field is left
empty with `(multiple values)` placeholder. If all tracks have no value, the field
is empty with no placeholder.

**Dirty tracking:** A field is dirty when:
- It was `(multiple values)` and the user has typed a value, or
- It was pre-populated and the user has changed it

Only dirty fields are included in the suggestion payload. The Apply button is
disabled when nothing is dirty. Dirty fields get a subtle `border-accent/60` tint.

**Reset on selection change:** The panel remounts (via `key` prop encoding the
sorted selection IDs) whenever the selection changes. This resets all field state
to match the new selection. There is no carry-over of edits.

### Right-click context menu

Items: Lookup, Search, Select All, Deselect All, Copy Path.

Right-clicking a non-selected track selects only that track before showing the
menu (consistent with foobar2000 behaviour). Right-clicking a selected track
keeps the current selection.

The menu dismisses on any click or scroll event. It is positioned at cursor
coordinates using `position: fixed` — no portal needed.

### `⋯` expand row

Retains: Lookup, Search, Alt… (when alternatives exist), pending suggestion
display with Accept/Reject.

Removed: Edit Tags button.

The `⋯` button uses `stopPropagation` to prevent triggering row selection when
clicked.

## Implementation Notes

`getTrackTagValue(track, key)` reads top-level `Track` fields first (title,
artist, albumartist, album, tracknumber, discnumber, totaldiscs, totaltracks,
date, genre, label, catalognumber), then falls back to `track.tags[key]` for
fields stored only in the JSONB tags blob (sort fields, MB IDs, release metadata).

`flatTracks` is a memo that flattens `displayGroups` in display order — used
exclusively for range selection index lookups.

No backend changes. Tag suggestions continue to be created via the existing
`POST /tag-suggestions` endpoint with `confidence: 1.0`.
