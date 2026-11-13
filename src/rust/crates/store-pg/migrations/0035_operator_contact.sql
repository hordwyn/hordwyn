-- v1-books: the instance setting `instance.operator_contact` (NULL: not set here), an e-mail
-- address or https URL sent in the User-Agent of requests to Open Library and Wikidata.
ALTER TABLE instance_settings ADD COLUMN operator_contact text
    CHECK (operator_contact IS NULL OR char_length(operator_contact) BETWEEN 1 AND 200);

-- One pace per source for every process of the instance (review BOOK-8): a source without a
-- key limits the requests of an address, and the web role, a separate worker and the shell
-- commands (`cli refresh-failed`, `cli match-books`) each have their own token bucket.
-- `next_free` is when the next request to the source may be sent; a caller moves it on by
-- the source's interval and waits for the slot it got (`catalog_core::pg::PgPace`).
CREATE TABLE source_pace (
    source     text PRIMARY KEY,
    next_free  timestamptz NOT NULL
);

-- What `cli match-books --apply` changed (review BOOK-9): the item as it was before it got
-- the id of a source by a title match, so a wrong match can be found (`cli match-books
-- --applied`) and put back. No foreign key: the line outlives the item.
CREATE TABLE catalog_match_log (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    matched_at   timestamptz NOT NULL DEFAULT now(),
    item_id      uuid NOT NULL,
    source       text NOT NULL,
    external_id  text NOT NULL,
    before       jsonb NOT NULL
);
