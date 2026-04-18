CREATE TABLE libraries (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    name                    TEXT NOT NULL,
    root_path               TEXT NOT NULL UNIQUE,
    format                  TEXT NOT NULL,
    encoding_profile_id     INTEGER,
    parent_library_id       INTEGER REFERENCES libraries(id) ON DELETE SET NULL,
    scan_enabled            INTEGER NOT NULL DEFAULT 1,
    scan_interval_secs      INTEGER NOT NULL DEFAULT 3600,
    auto_transcode_on_ingest INTEGER NOT NULL DEFAULT 0,
    auto_organize_on_ingest  INTEGER NOT NULL DEFAULT 0,
    created_at              TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX libraries_parent_library_id_idx ON libraries(parent_library_id);
