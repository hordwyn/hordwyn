-- WP4-8 (DEC-453): TMDB terms 1.C forbid caching its data for longer than six months, and an
-- item nobody opens is never refreshed. `provider_data_at` says when the connector data of an
-- item was obtained: set when the row is inserted, moved by every change of `fetched_at` (a
-- successful refresh), cleared when the data is purged. The retention job reads it.
ALTER TABLE catalog_items ADD COLUMN provider_data_at timestamptz;
UPDATE catalog_items SET provider_data_at = COALESCE(fetched_at, updated_at);
CREATE INDEX catalog_items_provider_data_at ON catalog_items (provider_data_at)
    WHERE provider_data_at IS NOT NULL;
