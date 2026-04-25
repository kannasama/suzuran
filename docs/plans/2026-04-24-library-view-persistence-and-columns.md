# Library View — Persistence, Resizable Columns, and Themed Checkboxes

**Date:** 2026-04-24
**Status:** Approved

## Overview

Four improvements to the library track list:

1. Group-by and sort selection persist across reloads (previously reset on every page load)
2. Column widths persist after resizing
3. Column headers are resizable (except the checkbox column), with visible division lines, centered labels, and a grip-dot handle at each right edge
4. Checkboxes are replaced with a fully custom-styled component that matches the design system

Preferences sync to a new per-user backend API so settings follow the user across machines. A hybrid localStorage-first strategy ensures instant render with no loading flicker.

---

## 1. Backend: User Preferences API

### Database

New table `user_preferences`:

```sql
CREATE TABLE user_preferences (
  user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  key       TEXT    NOT NULL,
  value     TEXT    NOT NULL,
  PRIMARY KEY (user_id, key)
);
```

Values are always JSON-encoded text. No schema changes are needed when new preference keys are added.

### Routes

Both routes are `AuthUser`-gated (not admin-only). Mounted at `/user/prefs`.

- `GET /user/prefs` — returns all preferences for the authenticated user as `[{ key, value }]`
- `PUT /user/prefs/:key` — upserts a single preference for the authenticated user

Request body for PUT: `{ "value": "<json-encoded string>" }`

### DAL

New DAL methods added to both Postgres and SQLite implementations:

- `get_user_prefs(user_id) -> Vec<UserPref>`
- `set_user_pref(user_id, key, value) -> UserPref`

Both use upsert semantics (`INSERT … ON CONFLICT (user_id, key) DO UPDATE SET value = excluded.value`).

### Preference Keys

| Key | Type | Description |
|-----|------|-------------|
| `library.groupBy` | `GroupByKey` string | Active group-by selection |
| `library.sortLevels` | `SortLevel[]` JSON | Active sort stack |
| `library.columnWidths` | `Record<string, number>` JSON | Column key → pixel width |
| `library.visibleColumns` | `string[]` JSON | Visible column keys |

---

## 2. Frontend: Hybrid Preference Sync

### `useUserPrefs` Hook

New file: `ui/src/hooks/useUserPrefs.ts`. Manages reading, writing, and syncing all four preference keys.

**On mount:**
1. Read all four keys from localStorage immediately — these values drive the initial render (no loading state)
2. Fire `GET /user/prefs` in the background
3. When the response arrives, for each key present in the response: parse the JSON value, update React state, and refresh localStorage

**On change:**
1. Write the new value to localStorage synchronously (instant state update)
2. Fire `PUT /user/prefs/:key` in the background (fire-and-forget; failures are silent — soft preference)

### localStorage Migration

The existing `suzuran:column-visibility` key is retired. On first load:
- If `library.visibleColumns` is absent in localStorage but `suzuran:column-visibility` exists, migrate the value automatically
- Remove the old key after migration

### Integration in `LibraryPage`

Replace the four `useState` initializers for `groupBy`, `sortLevels`, `visibleColumns`, and `columnWidths` with values from `useUserPrefs`. The `setGroupBy`, `setSortLevels`, `toggleColumn`, and resize-completion handlers call the hook's setters instead of writing localStorage directly.

---

## 3. Column Resizing

### Layout Model

The current Tailwind flex-ratio and fixed-width classes are replaced with explicit pixel widths applied via inline `style={{ width: px, flexShrink: 0 }}`. This is required because flex ratios cannot be overridden at runtime.

**Default widths** (used when no saved preference exists):

| Column | Default px |
|--------|-----------|
| num | 28 |
| title | 240 |
| artist | 160 |
| album | 160 |
| year | 44 |
| genre | 100 |
| format | 52 |
| bitrate | 96 |
| duration | 44 |
| actions | 64 |

The checkbox column is fixed at 24px and is never resizable.

Minimum column width: 40px (enforced during drag).

### Resize Interaction

Implemented with direct DOM mouse events — no library needed.

1. `mousedown` on a resize handle: record `startX` and the column's current pixel width
2. `mousemove` on `document`: compute `delta = e.clientX - startX`, set `colWidths[key] = Math.max(40, startWidth + delta)` in state
3. `mouseup` on `document`: release capture, call `setColWidths(final)` which persists via the prefs hook

A `resizing` ref prevents text selection during drag. The `mousemove` and `mouseup` listeners are added on `mousedown` and removed on `mouseup`.

### Props

`TrackRow` and `DerivedTrackRow` receive `colWidths: Record<string, number>` as a prop. Both apply `style={{ width: colWidths[col.key] }}` to each cell. The header row reads the same map.

---

## 4. Column Header Visual

Based on style B (full-height separators, grip dots, no accent bottom bar).

### Structure

Each resizable header cell is `position: relative` with:
- Centered label text (`text-center`)
- `border-left: 1px solid var(--border)` (all cells except the checkbox cell)
- An absolutely-positioned resize handle `div` at the right edge: 8px wide, full height, `cursor: col-resize`
- The resize handle contains a `⋮` grip icon (`text-text-muted`); on hover it turns accent-colored

**Hover state:** cell background tints subtly (`bg-bg-row-hover`); grip icon brightens to accent.

### Checkbox Column Header

- Fixed 24px width, no resize handle, no left border
- Contains only the select-all `Checkbox` component, centered

### Typography

- Labels: `text-[11px] uppercase tracking-wider text-text-muted text-center`
- Unchanged from current style except alignment (was left-aligned)

---

## 5. Themed Checkbox Component

### `Checkbox` Component

New file: `ui/src/components/Checkbox.tsx`. Replaces all `<input type="checkbox" className="accent-[color:var(--accent)]" …>` instances in `LibraryPage`.

**Props:** `checked`, `onChange`, `indeterminate?`, `title?`, `className?`

**Visual states:**

| State | Background | Border | Mark |
|-------|-----------|--------|------|
| Unchecked | `bg-bg-base` | `#555566` | — |
| Unchecked hover | `bg-bg-base` | accent | — |
| Checked | accent fill | accent | White checkmark (`::after`) |
| Indeterminate | accent fill | accent | White horizontal bar (`::after`) |

**Dimensions:** 13×13px, 2px border-radius.

**Checkmark geometry** (CSS `::after`):
- Checked: `left: 3px; top: 1px; width: 5px; height: 8px; border: 1.5px solid` bg-base; top and left borders removed; `rotate(45deg)`
- Indeterminate: `left: 2px; top: 5px; width: 7px; height: 1.5px; background:` bg-base

### Global CSS

Three rules added to `index.css` under `.cb-themed`:

```css
.cb-themed { appearance: none; … }
.cb-themed:checked::after { … }
.cb-themed:indeterminate::after { … }
```

The `Checkbox` component applies `cb-themed` plus Tailwind utility classes for colors and transitions.

### Usage Sites

Four locations in `LibraryPage`:
1. Header row — select-all (supports `indeterminate`)
2. `TrackRow` — per-row selection checkbox
3. `BulkEditPanel` column picker — column visibility toggles
4. `SuggestionReviewPane` — field diff row checkboxes

---

## Out of Scope

- Resetting preferences to defaults (no reset UI in this feature)
- Preference sync conflict resolution beyond "backend wins on load"
- Any preferences beyond the four `library.*` keys listed above
