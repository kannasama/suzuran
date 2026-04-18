-- migrations/postgres/0005_tracks.sql

CREATE TABLE tracks (
    id                  BIGSERIAL PRIMARY KEY,
    library_id          BIGINT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    relative_path       TEXT NOT NULL,
    file_hash           TEXT NOT NULL,

    -- Indexed columns: common fields used for display, search, sort, grouping
    title               TEXT,
    artist              TEXT,
    albumartist         TEXT,
    album               TEXT,
    tracknumber         TEXT,
    discnumber          TEXT,
    totaldiscs          TEXT,
    totaltracks         TEXT,
    date                TEXT,
    genre               TEXT,
    composer            TEXT,
    label               TEXT,
    catalognumber       TEXT,

    -- Full MusicBrainz/Picard tag catalog (complete key/value store)
    tags                JSONB NOT NULL DEFAULT '{}',

    -- Audio properties (populated during scan)
    duration_secs       REAL,
    bitrate             BIGINT,
    sample_rate         BIGINT,
    channels            BIGINT,
    has_embedded_art    BOOLEAN NOT NULL DEFAULT FALSE,
    acoustid_fingerprint TEXT,

    last_scanned_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (library_id, relative_path)
);

CREATE INDEX tracks_library_id_idx ON tracks(library_id);
CREATE INDEX tracks_artist_idx ON tracks(artist);
CREATE INDEX tracks_albumartist_idx ON tracks(albumartist);
CREATE INDEX tracks_album_idx ON tracks(album);
CREATE INDEX tracks_date_idx ON tracks(date);
CREATE INDEX tracks_file_hash_idx ON tracks(file_hash);
