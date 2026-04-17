> **Note:** suzuran does not follow Semantic Versioning (SemVer). The MAJOR.MINOR.PATCH
> scheme used here has project-specific meaning described below — MINOR and PATCH have
> different semantics than the SemVer standard.

# Versioning Policy

suzuran uses a `MAJOR.MINOR.PATCH[-N]` versioning scheme with project-specific semantics
for each segment.

---

## Segments

| Segment | Meaning | Trigger |
|---------|---------|---------|
| **MAJOR** | Breaking or paradigm-shifting release | Incompatible API/schema changes requiring manual intervention, or a fundamental architectural rewrite |
| **MINOR** | New conceptual capability | Changes how a user thinks about or interacts with the system — new workflows, new paradigms, new integrations that stand on their own |
| **PATCH** | Fix or incremental extension | Bug fixes, corrections, and extensions of existing capability that don't change the mental model (new config option, new filter, new export format, small self-contained feature) |
| **-N** | Release revision (errata) | Corrections to the release artifact itself; starts at `-1` for each new base release |

---

## The MINOR/PATCH Decision Test

> "Does this change how a user reasons about the system, or does it extend something that already exists?"
> - New paradigm, new workflow, new conceptual layer → **MINOR**
> - Extends existing capability, fixes something, adds a setting → **PATCH**

---

## Pre-1.0 Development (0.x.y)

Before v1.0.0, the version scheme maps directly to the implementation phases in the design doc.
Each completed phase advances the MINOR version; subphase plan completions and fixes advance PATCH.

| Version range | Scope |
|---------------|-------|
| `v0.1.x` | Phase 1 — Foundation (scaffold, auth, scanner, job queue, UI shell) |
| `v0.2.x` | Phase 2 — Organization (rule engine, path templates, multi-library DAG) |
| `v0.3.x` | Phase 3 — MusicBrainz Integration (fingerprinting, AcoustID, tag suggestions, Inbox) |
| `v0.4.x` | Phase 4 — Transcoding & Album Art (ffmpeg pipeline, encoding profiles, art standardization) |
| `v1.0.0`  | First full release — all four phases complete, Docker image published |

**`v0.x.0` tags** are cut when a full phase is complete and all its tests pass.
**`v0.x.y` (y > 0)** covers fixes and incremental extensions within a phase's scope.
Individual subphase plan completions (1.1, 1.2, … 1.10) are not tagged — they land on
the phase branch and are included in the `v0.x.0` tag.

---

## Release Revisions (-N)

The `-N` suffix identifies a **release revision** (errata) — a correction to the release itself
rather than new functionality. It is distinct from a new PATCH release.

### Qualifies as -N (errata)

- A migration file was wrong or missing from the release
- A setting or feature explicitly committed to the release scope shipped incomplete or broken
- A Docker image build defect in the release artifact

### Does NOT qualify as -N

- New functionality not originally scoped to the release → new PATCH release
- A bug discovered after release that wasn't part of the release scope → new PATCH release

### Behavior

- `-N` starts at `-1` for each new base release (e.g., `v0.1.0-1`, not `v0.0.9-3`)
- Multiple errata on the same base are valid (`v0.1.0-1`, `v0.1.0-2`, …); if errata accumulate
  beyond two or three it is a signal to cut a new PATCH release instead
- Base release tags (`v0.1.0`) remain on the original commit and are never overwritten; `-N`
  tags point to the corrected commits that follow
- Docker image tags follow the same convention — `v0.1.0` is never re-pushed; only
  `v0.1.0-1`, `v0.1.0-2`, etc. receive new image pushes

---

## Branch Strategy

| Branch | Purpose |
|--------|---------|
| `main` | Stable. Tagged releases only. Direct commits only for docs and repo config. |
| `0.x` | Phase development branch (e.g., `0.1`). All subphase plan work merges here. |
| `0.x.y` | One branch per subphase plan (e.g., `0.1.1`). Merges into `0.x`. |
| `fix/description` | Bug fixes against a release branch or `main`. |

**Workflow per subphase plan:**
1. Branch from the current `0.x` branch: `git checkout -b 0.1.1 0.1`
2. Implement the plan, committing per the plan's commit steps
3. Open a PR into `0.x` (or merge directly for solo work)
4. When all subphase plans for a phase are merged, tag `v0.x.0` on `0.x` and merge `0.x` → `main`

**Starting Phase 1:** Create `0.1` branch from `main` before beginning Phase 1.1:
```bash
git checkout main
git checkout -b 0.1
```

---

## Examples

| Release | Rationale |
|---------|-----------|
| `v0.1.0` | MINOR (pre-1.0) — Phase 1 complete: auth, scanner, job queue, streaming groundwork, UI shell |
| `v0.1.1` | PATCH — Fix to scanner hash detection logic |
| `v0.1.0-1` | Errata — Missing SQLite migration in release artifact |
| `v0.2.0` | MINOR (pre-1.0) — Phase 2 complete: organization rules, path templates, multi-library DAG |
| `v0.3.0` | MINOR (pre-1.0) — Phase 3 complete: MusicBrainz integration, Inbox, AcoustID |
| `v0.4.0` | MINOR (pre-1.0) — Phase 4 complete: transcoding, encoding profiles, album art |
| `v1.0.0` | First public release — all phases complete, Docker image published |

---

## Release History

| Release | Rationale |
|---------|-----------|
| *(none yet — pre-development)* | |
