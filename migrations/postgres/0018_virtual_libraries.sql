CREATE TABLE virtual_libraries (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    root_path   TEXT NOT NULL,
    link_type   TEXT NOT NULL DEFAULT 'symlink'
                    CHECK (link_type IN ('symlink', 'hardlink')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE virtual_library_sources (
    virtual_library_id  BIGINT NOT NULL REFERENCES virtual_libraries(id) ON DELETE CASCADE,
    library_id          BIGINT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    priority            INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (virtual_library_id, library_id)
);

CREATE INDEX idx_vls_priority ON virtual_library_sources(virtual_library_id, priority);

CREATE TABLE virtual_library_tracks (
    virtual_library_id  BIGINT NOT NULL REFERENCES virtual_libraries(id) ON DELETE CASCADE,
    source_track_id     BIGINT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    link_path           TEXT NOT NULL,
    PRIMARY KEY (virtual_library_id, source_track_id)
);
