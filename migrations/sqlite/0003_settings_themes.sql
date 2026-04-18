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
