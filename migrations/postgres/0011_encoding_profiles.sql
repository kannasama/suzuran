CREATE TABLE encoding_profiles (
    id            BIGSERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    codec         TEXT NOT NULL,
    bitrate       TEXT,
    sample_rate   INTEGER,
    channels      INTEGER,
    bit_depth     INTEGER,   -- max acceptable source bit depth (lossless profiles; NULL = no limit)
    advanced_args TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
