CREATE TABLE art_profiles (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    name                TEXT NOT NULL,
    max_width_px        INTEGER NOT NULL DEFAULT 500,
    max_height_px       INTEGER NOT NULL DEFAULT 500,
    max_size_bytes      INTEGER,
    format              TEXT NOT NULL DEFAULT 'jpeg'
                            CHECK (format IN ('jpeg', 'png')),
    quality             INTEGER NOT NULL DEFAULT 90
                            CHECK (quality BETWEEN 1 AND 100),
    apply_to_library_id INTEGER REFERENCES libraries(id) ON DELETE SET NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);
