-- WP3-7 imports page (design.md 6.11): a cancel request kept apart from the record, which the
-- running import overwrites at every checkpoint; the user's imports listed newest first.
ALTER TABLE imports ADD COLUMN cancel_requested boolean NOT NULL DEFAULT false;
CREATE INDEX imports_user_newest ON imports (user_id, id DESC);
