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
| `src/app.rs` | Axum router — `GET /health` + mounts `/api/v1` + `ServeDir(config.uploads_dir)` at `/uploads` + `ServeDir("ui/dist")` SPA fallback |
| `src/config.rs` | `Config` struct — `from_env()` reads `DATABASE_URL`, `JWT_SECRET`, `PORT`, `LOG_LEVEL`, `RP_ID`, `RP_ORIGIN`, `UPLOADS_DIR` (default `/app/uploads`) |
| `src/error.rs` | `AppError` enum — `IntoResponse` impl; maps DB/internal errors to JSON |
| `src/state.rs` | `AppState` — holds `Arc<dyn Store>` as `db`, `Arc<Config>`, `Arc<Webauthn>`, `Arc<MusicBrainzService>`, `Arc<FreedBService>`, shared via Axum `State` extractor |
| `src/models/mod.rs` | `User`, `Session`, `TotpEntry`, `WebauthnCredential`, `WebauthnChallenge`, `Setting`, `Theme`, `Library` (id, name, root_path, format, scan_enabled, scan_interval_secs, auto_organize_on_ingest, tag_encoding, organization_rule_id, is_default, maintenance_interval_secs), `LibraryProfile` (id, library_id, encoding_profile_id, derived_dir_name, include_on_submit, auto_include_above_hz, created_at), `UpsertLibraryProfile`, `Track` (includes `status: String`, `library_profile_id: Option<i64>`, `bit_depth: Option<i64>`), `Job` (includes `run_after: Option<DateTime<Utc>>`), `OrganizationRule`, `TagSuggestion` (includes `alternatives: Option<serde_json::Value>`), `UpsertTagSuggestion` (includes `alternatives: Option<serde_json::Value>`), `EncodingProfile`, `UpsertEncodingProfile`, `ArtProfile`, `UpsertArtProfile`, `TrackLink`, `VirtualLibrary`, `UpsertVirtualLibrary`, `VirtualLibrarySource` (id, library_id, library_profile_id), `VirtualLibraryTrack`, `UserPref` (key, value) with `sqlx::FromRow` and `serde` derives |
| `src/dal/mod.rs` | `Store` trait + `UpsertTrack` DTO (has explicit `Default` impl; includes `status: String`, `library_profile_id: Option<i64>`, `bit_depth: Option<i64>`) + `VirtualLibrarySourceInput` (library_id, library_profile_id, priority) — health check, user/session CRUD, TOTP CRUD, WebAuthn credential/challenge CRUD, settings/themes CRUD, library CRUD (`update_library` no longer takes normalize_on_ingest; no set_library_encoding_profile/set_library_ingest_dir/list_child_libraries), library_profiles CRUD (`create_library_profile`, `get_library_profile`, `list_library_profiles`, `update_library_profile`, `delete_library_profile`), track CRUD (incl. `update_track_path`, `set_track_status`, `list_tracks_by_status`, `list_tracks_by_profile`, `find_active_source_track_by_mb_id`, `find_active_source_track_by_tags`, `find_active_source_track_by_fingerprint`, `set_track_library_profile`, `delete_track(id)`), job queue CRUD (`enqueue_job`, `enqueue_job_after(type, payload, priority, run_after: DateTime<Utc>)`, `claim_next_job` skips jobs where `run_after > NOW()`), organization rule CRUD, tag suggestion CRUD, encoding profile CRUD, art profile CRUD, track link CRUD (`create_track_link` no encoding_profile_id param), virtual library CRUD + sources (`set_virtual_library_sources` uses `VirtualLibrarySourceInput` with library_profile_id) + tracks, user prefs CRUD (`get_user_prefs(user_id)`, `set_user_pref(user_id, key, value)` upsert); exports `UpsertTagSuggestion` + `UpsertEncodingProfile` + `UpsertArtProfile` + `UpsertVirtualLibrary` + `UpsertLibraryProfile` + `LibraryProfile` + `UserPref` |
| `src/dal/postgres.rs` | `PgStore` — Postgres impl of `Store`; runs migrations; library + track queries |
| `src/dal/sqlite.rs` | `SqliteStore` — SQLite impl of `Store`; runs migrations; library + track queries |
| `src/organizer/mod.rs` | Organizer module root — re-exports `conditions`, `rules`, and `template` submodules |
| `src/organizer/conditions.rs` | `Condition` enum + `eval_condition` — serde-tagged condition tree evaluator; supports comparison (eq/ne/contains/starts_with/ends_with), and/or/not, empty/nonempty; all comparisons case-insensitive |
| `src/organizer/rules.rs` | `match_rule` / `apply_rules` — evaluates a priority-ordered rule list against a tag map; returns first matching rendered path template |
| `src/organizer/template.rs` | `render_template` — renders path templates from tag maps; supports `{field}`, `{field:02}` zero-pad, `{field\|fallback}`, `{discfolder}` synthetic token |
| `src/cue/mod.rs` | `parse_cue` — line-by-line CUE sheet parser; returns `CueSheet` (`album_title`, `performer`, `date`, `genre`, `audio_file`, `tracks: Vec<CueTrack>`); `CueTrack` holds `number`, `title`, `performer`, `index_01_secs` (converted from MM:SS:FF); handles album-level and per-track TITLE/PERFORMER |
| `src/tagger/mod.rs` | `read_tags` / `write_tags` — lofty-based tag read/write; returns `HashMap<String,String>` keyed by MusicBrainz field names + `AudioProperties` (includes `bit_depth: Option<i64>` from lofty `AudioFile::properties().bit_depth()`) |
| `src/scanner/mod.rs` | `scan_library` — two-pass walk: `ingest/` → staged tracks (status="staged"); `source/` → active tracks (status="active"); SHA-256 hashes, diffs against DB, upserts/removes tracks, enqueues `fingerprint` for new tracks; Pass 3 enqueues `cue_split` for discovered CUE sheets; no auto-transcode, no list_child_libraries; `AUDIO_EXTENSIONS` includes `wv`, `ape`, `tta` |
| `src/jobs/mod.rs` | `JobHandler` trait + `ScanPayload` + `OrganizePayload` + `FingerprintPayload` + `CueSplitPayload` + `TranscodePayload` (library_profile_id: i64; no child_library_id) + `ArtProcessPayload` + `NormalizePayload` (encoding_profile_id: Option<i64>) + `VirtualSyncPayload` + `ProcessStagedPayload` (track_id, tag_suggestion_id, cover_art_url, write_folder_art, profile_ids) + `MaintenancePayload` DTOs |
| `src/jobs/art_process.rs` | `ArtProcessJobHandler` — three actions: `embed` (download art from URL via reqwest, embed via lofty), `extract` (read embedded art, write to `{stem}.cover.{ext}` alongside audio), `standardize` (resize/recompress via `image` crate to fit art profile constraints); calls `set_track_has_embedded_art` after embed/standardize |
| `src/jobs/cue_split.rs` | `CueSplitJobHandler` — reads+parses CUE sheet, spawns `ffmpeg -c:a copy` for each track (with `-ss`/`-to`), writes tags via lofty `write_tags`, hashes output, upserts track to DB, enqueues `fingerprint`; writes output to `source_out_dir` (replaces `ingest/` prefix with `source/`); removes original CUE+audio after split; idempotent; `hash_file` (pub, reused by transcode + migrate) + `sanitize_filename` helpers |
| `src/jobs/transcode.rs` | `TranscodeJobHandler` — ffmpeg transcode pipeline: fetches source track + `get_library_profile(library_profile_id)`, checks `is_compatible`, builds ffmpeg args via `build_ffmpeg_args(profile)`, runs transcode, writes tags, hashes output, upserts derived track (library_profile_id set), calls `create_track_link(src, derived)` (no encoding_profile_id); output path under `derived_dir_name/`; `codec_extension(codec)` + `build_ffmpeg_args(profile)` pub helpers |
| `src/jobs/scan.rs` | `ScanJobHandler` — runs `scan_library`, logs result, returns JSON summary |
| `src/jobs/organize.rs` | `OrganizeJobHandler` — evaluates rules against a track, moves the file via `tokio::fs::rename`, updates `tracks.relative_path` in DB; supports `dry_run` mode |
| `src/jobs/fingerprint.rs` | `FingerprintJobHandler` — spawns `fpcalc -json` as async subprocess, parses fingerprint + duration, calls `update_track_fingerprint`; always enqueues `mb_lookup` after fingerprint (no normalize_on_ingest check) |
| `src/jobs/normalize.rs` | `NormalizeJobHandler` — in-place format conversion: fetches track + library, skips if no profile or already correct format; runs `is_compatible` quality guard; spawns ffmpeg; verifies output, deletes source, hashes output, calls `update_track_path`; always enqueues `mb_lookup` in all skip paths |
| `src/jobs/virtual_sync.rs` | `VirtualSyncJobHandler` — builds identity→(Library,Track) map from priority-ordered sources; `VirtualLibrarySource.library_profile_id` drives source selection: NULL → `list_tracks_by_profile(lib_id, None)`, Some(id) → `list_tracks_by_profile(lib_id, Some(id))`; clears stale filesystem links + DB records, re-materializes as symlinks or hardlinks, upserts `virtual_library_tracks`; `track_identity` prefers `musicbrainz_recordingid`, falls back to normalized (albumartist, album, disc, track) tuple |
| `src/jobs/freedb_lookup.rs` | `FreedBLookupJobHandler` — reads `DISCID` tag, calls `FreedBService::disc_lookup`, creates one `tag_suggestion` row with `confidence = 0.5`; skips cleanly if no DISCID |
| `src/jobs/mb_lookup.rs` | `MbLookupJobHandler` — three-tier fallback: AcoustID ≥0.8 → one suggestion per recording (best-scored release primary, alternatives JSONB); AcoustID 0 → MB text search (source="mb_search", confidence ≤0.6); text search 0 + DISCID → `freedb_lookup`; `pick_best_release` uses `score_release` with existing track tags as seed |
| `src/jobs/process_staged.rs` | `ProcessStagedJobHandler` — writes tags, embeds art (with folder art if `folder_art_filename` setting set), moves `ingest/` → `source/`, hashes, updates track path+status to "active"; if `supersede_track_id` set: moves old file to derived dir (or discards if `supersede_profile_id` is None), sets old track's `library_profile_id`, creates `track_link`; enqueues one `transcode` per `profile_id`; returns `{ track_id, profiles_enqueued }` |
| `src/scheduler/mod.rs` | `Scheduler` — Tokio poll loop; claims pending jobs, semaphore-caps concurrency per type, retries on failure; `cue_split`, `transcode`, `normalize`, `process_staged` each registered with concurrency=2; `art_process` registered with concurrency=4; `virtual_sync`, `maintenance`, `delete_tracks` registered with concurrency=1 |
| `src/jobs/delete_tracks.rs` | `DeleteTracksJobHandler` — accepts `{track_ids: [i64]}`; for each ID: resolves abs_path (track + library), calls `tokio::fs::remove_file` (ignores NotFound), calls `db.delete_track`; returns `{deleted, errors}` |
| `src/services/mod.rs` | Re-exports `auth`, `freedb`, `musicbrainz`, `tagging`, `totp`, `transcode_compat`, `webauthn` service modules |
| `src/services/auth.rs` | `AuthService` — Argon2 hashing, JWT sign/verify, login flow with `LoginResult` enum, `2fa_pending` token issue/decode, `create_full_session` |
| `src/services/freedb.rs` | `FreedBService` — gnudb.org CDDB disc-ID lookup (query + read, two HTTP calls), XMCD response parsing, `to_tag_map` (candidate → tag HashMap); `text_search(artist, album)` hits gnudb.org `/search/search` HTML endpoint; `parse_search_html` helper; HashSet dedup for disc IDs |
| `src/services/musicbrainz.rs` | `MusicBrainzService` — AcoustID fingerprint lookup, MusicBrainz recording fetch (`inc=releases+release-groups+artist-credits+media`, 1.1s rate limit), `to_tag_map` (recording+release → tag HashMap), `score_release(release, existing_tags)` (MBP-style scoring: Official+30, Album+40, date decay, tag-seed matching), `caa_url` (Cover Art Archive URL); `search_recordings(title, artist, album)` returns `Vec<(HashMap<String,String>, f64)>` with confidence capped at 0.6; `MbRelease` now includes `status: Option<String>` |
| `src/services/totp.rs` | `TotpService` — TOTP secret generation, otpauth URI, code verification |
| `src/services/webauthn.rs` | `WebauthnService` — passkey registration/authentication start+finish flows |
| `src/api/mod.rs` | `api_router()` — mounts `/auth`, `/totp`, `/webauthn`, `/settings`, `/themes`, `/libraries`, `/jobs`, `/tracks`, `/organization-rules`, `/tag-suggestions`, `/encoding-profiles`, `/art-profiles`, `/virtual-libraries`, `/uploads`, `/library-profiles`, `/ingest`, `/search`, `/admin`, `/user/prefs` subrouters; `.merge(transcode::router())` and `.merge(art::router())` for direct-path routes |
| `src/api/user_prefs.rs` | `GET /user/prefs` (AuthUser → `Vec<UserPref>`), `PUT /user/prefs/:key` (AuthUser → upsert `{value}` body, returns `UserPref`); uses `auth.0.id` from tuple struct `AuthUser(User)` |
| `src/api/uploads.rs` | `POST /images` (auth) — accepts multipart file upload, validates MIME type (jpeg/png/webp/gif), enforces 10 MiB limit, saves to `config.uploads_dir/{uuid}.{ext}`, returns `{ url: "/uploads/{filename}" }` (201) |
| `src/api/virtual_libraries.rs` | Handlers: `GET /` (list, auth), `POST /` (admin, create → 201), `GET /:id` (auth), `PUT /:id` (admin), `DELETE /:id` (admin → 204), `GET /:id/sources` (auth, ordered by priority), `PUT /:id/sources` (admin, atomically replace `[{library_id, library_profile_id, priority}]` → 204), `POST /:id/sync` (auth, enqueue `virtual_sync` job → 202) |
| `src/api/libraries.rs` | Handlers: `GET /` (list), `GET /:id`, `POST /` (admin — creates `source/` + `ingest/` subdirs), `PUT /:id` (admin), `DELETE /:id` (admin), `GET /:id/tracks` (optional `?status=` param defaulting to "active") |
| `src/api/library_profiles.rs` | CRUD at `/library-profiles`; `GET /` requires `?library_id=N`; AdminUser for mutations, AuthUser for reads |
| `src/api/ingest.rs` | `GET /staged` (AuthUser) lists all staged tracks across libraries; `POST /submit` (AdminUser) enqueues `process_staged` job → 202; `POST /supersede-check` (AuthUser) — accepts `{track_ids}`, returns per-track supersede candidates with quality delta and profile match; three-tier identity matching (MB recording ID → tag tuple → AcoustID fingerprint) |
| `src/api/search.rs` | `POST /mb` (AuthUser) calls `mb_service.search_recordings()`; `POST /freedb` (AuthUser) calls `freedb_service` disc_lookup or text_search |
| `src/api/migrate.rs` | `POST /admin/migrate-library-files/:library_id` (AdminUser) — fetches active source tracks (`library_profile_id IS NULL`), skips those already under `source/`, moves flat root_path/ files to root_path/source/ (rename with EXDEV copy+delete fallback), rehashes, updates DB path+hash; returns `{ moved, skipped, errors }` |
| `src/api/jobs.rs` | Handlers: `GET /` (list+filter), `GET /:id`, `POST /:id/cancel` (admin), `POST /scan` (admin, enqueue scan) |
| `src/api/auth.rs` | Handlers: `POST /register`, `POST /login` (returns 204 or 200+2fa token), `POST /logout`, `GET /me` |
| `src/api/totp.rs` | Handlers: `POST /enroll`, `POST /verify`, `POST /complete` (2fa→session), `DELETE /disenroll` |
| `src/api/webauthn.rs` | Handlers: register/authenticate challenge+complete, `GET /credentials`, `DELETE /credentials/:id` |
| `src/api/settings.rs` | Handlers: `GET /` (list), `GET /:key`, `PUT /:key` (admin-only write) |
| `src/api/themes.rs` | Handlers: `GET /`, `POST /` (admin), `GET /:id`, `PUT /:id` (admin), `DELETE /:id` (admin) |
| `src/api/tracks.rs` | `GET /:id` (auth, returns `Track` JSON, 404 if missing); `GET/HEAD /:id/stream` — byte-range streaming with `Content-Range`, `Accept-Ranges`, `X-File-Size`, `X-Duration-Secs`, `X-Bitrate`, `X-Sample-Rate` headers; `POST /:id/lookup` (auth) — enqueues `fingerprint` job → 202; `POST /delete` (auth, body `{ids: [i64]}`) — schedules `delete_tracks` job with 15-min `run_after` delay → 202 `{job_id, run_after}` |
| `src/api/organization_rules.rs` | Handlers: `GET /` (list, optional `?library_id=N`), `POST /` (admin, create → 201), `GET /:id`, `PUT /:id` (admin), `DELETE /:id` (admin → 204), `POST /preview` (admin, dry-run path proposals), `POST /apply` (admin, enqueue organize jobs) |
| `src/api/tag_suggestions.rs` | Handlers: `GET /` (list pending, optional `?track_id=N`, auth), `GET /count` (public nav badge), `GET /:id` (auth, 404 if missing), `POST /` (AuthUser, create suggestion; source validated against allowed list → 201), `POST /:id/accept` (auth, calls tagging stub + sets status), `POST /:id/reject` (auth), `POST /batch-accept` (auth, body `{min_confidence}`) |
| `src/api/encoding_profiles.rs` | Handlers: `GET /` (list, auth), `POST /` (admin, create → 201), `GET /:id` (auth), `PUT /:id` (admin), `DELETE /:id` (admin → 204); body `EncodingProfileBody` maps to `UpsertEncodingProfile` |
| `src/api/art_profiles.rs` | Handlers: `GET /` (list, auth), `POST /` (admin, create → 201), `GET /:id` (auth), `PUT /:id` (admin), `DELETE /:id` (admin → 204); body `ArtProfileBody` maps to `UpsertArtProfile` |
| `src/api/transcode.rs` | Handlers: `POST /tracks/:id/transcode` (auth, enqueue transcode → 202), `POST /libraries/:id/transcode` (auth, enqueue all tracks → 202+count), `POST /libraries/:id/transcode-sync` (auth, enqueue missing-only → 202+count); all require `AuthUser` |
| `src/api/art.rs` | Handlers: `POST /tracks/:id/art/embed` (auth, enqueue art_process action=embed), `POST /tracks/:id/art/extract` (auth, action=extract), `POST /tracks/:id/art/standardize` (auth, action=standardize), `POST /libraries/:id/art/standardize` (auth, enqueue for tracks with has_embedded_art=true → 202+count) |
| `src/services/tagging.rs` | `apply_suggestion` — merges existing track tags with suggestion tags, writes to audio file via `tagger::write_tags`, updates DB via `update_track_tags`; enqueues `art_process` embed job if `cover_art_url` is present |
| `src/services/transcode_compat.rs` | `is_compatible(src_format, src_sample_rate, src_bit_depth, src_bitrate, profile)` — quality-matching rules: rejects lossy→lossless, sample-rate upsampling, bit-depth inflation, bitrate upscaling; `is_lossless(format)` pub helper; `quality_rank(format, sr, bd, br) -> u64` + `quality_cmp(...)` for upgrade detection; `format_from_path(path)` + `codecs_match(file_format, codec)` for profile matching; `parse_bitrate_kbps` now pub |
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
| `tests/scanner_extended_formats.rs` | Integration tests: WavPack (`.wv`), APE (`.ape`), TrueAudio (`.tta`) files are ingested by the scanner |
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
| `tests/common/mod.rs` | Shared test helpers: `make_db()`, `setup_store()`, `setup_with_fingerprinted_track()`, `setup_with_discid_track()`, `setup_with_track()`, `setup_with_audio_track()` (FLAC with VORBISCOMMENT), `setup_cue_library()`, `setup_transcode_scenario_no_profile()`, `setup_transcode_lossy_to_lossless_scenario()`, `ffmpeg_available()` (checks ffmpeg on PATH), `TAGGED_FLAC` bytes constant |
| `tests/encoding_profiles_dal.rs` | DAL tests for encoding_profiles CRUD — create, list, get, update, delete; full flow with `UpsertEncodingProfile` |
| `tests/art_process_job.rs` | Integration tests for `ArtProcessJobHandler` — unknown action error, missing track error, missing track_id field, embed without URL, standardize without profile, extract with no art, embed from wiremock URL (7 tests) |
| `tests/art_profiles_dal.rs` | DAL tests for art_profiles CRUD — create, list, get, update, delete; full flow with `UpsertArtProfile` |
| `tests/track_links_dal.rs` | DAL tests for track_links — create link between two tracks, list_derived_tracks, list_source_tracks; verifies FK constraint satisfaction |
| `tests/tagging_service.rs` | Integration tests for `apply_suggestion` — file + DB updated, indexed artist column correct, title preserved from merge, NotFound on missing track |
| `tests/cue_parser.rs` | Unit tests for `parse_cue` — album-level fields, per-track fields, INDEX 01 time conversion (MM:SS:FF → seconds), 3-track parse, duration calc via next-track start |
| `tests/cue_split_job.rs` | Integration tests for `CueSplitJobHandler` — creates 3 tracks from CUE+FLAC (skips gracefully if ffmpeg absent), idempotency (second run returns 0), scanner skips CUE-backed audio and enqueues cue_split job |
| `tests/transcode_compat.rs` | Unit tests for `is_compatible` — 6 tests covering lossy→lossless rejection, lossless→lossy allowed, upsample rejection, bit-depth inflation rejection, bitrate upscale rejection, unknown-values pass-through |
| `tests/transcode_job.rs` | Tests for `TranscodeJobHandler` — `codec_extension` unit tests, fails without library_profile_id, skips lossy→lossless (no ffmpeg needed), errors on missing source track |
| `tests/encoding_profiles_api.rs` | Integration tests for encoding profiles REST API — full CRUD flow (create → 201, list, get, update, delete → 204) and auth guards (unauthenticated → 401, non-admin POST → 403) |
| `tests/art_profiles_api.rs` | Integration tests for art profiles REST API — full CRUD flow (create → 201, list, get, update, delete → 204) and auth guards (unauthenticated → 401, non-admin POST → 403) |
| `tests/transcode_api.rs` | Integration tests for transcode REST API — auth guards (401), 404 on missing track, single-track enqueue, library bulk enqueue (count), transcode-sync skips already-linked tracks |
| `tests/art_api.rs` | Integration tests for art REST API — auth guards (401) for all 4 endpoints, 404 on missing track, embed/extract/standardize enqueue jobs, library standardize filters by has_embedded_art (count) |
| `tests/normalize_job.rs` | Tests for `NormalizeJobHandler` — skips when already correct format, skips when no encoding profile; all skip paths enqueue `mb_lookup`; fingerprint chaining verification |
| `tests/virtual_libraries_api.rs` | Integration tests for virtual libraries REST API — CRUD flow (create → 201, list, get, update, delete → 204), auth guards (unauthenticated → 401, non-admin POST → 403), sources test (set + get + atomic replace), sync enqueue test (POST /:id/sync → 202 + pending virtual_sync job) |
| `tests/virtual_libraries_dal.rs` | DAL tests for virtual libraries — CRUD flow with `UpsertVirtualLibrary`, atomic source replacement (`set_virtual_library_sources` with priority ordering), track upsert/list/clear |
| `tests/virtual_sync_job.rs` | Integration tests for `VirtualSyncJobHandler` — symlink creation, priority dedup (lib1 priority 1 wins over lib2 priority 2), idempotency (second run produces no duplicate DB rows) |
| `tests/uploads_api.rs` | Integration tests for uploads REST API — image upload returns 201 + `/uploads/{uuid}.png` URL, file is serveable at that URL (200, image/png), non-image MIME → 400, unauthenticated → 401 |
| `tests/fixtures/1x1.png` | Minimal 1×1 white PNG for upload tests (generated via imagemagick alpine Docker run) |

## Migrations

### `migrations/postgres/`

| File | Owns |
|------|------|
| `0001_users.sql` | `users`, `sessions`, `api_tokens`, `audit_log` |
| `0002_two_factor.sql` | `totp_entries`, `webauthn_credentials`, `webauthn_challenges` |
| `0003_settings_themes.sql` | `settings` (key-value + seed data), `themes` |
| `0004_libraries.sql` | `libraries` (base table) |
| `0005_tracks.sql` | `tracks` (JSONB `tags`, indexed common fields) |
| `0006_jobs.sql` | `jobs` (type + status CHECK constraints, priority index) |
| `0007_webauthn_challenge_uq.sql` | `UNIQUE (user_id, kind)` constraint on `webauthn_challenges` (enables upsert) |
| `0008_organization_rules.sql` | `organization_rules` table (BIGSERIAL id, JSONB conditions, priority, path_template, enabled) with library FK |
| `0009_tag_suggestions.sql` | `tag_suggestions` table (BIGSERIAL id, track FK, source CHECK, JSONB suggested_tags, confidence, mb IDs, status CHECK) |
| `0010_jobs_add_freedb_lookup.sql` | Expands `job_type` CHECK constraint to include `freedb_lookup` via ALTER TABLE DROP/ADD CONSTRAINT |
| `0011_encoding_profiles.sql` | `encoding_profiles` table (BIGSERIAL id, name, codec, bitrate, sample_rate, channels, bit_depth, advanced_args, created_at) |
| `0012_art_profiles.sql` | `art_profiles` table (BIGSERIAL id, name, max_width_px, max_height_px, max_size_bytes, format CHECK jpeg/png, quality CHECK 1-100, apply_to_library_id FK, created_at) |
| `0013_track_links.sql` | `track_links` table (composite PK source+derived, BIGINT FKs to tracks ON DELETE CASCADE, TIMESTAMPTZ created_at, two indexes) |
| `0014_jobs_add_cue_split.sql` | Expands `job_type` CHECK constraint to include `cue_split` via ALTER TABLE DROP/ADD CONSTRAINT |
| `0015_tracks_add_bit_depth.sql` | `ALTER TABLE tracks ADD COLUMN IF NOT EXISTS bit_depth INTEGER` |
| `0016_libraries_normalize_on_ingest.sql` | `ALTER TABLE libraries ADD COLUMN IF NOT EXISTS normalize_on_ingest BOOLEAN NOT NULL DEFAULT FALSE` |
| `0017_jobs_add_normalize.sql` | Expands `job_type` CHECK constraint to include `normalize` via ALTER TABLE DROP/ADD CONSTRAINT |
| `0018_virtual_libraries.sql` | `virtual_libraries` (BIGSERIAL id, name, root_path, link_type CHECK symlink/hardlink, created_at), `virtual_library_sources` (composite PK, priority, FK to libraries), `virtual_library_tracks` (composite PK, link_path, FK to tracks ON DELETE CASCADE), `idx_vls_priority` index |
| `0019_jobs_add_virtual_sync.sql` | Expands `job_type` CHECK constraint to include `virtual_sync` via ALTER TABLE DROP/ADD CONSTRAINT |
| `0020_libraries_tag_encoding.sql` | `ALTER TABLE libraries ADD COLUMN tag_encoding TEXT NOT NULL DEFAULT 'utf8'` |
| `0021_settings_allow_registration.sql` | Seeds `allow_registration = 'true'` into settings |
| `0022_libraries_ingest_dir.sql` | `ALTER TABLE libraries ADD COLUMN IF NOT EXISTS ingest_dir TEXT` (later dropped by 0028) |
| `0023_libraries_org_rule.sql` | `ALTER TABLE libraries ADD COLUMN IF NOT EXISTS organization_rule_id BIGINT REFERENCES organization_rules` |
| `0024_fix_integer_to_bigint.sql` | Alters art_profiles (max_width_px, max_height_px, max_size_bytes, quality) and encoding_profiles (sample_rate, channels, bit_depth) columns from INTEGER to BIGINT |
| `0025_library_profiles.sql` | Drops parent_library_id, encoding_profile_id, auto_transcode_on_ingest, normalize_on_ingest from libraries; creates `library_profiles` table (id, library_id, encoding_profile_id, derived_dir_name, include_on_submit, auto_include_above_hz, created_at) |
| `0026_tracks_ingest_columns.sql` | Adds `tracks.status` CHECK(staged/active/removed) DEFAULT active; adds `tracks.library_profile_id` FK → library_profiles |
| `0027_redesign_remaining.sql` | Drops `track_links.encoding_profile_id`; adds surrogate id + library_profile_id to virtual_library_sources; adds `process_staged` to jobs CHECK; seeds `folder_art_filename` setting |
| `0028_drop_ingest_dir.sql` | Drops `libraries.ingest_dir` |
| `0029_tracks_duration_secs_float8.sql` | `ALTER TABLE tracks ALTER COLUMN duration_secs TYPE DOUBLE PRECISION` — fixes FLOAT4/FLOAT8 mismatch with Rust `Option<f64>` |
| `0030_fix_integer_to_bigint_2.sql` | Widens `tracks.bit_depth`, `virtual_library_sources.priority`, `library_profiles.auto_include_above_hz` from INTEGER to BIGINT to match Rust `i64` fields |
| `0031_tag_suggestions_alternatives.sql` | `ALTER TABLE tag_suggestions ADD COLUMN alternatives JSONB` — stores ranked alternative releases alongside the primary suggestion |
| `0032_libraries_default_maintenance.sql` | `ALTER TABLE libraries ADD COLUMN is_default BOOLEAN NOT NULL DEFAULT FALSE` + `maintenance_interval_secs BIGINT` |
| `0033_jobs_add_maintenance.sql` | Expands `job_type` CHECK constraint to include `maintenance` via ALTER TABLE DROP/ADD CONSTRAINT |
| `0034_issues.sql` | `issues` table (BIGSERIAL id, library_id FK, track_id FK nullable, issue_type CHECK, detail, severity CHECK, dismissed, resolved, created_at, updated_at); indexes on library_id, track_id; unique index on (track_id, issue_type) WHERE track_id IS NOT NULL |
| `0035_user_preferences.sql` | `user_preferences` table (user_id BIGINT FK, key TEXT, value TEXT; PK (user_id, key)) |
| `0036_jobs_run_after_and_delete_tracks.sql` | `ALTER TABLE jobs ADD COLUMN IF NOT EXISTS run_after TIMESTAMPTZ`; drops+re-adds `job_type` CHECK constraint to include `delete_tracks` |

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
| `0011_encoding_profiles.sql` | `encoding_profiles` table (INTEGER id AUTOINCREMENT, name, codec, bitrate, sample_rate, channels, bit_depth, advanced_args, created_at TEXT) |
| `0012_art_profiles.sql` | `art_profiles` table (INTEGER id AUTOINCREMENT, name, max_width_px, max_height_px, max_size_bytes, format CHECK jpeg/png, quality CHECK 1-100, apply_to_library_id FK, created_at TEXT DEFAULT (datetime('now'))) |
| `0013_track_links.sql` | `track_links` table (composite PK source+derived, INTEGER FKs to tracks ON DELETE CASCADE, TEXT created_at DEFAULT (datetime('now')), two indexes) |
| `0014_jobs_add_cue_split.sql` | Recreates `jobs` table to add `cue_split` to the `job_type` CHECK constraint |
| `0015_tracks_add_bit_depth.sql` | `ALTER TABLE tracks ADD COLUMN bit_depth INTEGER` |
| `0016_libraries_normalize_on_ingest.sql` | `ALTER TABLE libraries ADD COLUMN normalize_on_ingest INTEGER NOT NULL DEFAULT 0` |
| `0017_jobs_add_normalize.sql` | Recreates `jobs` table to add `normalize` to the `job_type` CHECK constraint |
| `0018_virtual_libraries.sql` | `virtual_libraries` (INTEGER id AUTOINCREMENT, name, root_path, link_type CHECK symlink/hardlink, created_at TEXT), `virtual_library_sources` (composite PK, priority, FK to libraries), `virtual_library_tracks` (composite PK, link_path, FK to tracks ON DELETE CASCADE), `idx_vls_priority` index |
| `0019_jobs_add_virtual_sync.sql` | Recreates `jobs` table to add `virtual_sync` to the `job_type` CHECK constraint |
| `0020_libraries_tag_encoding.sql` | `ALTER TABLE libraries ADD COLUMN tag_encoding TEXT NOT NULL DEFAULT 'utf8'` |
| `0021_settings_allow_registration.sql` | Seeds `allow_registration = 'true'` into settings |
| `0022_libraries_ingest_dir.sql` | `ALTER TABLE libraries ADD COLUMN ingest_dir TEXT` (later dropped by 0028) |
| `0023_libraries_org_rule.sql` | `ALTER TABLE libraries ADD COLUMN organization_rule_id INTEGER REFERENCES organization_rules` |
| `0024_fix_integer_to_bigint.sql` | No-op on SQLite (INTEGER is already 64-bit); migration file recreates no tables |
| `0025_library_profiles.sql` | Drops parent_library_id, encoding_profile_id, auto_transcode_on_ingest, normalize_on_ingest from libraries; creates `library_profiles` table (SQLite types) |
| `0026_tracks_ingest_columns.sql` | Adds `tracks.status` CHECK(staged/active/removed) DEFAULT active; adds `tracks.library_profile_id` FK → library_profiles |
| `0027_redesign_remaining.sql` | Drops `track_links.encoding_profile_id` (table recreate); adds surrogate id + library_profile_id to virtual_library_sources; adds `process_staged` to jobs CHECK; seeds `folder_art_filename` setting |
| `0028_drop_ingest_dir.sql` | Drops `libraries.ingest_dir` (table recreate) |
| `0029_tracks_duration_secs_float8.sql` | No-op — SQLite REAL is already 8-byte; exists to keep migration numbers in sync with Postgres |
| `0030_fix_integer_to_bigint_2.sql` | No-op — SQLite INTEGER is already 64-bit; exists to keep migration numbers in sync with Postgres |
| `0031_tag_suggestions_alternatives.sql` | `ALTER TABLE tag_suggestions ADD COLUMN alternatives TEXT` — stores alternative releases as a JSON string; mirrors Postgres 0031 |
| `0032_libraries_default_maintenance.sql` | `ALTER TABLE libraries ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0` + `maintenance_interval_secs INTEGER` |
| `0033_jobs_add_maintenance.sql` | Recreates `jobs` table to add `maintenance` to the `job_type` CHECK constraint (SQLite table-recreate pattern) |
| `0034_issues.sql` | `issues` table (INTEGER id AUTOINCREMENT, library_id/track_id FKs, issue_type/severity CHECK constraints, dismissed/resolved as INTEGER 0/1); mirrors Postgres 0034 |
| `0035_user_preferences.sql` | `user_preferences` table (user_id INTEGER FK, key TEXT, value TEXT; PK (user_id, key)) |
| `0036_jobs_run_after_and_delete_tracks.sql` | Table-recreate adds `run_after TEXT` column to `jobs` and adds `delete_tracks` to `job_type` CHECK constraint |

## Directories

| Directory | Owns |
|-----------|------|
| `docs/plans/` | Implementation plans — date-prefixed kebab-case filenames; latest: `2026-04-24-library-view-persistence-and-columns.md` |
| `migrations/postgres/` | Postgres SQL migrations (0001–0036) |
| `migrations/sqlite/` | SQLite SQL migrations (0001–0036) |
| `resources/` | App assets (logos, icons, etc.) |
| `scripts/` | Developer tooling scripts |
| `secrets/` | Local secret files (gitignored except README) |
| `ui/` | React + Vite + Tailwind SPA — `npm run build` → `ui/dist/` |
| `ui/src/theme/` | `tokens.ts` (dark/light CSS vars, `ACCENT_COLORS` named palette, `hexToRgbChannels`, `applyTokens` with `extraVars` param) + `ThemeProvider.tsx` (loads active theme from DB via `useQuery`, persists `activeThemeId` in `localStorage`, exposes `setActiveTheme`, overlays `css_vars` + accent on base tokens, sets `--theme-bg-image`) |
| `ui/src/utils/` | `extractPalette.ts` — canvas 2D histogram-based dominant-hue extraction; returns `ExtractedPalette { accent, isDark, appliedTone, themeVars }` with tinted RGBA surface vars |
| `ui/src/types/` | `tagSuggestion.ts` — `TagSuggestion` (includes `alternatives?: AlternativeRelease[]`) + `AlternativeRelease` interface; `track.ts` — `Track` (includes status, library_profile_id, bit_depth); `encodingProfile.ts` — `EncodingProfile` + `UpsertEncodingProfile`; `artProfile.ts` — `ArtProfile` + `UpsertArtProfile`; `virtualLibrary.ts` — `VirtualLibrary` + `VirtualLibrarySource` (id, library_profile_id) + `UpsertVirtualLibrary`; `libraryProfile.ts` — `LibraryProfile` + `UpsertLibraryProfile` |
| `ui/src/api/` | `client.ts` (Axios baseURL `/api/v1`), `auth.ts`, `libraries.ts` (`getLibrary`, `listLibraryTracks(id, status?)`, no deprecated parent/child fields), `organizationRules.ts`, `tagSuggestions.ts` (includes `create()`), `tracks.ts` (`tracksApi.getTrack`, `enqueueLookup(id)`, `scheduleDelete(ids)` → `{job_id, run_after}`), `encodingProfiles.ts`, `artProfiles.ts`, `virtualLibraries.ts` (`setSources` with library_profile_id per entry), `transcode.ts`, `art.ts`, `themes.ts`, `settings.ts`, `libraryProfiles.ts`, `ingest.ts` (`getStagedTracks`, `submitTrack` with `supersede_track_id`/`supersede_profile_id`, `checkSupersede(trackIds)` → `SupersedeCheckResult[]`), `search.ts`, `jobs.ts` (`listJobs`, `getJob`, `cancelJob`), `userPrefs.ts` (`getUserPrefs()` → `UserPref[]`, `setUserPref(key, value)`) |
| `ui/src/contexts/` | `AuthContext.tsx` — current user context, `useAuth` hook |
| `ui/src/pages/` | `LoginPage.tsx`, `RegisterPage.tsx`, `LibraryPage.tsx` (library track list; shift-click multi-select; groupBy+sort persisted via useUserPrefs; resizable columns with divider lines; themed Checkbox; Actions(N) dropdown when tracks selected (AcoustID Lookup, Delete N tracks); album group rows with ⋯ delete-album button; per-track delete in context menu and ⋯ row menu; DeleteConfirmModal — 15-min delay warning, Jobs cancel note, red Schedule Deletion button; BulkEditPanel at bottom — 25 fields in 3-col grid, Apply to Selected fans out one suggestion per track), `OrganizationPage.tsx`, `IngestPage.tsx` (album-grouped staged tracks; per-track: Accept/Edit/Reject/Alt…/Search/Lookup; supersede badge ("Replaces existing" — sky blue or amber if no profile) with expandable quality comparison row; AlbumEditPanel — 17 album-scope fields with Apply to All; album art state with Add Art/Change Art toggle and inline ImageUpload panel; art presetArtUrl carried into SubmitDialog; SubmitDialog — tags summary, art picker with Skip, Supersedes section (per-track resolution: replace/keep/discard; Import blocked until warnings resolved), profile checklist, Import), `SettingsPage.tsx` (tabs: General — per-field save for 7 settings; Encoding Profiles; Art Profiles; Virtual Libraries — with SourcePriorityList and Sync; Themes — accent swatches, background upload with palette extraction, Apply/Remove per row), `JobsPage.tsx` (job list with status filter tabs, 5s polling, cancel for admins) |
| `ui/src/components/` | `TopNav.tsx`, `LibraryTree.tsx`, `LibraryFormModal.tsx` (includes Profiles section with reorder), `RuleEditor.tsx`, `TemplatePreview.tsx`, `TagDiffTable.tsx`, `EncodingProfileForm.tsx`, `ArtProfileForm.tsx`, `TranscodeDialog.tsx`, `SourcePriorityList.tsx`, `VirtualLibraryForm.tsx`, `ImageUpload.tsx`, `IngestSearchDialog.tsx` (MB + FreeDB tabs), `TrackEditPanel.tsx` (inline tag editor; creates suggestion with confidence 1.0; pre-populates from track fields; used in both Ingest and Library views), `AlternativesPanel.tsx` (inline picker for alternative releases from a suggestion's `alternatives` array; "Use this" creates new suggestion; used in both Ingest and Library views), `Checkbox.tsx` (themed checkbox: `appearance:none`, `::after` checkmark/dash, accent-colored when checked/indeterminate, supports `indeterminate` prop via ref) |
| `ui/src/hooks/` | `useUserPrefs.ts` — localStorage-first + backend-sync hook; exports `GroupByKey`, `SortByKey`, `SortLevel`, `DEFAULT_COL_WIDTHS`; pref keys: `library.groupBy`, `library.sortLevels`, `library.columnWidths`, `library.visibleColumns`; migrates legacy `suzuran:column-visibility`; returns `{ groupBy, setGroupBy, sortLevels, setSortLevels, colWidths, setColWidths, visibleCols, toggleColumn }` |
