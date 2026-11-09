-- Who hears about a change of a catalog item (DEC-490). The change goes to the users the item
-- matters to instead of every signed-in user; the lookups by item below need these indexes
-- (they also serve the retention delete's reference checks).
CREATE INDEX library_entries_item ON library_entries (item_id);
CREATE INDEX list_items_item ON list_items (item_id);
CREATE INDEX sessions_item ON sessions (item_id);

-- An open item page: the viewer hears about the item until `until`, even when it is not in
-- their library (a stub being fetched). Not a reference: it goes with the item.
CREATE TABLE catalog_watchers (
    item_id  uuid NOT NULL REFERENCES catalog_items (id) ON DELETE CASCADE,
    user_id  uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    until    timestamptz NOT NULL,
    PRIMARY KEY (item_id, user_id)
);
CREATE INDEX catalog_watchers_user ON catalog_watchers (user_id, until);
