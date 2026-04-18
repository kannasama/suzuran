---
name: Codebase file map
description: Lightweight index of every significant file — what it does and what it owns, to avoid re-exploring the codebase each session
type: reference
---

> **Usage:** Check this before reading any file. If the description is enough, skip the read.
> **Maintenance:** Update entries when files are created, deleted, or significantly changed.

## Build Commands

```bash
docker buildx build --progress=plain -t suzuran:dev .
docker compose up --build -d
docker compose down
docker compose logs -f app
```

## Project Root

| File | Owns |
|------|------|
| `CLAUDE.md` | Claude Code guidance: design context, workflow rules, repo layout |
| `CHANGELOG.md` | Release history |
| `TODO.md` | Informal task list and ideas |
| `.impeccable.md` | Design context for impeccable skills |
| `.env.example` | Required env vars with safe defaults |
| `.dockerignore` | Docker build exclusions |
| `Cargo.toml` | Rust package manifest — bin + lib targets, all dependencies |
| `Cargo.lock` | Locked dependency versions |
| `Dockerfile` | 3-stage build: rust-builder (1.88) → ui-builder placeholder → debian:bookworm-slim |
| `docker-compose.yml` | App + Postgres (16-alpine) services |
| `tasks/lessons.md` | Process rules and lessons learned (authoritative, git-tracked) |
| `tasks/codebase-filemap.md` | This file — lightweight codebase index |

## Source

| File | Owns |
|------|------|
| `src/lib.rs` | Crate root — exposes `config`, `dal`, `error`, `state` modules; re-exports `build_router()` |
| `src/main.rs` | Entry point — loads `Config`, connects DB, runs migrations, builds `AppState`, starts `axum::serve` |
| `src/app.rs` | Axum router — `build_router(AppState)`, `GET /health` with live DB check |
| `src/config.rs` | `Config` struct — `from_env()` reads `DATABASE_URL`, `JWT_SECRET`, `PORT`, `LOG_LEVEL` |
| `src/error.rs` | `AppError` enum — `IntoResponse` impl; maps DB/internal errors to JSON error responses |
| `src/state.rs` | `AppState` — holds `Arc<dyn Store>` and `Arc<Config>`, shared via Axum `State` extractor |
| `src/dal/mod.rs` | `Store` trait — `async health_check()` |
| `src/dal/postgres.rs` | `PgStore` — wraps `sqlx::PgPool`, runs Postgres migrations, implements `Store` |
| `src/dal/sqlite.rs` | `SqliteStore` — wraps `sqlx::SqlitePool`, runs SQLite migrations, implements `Store` |

## Tests

| File | Owns |
|------|------|
| `tests/health.rs` | Integration test: builds `AppState` with in-memory SQLite, asserts `GET /health` → `{"status":"ok"}` |

## Migrations

### `migrations/postgres/`

| File | Owns |
|------|------|
| `0001_users.sql` | `users`, `sessions`, `api_tokens`, `audit_log` |
| `0002_two_factor.sql` | `totp_entries`, `webauthn_credentials`, `webauthn_challenges` |
| `0003_settings_themes.sql` | `settings` (key-value + seed data), `themes` |
| `0004_libraries.sql` | `libraries` (self-referential via `parent_library_id`) |
| `0005_tracks.sql` | `tracks` (JSONB `tags`, indexed common fields) |
| `0006_jobs.sql` | `jobs` (type + status CHECK constraints, priority index) |

### `migrations/sqlite/`

| File | Owns |
|------|------|
| `0001_users.sql` | Same as Postgres equivalent — SQLite types (`INTEGER`, `TEXT`) |
| `0002_two_factor.sql` | Same as Postgres equivalent — SQLite types |
| `0003_settings_themes.sql` | Same as Postgres equivalent — `css_vars` as `TEXT` (not JSONB) |
| `0004_libraries.sql` | Same as Postgres equivalent — SQLite types |
| `0005_tracks.sql` | Same as Postgres equivalent — `tags` as `TEXT` (not JSONB) |
| `0006_jobs.sql` | Same as Postgres equivalent — `payload`/`result` as `TEXT` |

## Directories

| Directory | Owns |
|-----------|------|
| `docs/plans/` | Implementation plans — date-prefixed kebab-case filenames |
| `migrations/postgres/` | Postgres SQL migrations (0001–0006, Phase 1 schema) |
| `migrations/sqlite/` | SQLite SQL migrations (0001–0006, Phase 1 schema) |
| `resources/` | App assets (logos, icons, etc.) |
| `scripts/` | Developer tooling scripts |
| `secrets/` | Local secret files (gitignored except README) |
| `ui/` | Web frontend source _(to be populated)_ |
