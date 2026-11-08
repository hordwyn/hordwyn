-- WP3-6 (design.md 3.6, 6.5, 6.7, 5.10, 5.11): undoing a merge from its record (DEC-017) and
-- "delete keeps history" for a person who is named in sessions; the lists every member may
-- read ("On this instance", "Shared with me").
ALTER TABLE companions
    ADD COLUMN merge_record jsonb,
    ADD COLUMN removed_at   timestamptz;
CREATE INDEX lists_instance ON lists (updated_at DESC) WHERE visibility ->> 'type' = 'instance';
CREATE INDEX link_requests_from ON link_requests (from_user) WHERE state = 'pending';
