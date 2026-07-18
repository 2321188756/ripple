-- Canonical immutable source/revision and durable job foundation.
CREATE TABLE object_blobs (
    object_key TEXT PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    sha256 BYTEA NOT NULL CHECK (octet_length(sha256) = 32),
    byte_size BIGINT NOT NULL CHECK (byte_size >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (organization_id, sha256)
);

CREATE TABLE sources (
    id UUID PRIMARY KEY,
    collection_id UUID NOT NULL REFERENCES collections(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('upload', 'folder_file', 'url', 'agent_memory')),
    identity_key TEXT NOT NULL,
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 512),
    canonical_origin TEXT,
    active_revision_id UUID,
    state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'disabled', 'deleted')),
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ,
    UNIQUE (collection_id, kind, identity_key)
);

CREATE TABLE source_revisions (
    id UUID PRIMARY KEY,
    source_id UUID NOT NULL REFERENCES sources(id) ON DELETE RESTRICT,
    content_sha256 BYTEA NOT NULL CHECK (octet_length(content_sha256) = 32),
    byte_size BIGINT NOT NULL CHECK (byte_size >= 0),
    mime_type TEXT NOT NULL,
    original_object_key TEXT REFERENCES object_blobs(object_key) ON DELETE RESTRICT,
    normalized_object_key TEXT REFERENCES object_blobs(object_key) ON DELETE RESTRICT,
    extractor_id TEXT,
    extractor_version TEXT,
    state TEXT NOT NULL CHECK (state IN ('pending', 'processing', 'ready', 'failed', 'cancelled')),
    warnings JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(warnings) = 'array'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ,
    UNIQUE (source_id, content_sha256)
);

ALTER TABLE sources
    ADD CONSTRAINT sources_active_revision_fk
    FOREIGN KEY (active_revision_id) REFERENCES source_revisions(id) ON DELETE RESTRICT;

CREATE TABLE ingestion_jobs (
    id UUID PRIMARY KEY,
    collection_id UUID NOT NULL REFERENCES collections(id) ON DELETE RESTRICT,
    source_id UUID REFERENCES sources(id) ON DELETE RESTRICT,
    revision_id UUID REFERENCES source_revisions(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('ingest_revision', 'refresh_url', 'reconcile_folder', 'rebuild_index')),
    dedupe_key BYTEA NOT NULL CHECK (octet_length(dedupe_key) = 32),
    state TEXT NOT NULL CHECK (state IN ('queued', 'leased', 'running', 'retry_scheduled', 'succeeded', 'failed', 'cancelled')),
    attempt INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0),
    max_attempts INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts BETWEEN 1 AND 20),
    lease_owner TEXT,
    lease_expires_at TIMESTAMPTZ,
    next_run_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    cancel_requested_at TIMESTAMPTZ,
    progress_current BIGINT NOT NULL DEFAULT 0 CHECK (progress_current >= 0),
    progress_total BIGINT,
    error_code TEXT,
    correlation_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX ingestion_jobs_active_dedupe_idx ON ingestion_jobs(dedupe_key)
WHERE state IN ('queued', 'leased', 'running', 'retry_scheduled');
CREATE INDEX ingestion_jobs_dispatch_idx ON ingestion_jobs(state, next_run_at, created_at)
WHERE state IN ('queued', 'retry_scheduled');

CREATE TABLE ingestion_job_events (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES ingestion_jobs(id) ON DELETE CASCADE,
    state TEXT NOT NULL,
    progress_current BIGINT,
    progress_total BIGINT,
    error_code TEXT,
    correlation_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
