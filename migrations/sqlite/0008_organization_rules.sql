CREATE TABLE organization_rules (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    library_id    INTEGER REFERENCES libraries(id) ON DELETE CASCADE,
    priority      INTEGER NOT NULL DEFAULT 0,
    conditions    TEXT,
    path_template TEXT NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_org_rules_library_id ON organization_rules(library_id);
CREATE INDEX idx_org_rules_priority   ON organization_rules(priority);
