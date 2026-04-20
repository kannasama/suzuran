CREATE TABLE track_links (
    source_track_id     INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    derived_track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    encoding_profile_id INTEGER REFERENCES encoding_profiles(id) ON DELETE SET NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (source_track_id, derived_track_id)
);

CREATE INDEX idx_track_links_source  ON track_links(source_track_id);
CREATE INDEX idx_track_links_derived ON track_links(derived_track_id);
