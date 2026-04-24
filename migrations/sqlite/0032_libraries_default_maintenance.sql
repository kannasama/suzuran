ALTER TABLE libraries ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0;
ALTER TABLE libraries ADD COLUMN maintenance_interval_secs INTEGER;
