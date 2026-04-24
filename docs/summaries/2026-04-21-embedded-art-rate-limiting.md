# Session Summary — Embedded Art Awareness + Service Rate Limiting

**Date:** 2026-04-21  
**Branch:** 0.6  
**Commit:** 99f1c2b

## What Was Done

### 1. Embedded art awareness in ingest UI

**Problem:** The ingest page assumed album art had to come from an upload or a lookup suggestion. Tracks that already had art embedded in the audio file were treated as having no art.

**Changes — `ui/src/pages/IngestPage.tsx`:**
- `AlbumGroup` header: added "Embedded art" emerald pill badge when `hasEmbeddedArt && !displayArtUrl`
- "Add Art" button label changes to "Replace Art" when `hasEmbeddedArt` (even without an explicit URL)
- `SubmitDialog`: replaced boolean `artSkipped` with three-way `artMode: 'use' | 'keep_embedded' | 'skip'`
  - Default is `'keep_embedded'` when embedded art exists and no suggested/uploaded art is available
  - "Keep embedded art" radio option shown only when `albumHasEmbeddedArt`
  - `writeFolderArt` updated: `folderArtFilename !== '' && (selectedArtUrl != null || artMode === 'keep_embedded')`
  - Folder art note shows "(extracted from embedded art)" when in keep_embedded mode
  - "Use suggested art" link appears when in keep_embedded/skip mode but suggested art exists

**Changes — `src/jobs/process_staged.rs`:**
- Added step 8.5: after moving file to `source/`, if `write_folder_art && cover_art_url.is_none()`, extract embedded art from the moved file and write it as `folder_art_filename` in the destination directory
- New sync helper `extract_embedded_art_sync()` reads primary tag, finds CoverFront picture (falls back to first picture)

### 2. Service rate limiting

**Problem:** `MusicBrainzService` used `std::sync::Mutex` for rate limiting — the lock was dropped before the `sleep()` call, creating a race window where concurrent requests could burst. FreeDB had no rate limiting at all.

**Changes — `src/services/musicbrainz.rs`:**
- Switched to `tokio::sync::Mutex` for both `last_mb_request` and `last_acoustid_request`
- Lock is held across the `sleep()` call, making rate limiting strictly serializing
- MB rate limit: 1100ms; AcoustID rate limit: 350ms
- `acoustid_rate_limit()` added and called in `acoustid_lookup()`
- Inline rate-limit blocks in `get_recording()` and `search_recordings()` replaced with method calls

**Changes — `src/services/freedb.rs`:**
- Added `last_request: Arc<tokio::sync::Mutex<Option<Instant>>>` to `FreedBService`
- Added `rate_limit()` async method (1000ms)
- Called at the top of `cddb_request()` and before HTML fetch in `text_search()`

## Key Technical Notes

- `std::sync::Mutex` cannot be held across `.await` — it must be released before any async suspension point, which made the old MB rate limiter ineffective under concurrent load
- `tokio::sync::Mutex` is designed for exactly this pattern: lock → check elapsed → sleep if needed → update timestamp → release
- The embedded art extraction in process_staged is a no-op (silently skipped) if no embedded art is found — not an error condition
