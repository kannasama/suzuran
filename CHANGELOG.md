# Changelog

All notable changes to suzuran will be documented in this file.

## [Unreleased]

## [0.1.1] — Phase 1.1 Scaffold (2026-04-17)

### Added
- Rust/Axum server binary (`suzuran-server`) with `GET /health` endpoint returning `"ok"`
- `src/lib.rs` exposing `build_router()` as a library function for integration tests
- Integration test (`tests/health.rs`) asserting health endpoint returns 200
- 3-stage Dockerfile: `rust:1.85` builder → Node 20 UI placeholder → `debian:bookworm-slim` final image with `ffmpeg` and `fpcalc`
- `docker-compose.yml` with `app` and `db` (Postgres 16) services
- `.env.example` with `DATABASE_URL`, `JWT_SECRET`, `PORT`, `LOG_LEVEL`
- `.dockerignore` excluding build artifacts, secrets, and docs from the Docker context
