CREATE TABLE tracks (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id          INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    relative_path       TEXT NOT NULL,
    file_hash           TEXT NOT NULL,

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

    tags                TEXT NOT NULL DEFAULT '{}',

    duration_secs       REAL,
    bitrate             INTEGER,
    sample_rate         INTEGER,
    channels            INTEGER,
    has_embedded_art    INTEGER NOT NULL DEFAULT 0,
    acoustid_fingerprint TEXT,

    last_scanned_at     TEXT NOT NULL DEFAULT (datetime('now')),
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE (library_id, relative_path)
);

CREATE INDEX tracks_library_id_idx ON tracks(library_id);
CREATE INDEX tracks_artist_idx ON tracks(artist);
CREATE INDEX tracks_albumartist_idx ON tracks(albumartist);
CREATE INDEX tracks_album_idx ON tracks(album);
CREATE INDEX tracks_date_idx ON tracks(date);
CREATE INDEX tracks_file_hash_idx ON tracks(file_hash);
