-- SQLite INTEGER is already 64-bit regardless of declared width.
-- No schema change needed. Exists to keep migration numbers in sync with Postgres.
SELECT 1;
