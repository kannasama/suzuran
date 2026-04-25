-- Add run_after column to jobs for delayed/scheduled execution.
-- NULL means run immediately (existing behaviour preserved).
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS run_after TIMESTAMPTZ;

-- Expand job_type CHECK to include delete_tracks.
ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_job_type_check;

ALTER TABLE jobs
    ADD CONSTRAINT jobs_job_type_check CHECK (job_type IN (
        'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
        'transcode', 'art_process', 'organize', 'cue_split',
        'normalize', 'virtual_sync', 'process_staged', 'maintenance',
        'delete_tracks'
    ));
