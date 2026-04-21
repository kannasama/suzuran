# Design Gap Analysis

**Date:** 2026-04-21
**Status:** Reference
**Scope:** Comparison of `docs/plans/2026-04-16-suzuran-design.md` against the current codebase.

---

## Schema / Data Layer — Implemented but unused

**`api_tokens` table** (`migrations/postgres/0001_users.sql`) — Table exists; no DAL methods, no API
endpoints, no middleware that authenticates via token. Completely dead.

**`audit_log` table** (`migrations/postgres/0001_users.sql`) — Table exists, but nothing writes to it
anywhere in the codebase. Completely dead.

---

## Auth / Users — Gaps

**Per-user 2FA enforcement** — `totp_required` and `webauthn_required` fields exist on `users`, but:
- The auth middleware never checks them to gate access
- No admin API to set them per-user
- `force_password_change` exists but is never checked or enforced post-login

**User profile update** — No API endpoint to update `display_name`, `accent_color`, `base_theme`, or
`theme_id` per user. The `/account` nav link exists in `TopNav` but there is no account page or route.

**TOTP secret unencrypted** — Design says "secret (encrypted)". The model comment says
`// store encrypted in future`.

---

## Libraries — Gaps

**No scheduled auto-scan** — `scan_enabled` and `scan_interval_secs` exist in DB and model, and the
update API accepts them, but there is no background task that reads these per-library settings and
periodically enqueues scan jobs. The scheduler only processes jobs; it never creates them proactively.

**No filesystem watch** — Design specifies the `notify` crate (inotify on Linux, FSEvents on macOS)
as a scan trigger. Not present anywhere in the codebase.

**`auto_organize_on_ingest` not wired up** — The flag exists in DB, model, and the update API, but
`src/scanner/mod.rs` never enqueues `organize` jobs when it is true. The scanner auto-enqueues
`fingerprint` and `transcode`, but `organize` is missing from that path.

**Library form hides scan/ingest toggle fields** — `LibraryFormModal.tsx` passes `scan_enabled`,
`scan_interval_secs`, `auto_transcode_on_ingest`, and `auto_organize_on_ingest` through from the
existing record without exposing them as editable fields (they are forwarded as `library!.X` in the
update mutation with no corresponding UI controls).

---

## MusicBrainz / Inbox — Gaps

**No MusicBrainz text search fallback** — Design says: fall back to "MusicBrainz text search using
existing filename/tags if AcoustID returns no results." The `mb_lookup` job falls back to
`freedb_lookup` (FreeDB/CDDB) instead. `MusicBrainzService` has no text search method at all.

**Inbox missing Edit and Search actions** — Design specifies Accept · Edit · Reject · Search. Only
Accept and Reject are implemented in `InboxPage.tsx`. Edit (manual override before accepting) and
Search (manual MusicBrainz query) are absent.

**Inbox missing multiple-candidate selection** — Design: "Multiple candidates if available (user
selects the correct release/edition)." The current `SuggestionCard` renders one card per suggestion.
There is no grouping by `track_id` to present a "pick the correct release" UX.

**Batch-accept threshold hardcoded** — Design says "configurable threshold". The button in
`InboxPage.tsx` hardcodes `0.8` — no input field to adjust it.

---

## UI / Navigation — Gaps

**Issues page missing** — `TopNav` has the Issues nav link (`/issues`) but there is no page, route,
or backend for it.

**`library_admin` role does not exist in DB** — `LibraryPage.tsx` and `TopNav.tsx` check
`user?.role === 'library_admin'`, but the `users.role` column has a
`CHECK (role IN ('admin', 'user'))` constraint. A `library_admin` user can never be created at the
DB level.

**Track list is a stub** — `LibraryPage.tsx` explicitly renders "Track list coming in a future
subphase." The following design-specified features are absent:
- Actual track data rendered in the table
- Grouping (Group dropdown button exists but does nothing)
- Multi-column sort with priority stack (Sort button exists but does nothing)
- Column order customization (visibility persists via localStorage; order does not)
- Album header rows with thumbnail, track count, format summary, and per-album actions
- "No [field] tag" group with Fix tags shortcut

---

## Summary

| Area | Status |
|------|--------|
| `api_tokens` management | Missing — table only |
| `audit_log` writes | Missing — table only |
| Per-user 2FA enforcement | Missing |
| `force_password_change` enforcement | Missing |
| User profile update API | Missing |
| TOTP secret encryption | Deferred (noted in code comment) |
| Scheduled auto-scan | Missing |
| Filesystem watch (notify crate) | Missing |
| `auto_organize_on_ingest` wiring in scanner | Missing |
| Library form scan/ingest toggle controls | UI gap |
| MusicBrainz text search fallback | Missing (FreeDB used instead) |
| Inbox Edit + Search actions | Missing |
| Inbox multiple-candidate UX | Missing |
| Inbox configurable batch threshold | Hardcoded at 0.8 |
| Issues page | Nav link only |
| `library_admin` role in DB | Does not exist |
| Track list content | Stub only |
| Grouping and sort in track list | Stub buttons only |

---

## Additions Beyond the Design Spec

These features are present in the codebase but were not specified in the original design doc:

| Feature | Notes |
|---------|-------|
| Virtual Libraries | Symlink/hardlink materialized views across real libraries; `virtual_libraries`, `virtual_library_sources`, `virtual_library_tracks` tables + `virtual_sync` job |
| FreeDB/CDDB lookup | `freedb_lookup` job + `FreedBService` (gnudb.org); used as fallback when AcoustID finds nothing |
| CUE sheet splitting | `cue_split` job + `src/cue/mod.rs`; splits CUE+single-file albums into per-track DB rows |
| Normalize-on-ingest | `normalize` job; in-place format conversion triggered after fingerprinting when library `normalize_on_ingest=true` |
| Extended lossless formats | WavPack (`.wv`), Monkey's Audio (`.ape`), TrueAudio (`.tta`) in `AUDIO_EXTENSIONS` |
| `bit_depth` on tracks and encoding profiles | FLAC-relevant; stored from lofty properties, used in quality-guard logic |
| `tag_encoding` / Shift-JIS re-decode | Library-level tag encoding setting; scanner re-decodes Latin-1 frames as Shift-JIS when set |
| `ingest_dir` per library | Separate watched directory for incoming files |
| `organization_rule_id` on library | Direct library→rule association; inverts/extends the spec's rule-scoped-to-library model |
