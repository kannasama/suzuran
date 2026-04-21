-- migrations/postgres/0025_library_profiles.sql

-- Remove columns replaced by library_profiles join table
ALTER TABLE libraries
    DROP COLUMN IF EXISTS parent_library_id,
    DROP COLUMN IF EXISTS encoding_profile_id,
    DROP COLUMN IF EXISTS auto_transcode_on_ingest,
    DROP COLUMN IF EXISTS normalize_on_ingest;

DROP INDEX IF EXISTS libraries_parent_library_id_idx;

-- New table: one row per derived format per library
CREATE TABLE library_profiles (
    id                   BIGSERIAL PRIMARY KEY,
    library_id           BIGINT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    encoding_profile_id  BIGINT NOT NULL REFERENCES encoding_profiles(id) ON DELETE RESTRICT,
    derived_dir_name     TEXT NOT NULL,
    include_on_submit    BOOLEAN NOT NULL DEFAULT TRUE,
    auto_include_above_hz INTEGER NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (library_id, derived_dir_name)
);
