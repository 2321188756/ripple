-- One-time bootstrap secret state. The value is only a SHA-256 digest.
CREATE TABLE service_bootstrap (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    bootstrap_token_digest BYTEA NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
