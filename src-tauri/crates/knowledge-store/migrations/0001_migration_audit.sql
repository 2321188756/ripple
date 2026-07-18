-- Canonical migration integrity ledger for the Knowledge Service.
-- Business tables begin in the next migration after auth/ACL contracts land.
CREATE TABLE IF NOT EXISTS schema_migration_audit (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
