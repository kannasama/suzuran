-- duration_secs was created as REAL (FLOAT4) but the Rust model uses f64 (FLOAT8).
-- Widen to DOUBLE PRECISION so sqlx can decode Option<f64> without a type mismatch.
ALTER TABLE tracks ALTER COLUMN duration_secs TYPE DOUBLE PRECISION;
