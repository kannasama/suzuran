CREATE TABLE issues (
    id          BIGSERIAL PRIMARY KEY,
    library_id  BIGINT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    track_id    BIGINT REFERENCES tracks(id) ON DELETE CASCADE,
    issue_type  TEXT NOT NULL CHECK (issue_type IN (
                    'missing_file', 'bad_audio_properties', 'untagged', 'duplicate_mb_id'
                )),
    detail      TEXT,
    severity    TEXT NOT NULL CHECK (severity IN ('high', 'medium', 'low')),
    dismissed   BOOLEAN NOT NULL DEFAULT FALSE,
    resolved    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX issues_library_id_idx ON issues(library_id);
CREATE INDEX issues_track_id_idx ON issues(track_id);
CREATE UNIQUE INDEX issues_track_type_uq ON issues(track_id, issue_type) WHERE track_id IS NOT NULL;
