# Phase 1.2 — Database Migrations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Write all Phase 1 SQL migration files for both Postgres and SQLite backends, and verify they apply cleanly against a running Postgres container and a SQLite file.

**Architecture:** `migrations/postgres/` and `migrations/sqlite/` each contain 6 sequentially-numbered `.sql` files. Migrations are applied at server startup via `sqlx::migrate!()` (wired in Phase 1.3). This plan only creates the SQL files and verifies them with `sqlx-cli`. No application Rust code is written here.

**Tech Stack:** PostgreSQL 16, SQLite 3, sqlx-cli 0.7.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `migrations/postgres/0001_users.sql` | Create | users, sessions, api_tokens, audit_log |
| `migrations/postgres/0002_two_factor.sql` | Create | totp_entries, webauthn_credentials |
| `migrations/postgres/0003_settings_themes.sql` | Create | settings (key-value), themes |
| `migrations/postgres/0004_libraries.sql` | Create | libraries |
| `migrations/postgres/0005_tracks.sql` | Create | tracks |
| `migrations/postgres/0006_jobs.sql` | Create | jobs |
| `migrations/sqlite/0001_users.sql` | Create | same tables, SQLite types |
| `migrations/sqlite/0002_two_factor.sql` | Create | same tables, SQLite types |
| `migrations/sqlite/0003_settings_themes.sql` | Create | same tables, SQLite types |
| `migrations/sqlite/0004_libraries.sql` | Create | same tables, SQLite types |
| `migrations/sqlite/0005_tracks.sql` | Create | same tables, SQLite types |
| `migrations/sqlite/0006_jobs.sql` | Create | same tables, SQLite types |

**Type mapping (Postgres → SQLite):**
- `BIGSERIAL PRIMARY KEY` → `INTEGER PRIMARY KEY AUTOINCREMENT`
- `BIGINT` → `INTEGER`
- `BOOLEAN` → `INTEGER` (0/1)
- `JSONB` → `TEXT`
- `TIMESTAMPTZ` → `TEXT`
- `REAL` → `REAL`
- `TEXT` → `TEXT` (unchanged)

---

## Task 1: Postgres migration 0001 — users, sessions, api_tokens, audit_log

**Files:**
- Create: `migrations/postgres/0001_users.sql`

- [ ] **Step 1: Write the migration**

```sql
-- migrations/postgres/0001_users.sql

CREATE TABLE users (
    id              BIGSERIAL PRIMARY KEY,
    username        TEXT NOT NULL UNIQUE,
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    role            TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'user')),
    force_password_change BOOLEAN NOT NULL DEFAULT FALSE,
    totp_required   BOOLEAN NOT NULL DEFAULT FALSE,
    webauthn_required BOOLEAN NOT NULL DEFAULT FALSE,
    accent_color    TEXT,
    base_theme      TEXT NOT NULL DEFAULT 'dark' CHECK (base_theme IN ('dark', 'light')),
    theme_id        BIGINT,
    display_name    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sessions (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX sessions_token_hash_idx ON sessions(token_hash);
CREATE INDEX sessions_user_id_idx ON sessions(user_id);

CREATE TABLE api_tokens (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    token_hash  TEXT NOT NULL UNIQUE,
    last_used_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX api_tokens_token_hash_idx ON api_tokens(token_hash);

CREATE TABLE audit_log (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL,
    target_type TEXT,
    target_id   BIGINT,
    detail      TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX audit_log_user_id_idx ON audit_log(user_id);
CREATE INDEX audit_log_created_at_idx ON audit_log(created_at);
```

---

## Task 2: Postgres migration 0002 — TOTP and WebAuthn

**Files:**
- Create: `migrations/postgres/0002_two_factor.sql`

- [ ] **Step 1: Write the migration**

```sql
-- migrations/postgres/0002_two_factor.sql

CREATE TABLE totp_entries (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    secret      TEXT NOT NULL,  -- encrypted TOTP secret
    verified    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE webauthn_credentials (
    id              BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id   TEXT NOT NULL UNIQUE,
    public_key      TEXT NOT NULL,  -- CBOR-encoded public key, base64
    sign_count      BIGINT NOT NULL DEFAULT 0,
    name            TEXT NOT NULL DEFAULT 'Security Key',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at    TIMESTAMPTZ
);

CREATE INDEX webauthn_credentials_user_id_idx ON webauthn_credentials(user_id);
CREATE INDEX webauthn_credentials_credential_id_idx ON webauthn_credentials(credential_id);

-- Stores in-flight WebAuthn challenge state (short-lived, cleaned up on completion)
CREATE TABLE webauthn_challenges (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    challenge   TEXT NOT NULL,  -- JSON-serialized PasskeyRegistration or PasskeyAuthentication state
    kind        TEXT NOT NULL CHECK (kind IN ('registration', 'authentication')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX webauthn_challenges_user_id_idx ON webauthn_challenges(user_id);
```

---

## Task 3: Postgres migration 0003 — settings and themes

**Files:**
- Create: `migrations/postgres/0003_settings_themes.sql`

- [ ] **Step 1: Write the migration**

```sql
-- migrations/postgres/0003_settings_themes.sql

-- Key-value settings table. All app configuration beyond the minimal env vars lives here.
CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed with safe defaults
INSERT INTO settings (key, value) VALUES
    ('acoustid_api_key',         ''),
    ('mb_user_agent',            'suzuran/0.1 (https://github.com/user/suzuran)'),
    ('mb_rate_limit_ms',         '1000'),
    ('scan_concurrency',         '4'),
    ('transcode_concurrency',    '2'),
    ('mb_confidence_threshold',  '0.8'),
    ('default_art_profile_id',   '');

CREATE TABLE themes (
    id              BIGSERIAL PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    css_vars        JSONB NOT NULL DEFAULT '{}',
    accent_color    TEXT,
    background_url  TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

---

## Task 4: Postgres migration 0004 — libraries

**Files:**
- Create: `migrations/postgres/0004_libraries.sql`

- [ ] **Step 1: Write the migration**

```sql
-- migrations/postgres/0004_libraries.sql

CREATE TABLE libraries (
    id                      BIGSERIAL PRIMARY KEY,
    name                    TEXT NOT NULL,
    root_path               TEXT NOT NULL UNIQUE,
    format                  TEXT NOT NULL,  -- flac, aac, mp3, opus, etc.
    -- encoding_profile_id FK added in Phase 4 when encoding_profiles table exists
    encoding_profile_id     BIGINT,
    parent_library_id       BIGINT REFERENCES libraries(id) ON DELETE SET NULL,
    scan_enabled            BOOLEAN NOT NULL DEFAULT TRUE,
    scan_interval_secs      BIGINT NOT NULL DEFAULT 3600,
    auto_transcode_on_ingest BOOLEAN NOT NULL DEFAULT FALSE,
    auto_organize_on_ingest  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX libraries_parent_library_id_idx ON libraries(parent_library_id);
```

---

## Task 5: Postgres migration 0005 — tracks

**Files:**
- Create: `migrations/postgres/0005_tracks.sql`

- [ ] **Step 1: Write the migration**

```sql
-- migrations/postgres/0005_tracks.sql

CREATE TABLE tracks (
    id                  BIGSERIAL PRIMARY KEY,
    library_id          BIGINT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    relative_path       TEXT NOT NULL,
    file_hash           TEXT NOT NULL,

    -- Indexed columns: common fields used for display, search, sort, grouping
    title               TEXT,
    artist              TEXT,
    albumartist         TEXT,
    album               TEXT,
    tracknumber         TEXT,
    discnumber          TEXT,
    totaldiscs          TEXT,
    totaltracks         TEXT,
    date                TEXT,
    genre               TEXT,
    composer            TEXT,
    label               TEXT,
    catalognumber       TEXT,

    -- Full MusicBrainz/Picard tag catalog (complete key/value store)
    tags                JSONB NOT NULL DEFAULT '{}',

    -- Audio properties (populated during scan)
    duration_secs       REAL,
    bitrate             BIGINT,
    sample_rate         BIGINT,
    channels            BIGINT,
    has_embedded_art    BOOLEAN NOT NULL DEFAULT FALSE,
    acoustid_fingerprint TEXT,

    last_scanned_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (library_id, relative_path)
);

CREATE INDEX tracks_library_id_idx ON tracks(library_id);
CREATE INDEX tracks_artist_idx ON tracks(artist);
CREATE INDEX tracks_albumartist_idx ON tracks(albumartist);
CREATE INDEX tracks_album_idx ON tracks(album);
CREATE INDEX tracks_date_idx ON tracks(date);
CREATE INDEX tracks_file_hash_idx ON tracks(file_hash);
```

---

## Task 6: Postgres migration 0006 — jobs

**Files:**
- Create: `migrations/postgres/0006_jobs.sql`

- [ ] **Step 1: Write the migration**

```sql
-- migrations/postgres/0006_jobs.sql

CREATE TABLE jobs (
    id          BIGSERIAL PRIMARY KEY,
    job_type    TEXT NOT NULL CHECK (job_type IN (
                    'scan', 'fingerprint', 'mb_lookup',
                    'transcode', 'art_process', 'organize'
                )),
    status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
                    'pending', 'running', 'completed', 'failed', 'cancelled'
                )),
    payload     JSONB NOT NULL DEFAULT '{}',
    result      JSONB,
    priority    BIGINT NOT NULL DEFAULT 0,
    attempts    BIGINT NOT NULL DEFAULT 0,
    error       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at  TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

CREATE INDEX jobs_status_priority_idx ON jobs(status, priority DESC, created_at ASC);
CREATE INDEX jobs_job_type_status_idx ON jobs(job_type, status);
```

- [ ] **Step 2: Commit all Postgres migrations**

```bash
git add migrations/postgres/
git commit -m "feat: Postgres migrations 0001–0006 (Phase 1 schema)"
```

---

## Task 7: SQLite migrations (0001–0006)

**Files:**
- Create: `migrations/sqlite/0001_users.sql` through `migrations/sqlite/0006_jobs.sql`

- [ ] **Step 1: Write `migrations/sqlite/0001_users.sql`**

```sql
CREATE TABLE users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT NOT NULL UNIQUE,
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    role            TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'user')),
    force_password_change INTEGER NOT NULL DEFAULT 0,
    totp_required   INTEGER NOT NULL DEFAULT 0,
    webauthn_required INTEGER NOT NULL DEFAULT 0,
    accent_color    TEXT,
    base_theme      TEXT NOT NULL DEFAULT 'dark' CHECK (base_theme IN ('dark', 'light')),
    theme_id        INTEGER,
    display_name    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX sessions_token_hash_idx ON sessions(token_hash);
CREATE INDEX sessions_user_id_idx ON sessions(user_id);

CREATE TABLE api_tokens (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    token_hash  TEXT NOT NULL UNIQUE,
    last_used_at TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX api_tokens_token_hash_idx ON api_tokens(token_hash);

CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL,
    target_type TEXT,
    target_id   INTEGER,
    detail      TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX audit_log_user_id_idx ON audit_log(user_id);
CREATE INDEX audit_log_created_at_idx ON audit_log(created_at);
```

- [ ] **Step 2: Write `migrations/sqlite/0002_two_factor.sql`**

```sql
CREATE TABLE totp_entries (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    secret      TEXT NOT NULL,
    verified    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE webauthn_credentials (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id   TEXT NOT NULL UNIQUE,
    public_key      TEXT NOT NULL,
    sign_count      INTEGER NOT NULL DEFAULT 0,
    name            TEXT NOT NULL DEFAULT 'Security Key',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at    TEXT
);

CREATE INDEX webauthn_credentials_user_id_idx ON webauthn_credentials(user_id);
CREATE INDEX webauthn_credentials_credential_id_idx ON webauthn_credentials(credential_id);

CREATE TABLE webauthn_challenges (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    challenge   TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('registration', 'authentication')),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX webauthn_challenges_user_id_idx ON webauthn_challenges(user_id);
```

- [ ] **Step 3: Write `migrations/sqlite/0003_settings_themes.sql`**

```sql
CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO settings (key, value) VALUES
    ('acoustid_api_key',         ''),
    ('mb_user_agent',            'suzuran/0.1 (https://github.com/user/suzuran)'),
    ('mb_rate_limit_ms',         '1000'),
    ('scan_concurrency',         '4'),
    ('transcode_concurrency',    '2'),
    ('mb_confidence_threshold',  '0.8'),
    ('default_art_profile_id',   '');

CREATE TABLE themes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL UNIQUE,
    css_vars        TEXT NOT NULL DEFAULT '{}',
    accent_color    TEXT,
    background_url  TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- [ ] **Step 4: Write `migrations/sqlite/0004_libraries.sql`**

```sql
CREATE TABLE libraries (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    name                    TEXT NOT NULL,
    root_path               TEXT NOT NULL UNIQUE,
    format                  TEXT NOT NULL,
    encoding_profile_id     INTEGER,
    parent_library_id       INTEGER REFERENCES libraries(id) ON DELETE SET NULL,
    scan_enabled            INTEGER NOT NULL DEFAULT 1,
    scan_interval_secs      INTEGER NOT NULL DEFAULT 3600,
    auto_transcode_on_ingest INTEGER NOT NULL DEFAULT 0,
    auto_organize_on_ingest  INTEGER NOT NULL DEFAULT 0,
    created_at              TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX libraries_parent_library_id_idx ON libraries(parent_library_id);
```

- [ ] **Step 5: Write `migrations/sqlite/0005_tracks.sql`**

```sql
CREATE TABLE tracks (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id          INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    relative_path       TEXT NOT NULL,
    file_hash           TEXT NOT NULL,

    title               TEXT,
    artist              TEXT,
    albumartist         TEXT,
    album               TEXT,
    tracknumber         TEXT,
    discnumber          TEXT,
    totaldiscs          TEXT,
    totaltracks         TEXT,
    date                TEXT,
    genre               TEXT,
    composer            TEXT,
    label               TEXT,
    catalognumber       TEXT,

    tags                TEXT NOT NULL DEFAULT '{}',

    duration_secs       REAL,
    bitrate             INTEGER,
    sample_rate         INTEGER,
    channels            INTEGER,
    has_embedded_art    INTEGER NOT NULL DEFAULT 0,
    acoustid_fingerprint TEXT,

    last_scanned_at     TEXT NOT NULL DEFAULT (datetime('now')),
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE (library_id, relative_path)
);

CREATE INDEX tracks_library_id_idx ON tracks(library_id);
CREATE INDEX tracks_artist_idx ON tracks(artist);
CREATE INDEX tracks_albumartist_idx ON tracks(albumartist);
CREATE INDEX tracks_album_idx ON tracks(album);
CREATE INDEX tracks_date_idx ON tracks(date);
CREATE INDEX tracks_file_hash_idx ON tracks(file_hash);
```

- [ ] **Step 6: Write `migrations/sqlite/0006_jobs.sql`**

```sql
CREATE TABLE jobs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type    TEXT NOT NULL CHECK (job_type IN (
                    'scan', 'fingerprint', 'mb_lookup',
                    'transcode', 'art_process', 'organize'
                )),
    status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
                    'pending', 'running', 'completed', 'failed', 'cancelled'
                )),
    payload     TEXT NOT NULL DEFAULT '{}',
    result      TEXT,
    priority    INTEGER NOT NULL DEFAULT 0,
    attempts    INTEGER NOT NULL DEFAULT 0,
    error       TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    started_at  TEXT,
    completed_at TEXT
);

CREATE INDEX jobs_status_priority_idx ON jobs(status, priority DESC, created_at ASC);
CREATE INDEX jobs_job_type_status_idx ON jobs(job_type, status);
```

- [ ] **Step 7: Commit all SQLite migrations**

```bash
git add migrations/sqlite/
git commit -m "feat: SQLite migrations 0001–0006 (Phase 1 schema)"
```

---

## Task 8: Verify Postgres migrations with sqlx-cli

- [ ] **Step 1: Start the Postgres container**

```bash
docker compose up -d db
docker compose exec db pg_isready -U suzuran
```

Expected: `/var/run/postgresql:5432 - accepting connections`

- [ ] **Step 2: Install sqlx-cli (if not already installed)**

```bash
cargo install sqlx-cli --no-default-features --features postgres,sqlite
```

- [ ] **Step 3: Run Postgres migrations**

```bash
export DATABASE_URL=postgres://suzuran:suzuran@localhost:5432/suzuran
sqlx migrate run --source migrations/postgres
```

Expected output (6 lines):
```
Applied 0001_users.sql (Xms)
Applied 0002_two_factor.sql (Xms)
Applied 0003_settings_themes.sql (Xms)
Applied 0004_libraries.sql (Xms)
Applied 0005_tracks.sql (Xms)
Applied 0006_jobs.sql (Xms)
```

- [ ] **Step 4: Spot-check a table exists**

```bash
docker compose exec db psql -U suzuran -c "\dt"
```

Expected: table list includes `users`, `sessions`, `tracks`, `jobs`, `settings`, `themes`, `libraries`, etc.

- [ ] **Step 5: Tear down**

```bash
docker compose down -v
```

---

## Task 9: Verify SQLite migrations with sqlx-cli

- [ ] **Step 1: Run SQLite migrations against a temp file**

```bash
export DATABASE_URL=sqlite:///tmp/suzuran_test.db
sqlx migrate run --source migrations/sqlite
```

Expected output: 6 `Applied ...` lines.

- [ ] **Step 2: Spot-check**

```bash
sqlite3 /tmp/suzuran_test.db ".tables"
```

Expected: `api_tokens  audit_log  jobs  libraries  sessions  settings  themes  totp_entries  tracks  users  webauthn_challenges  webauthn_credentials`

- [ ] **Step 3: Clean up**

```bash
rm /tmp/suzuran_test.db
```

---

## Task 10: Update filemap

**Files:**
- Modify: `tasks/codebase-filemap.md`

- [ ] **Step 1: Add migration file entries to the filemap**

Add a `migrations/` section listing all 12 files.

- [ ] **Step 2: Commit**

```bash
git add tasks/codebase-filemap.md
git commit -m "docs: update filemap for Phase 1.2 migrations"
```
