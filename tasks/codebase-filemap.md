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
| `src/app.rs` | Axum router — `GET /health` + mounts `/api/v1` + `ServeDir("ui/dist")` SPA fallback |
| `src/config.rs` | `Config` struct — `from_env()` reads `DATABASE_URL`, `JWT_SECRET`, `PORT`, `LOG_LEVEL`, `RP_ID`, `RP_ORIGIN` |
| `src/error.rs` | `AppError` enum — `IntoResponse` impl; maps DB/internal errors to JSON |
| `src/state.rs` | `AppState` — holds `Arc<dyn Store>`, `Arc<Config>`, `Arc<Webauthn>`, `Arc<MusicBrainzService>`, `Arc<FreedBService>`, shared via Axum `State` extractor |
| `src/models/mod.rs` | `User`, `Session`, `TotpEntry`, `WebauthnCredential`, `WebauthnChallenge`, `Setting`, `Theme`, `Library`, `Track`, `Job`, `OrganizationRule`, `TagSuggestion`, `UpsertTagSuggestion` with `sqlx::FromRow` and `serde` derives |
| `src/dal/mod.rs` | `Store` trait + `UpsertTrack` DTO — health check, user/session CRUD, TOTP CRUD, WebAuthn credential/challenge CRUD, settings/themes CRUD, library/track CRUD (incl. `update_track_path`, `update_track_fingerprint`, `update_track_tags`), job queue CRUD, organization rule CRUD, tag suggestion CRUD (`create`, `list_pending`, `get` → `Option<TagSuggestion>`, `set_status` with rows_affected 404 guard, `count`) |
| `src/dal/postgres.rs` | `PgStore` — Postgres impl of `Store`; runs migrations; library + track queries |
| `src/dal/sqlite.rs` | `SqliteStore` — SQLite impl of `Store`; runs migrations; library + track queries |
| `src/organizer/mod.rs` | Organizer module root — re-exports `conditions`, `rules`, and `template` submodules |
| `src/organizer/conditions.rs` | `Condition` enum + `eval_condition` — serde-tagged condition tree evaluator; supports comparison (eq/ne/contains/starts_with/ends_with), and/or/not, empty/nonempty; all comparisons case-insensitive |
| `src/organizer/rules.rs` | `match_rule` / `apply_rules` — evaluates a priority-ordered rule list against a tag map; returns first matching rendered path template |
| `src/organizer/template.rs` | `render_template` — renders path templates from tag maps; supports `{field}`, `{field:02}` zero-pad, `{field\|fallback}`, `{discfolder}` synthetic token |
| `src/tagger/mod.rs` | `read_tags` / `write_tags` — lofty-based tag read/write; returns `HashMap<String,String>` keyed by MusicBrainz field names + `AudioProperties` |
| `src/scanner/mod.rs` | `scan_library` — walks root with walkdir, SHA-256 hashes files, diffs against DB, upserts/removes tracks; enqueues `fingerprint` job for each newly inserted track |
| `src/jobs/mod.rs` | `JobHandler` trait + `ScanPayload` + `OrganizePayload` + `FingerprintPayload` DTOs |
| `src/jobs/scan.rs` | `ScanJobHandler` — runs `scan_library`, logs result, returns JSON summary |
| `src/jobs/organize.rs` | `OrganizeJobHandler` — evaluates rules against a track, moves the file via `tokio::fs::rename`, updates `tracks.relative_path` in DB; supports `dry_run` mode |
| `src/jobs/fingerprint.rs` | `FingerprintJobHandler` — spawns `fpcalc -json` as async subprocess, parses fingerprint + duration, calls `update_track_fingerprint` |
| `src/jobs/freedb_lookup.rs` | `FreedBLookupJobHandler` — reads `DISCID` tag, calls `FreedBService::disc_lookup`, creates one `tag_suggestion` row with `confidence = 0.5`; skips cleanly if no DISCID |
| `src/jobs/mb_lookup.rs` | `MbLookupJobHandler` — AcoustID lookup via `MusicBrainzService`, creates `tag_suggestion` rows for results ≥ 0.8; enqueues `freedb_lookup` fallback if zero suggestions |
| `src/scheduler/mod.rs` | `Scheduler` — Tokio poll loop; claims pending jobs, semaphore-caps concurrency per type, retries on failure; takes `Arc<MusicBrainzService>` + `Arc<FreedBService>` to construct handlers |
| `src/services/mod.rs` | Re-exports `auth`, `freedb`, `musicbrainz`, `tagging`, `totp`, `webauthn` service modules |
| `src/services/auth.rs` | `AuthService` — Argon2 hashing, JWT sign/verify, login flow with `LoginResult` enum, `2fa_pending` token issue/decode, `create_full_session` |
| `src/services/freedb.rs` | `FreedBService` — gnudb.org CDDB disc-ID lookup (query + read, two HTTP calls), XMCD response parsing, `to_tag_map` (candidate → tag HashMap) |
| `src/services/musicbrainz.rs` | `MusicBrainzService` — AcoustID fingerprint lookup, MusicBrainz recording fetch (with 1.1s rate limit), `to_tag_map` (recording+release → tag HashMap), `caa_url` (Cover Art Archive URL) |
| `src/services/totp.rs` | `TotpService` — TOTP secret generation, otpauth URI, code verification |
| `src/services/webauthn.rs` | `WebauthnService` — passkey registration/authentication start+finish flows |
| `src/api/mod.rs` | `api_router()` — mounts `/auth`, `/totp`, `/webauthn`, `/settings`, `/themes`, `/libraries`, `/jobs`, `/tracks`, `/organization-rules`, `/tag-suggestions` subrouters |
| `src/api/libraries.rs` | Handlers: `GET /` (list), `GET /:id`, `POST /` (admin), `PUT /:id` (admin), `DELETE /:id` (admin), `GET /:id/tracks` |
| `src/api/jobs.rs` | Handlers: `GET /` (list+filter), `GET /:id`, `POST /:id/cancel` (admin), `POST /scan` (admin, enqueue scan) |
| `src/api/auth.rs` | Handlers: `POST /register`, `POST /login` (returns 204 or 200+2fa token), `POST /logout`, `GET /me` |
| `src/api/totp.rs` | Handlers: `POST /enroll`, `POST /verify`, `POST /complete` (2fa→session), `DELETE /disenroll` |
| `src/api/webauthn.rs` | Handlers: register/authenticate challenge+complete, `GET /credentials`, `DELETE /credentials/:id` |
| `src/api/settings.rs` | Handlers: `GET /` (list), `GET /:key`, `PUT /:key` (admin-only write) |
| `src/api/themes.rs` | Handlers: `GET /`, `POST /` (admin), `GET /:id`, `PUT /:id` (admin), `DELETE /:id` (admin) |
| `src/api/tracks.rs` | `GET/HEAD /:id/stream` — byte-range streaming with `Content-Range`, `Accept-Ranges`, `X-File-Size`, `X-Duration-Secs`, `X-Bitrate`, `X-Sample-Rate` headers |
| `src/api/organization_rules.rs` | Handlers: `GET /` (list, optional `?library_id=N`), `POST /` (admin, create → 201), `GET /:id`, `PUT /:id` (admin), `DELETE /:id` (admin → 204), `POST /preview` (admin, dry-run path proposals), `POST /apply` (admin, enqueue organize jobs) |
| `src/api/tag_suggestions.rs` | Handlers: `GET /` (list pending, optional `?track_id=N`, auth), `GET /count` (public nav badge), `GET /:id` (auth, 404 if missing), `POST /:id/accept` (auth, calls tagging stub + sets status), `POST /:id/reject` (auth), `POST /batch-accept` (auth, body `{min_confidence}`) |
| `src/services/tagging.rs` | `apply_suggestion` — merges existing track tags with suggestion tags, writes to audio file via `tagger::write_tags`, updates DB via `update_track_tags` |
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
| `tests/streaming.rs` | Integration tests: full file stream, byte-range (206), HEAD metadata headers, auth guard |
| `tests/organization_rules.rs` | DAL tests: CRUD for organization_rules — create global/scoped rules, list, get, update, delete |
| `tests/organizer_conditions.rs` | Unit tests for `eval_condition`, `match_rule`, `apply_rules` — 18 cases covering all condition types, logical composites, presence checks, and rule priority |
| `tests/organizer_template.rs` | Unit tests for `render_template` — 12 cases covering all token types and edge cases |
| `tests/organize_job.rs` | Integration tests for `OrganizeJobHandler` — file move + DB path update, dry-run mode |
| `tests/organization_rules_api.rs` | Integration tests for organization rules REST API — full CRUD flow (create, list, list-filtered, get, update, delete) and auth guard (unauthenticated → 401) |
| `tests/tag_suggestions_dal.rs` | DAL tests for tag_suggestions CRUD — create, list pending (unfiltered + by track_id), set status, count, get by id (returns `Option`) |
| `tests/tag_suggestions_api.rs` | Integration tests for tag suggestions REST API — auth guards, public count, 404 on missing id, list/filter, reject, accept, batch-accept threshold (15 tests) |
| `tests/fingerprint_job.rs` | Tests for `FingerprintJobHandler` — DAL fingerprint update (with tag merge + duration), error cases (missing/nonexistent track_id), scan auto-enqueue; fpcalc integration test skips gracefully when fpcalc not on PATH |
| `tests/musicbrainz_service.rs` | wiremock tests for `MusicBrainzService` — AcoustID lookup (scored results, empty results), MB recording fetch, `to_tag_map` field extraction |
| `tests/mb_lookup_job.rs` | wiremock integration tests for `MbLookupJobHandler` — creates suggestion on ≥0.8 score, skips below threshold + enqueues freedb_lookup, errors on missing fingerprint |
| `tests/freedb_service.rs` | wiremock tests for `FreedBService` — disc lookup (two-mock query+read), 202 no-match, read failure, `to_tag_map` field extraction |
| `tests/freedb_lookup_job.rs` | wiremock integration tests for `FreedBLookupJobHandler` — creates suggestion for DISCID track, skips without DISCID, zero suggestions on no match, error on missing track |
| `tests/common/mod.rs` | Shared test helpers: `make_db()`, `setup_with_fingerprinted_track()`, `setup_with_discid_track()`, `setup_with_track()`, `setup_with_audio_track()` (FLAC with VORBISCOMMENT for tagging tests), `TAGGED_FLAC` bytes constant |
| `tests/tagging_service.rs` | Integration tests for `apply_suggestion` — file + DB updated, indexed artist column correct, title preserved from merge, NotFound on missing track |

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
| `0008_organization_rules.sql` | `organization_rules` table (BIGSERIAL id, JSONB conditions, priority, path_template, enabled) with library FK |
| `0009_tag_suggestions.sql` | `tag_suggestions` table (BIGSERIAL id, track FK, source CHECK, JSONB suggested_tags, confidence, mb IDs, status CHECK) |
| `0010_jobs_add_freedb_lookup.sql` | Expands `job_type` CHECK constraint to include `freedb_lookup` via ALTER TABLE DROP/ADD CONSTRAINT |

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
| `0008_organization_rules.sql` | `organization_rules` table (INTEGER id, TEXT conditions, priority, path_template, enabled) with library FK |
| `0009_tag_suggestions.sql` | `tag_suggestions` table (INTEGER id, track FK, TEXT source, TEXT suggested_tags, confidence, mb IDs, status) |
| `0010_jobs_add_freedb_lookup.sql` | Recreates `jobs` table to add `freedb_lookup` to the `job_type` CHECK constraint |

## Directories

| Directory | Owns |
|-----------|------|
| `docs/plans/` | Implementation plans — date-prefixed kebab-case filenames |
| `migrations/postgres/` | Postgres SQL migrations (0001–0007, Phase 1 schema) |
| `migrations/sqlite/` | SQLite SQL migrations (0001–0007, Phase 1 schema) |
| `resources/` | App assets (logos, icons, etc.) |
| `scripts/` | Developer tooling scripts |
| `secrets/` | Local secret files (gitignored except README) |
| `ui/` | React + Vite + Tailwind SPA — `npm run build` → `ui/dist/` |
| `ui/src/theme/` | `tokens.ts` (dark/light CSS vars) + `ThemeProvider.tsx` (context + `applyTokens`) |
| `ui/src/types/` | `tagSuggestion.ts` — `TagSuggestion` interface (id, track_id, source, suggested_tags, confidence, mb IDs, cover_art_url, status, created_at) |
| `ui/src/api/` | `client.ts` (Axios), `auth.ts` (login/register/logout/me), `libraries.ts` (list, create, update, delete), `organizationRules.ts` (list, create, update, delete org rules), `tagSuggestions.ts` (listPending, count, accept, reject, batchAccept) |
| `ui/src/contexts/` | `AuthContext.tsx` — current user context, `useAuth` hook |
| `ui/src/pages/` | `LoginPage.tsx`, `RegisterPage.tsx`, `LibraryPage.tsx` (two-pane layout; wires `useAuth` → `isAdmin` + `selectedLibraryId` → `LibraryTree`), `OrganizationPage.tsx` (organization rules management, admin only), `InboxPage.tsx` (tag suggestion review — list, accept/reject per item, batch-accept ≥80%) |
| `ui/src/components/` | `TopNav.tsx` (nav bar), `LibraryTree.tsx` (real data, hierarchy, admin create/edit/delete), `LibraryFormModal.tsx` (create/edit modal with TanStack Query mutations), `RuleEditor.tsx` (modal for create/edit organization rules), `TemplatePreview.tsx` (client-side template renderer for live preview) |
