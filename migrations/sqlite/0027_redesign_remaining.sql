-- migrations/sqlite/0027_redesign_remaining.sql
PRAGMA foreign_keys=OFF;

-- track_links: drop encoding_profile_id by recreating
CREATE TABLE track_links_new (
    source_track_id  INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    derived_track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (source_track_id, derived_track_id)
);
INSERT INTO track_links_new (source_track_id, derived_track_id, created_at)
    SELECT source_track_id, derived_track_id, created_at FROM track_links;
DROP TABLE track_links;
ALTER TABLE track_links_new RENAME TO track_links;
CREATE INDEX idx_track_links_source  ON track_links(source_track_id);
CREATE INDEX idx_track_links_derived ON track_links(derived_track_id);

-- virtual_library_sources: add surrogate id + library_profile_id
CREATE TABLE virtual_library_sources_new (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    virtual_library_id INTEGER NOT NULL REFERENCES virtual_libraries(id) ON DELETE CASCADE,
    library_id         INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    library_profile_id INTEGER NULL REFERENCES library_profiles(id) ON DELETE CASCADE,
    priority           INTEGER NOT NULL DEFAULT 0,
    UNIQUE (virtual_library_id, library_id, library_profile_id)
);
INSERT INTO virtual_library_sources_new (virtual_library_id, library_id, priority)
    SELECT virtual_library_id, library_id, priority FROM virtual_library_sources;
DROP TABLE virtual_library_sources;
ALTER TABLE virtual_library_sources_new RENAME TO virtual_library_sources;
CREATE INDEX idx_vls_priority ON virtual_library_sources(virtual_library_id, priority);

-- jobs: add process_staged (copy-recreate pattern)
CREATE TABLE jobs_new (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type     TEXT NOT NULL CHECK (job_type IN (
                     'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
                     'transcode', 'art_process', 'organize', 'cue_split',
                     'normalize', 'virtual_sync', 'process_staged'
                 )),
    status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
                     'pending', 'running', 'completed', 'failed', 'cancelled'
                 )),
    payload      TEXT NOT NULL DEFAULT '{}',
    result       TEXT,
    priority     INTEGER NOT NULL DEFAULT 0,
    attempts     INTEGER NOT NULL DEFAULT 0,
    error        TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    started_at   TEXT,
    completed_at TEXT
);
INSERT INTO jobs_new (id, job_type, status, payload, result, priority, attempts, error, created_at, started_at, completed_at)
    SELECT id, job_type, status, payload, result, priority, attempts, error, created_at, started_at, completed_at FROM jobs;
DROP TABLE jobs;
ALTER TABLE jobs_new RENAME TO jobs;
CREATE INDEX jobs_status_priority_idx ON jobs(status, priority DESC, created_at ASC);
CREATE INDEX jobs_job_type_status_idx ON jobs(job_type, status);

PRAGMA foreign_keys=ON;

-- settings: seed folder_art_filename
INSERT OR IGNORE INTO settings (key, value) VALUES ('folder_art_filename', 'folder.jpg');
