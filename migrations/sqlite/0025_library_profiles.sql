-- migrations/sqlite/0025_library_profiles.sql
PRAGMA foreign_keys=OFF;

CREATE TABLE libraries_new (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    name                    TEXT NOT NULL,
    root_path               TEXT NOT NULL UNIQUE,
    format                  TEXT NOT NULL,
    scan_enabled            INTEGER NOT NULL DEFAULT 1,
    scan_interval_secs      INTEGER NOT NULL DEFAULT 3600,
    auto_organize_on_ingest INTEGER NOT NULL DEFAULT 0,
    tag_encoding            TEXT NOT NULL DEFAULT 'utf8',
    ingest_dir              TEXT,
    organization_rule_id    INTEGER REFERENCES organization_rules(id),
    created_at              TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO libraries_new
    SELECT id, name, root_path, format, scan_enabled, scan_interval_secs,
           auto_organize_on_ingest, tag_encoding, ingest_dir, organization_rule_id, created_at
    FROM libraries;

DROP TABLE libraries;
ALTER TABLE libraries_new RENAME TO libraries;

CREATE TABLE library_profiles (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id            INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    encoding_profile_id   INTEGER NOT NULL REFERENCES encoding_profiles(id) ON DELETE RESTRICT,
    derived_dir_name      TEXT NOT NULL,
    include_on_submit     INTEGER NOT NULL DEFAULT 1,
    auto_include_above_hz INTEGER NULL,
    created_at            TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (library_id, derived_dir_name)
);

PRAGMA foreign_keys=ON;
