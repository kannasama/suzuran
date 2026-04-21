-- migrations/sqlite/0026_tracks_ingest_columns.sql

ALTER TABLE tracks ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('staged', 'active', 'removed'));
ALTER TABLE tracks ADD COLUMN library_profile_id INTEGER NULL
    REFERENCES library_profiles(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_tracks_status ON tracks(status);
CREATE INDEX IF NOT EXISTS idx_tracks_library_profile_id ON tracks(library_profile_id);
