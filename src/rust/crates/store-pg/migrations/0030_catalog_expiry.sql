-- UX4-9 (strict reading of TMDB terms 1.C, an orchestrator decision the owner may reverse): a
-- purge drops the title and year too. A purged item carries the time from which the retention
-- job may fetch it again once its source can answer (the purge time, then a day after each
-- attempt); a refresh that brings a title back clears it.
ALTER TABLE catalog_items ADD COLUMN provider_refetch_after timestamptz;
CREATE INDEX catalog_items_provider_refetch_after ON catalog_items (provider_refetch_after)
    WHERE provider_refetch_after IS NOT NULL;
