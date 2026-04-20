-- Expand the job_type CHECK constraint to include 'freedb_lookup'
-- (required for the AcoustID → FreeDB fallback path).

ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_job_type_check;

ALTER TABLE jobs
    ADD CONSTRAINT jobs_job_type_check CHECK (job_type IN (
        'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
        'transcode', 'art_process', 'organize'
    ));
