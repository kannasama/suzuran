CREATE TABLE tag_suggestions (
    id              BIGSERIAL PRIMARY KEY,
    track_id        BIGINT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    source          TEXT NOT NULL CHECK (source IN ('acoustid', 'mb_search', 'freedb')),
    suggested_tags  JSONB NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.0,
    mb_recording_id TEXT,
    mb_release_id   TEXT,
    cover_art_url   TEXT,
    status          TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'accepted', 'rejected')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tag_suggestions_track_id ON tag_suggestions(track_id);
CREATE INDEX idx_tag_suggestions_status   ON tag_suggestions(status);
