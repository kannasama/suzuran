# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Session Start

At the beginning of every session, before responding to any request:
1. Read `tasks/lessons.md` — this is the authoritative, git-tracked record of all process rules,
   lessons, and user feedback. It mirrors the machine-local memory files and must be checked first.
2. Check `tasks/codebase-filemap.md` if the session involves code work.

Do not skip step 1 even if the task seems simple. The rules in `tasks/lessons.md` override default
behavior and have been established through repeated correction.

## Project

**suzuran** — a self-hosted music library manager (beets alternative). Rust/Axum backend + React/Vite/Tailwind SPA, deployed as a Docker image. Core features: multi-format library management, MusicBrainz tagging, rule-based file organization, and audio transcoding. See `docs/plans/2026-04-16-suzuran-design.md` for the full system design.

## Build & Test Commands

```bash
# Docker (canonical build — use this to verify before committing)
docker buildx build --progress=plain -t suzuran:dev .
docker compose up --build -d         # start full stack
docker compose down                  # stop stack
docker compose logs -f app           # follow app logs
```

## Docker Hub

Published image: `kannasama/suzuran`
`docker-compose.yml` defaults to `kannasama/suzuran:latest`; set `build: .` for local dev.

## Design Context

See `.impeccable.md` for the full design context. Summary:

- **Users:** Developer + potential public users; power users who manage their library with precision
- **Tone:** Technical & precise, clean & modern — foobar2000 DarkOne series is the reference aesthetic
- **Color mode:** Dark-first, both modes supported (system default + user toggle)
- **Accent:** Cool blue / indigo on near-black backgrounds
- **Visual direction:** Dense-utilitarian × elegant-refined — tabular data layouts, minimal chrome, no consumer streaming conventions

### Core Design Principles
1. **Data is the interface** — metadata, file paths, and tags are primary citizens
2. **Density with breath** — pack information like foobar2000, but with consistent spacing
3. **Dark-first, light-parity** — design for dark mode first; light mode matches the same density
4. **Precision over decoration** — every visual element must earn its place
5. **Power-user affordances** — keyboard nav, multi-select, bulk ops are first-class

## Repository Layout

- `docs/plans/` — Implementation plans (date-prefixed kebab-case: `2026-04-16-feature-name.md`)
- `migrations/postgres/` and `migrations/sqlite/` — DB migrations (numeric-prefix: `0001_initial.sql`)
- `resources/` — App assets (logos, icons)
- `scripts/` — Developer tooling (`setup-dev-hooks.sh`)
- `secrets/` — Local secret files (gitignored except README)
- `src/` — Rust/Axum backend
- `ui/` — Web frontend — Vite + React + Tailwind CSS
- `tests/` — Integration tests
- `tasks/codebase-filemap.md` — Lightweight index of every significant file; check before reading code
- `tasks/lessons.md` — Authoritative process rules and lessons learned (git-tracked)

## Development Workflow

- `/write-plan` — Create implementation plans before touching code
- `/execute-plan` — Execute a written plan with review checkpoints
- `/brainstorm` — Explore requirements before implementing features
- `/systematic-debugging` — Structured debugging workflow

**Before implementing any non-trivial task:** write and present a plan, wait for explicit approval.

**Versioning:** `docs/VERSIONING.md` defines the scheme. Pre-1.0 phases map to `v0.x.0` releases.

**Branch discipline:**
- `main` — stable, tagged releases only
- `0.x` — phase development branch (e.g., `0.1` for Phase 1 work)
- `0.x.y` — one branch per subphase plan (e.g., `0.1.1`)
- All implementation work branches from `0.x`, not `main`

Start Phase 1 by creating the `0.1` branch:
```bash
git checkout main && git checkout -b 0.1
```
Then branch each subphase plan from `0.1`: `git checkout -b 0.1.1 0.1`

## New Machine Setup

After cloning, install the memory-sync hook once:

```bash
bash scripts/setup-dev-hooks.sh
```

This writes `.claude/settings.json` (gitignored) with a hook that reminds Claude to sync any
memory write to `tasks/lessons.md`.
