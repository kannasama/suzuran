-- migrations/postgres/0028_drop_ingest_dir.sql
ALTER TABLE libraries DROP COLUMN IF EXISTS ingest_dir;
