-- Postgres search fallback (D-13): one row per document of the catalog, library and people
-- indexes. Full-text search with the language-neutral `simple` configuration plus trigram
-- similarity for typos and partial words.
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE search_docs (
    index_name  text NOT NULL,
    id          text NOT NULL,
    owner_id    uuid,
    fields      jsonb NOT NULL,
    body        text NOT NULL,
    tsv         tsvector NOT NULL,
    PRIMARY KEY (index_name, id)
);
CREATE INDEX search_docs_tsv ON search_docs USING gin (tsv);
CREATE INDEX search_docs_trgm ON search_docs USING gin (body gin_trgm_ops);
CREATE INDEX search_docs_owner ON search_docs (index_name, owner_id);
