CREATE TABLE jobs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type    TEXT NOT NULL CHECK (job_type IN (
                    'scan', 'fingerprint', 'mb_lookup',
                    'transcode', 'art_process', 'organize'
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
    completed_at TEXT
);

CREATE INDEX jobs_status_priority_idx ON jobs(status, priority DESC, created_at ASC);
CREATE INDEX jobs_job_type_status_idx ON jobs(job_type, status);
