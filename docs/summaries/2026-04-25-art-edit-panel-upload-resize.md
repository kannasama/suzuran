# Art in Edit Panel, Upload/Drag-Drop, Resizable Pane

**Branch:** `0.8`  
**Commit:** `6db2812`

## What Was Implemented

### TrackEditPanel — art zone + layout restructure
- Added 96×96 art zone on the left side of the panel with a vertical `border-r` divider before the tag fields
- Zone shows embedded track art (`/api/v1/tracks/{id}/art`) or suggestion `cover_art_url` as preview
- Drag-and-drop image files onto zone; click to browse via hidden file input
- Upload path: POST to `/api/v1/uploads/images` → URL → stored as `artUrl` state
- `cover_art_url` passed into `tagSuggestionsApi.create()` on save — enables per-track/per-disc art
- Clear button appears when an art URL is set

### ArtUpdateModal — file upload + drag-and-drop
- Replaced plain URL text input with a drop zone (dashed border, accent highlight on dragover) + click-to-browse
- Preview thumbnail renders once a URL is set (upload or typed)
- URL input still present below drop zone for manual entry
- Embed button disabled while upload is in progress

### BulkEditPanel — resizable height, persisted
- Wrapped BulkEditPanel in a sized container with a 6px drag handle at the top edge
- `mousedown` → `mousemove` on `window` → clamps height 160px–80vh
- On `mouseup`: persists to `useUserPrefs` under key `library.editPanelHeight`
- localStorage-first, backend-synced (same pattern as column widths and sort prefs)
- `DEFAULT_EDIT_PANEL_HEIGHT = 320` exported from `useUserPrefs.ts`
- BulkEditPanel's own container changed from `maxHeight: 45vh` to `h-full` to fill its wrapper

## Key Decisions
- Art zone is `flex-shrink-0` at fixed 120px wide — keeps fields grid stable regardless of art state
- Per-disc art use case: since each TrackEditPanel targets a single track, art uploads are per-track by design; no album-scope art proliferation needed in this path
- Drag handle styled with `hover:bg-accent/20` — visible affordance without occupying layout space

## Feedback Captured
- User requested art on left with divider bar before fields (not above them)
- User explicitly requested resizable edit pane with persisted sizing
- Per-disc different covers cited as motivation for drag-and-drop in edit panel specifically
