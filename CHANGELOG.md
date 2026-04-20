# Changelog

All notable changes to suzuran will be documented in this file.

## [Unreleased]

## [v0.4.0] — 2026-04-20

### Added
- Extended ingest format support: WavPack (.wv), Monkey's Audio (.ape), TrueAudio (.tta)
- `tracks.bit_depth` populated from lofty for lossless formats
- CUE+audio sheet splitting — scanner detects paired CUE+audio files (any format), splits via `ffmpeg -c copy`, writes CUE metadata via lofty; idempotent on re-scan
- Encoding profiles — configurable codec, bitrate, sample rate, channels, bit_depth ceiling, advanced ffmpeg args
- Art profiles — max dimensions, size limit, JPEG/PNG format, quality setting
- Track links — records source→derived relationships for transcoded tracks
- Transcode compatibility rules — no lossy→lossless, no upsampling, no bit-depth inflation, no bitrate upscaling; incompatible jobs skip cleanly
- Transcode job — ffmpeg pipeline from encoding profile, tag copy, track_links creation
- Art process job — embed (from URL), extract, standardize (resize/recompress via `image` crate)
- Normalize-on-ingest — `libraries.normalize_on_ingest` flag converts ingested files to the library's encoding profile in-place, deletes source after verified transcode; incompatible sources preserved
- Auto-transcode on ingest — child libraries with `auto_transcode_on_ingest=true` receive transcode jobs automatically
- Auto-embed art on suggestion accept — `art_process` job enqueued when suggestion has `cover_art_url`
- Virtual libraries — symlink or hardlink views of best-available tracks across priority-ordered source libraries; `virtual_sync` job materializes the view
- Track identity matching by MusicBrainz recording ID or normalized (albumartist, album, disc, track) tuple
- Transcode API: per-track, per-library bulk, and sync-missing modes
- Art API: per-track embed/extract/standardize; per-library standardize
- Settings UI: encoding profiles and art profiles management with inline forms
- Library UI: transcode dialog (all / sync) on track and library level
- Theme background image upload — `POST /api/v1/uploads/images` stores files under
  `UPLOADS_DIR` (default `/app/uploads`); files served at `/uploads/…`; mount as Docker volume

## [v0.3.0] — 2026-04-20

### Added
- Acoustic fingerprinting via fpcalc (Chromaprint) — runs automatically after scan for new tracks
- AcoustID + MusicBrainz metadata lookup job chain — suggestions written to `tag_suggestions` table
- gnudb.org (FreeDB) disc-ID lookup fallback — activates when DISCID tag present, mb_lookup finds no matches
- Tag suggestions REST API (`/api/v1/tag-suggestions`) — list, accept, reject, batch-accept
- Tagging service — apply_suggestion merges and writes tags to audio file via lofty, syncs DB
- Inbox UI — nav badge with live count, suggestion cards with tag diff view and cover art
- Batch accept action (≥ 80% confidence default)

## [0.1.1] — Phase 1.1 Scaffold (2026-04-17)

### Added
- Rust/Axum server binary (`suzuran-server`) with `GET /health` endpoint returning `"ok"`
- `src/lib.rs` exposing `build_router()` as a library function for integration tests
- Integration test (`tests/health.rs`) asserting health endpoint returns 200
- 3-stage Dockerfile: `rust:1.85` builder → Node 20 UI placeholder → `debian:bookworm-slim` final image with `ffmpeg` and `fpcalc`
- `docker-compose.yml` with `app` and `db` (Postgres 16) services
- `.env.example` with `DATABASE_URL`, `JWT_SECRET`, `PORT`, `LOG_LEVEL`
- `.dockerignore` excluding build artifacts, secrets, and docs from the Docker context
