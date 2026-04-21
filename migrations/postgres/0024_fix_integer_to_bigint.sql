-- art_profiles: max_width_px, max_height_px, max_size_bytes, quality were
-- created as INTEGER (INT4) but the Rust model maps them to i64 (INT8).
ALTER TABLE art_profiles
    ALTER COLUMN max_width_px  TYPE BIGINT,
    ALTER COLUMN max_height_px TYPE BIGINT,
    ALTER COLUMN max_size_bytes TYPE BIGINT,
    ALTER COLUMN quality        TYPE BIGINT;

-- encoding_profiles: same issue for sample_rate, channels, bit_depth.
ALTER TABLE encoding_profiles
    ALTER COLUMN sample_rate TYPE BIGINT,
    ALTER COLUMN channels    TYPE BIGINT,
    ALTER COLUMN bit_depth   TYPE BIGINT;
