-- Add run_after column to jobs for delayed/scheduled execution.
ALTER TABLE jobs ADD COLUMN run_after TEXT;

-- Recreate jobs table to expand the job_type CHECK constraint to include 'delete_tracks'.
-- SQLite does not support ALTER TABLE ... DROP CONSTRAINT.

PRAGMA foreign_keys=OFF;

CREATE TABLE jobs_new (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type    TEXT NOT NULL CHECK (job_type IN (
                    'scan', 'fingerprint', 'mb_lookup', 'freedb_lookup',
                    'transcode', 'art_process', 'organize', 'cue_split',
                    'normalize', 'virtual_sync', 'process_staged', 'maintenance',
                    'delete_tracks'
                )),
    status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
                    'pending', 'running', 'completed', 'failed', 'cancelled'
                )),
    payload     TEXT NOT NULL DEFAULT '{}',
    result      TEXT,
    priority    INTEGER NOT NULL DEFAULT 0,
    attempts    INTEGER NOT NULL DEFAULT 0,
    error       TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    started_at  TEXT,
    completed_at TEXT,
    run_after   TEXT
);

INSERT INTO jobs_new
    SELECT id, job_type, status, payload, result, priority, attempts,
           error, created_at, started_at, completed_at, run_after
    FROM jobs;

DROP TABLE jobs;
ALTER TABLE jobs_new RENAME TO jobs;

CREATE INDEX jobs_status_priority_idx ON jobs(status, priority DESC, created_at ASC);
CREATE INDEX jobs_job_type_status_idx ON jobs(job_type, status);

PRAGMA foreign_keys=ON;
