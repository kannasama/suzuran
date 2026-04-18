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
| `src/lib.rs` | Crate root — exposes all modules; re-exports `build_router()` |
| `src/main.rs` | Entry point — loads `Config`, connects DB, runs migrations, builds `AppState`, starts `axum::serve` |
| `src/app.rs` | Axum router — `GET /health` + mounts `/api/v1` |
| `src/config.rs` | `Config` struct — `from_env()` reads `DATABASE_URL`, `JWT_SECRET`, `PORT`, `LOG_LEVEL`, `RP_ID`, `RP_ORIGIN` |
| `src/error.rs` | `AppError` enum — `IntoResponse` impl; maps DB/internal errors to JSON |
| `src/state.rs` | `AppState` — holds `Arc<dyn Store>`, `Arc<Config>`, `Arc<Webauthn>`, shared via Axum `State` extractor |
| `src/models/mod.rs` | `User`, `Session`, `TotpEntry`, `WebauthnCredential`, `WebauthnChallenge`, `Setting`, `Theme`, `Library`, `Track`, `Job` with `sqlx::FromRow` and `serde` derives |
| `src/dal/mod.rs` | `Store` trait + `UpsertTrack` DTO — health check, user/session CRUD, TOTP CRUD, WebAuthn credential/challenge CRUD, settings/themes CRUD, library/track CRUD, job queue CRUD |
| `src/dal/postgres.rs` | `PgStore` — Postgres impl of `Store`; runs migrations; library + track queries |
| `src/dal/sqlite.rs` | `SqliteStore` — SQLite impl of `Store`; runs migrations; library + track queries |
| `src/tagger/mod.rs` | `read_tags` / `write_tags` — lofty-based tag read/write; returns `HashMap<String,String>` keyed by MusicBrainz field names + `AudioProperties` |
| `src/scanner/mod.rs` | `scan_library` — walks root with walkdir, SHA-256 hashes files, diffs against DB, upserts/removes tracks |
| `src/jobs/mod.rs` | `JobHandler` trait + `ScanPayload` DTO |
| `src/jobs/scan.rs` | `ScanJobHandler` — runs `scan_library`, logs result, returns JSON summary |
| `src/scheduler/mod.rs` | `Scheduler` — Tokio poll loop; claims pending jobs, semaphore-caps concurrency per type, retries on failure |
| `src/services/mod.rs` | Re-exports `auth`, `totp`, `webauthn` service modules |
| `src/services/auth.rs` | `AuthService` — Argon2 hashing, JWT sign/verify, login flow with `LoginResult` enum, `2fa_pending` token issue/decode, `create_full_session` |
| `src/services/totp.rs` | `TotpService` — TOTP secret generation, otpauth URI, code verification |
| `src/services/webauthn.rs` | `WebauthnService` — passkey registration/authentication start+finish flows |
| `src/api/mod.rs` | `api_router()` — mounts `/auth`, `/totp`, `/webauthn`, `/settings`, `/themes`, `/libraries`, `/jobs` subrouters |
| `src/api/libraries.rs` | Handlers: `GET /` (list), `GET /:id`, `POST /` (admin), `PUT /:id` (admin), `DELETE /:id` (admin), `GET /:id/tracks` |
| `src/api/jobs.rs` | Handlers: `GET /` (list+filter), `GET /:id`, `POST /:id/cancel` (admin), `POST /scan` (admin, enqueue scan) |
| `src/api/auth.rs` | Handlers: `POST /register`, `POST /login` (returns 204 or 200+2fa token), `POST /logout`, `GET /me` |
| `src/api/totp.rs` | Handlers: `POST /enroll`, `POST /verify`, `POST /complete` (2fa→session), `DELETE /disenroll` |
| `src/api/webauthn.rs` | Handlers: register/authenticate challenge+complete, `GET /credentials`, `DELETE /credentials/:id` |
| `src/api/settings.rs` | Handlers: `GET /` (list), `GET /:key`, `PUT /:key` (admin-only write) |
| `src/api/themes.rs` | Handlers: `GET /`, `POST /` (admin), `GET /:id`, `PUT /:id` (admin), `DELETE /:id` (admin) |
| `src/api/middleware/mod.rs` | Re-exports `auth` and `admin` middleware modules |
| `src/api/middleware/auth.rs` | `AuthUser` extractor — verifies session cookie JWT + DB session row; rejects `tfa:true` tokens |
| `src/api/middleware/admin.rs` | `AdminUser` extractor — wraps `AuthUser`, additionally requires `role = "admin"` |

## Tests

| File | Owns |
|------|------|
| `tests/health.rs` | Integration test: `GET /health` → `{"status":"ok"}` |
| `tests/settings.rs` | Integration tests: settings auth gate, default seed data, admin update, themes CRUD |
| `tests/auth.rs` | Integration tests: register→admin, login sets cookie, `/me` requires auth, `/me` returns user |
| `tests/totp.rs` | Integration tests: TOTP enroll returns otpauth URI, enroll then disenroll |
| `tests/scanner.rs` | Integration tests: scanner inserts new files, removes deleted files, skips unchanged files |
| `tests/scheduler.rs` | Integration test: end-to-end scan job enqueue → scheduler picks up → track appears in library |

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
| `0007_webauthn_challenge_uq.sql` | `UNIQUE (user_id, kind)` constraint on `webauthn_challenges` (enables upsert) |

### `migrations/sqlite/`

| File | Owns |
|------|------|
| `0001_users.sql` | Same as Postgres equivalent — SQLite types (`INTEGER`, `TEXT`) |
| `0002_two_factor.sql` | Same as Postgres equivalent — SQLite types |
| `0003_settings_themes.sql` | Same as Postgres equivalent — `css_vars` as `TEXT` (not JSONB) |
| `0004_libraries.sql` | Same as Postgres equivalent — SQLite types |
| `0005_tracks.sql` | Same as Postgres equivalent — `tags` as `TEXT` (not JSONB) |
| `0006_jobs.sql` | Same as Postgres equivalent — `payload`/`result` as `TEXT` |
| `0007_webauthn_challenge_uq.sql` | Unique index on `webauthn_challenges(user_id, kind)` (enables upsert) |

## Directories

| Directory | Owns |
|-----------|------|
| `docs/plans/` | Implementation plans — date-prefixed kebab-case filenames |
| `migrations/postgres/` | Postgres SQL migrations (0001–0007, Phase 1 schema) |
| `migrations/sqlite/` | SQLite SQL migrations (0001–0007, Phase 1 schema) |
| `resources/` | App assets (logos, icons, etc.) |
| `scripts/` | Developer tooling scripts |
| `secrets/` | Local secret files (gitignored except README) |
| `ui/` | Web frontend source _(to be populated)_ |
