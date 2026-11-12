-- OT1B-3 (DEC-611): how well known an item is at its source (TMDB: the number of ratings),
-- when the source says so. Only a tie-break of the search order; additive, NULL = unknown.
ALTER TABLE catalog_items ADD COLUMN popularity double precision;
