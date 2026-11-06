-- Library pages, most recently added first (M17 load test: 50 000 entries).
CREATE INDEX library_entries_recent ON library_entries (user_id, added_at DESC, id DESC);
