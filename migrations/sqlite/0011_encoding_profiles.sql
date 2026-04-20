CREATE TABLE encoding_profiles (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    codec         TEXT NOT NULL,
    bitrate       TEXT,
    sample_rate   INTEGER,
    channels      INTEGER,
    bit_depth     INTEGER,   -- max acceptable source bit depth (lossless profiles; NULL = no limit)
    advanced_args TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
