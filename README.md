# suzuran

A self-hosted music library manager built in Rust, containerised via Docker.
Designed for personal use by people who care about their audio collection.
Exposes a REST API with a bundled React web UI.

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](LICENSE)

## Background and Disclosure

I am first and foremost a network engineer. I run a fairly involved self-hosted environment,
both at home and in colocation. Over time I've accumulated a music collection that's sprawled
across formats and locations, and every tool I tried for managing it has frustrated me in some
way: too opinionated, too dependent on a running daemon, poor transcoding support, or just
abandoned. I eventually decided the right answer was to build exactly what I wanted.

I'll be upfront about the development approach, because I think it matters. Like its sibling
projects [rssekai](https://github.com/mjhill/rssekai) and
[Meridian DNS](https://github.com/mjhill/dns-orchestrator), **this project is
*technically* vibe coded** — built with Claude Code as the primary implementation tool.
Before you decide to shame me for it, let me explain my approach, because I've tried to do
this in a way that isn't reckless:

- **Design before code.** A complete design document and architecture specification were
  written before any implementation began, covering the data model, DAL design, authentication
  model, job system, deployment model, and feature taxonomy. Many projects that get derisively
  labeled as *vibe coded* skip this entirely. I did not. The design doc is the source of truth;
  the code implements it.
- **Language I wanted to learn.** I chose Rust as a deliberate learning exercise. I am not
  a Rust developer. I chose it because I was curious, and because, frankly, it's the popular
  choice these days. That said — I can read and follow what was produced, which I believe is
  a prerequisite for shipping code.
- **Explicit coding standards.** A code standards document was established upfront and is
  enforced via `rustfmt` and `clippy`. Consistency and predictability make review tractable.
- **Security-first architecture.** The API server owns all database interaction; the UI is
  a pure client. Passwords are Argon2id. Sessions are JWT-backed with server-side revocation.
  These decisions were made in the design phase, not retrofitted.

I built this to meet my own needs, and I've made it public because others may find it useful.

## Features

### Core

- **Multi-format scanning** — FLAC, MP3, AAC, Ogg Vorbis, Opus, WavPack (.wv), Monkey's
  Audio (.ape), TrueAudio (.tta), AIFF, and more via lofty
- **SHA-256 deduplication** — files are tracked by content hash; re-scans are incremental
- **Acoustic fingerprinting** — Chromaprint (fpcalc) integration; runs automatically after
  ingest for new tracks
- **MusicBrainz + AcoustID lookup** — automatic metadata suggestions with confidence scoring;
  Cover Art Archive integration
- **gnudb.org (FreeDB) fallback** — disc-ID lookup when DISCID tag is present and MusicBrainz
  finds no matches
- **CUE+audio splitting** — scanner detects paired CUE sheets; splits via `ffmpeg -c:a copy`
  and writes per-track tags; idempotent on re-scan
- **Tag suggestions inbox** — review diff-view cards, accept or reject individually or batch
  (≥ 80% confidence default), with cover art preview

### Organization

- **Rule-based organization** — condition trees (field / operator / value, with AND/OR/NOT)
  evaluated in priority order; first matching rule wins
- **Path template engine** — `{field}`, `{field:02}` zero-pad, `{field|fallback}`,
  `{discfolder}` synthetic token
- **Dry-run preview** — inspect the rename plan before applying
- **Auto-organize on ingest** — libraries can apply their rule set automatically after scan

### Transcoding & Album Art

- **Encoding profiles** — configurable codec, bitrate, sample rate, channels, bit depth
  ceiling, and advanced ffmpeg args
- **Quality compatibility guard** — rejects lossy-to-lossless, upsampling, bit-depth
  inflation, and bitrate upscaling; incompatible jobs skip cleanly rather than failing
- **Normalize-on-ingest** — optional in-place format conversion at ingest time; source deleted
  only after verified transcode
- **Auto-transcode to child libraries** — child libraries with `auto_transcode_on_ingest`
  receive jobs automatically
- **Art profiles** — max dimensions, size limit, JPEG/PNG format, quality setting
- **Art embed / extract / standardize** — resize and recompress via the `image` crate;
  auto-embed on suggestion accept if a cover art URL is available
- **Track links** — records source-to-derived relationships for transcoded tracks

### Libraries & Virtual Views

- **Multi-library DAG** — hierarchical parent/child relationships between libraries
- **Virtual libraries** — symlink or hardlink views of the best-available tracks across
  priority-ordered source libraries; `virtual_sync` job materializes the view
- **Track identity matching** — prefers MusicBrainz recording ID; falls back to normalized
  (albumartist, album, disc, track) tuple

### Security

- **Argon2id password hashing** — memory-hard, timing-attack resistant
- **JWT sessions with server-side tracking** — immediate revocation; server validates session
  record on every request
- **TOTP 2FA** — RFC 6238 time-based one-time passwords with otpauth URI and QR setup flow
- **WebAuthn / FIDO2 passkeys** — hardware key and platform authenticator support
- **Per-user and global 2FA enforcement** — admin can require 2FA for all users or per user
- **Parameterized SQL everywhere** — no string interpolation of user input into queries

### UI

- **React + TypeScript SPA** — served as static assets by the same Axum process
- **Dark / light modes** — system default with user toggle
- **Per-user accent colour** — cool-blue default; configurable per account
- **Custom themes** — full CSS variable surface; background image upload support
- **Inline settings** — encoding profiles, art profiles, virtual libraries, and themes
  managed without leaving the app
- **Live inbox badge** — pending tag suggestion count shown in navigation

## Quick Start

### Docker Compose (recommended)

```bash
# 1. Create environment file
cp .env.example .env

# Generate a strong secret
sed -i "s/^JWT_SECRET=.*/JWT_SECRET=$(openssl rand -hex 32)/" .env

# Edit RP_ID and RP_ORIGIN if deploying behind a reverse proxy (see .env.example comments)
# Edit the MUSIC_DIR volume in docker-compose.yml to point at your music collection

# 2. Start the stack
docker compose up -d

# 3. Open browser
open http://localhost:3000
```

The first registered user becomes the admin. Register at `/register` after the stack starts.

### From Source

Docker is the canonical build environment — local toolchain builds are not supported.

```bash
git clone https://github.com/mjhill/suzuran
cd suzuran
cp .env.example .env
# Edit .env with your values
docker buildx build --progress=plain -t suzuran:dev .
docker compose up -d
```

## Documentation

| Document | Description |
|----------|-------------|
| [Design](docs/plans/2026-04-16-suzuran-design.md) | Data model, service design, API surface, job system, deployment |
| [Versioning Policy](docs/VERSIONING.md) | MAJOR.MINOR.PATCH[-N] semantics and release revision rules |
| [Changelog](CHANGELOG.md) | Release history and per-version change notes |

## Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 2021 edition (1.88 MSRV) |
| HTTP server | axum 0.7 + tower + tower-http |
| Async runtime | tokio 1 |
| Database | PostgreSQL 16 / SQLite via sqlx 0.7 |
| Media tagging | lofty 0.21 |
| Audio fingerprinting | Chromaprint (fpcalc / libchromaprint) |
| Audio processing | ffmpeg (transcoding, CUE splitting) |
| Image processing | image 0.25 (JPEG, PNG resize / recompress) |
| HTTP client | reqwest 0.12 (rustls, no OpenSSL) |
| Auth | argon2 0.5 (Argon2id), jsonwebtoken 9, totp-rs 5, webauthn-rs 0.5 |
| Serialization | serde + serde_json |
| Error handling | thiserror + anyhow |
| Logging | tracing + tracing-subscriber (structured) |
| Frontend | React + TypeScript + Vite + Tailwind CSS |
| Container | Multi-stage Docker (debian:bookworm-slim runtime) |

## Contributing

Contributions are welcome. Please open an issue before submitting a pull request for
non-trivial changes. Code must pass `cargo fmt --check` and `cargo clippy` before review.

## License

suzuran is licensed under the
[GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0-or-later).
