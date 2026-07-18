-- Lexical retrieval and durable citation provenance. Dense pgvector columns are
-- introduced with embedding profiles once a validated dimension is configured.
ALTER TABLE chunks ADD COLUMN search_vector tsvector
    GENERATED ALWAYS AS (to_tsvector('simple', content)) STORED;
CREATE INDEX chunks_search_vector_idx ON chunks USING GIN(search_vector);

CREATE TABLE retrieval_runs (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE RESTRICT,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    query_sha256 BYTEA NOT NULL CHECK (octet_length(query_sha256) = 32),
    mode TEXT NOT NULL CHECK (mode IN ('preview', 'automatic_context', 'deep_search_tool')),
    result_count INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE retrieval_hits (
    retrieval_run_id UUID NOT NULL REFERENCES retrieval_runs(id) ON DELETE CASCADE,
    chunk_id UUID NOT NULL REFERENCES chunks(id) ON DELETE RESTRICT,
    rank INTEGER NOT NULL CHECK (rank > 0),
    lexical_score DOUBLE PRECISION,
    fused_score DOUBLE PRECISION NOT NULL,
    selected BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (retrieval_run_id, chunk_id)
);

CREATE TABLE message_citations (
    id UUID PRIMARY KEY,
    retrieval_run_id UUID NOT NULL REFERENCES retrieval_runs(id) ON DELETE RESTRICT,
    message_external_id TEXT NOT NULL,
    chunk_id UUID NOT NULL REFERENCES chunks(id) ON DELETE RESTRICT,
    citation_key TEXT NOT NULL,
    char_start INTEGER NOT NULL,
    char_end INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (message_external_id, citation_key)
);
