-- migrations/postgres/0026_tracks_ingest_columns.sql

ALTER TABLE tracks
    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('staged', 'active', 'removed')),
    ADD COLUMN IF NOT EXISTS library_profile_id BIGINT NULL
        REFERENCES library_profiles(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_tracks_status ON tracks(status);
CREATE INDEX IF NOT EXISTS idx_tracks_library_profile_id ON tracks(library_profile_id);
