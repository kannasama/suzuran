CREATE TABLE organization_rules (
    id            BIGSERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    library_id    BIGINT REFERENCES libraries(id) ON DELETE CASCADE,
    priority      INTEGER NOT NULL DEFAULT 0,
    conditions    JSONB,
    path_template TEXT NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_org_rules_library_id ON organization_rules(library_id);
CREATE INDEX idx_org_rules_priority   ON organization_rules(priority);
