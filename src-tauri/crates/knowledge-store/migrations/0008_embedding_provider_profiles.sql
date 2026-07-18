CREATE TABLE embedding_provider_profiles (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    provider_kind TEXT NOT NULL CHECK (provider_kind = 'open_ai_compatible'),
    active_version_id UUID,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (organization_id, name)
);

CREATE TABLE embedding_provider_profile_versions (
    id UUID PRIMARY KEY,
    profile_id UUID NOT NULL REFERENCES embedding_provider_profiles(id) ON DELETE RESTRICT,
    version INTEGER NOT NULL CHECK (version > 0),
    base_url TEXT NOT NULL,
    model TEXT NOT NULL CHECK (length(model) BETWEEN 1 AND 200),
    expected_dimension INTEGER NOT NULL CHECK (expected_dimension BETWEEN 1 AND 65536),
    batch_size INTEGER NOT NULL CHECK (batch_size BETWEEN 1 AND 256),
    request_timeout_ms INTEGER NOT NULL CHECK (request_timeout_ms BETWEEN 1000 AND 300000),
    max_retries INTEGER NOT NULL CHECK (max_retries BETWEEN 0 AND 5),
    secret_ref TEXT NOT NULL CHECK (length(secret_ref) BETWEEN 1 AND 200),
    secret_digest BYTEA NOT NULL CHECK (octet_length(secret_digest) = 32),
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (profile_id, version),
    UNIQUE (profile_id, id)
);
ALTER TABLE embedding_provider_profiles
    ADD CONSTRAINT embedding_profile_active_version_fk
    FOREIGN KEY (id, active_version_id)
    REFERENCES embedding_provider_profile_versions(profile_id, id);

CREATE FUNCTION reject_embedding_profile_version_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'embedding profile versions are immutable';
END;
$$;
CREATE TRIGGER embedding_profile_versions_immutable
    BEFORE UPDATE OR DELETE ON embedding_provider_profile_versions
    FOR EACH ROW EXECUTE FUNCTION reject_embedding_profile_version_mutation();

ALTER TABLE source_revisions ADD COLUMN embedding_profile_id UUID;
ALTER TABLE source_revisions ADD COLUMN embedding_profile_version_id UUID;
ALTER TABLE source_revisions ADD COLUMN embedding_dimension INTEGER;
ALTER TABLE source_revisions ADD COLUMN embedding_state TEXT NOT NULL DEFAULT 'not_started'
    CHECK (embedding_state IN ('not_started','processing','ready','failed'));
ALTER TABLE source_revisions ADD COLUMN embedding_error_code TEXT;

CREATE TABLE chunk_embeddings (
    id UUID PRIMARY KEY,
    chunk_id UUID NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    source_revision_id UUID NOT NULL REFERENCES source_revisions(id) ON DELETE RESTRICT,
    profile_id UUID NOT NULL,
    profile_version_id UUID NOT NULL,
    dimension INTEGER NOT NULL CHECK (dimension > 0),
    encoding TEXT NOT NULL DEFAULT 'f32_le_v1' CHECK (encoding = 'f32_le_v1'),
    embedding_bytes BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (chunk_id, profile_version_id),
    FOREIGN KEY (profile_id, profile_version_id)
        REFERENCES embedding_provider_profile_versions(profile_id, id)
);
CREATE INDEX chunk_embeddings_revision_profile_idx
    ON chunk_embeddings(source_revision_id, profile_version_id);
