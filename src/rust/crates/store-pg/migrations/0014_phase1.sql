-- UX phase 1: tags on library entries, undo and status moves on progress events.

-- Tags are the user's own labels (design.md 3.1), private like notes. `tag` keeps the case as
-- typed, `tag_key` is its lower-case form; one entry has a tag once.
CREATE TABLE entry_tags (
    entry_id  uuid NOT NULL REFERENCES library_entries (id) ON DELETE CASCADE,
    user_id   uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    tag       text NOT NULL,
    tag_key   text NOT NULL,
    PRIMARY KEY (entry_id, tag_key)
);
CREATE INDEX entry_tags_user ON entry_tags (user_id, tag_key);

-- The status move an event caused (`{"from", "to", "stamp"}`), so undo can take it back.
ALTER TABLE progress_events ADD COLUMN status_change jsonb;
-- For an undo event: the event it takes back. Events stay append-only.
ALTER TABLE progress_events ADD COLUMN undoes uuid REFERENCES progress_events (id);
CREATE UNIQUE INDEX progress_events_undoes ON progress_events (undoes) WHERE undoes IS NOT NULL;
-- History pages, newest first.
CREATE INDEX progress_events_user_recent ON progress_events (user_id, occurred_at DESC, id DESC);
