-- WP3-5 (design.md 6.9, R-38): what a sender's card shows about its last call. A refused
-- body (422) is attributed to its integration with the offending field and the body, secrets
-- redacted and at most 4 KiB (checked in `integrations::remembered`); the last accepted call
-- leaves the item it recorded. A 401 has no known token and is attributed to nothing.
ALTER TABLE integrations
    ADD COLUMN last_status       smallint,
    ADD COLUMN last_error_field  text,
    ADD COLUMN last_payload      text CHECK (octet_length(last_payload) <= 4096),
    ADD COLUMN last_item_id      uuid REFERENCES catalog_items (id) ON DELETE SET NULL,
    ADD COLUMN last_item_title   text;
