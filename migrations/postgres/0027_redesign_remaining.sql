-- migrations/postgres/0027_redesign_remaining.sql

-- 1. track_links: drop encoding_profile_id (now redundant via library_profile_id on derived track)
ALTER TABLE track_links DROP COLUMN IF EXISTS encoding_profile_id;

-- 2. virtual_library_sources: add surrogate id + library_profile_id; replace composite PK
ALTER TABLE virtual_library_sources
    ADD COLUMN id BIGSERIAL,
    ADD COLUMN library_profile_id BIGINT NULL
        REFERENCES library_profiles(id) ON DELETE CASCADE;

ALTER TABLE virtual_library_sources DROP CONSTRAINT virtual_library_sources_pkey;
ALTER TABLE virtual_library_sources ADD PRIMARY KEY (id);

DROP INDEX IF EXISTS idx_vls_priority;
CREATE UNIQUE INDEX IF NOT EXISTS idx_vls_unique
    ON virtual_library_sources(virtual_library_id, library_id, library_profile_id);
CREATE INDEX IF NOT EXISTS idx_vls_priority
    ON virtual_library_sources(virtual_library_id, priority);

-- 3. jobs: add process_staged to CHECK (follow pattern from prior jobs migrations)
ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_job_type_check;
ALTER TABLE jobs ADD CONSTRAINT jobs_job_type_check CHECK (job_type IN (
    'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
    'transcode', 'art_process', 'organize', 'cue_split',
    'normalize', 'virtual_sync', 'process_staged'
));

-- 4. settings: seed folder_art_filename
INSERT INTO settings (key, value)
    VALUES ('folder_art_filename', 'folder.jpg')
    ON CONFLICT (key) DO NOTHING;
