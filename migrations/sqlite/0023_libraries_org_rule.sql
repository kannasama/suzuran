ALTER TABLE libraries
    ADD COLUMN organization_rule_id INTEGER
    REFERENCES organization_rules(id) ON DELETE SET NULL;
