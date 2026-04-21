-- tracks.bit_depth was added as INTEGER (INT4) in 0015 but missed by 0024.
-- virtual_library_sources.priority was INTEGER in 0018, not widened by 0027.
-- library_profiles.auto_include_above_hz was created as INTEGER in 0025.
-- All three Rust model fields are i64 (INT8) — widen to match.
ALTER TABLE tracks
    ALTER COLUMN bit_depth TYPE BIGINT;

ALTER TABLE virtual_library_sources
    ALTER COLUMN priority TYPE BIGINT;

ALTER TABLE library_profiles
    ALTER COLUMN auto_include_above_hz TYPE BIGINT;
