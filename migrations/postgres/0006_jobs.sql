-- migrations/postgres/0006_jobs.sql

CREATE TABLE jobs (
    id          BIGSERIAL PRIMARY KEY,
    job_type    TEXT NOT NULL CHECK (job_type IN (
                    'scan', 'fingerprint', 'mb_lookup',
                    'transcode', 'art_process', 'organize'
                )),
    status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
                    'pending', 'running', 'completed', 'failed', 'cancelled'
                )),
    payload     JSONB NOT NULL DEFAULT '{}',
    result      JSONB,
    priority    BIGINT NOT NULL DEFAULT 0,
    attempts    BIGINT NOT NULL DEFAULT 0,
    error       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at  TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

CREATE INDEX jobs_status_priority_idx ON jobs(status, priority DESC, created_at ASC);
CREATE INDEX jobs_job_type_status_idx ON jobs(job_type, status);
