-- Sharing (D-9, M15): per-user privacy for sessions seen by linked participants, and the
-- lookup of link-shared lists by their token.
ALTER TABLE users ADD COLUMN share_sessions boolean NOT NULL DEFAULT true;
CREATE UNIQUE INDEX lists_share_token ON lists ((visibility ->> 'token'))
    WHERE visibility ->> 'type' = 'link';
