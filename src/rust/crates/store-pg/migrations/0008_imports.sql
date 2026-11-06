-- Imports (D-16): one row per import with its cursor and report, and the mapping from a
-- source row to the catalog item it created or matched (keeps re-imports idempotent).
CREATE TABLE imports (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    record      jsonb NOT NULL,
    updated_at  timestamptz NOT NULL
);
CREATE INDEX imports_user ON imports (user_id);

CREATE TABLE import_rows (
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    format      text NOT NULL,
    source_id   text NOT NULL,
    item_id     uuid NOT NULL REFERENCES catalog_items (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, format, source_id)
);
