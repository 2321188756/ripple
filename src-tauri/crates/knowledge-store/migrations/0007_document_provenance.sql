-- Persist structured extraction locations without changing prior migrations.
CREATE TABLE document_segments (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    source_revision_id UUID NOT NULL REFERENCES source_revisions(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    char_start INTEGER NOT NULL CHECK (char_start >= 0),
    char_end INTEGER NOT NULL CHECK (char_end >= char_start),
    line_start INTEGER CHECK (line_start > 0),
    line_end INTEGER CHECK (line_end >= line_start),
    page_start INTEGER CHECK (page_start > 0),
    page_end INTEGER CHECK (page_end >= page_start),
    heading_path JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (document_id, ordinal)
);
CREATE INDEX document_segments_revision_ordinal_idx
    ON document_segments(source_revision_id, ordinal);

ALTER TABLE chunks ADD COLUMN line_start INTEGER CHECK (line_start > 0);
ALTER TABLE chunks ADD COLUMN line_end INTEGER CHECK (line_end >= line_start);
ALTER TABLE chunks ADD COLUMN page_start INTEGER CHECK (page_start > 0);
ALTER TABLE chunks ADD COLUMN page_end INTEGER CHECK (page_end >= page_start);
ALTER TABLE chunks ADD COLUMN heading_path JSONB NOT NULL DEFAULT '[]'::jsonb;
