-- Inbound integrations (M14): the "watched" webhook. Only a hash of the token is stored.
CREATE TABLE integrations (
    id            uuid PRIMARY KEY,
    user_id       uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name          text NOT NULL,
    token_hash    text NOT NULL UNIQUE,
    created_at    timestamptz NOT NULL,
    last_seen_at  timestamptz,
    last_error    text
);
CREATE INDEX integrations_user ON integrations (user_id);
