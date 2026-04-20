# Changelog

All notable changes to suzuran will be documented in this file.

## [Unreleased]

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
