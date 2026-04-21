ALTER TABLE libraries
    ADD COLUMN IF NOT EXISTS organization_rule_id BIGINT
    REFERENCES organization_rules(id) ON DELETE SET NULL;
