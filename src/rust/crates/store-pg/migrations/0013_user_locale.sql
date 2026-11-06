-- Per-user language (owner ruling DEC-171, DEC-201). NULL: no choice, the page follows the
-- browser's Accept-Language and then the brand's default locale.
ALTER TABLE users ADD COLUMN locale text;
