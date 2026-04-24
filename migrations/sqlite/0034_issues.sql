CREATE TABLE issues (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id  INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    track_id    INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    issue_type  TEXT NOT NULL CHECK (issue_type IN (
                    'missing_file', 'bad_audio_properties', 'untagged', 'duplicate_mb_id'
                )),
    detail      TEXT,
    severity    TEXT NOT NULL CHECK (severity IN ('high', 'medium', 'low')),
    dismissed   INTEGER NOT NULL DEFAULT 0,
    resolved    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX issues_library_id_idx ON issues(library_id);
CREATE INDEX issues_track_id_idx ON issues(track_id);
CREATE UNIQUE INDEX issues_track_type_uq ON issues(track_id, issue_type) WHERE track_id IS NOT NULL;
