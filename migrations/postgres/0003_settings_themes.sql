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
