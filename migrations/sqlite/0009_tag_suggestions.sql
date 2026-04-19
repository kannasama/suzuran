CREATE TABLE tag_suggestions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id        INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    source          TEXT NOT NULL,
    suggested_tags  TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.0,
    mb_recording_id TEXT,
    mb_release_id   TEXT,
    cover_art_url   TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_tag_suggestions_track_id ON tag_suggestions(track_id);
CREATE INDEX idx_tag_suggestions_status   ON tag_suggestions(status);
