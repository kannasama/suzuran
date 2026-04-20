-- Expand the job_type CHECK constraint to include 'virtual_sync'.

ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_job_type_check;

ALTER TABLE jobs
    ADD CONSTRAINT jobs_job_type_check CHECK (job_type IN (
        'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
        'transcode', 'art_process', 'organize', 'cue_split', 'normalize', 'virtual_sync'
    ));
