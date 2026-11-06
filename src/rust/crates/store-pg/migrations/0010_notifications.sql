-- Notifications (D-17, M13). Channel configs hold user-supplied secrets (tokens, webhook
-- secrets); the API never returns them.
CREATE TABLE notify_channels (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name        text NOT NULL,
    config      jsonb NOT NULL,
    enabled     boolean NOT NULL,
    created_at  timestamptz NOT NULL
);
CREATE INDEX notify_channels_user ON notify_channels (user_id);

-- user_id NULL: instance default set by an admin
CREATE TABLE notify_subscriptions (
    id                   uuid PRIMARY KEY,
    user_id              uuid REFERENCES users (id) ON DELETE CASCADE,
    event                text NOT NULL,
    filter               jsonb NOT NULL,
    targets              jsonb NOT NULL,
    mode                 jsonb NOT NULL,
    quiet_hours          jsonb,
    timezone             text NOT NULL,
    rate_limit_per_hour  integer,
    enabled              boolean NOT NULL,
    locked               boolean NOT NULL
);
CREATE INDEX notify_subscriptions_user ON notify_subscriptions (user_id);

CREATE TABLE notify_inbox (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    event       text NOT NULL,
    dedupe_key  text NOT NULL,
    title       text NOT NULL,
    body        text NOT NULL,
    link        text,
    created_at  timestamptz NOT NULL,
    read_at     timestamptz,
    UNIQUE (user_id, event, dedupe_key)
);
CREATE INDEX notify_inbox_recent ON notify_inbox (user_id, created_at DESC);

CREATE TABLE notify_deliveries (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    channel_id  uuid NOT NULL REFERENCES notify_channels (id) ON DELETE CASCADE,
    event       text NOT NULL,
    dedupe_key  text NOT NULL,
    title       text NOT NULL,
    body        text NOT NULL,
    link        text,
    state       text NOT NULL,
    attempts    integer NOT NULL,
    last_error  text,
    due_at      timestamptz NOT NULL,
    created_at  timestamptz NOT NULL,
    sent_at     timestamptz,
    UNIQUE (channel_id, event, dedupe_key)
);
CREATE INDEX notify_deliveries_digest ON notify_deliveries (channel_id, state, due_at);
CREATE INDEX notify_deliveries_recent ON notify_deliveries (user_id, created_at DESC);

-- admin overrides of notification texts
CREATE TABLE notify_templates (
    locale  text NOT NULL,
    key     text NOT NULL,
    text    text NOT NULL,
    PRIMARY KEY (locale, key)
);
