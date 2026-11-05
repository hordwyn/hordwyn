-- Provider cover URL (redirect fallback, D-7) and per-field provenance (D-6.7).
ALTER TABLE catalog_items
    ADD COLUMN cover_url text,
    ADD COLUMN provenance jsonb NOT NULL DEFAULT '{}'::jsonb;
