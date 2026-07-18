-- Deterministic extracted documents and chunks for first ingestion workers.
CREATE TABLE documents (
    id UUID PRIMARY KEY,
    source_revision_id UUID NOT NULL REFERENCES source_revisions(id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    normalized_text TEXT NOT NULL,
    language TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (source_revision_id)
);

CREATE TABLE chunks (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    source_revision_id UUID NOT NULL REFERENCES source_revisions(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    content TEXT NOT NULL,
    text_sha256 BYTEA NOT NULL CHECK (octet_length(text_sha256) = 32),
    token_count INTEGER NOT NULL CHECK (token_count > 0),
    char_start INTEGER NOT NULL CHECK (char_start >= 0),
    char_end INTEGER NOT NULL CHECK (char_end >= char_start),
    predecessor_id UUID,
    successor_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (source_revision_id, ordinal)
);
CREATE INDEX chunks_revision_ordinal_idx ON chunks(source_revision_id, ordinal);
