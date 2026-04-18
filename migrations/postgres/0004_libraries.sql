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
