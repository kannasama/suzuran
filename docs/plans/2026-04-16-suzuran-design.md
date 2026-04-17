# Suzuran — System Design

**Date:** 2026-04-16  
**Status:** Approved  
**Scope:** v1.0 full system design across four implementation phases. v1.1 streaming noted where relevant.

---

## Overview

Suzuran is a self-hosted music library manager — a server-based alternative to `beets`. It manages
multiple format libraries (FLAC, m4a-aac, MP3, etc.), handles audio tagging via MusicBrainz
integration, organizes files via rule-based path templates, and transcodes between formats. It is
accessed via a web UI and deployed as a Docker image.

**Target user:** A single operator (personal use) managing a large music collection across multiple
machines via SyncThing or similar, with multiple devices requiring different encoding formats.

---

## Architecture

### Runtime

Single Rust binary (`suzuran-server`) using **Axum** + **Tokio**. Layered:

- `src/api/` — Axum router, middleware, handlers
- `src/services/` — business logic layer
- `src/dal/` — `Store` trait + Postgres/SQLite backends (same DAL pattern as rssekai)
- `src/scanner/` — filesystem walker; discovers audio files, computes hashes, upserts tracks
- `src/tagger/` — tag read/write abstraction over audio formats via the `lofty` crate
- `src/jobs/` — individual job handler implementations (fingerprint, mb_lookup, transcode, art_process, scan, organize)
- `src/scheduler/` — DB-backed job queue poll loop with semaphore-capped concurrency

### Frontend

React + Vite + Tailwind CSS SPA. Compiled to static files, embedded in and served by the Axum
backend. Theming engine ported from rssekai (dark/light base theme, per-user accent color,
custom themes with CSS variables, optional per-theme background images).

### Database

Postgres primary, SQLite secondary. Same `Store` trait / DAL pattern as rssekai. Migrations
numbered sequentially under `migrations/postgres/` and `migrations/sqlite/`.

### Configuration

Minimal environment variables — only what is needed before the database is reachable:

| Variable | Required | Default | Purpose |
|----------|----------|---------|---------|
| `DATABASE_URL` | Yes | — | DB connection string |
| `JWT_SECRET` | Yes | — | Session signing key (security-sensitive) |
| `PORT` | No | `3000` | HTTP listen port |
| `LOG_LEVEL` | No | `info` | Log verbosity |

All other configuration (AcoustID API key, MusicBrainz user agent, scan intervals, job
concurrency limits, default art profile, etc.) lives in the `settings` table and is
configurable from the UI after first boot.

### Docker

Three-stage build: Rust build → Node/UI build → final image. `ffmpeg` and `fpcalc`
(chromaprint) installed in the final image. Music library roots mounted as Docker volumes.

### Streaming Groundwork (v1.0 passive)

A `GET /api/v1/tracks/:id/stream` endpoint serves the audio file with `Content-Type`,
`Accept-Ranges: bytes`, and `Content-Length` headers, supporting HTTP range requests.
No web player UI in v1.0. `duration_secs`, `bitrate`, `sample_rate`, and `channels` are
populated during scan so the endpoint can respond correctly to `HEAD` requests.
Full streaming (playlist, queue, playback state, web player) is a v1.1 goal.

---

## Data Model

### Users & Auth

```
users:        id, username, email, password_hash, role (admin/user),
              force_password_change, totp_required, webauthn_required,
              accent_color, base_theme, theme_id,
              display_name, created_at

sessions:     id, user_id, token_hash, expires_at, created_at
api_tokens:   id, user_id, name, token_hash, last_used_at, created_at
audit_log:    id, user_id, action, target_type, target_id, detail, created_at

totp_entries: id, user_id, secret (encrypted), verified, created_at
webauthn_credentials: id, user_id, credential_id, public_key, sign_count,
                      name, created_at, last_used_at
```

Both TOTP and WebAuthn/FIDO2 can be active simultaneously per user. Either method satisfies
the 2FA gate. Admins can require 2FA globally or per-user.

### Theming

Ported directly from rssekai:

```
themes: id, name, css_vars (jsonb), accent_color, background_url, created_at
```

### Libraries

```
libraries: id, name, root_path, format (flac/aac/mp3/opus/…),
           encoding_profile_id (nullable FK → encoding_profiles),
           parent_library_id (nullable FK → libraries),
           scan_enabled, scan_interval_secs, auto_transcode_on_ingest,
           auto_organize_on_ingest, created_at
```

Libraries form a directed acyclic graph (DAG) via `parent_library_id`. A library with no
parent is a source root. A library with a parent is a derived root. Multiple children of the
same parent are siblings (e.g., m4a-aac and MP3 both derived from FLAC).

### Encoding Profiles

```
encoding_profiles: id, name,
                   codec (aac/mp3/opus/flac/…),
                   bitrate (e.g. "256k"),
                   sample_rate (e.g. 44100),
                   channels (1/2),
                   advanced_args (nullable — appended to ffmpeg command),
                   created_at
```

Standard fields are exposed as dropdowns/inputs in the UI. `advanced_args` is an expandable
"Advanced" section for edge-case overrides. The ffmpeg command is built from standard fields
first; `advanced_args` is appended if set.

### Tracks

```
tracks: id, library_id, relative_path, file_hash,
        title, artist, albumartist, album, tracknumber, discnumber,
        totaldiscs, totaltracks, date, genre, composer, comment,
        label, catalognumber, isrc,
        musicbrainz_trackid, musicbrainz_albumid, musicbrainz_artistid,
        musicbrainz_albumartistid,
        duration_secs, bitrate, sample_rate, channels,
        has_embedded_art, fingerprint_id (nullable),
        last_scanned_at, created_at
```

Tag field names follow MusicBrainz/Picard standard field names throughout — in the DB, in
file tags, and in path template tokens. No translation layer.

### Track Links

```
track_links: source_track_id, derived_track_id,
             encoding_profile_id (nullable), created_at
```

Records the source→derived relationship when a track in a child library was transcoded from a
track in a parent library. A track with no `track_links` row pointing to it is standalone in
its library.

### Jobs

```
jobs: id, job_type (scan/fingerprint/mb_lookup/transcode/art_process/organize),
      status (pending/running/completed/failed/cancelled),
      payload (jsonb), result (jsonb nullable),
      priority, attempts, error, created_at, started_at, completed_at
```

### Tag Suggestions

```
tag_suggestions: id, track_id, source (acoustid/mb_search),
                 suggested_tags (jsonb), confidence (0.0–1.0),
                 mb_recording_id, mb_release_id,
                 cover_art_url (nullable),
                 status (pending/accepted/rejected),
                 created_at
```

### Organization Rules

```
organization_rules: id, name, library_id (nullable = global),
                    priority, conditions (jsonb expression tree),
                    path_template, enabled, created_at
```

Rules are evaluated in priority order. The first matching rule wins. A rule with no conditions
matches all tracks.

**Path template tokens** use MusicBrainz standard field names with modifiers:

| Token | Example output |
|-------|----------------|
| `{title}` | `Comfortably Numb` |
| `{artist}` | `Pink Floyd` |
| `{albumartist}` | `Pink Floyd` |
| `{album}` | `The Wall` |
| `{tracknumber:02}` | `06` |
| `{discnumber}` | `2` |
| `{discnumber:02}` | `02` |
| `{totaldiscs}` | `2` |
| `{date}` | `1979` |
| `{genre}` | `Rock` |
| `{label}` | `Harvest` |
| `{field\|fallback}` | Uses `fallback` if `field` is empty |
| `{discfolder}` | `Disc N/` when `totaldiscs > 1`, empty string otherwise |

**Multi-disc example:**
`{albumartist}/{date} - {album}/{discfolder}{tracknumber:02} - {title}`

Produces `Pink Floyd/1979 - The Wall/Disc 2/06 - Comfortably Numb` for a multi-disc album
and `Air/1998 - Moon Safari/01 - La Femme d'Argent` for a single-disc album.

### Album Art Profiles

```
art_profiles: id, name, max_width_px, max_height_px, max_size_bytes,
              format (jpeg/png), quality (1–100),
              apply_to_library_id (nullable = global), created_at
```

---

## Ingestion Pipeline

Triggered by: manual scan from UI, scheduled background scan (per-library interval), or
filesystem watch (`notify` crate — inotify on Linux, FSEvents on macOS).

**Scan phase:**
1. Walker traverses `root_path`, finds audio files by extension
2. Computes SHA-256 hash per file
3. Compares against existing `tracks` rows: new hash = insert, changed hash = rescan,
   missing file = mark removed
4. Reads tags via `lofty`, upserts `tracks` row

**Auto-analysis on new track (jobs enqueued, not applied):**
1. `fingerprint` job — runs `fpcalc` (Chromaprint CLI), stores fingerprint, enqueues `mb_lookup`
2. `mb_lookup` job — submits to AcoustID API; if confidence ≥ threshold (default 0.8), fetches
   full MusicBrainz recording/release metadata; writes `tag_suggestions` rows with `status = pending`.
   Falls back to MusicBrainz text search using existing filename/tags if AcoustID returns no results.
   Cover Art Archive queried for the matched release; URL stored on the suggestion.

No tags are modified automatically. All suggestions await user review in the Inbox.

**Rate limiting:** MusicBrainz API — max 1 request/second, descriptive `User-Agent` header
(configurable in settings). AcoustID API key stored in settings.

---

## Job System

A Tokio task spawned at startup polls the `jobs` table every few seconds for `pending` work.
Semaphores cap concurrency per job type (configurable in settings; defaults: 4 fingerprint/lookup,
2 transcode).

**Job lifecycle:** `pending` → `running` (claimed by scheduler) → `completed` or `failed`.
Failed jobs are retried up to 3 times with exponential backoff before permanently failing.
Users can cancel `pending` or `running` jobs from the UI.

**Job types:**

| Type | Mechanism | On success |
|------|-----------|------------|
| `scan` | Internal scanner | Upserts tracks, enqueues `fingerprint` per new track |
| `fingerprint` | `fpcalc` subprocess | Stores fingerprint, enqueues `mb_lookup` |
| `mb_lookup` | AcoustID + MusicBrainz APIs | Writes `tag_suggestions` rows |
| `transcode` | `ffmpeg` subprocess | Inserts derived `tracks` row + `track_links` row |
| `art_process` | `lofty` + `image` crate | Re-embeds resized/recompressed art in file |
| `organize` | Internal file mover | Renames/moves file, updates `tracks.relative_path` |

ffmpeg transcode progress is parsed from `-progress` output for percentage reporting in the UI.

---

## MusicBrainz Integration

See Ingestion Pipeline for the automated lookup flow. User review workflow:

The **Inbox** shows tracks with `pending` suggestions. Per track the user sees:
- Current tags (from file) vs. suggested tags (from MusicBrainz), differences highlighted
- Confidence score and source (AcoustID / text search)
- Multiple candidates if available (user selects the correct release/edition)
- Cover art preview from Cover Art Archive (if found)

**Actions:** Accept (writes tags to file + DB) · Reject (dismisses, keeps existing tags) ·
Edit (manual override before accepting) · Search (manual MusicBrainz query).

"Accept all high confidence" batch action available for suggestions above a configurable
threshold.

---

## Transcoding Pipeline

Source library → derived library via `ffmpeg`, using the target library's `encoding_profile`.

**Trigger modes:**
- **Manual** — user selects tracks/album/library, queues transcode jobs from UI
- **On ingest** — per-library setting; new source track auto-enqueues transcode to all child libraries
- **Sync** — diffs source vs derived library, enqueues jobs only for missing/stale derived tracks

**ffmpeg command:** `ffmpeg -i {source} {standard_args} {advanced_args} {output}`

Output path is derived by applying the target library's active organization rule to the source
track's tags. If no rule matches, source relative path is mirrored into the target root.

After completion: derived `tracks` row inserted, `track_links` row created, tags copied from
source and written to the new file.

---

## Album Art Management

**Tools:** `lofty` for embed/extract (pure Rust, no subprocess). `image` crate for
resize/recompress (pure Rust, no subprocess). ffmpeg is not used for art processing.

**Operations:**
- **Extract** — pull embedded art from file, save as external image
- **Embed** — write external image or Cover Art Archive fetch into file
- **Standardize** — resize/recompress embedded art to meet the assigned `art_profile`,
  re-embed; enqueued as `art_process` job

Art standardization can be triggered manually per track/album/library, or run automatically
when a tag suggestion with cover art is accepted.

---

## Organization Engine

Rules evaluated in priority order against a track's current tags; first match wins.

**Conditions** are field comparisons combined with AND/OR logic, stored as a JSON expression
tree. No conditions = match all.

**Apply modes:**
- **Preview** — shows what files would move, no changes made
- **Apply** — enqueues `organize` jobs for affected tracks
- **Auto-organize on ingest** — per-library setting; runs after tags are accepted

---

## Authentication

- **Password + HttpOnly cookie sessions** — base auth
- **TOTP** (RFC 6238) via `totp-rs` crate — authenticator app (Authy, Google Authenticator, etc.)
- **WebAuthn/FIDO2** via `webauthn-rs` crate — YubiKey hardware security keys and platform
  authenticators (Touch ID, Windows Hello, passkeys). Users can register multiple credentials.

Both 2FA methods can be active simultaneously per user; either satisfies the 2FA gate.
Admins can require 2FA globally or per-user. OIDC/SAML is out of scope for v1.0.

---

## UI Layout

**Navigation:** Top nav bar — Library · Inbox (badge count) · Issues · Jobs · Settings

**Library view:** Two-pane — left: library/artist/album/genre tree; right: track list.

**Track list:**
- Columns: `#`, Title, Artist, Album, Year, Genre, Format, Bitrate, Duration — plus any user-added columns (Disc #, Composer, Label, Catalog #, Sample Rate, etc.)
- Column visibility and order are user-customizable (⊕ button in column header row), persisted per user
- **Grouping:** optional — Group dropdown: None / Album / Artist / Genre / Year
- When grouping by Album: album header row shows thumbnail, title, year, track count, format summary, and per-album actions (Edit tags, Transcode)
- Tracks without the grouping field (e.g. no album tag) are collected under a "No [field] tag" group with a "Fix tags" shortcut
- **Sorting:** multi-column priority stack — field + asc/desc per level, drag to reorder priority, "Add level" to go deeper. Shift-click column header adds it as next sort level. Active sort shown condensed in toolbar (e.g. `Album, Disc, Track ▾`)
- Both grouping and sort settings persisted per user

**Non-library sections** (Inbox, Issues, Jobs) use full width below the nav — no tree panel.

**Inbox view:** Per-track cards showing current vs. suggested tags with diff highlighting,
confidence badge, cover art preview, and Accept / Edit / Reject / Search actions inline.
Batch "Accept all high confidence" action at top.

---

## Implementation Phases

### Phase 1 — Foundation
Library DB + file scanner + tag read/write (`lofty`) + job queue + basic UI shell +
auth (password, TOTP, WebAuthn/FIDO2) + theming engine + settings table + streaming endpoint groundwork.

### Phase 2 — Organization
Rule engine + path template system + multi-library DAG + file organize jobs + library management UI.

### Phase 3 — MusicBrainz Integration
`fpcalc` fingerprinting + AcoustID submission + MusicBrainz metadata fetch + tag suggestions +
Inbox review UI + Cover Art Archive integration.

### Phase 4 — Transcoding & Album Art
`ffmpeg` transcode pipeline + encoding profiles UI + transcode job management +
`image` crate art standardization + art profiles UI + art embed/extract operations.

### v1.1 — Streaming
`/stream` endpoint fully exercised + web audio player + playlist + queue management +
playback state.
