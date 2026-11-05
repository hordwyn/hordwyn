-- Core schema: users, companions, catalog, library, progress, reviews, lists, sessions,
-- outbox (D-13) and per-user change sequence (D-12).
-- HLC stamps are stored as sortable text: 16 hex wall-ms, '-', 8 hex counter, '-', 8 hex node.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE users (
    id              uuid PRIMARY KEY,
    username        text NOT NULL UNIQUE,
    display_name    text NOT NULL,
    email           text,
    role            text NOT NULL CHECK (role IN ('admin', 'member')),
    self_companion  uuid NOT NULL,
    created_at      timestamptz NOT NULL
);

CREATE TABLE companions (
    id            uuid PRIMARY KEY,
    owner_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name          text NOT NULL,
    avatar_asset  text,
    notes         text,
    linked_user   uuid REFERENCES users (id) ON DELETE SET NULL,
    snapshot      jsonb,
    is_self       boolean NOT NULL DEFAULT false,
    merged_into   uuid REFERENCES companions (id) ON DELETE SET NULL,
    created_at    timestamptz NOT NULL
);
CREATE INDEX companions_owner ON companions (owner_id);
CREATE INDEX companions_linked ON companions (linked_user) WHERE linked_user IS NOT NULL;
CREATE UNIQUE INDEX companions_one_self ON companions (owner_id) WHERE is_self;

ALTER TABLE users ADD CONSTRAINT users_self_companion_fk
    FOREIGN KEY (self_companion) REFERENCES companions (id) DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE link_requests (
    id            uuid PRIMARY KEY,
    companion_id  uuid NOT NULL REFERENCES companions (id) ON DELETE CASCADE,
    from_user     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    to_user       uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    state         text NOT NULL CHECK (state IN ('pending', 'accepted', 'declined', 'cancelled')),
    created_at    timestamptz NOT NULL
);
CREATE INDEX link_requests_to ON link_requests (to_user) WHERE state = 'pending';

CREATE TABLE invites (
    id            uuid PRIMARY KEY,
    companion_id  uuid NOT NULL REFERENCES companions (id) ON DELETE CASCADE,
    invited_by    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    email         text,
    token_hash    text NOT NULL UNIQUE,
    state         text NOT NULL CHECK (state IN ('pending', 'accepted', 'declined', 'cancelled')),
    expires_at    timestamptz NOT NULL,
    created_at    timestamptz NOT NULL
);

CREATE TABLE catalog_items (
    id              uuid PRIMARY KEY,
    kind            text NOT NULL,
    title           text NOT NULL,
    original_title  text,
    year            integer,
    description     text,
    cover_asset     text,
    attributes      jsonb NOT NULL DEFAULT '{}'::jsonb,
    state           text NOT NULL CHECK (state IN ('stub', 'partial', 'full', 'failed')),
    fetched_at      timestamptz,
    fresh_until     timestamptz,
    updated_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX catalog_items_kind ON catalog_items (kind);

CREATE TABLE external_ids (
    item_id    uuid NOT NULL REFERENCES catalog_items (id) ON DELETE CASCADE,
    kind       text NOT NULL,
    namespace  text NOT NULL,
    value      text NOT NULL,
    PRIMARY KEY (kind, namespace, value)
);
CREATE INDEX external_ids_item ON external_ids (item_id);

CREATE TABLE library_entries (
    id           uuid PRIMARY KEY,
    user_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    item_id      uuid NOT NULL REFERENCES catalog_items (id),
    kind         text NOT NULL,
    status       text NOT NULL,
    status_hlc   text NOT NULL,
    rating       smallint CHECK (rating BETWEEN 0 AND 100),
    rating_hlc   text NOT NULL,
    notes        text,
    notes_hlc    text NOT NULL,
    added_at     timestamptz NOT NULL,
    UNIQUE (user_id, item_id)
);

CREATE TABLE progress_events (
    id            uuid PRIMARY KEY,
    user_id       uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    item_id       uuid NOT NULL REFERENCES catalog_items (id),
    occurred_at   timestamptz NOT NULL,
    progress      jsonb NOT NULL,
    client_op_id  uuid,
    recorded_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, client_op_id)
);
CREATE INDEX progress_events_user_item ON progress_events (user_id, item_id, occurred_at);

CREATE TABLE reviews (
    id                 uuid PRIMARY KEY,
    user_id            uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    item_id            uuid NOT NULL REFERENCES catalog_items (id),
    body               text NOT NULL,
    contains_spoilers  boolean NOT NULL,
    visibility         jsonb NOT NULL,
    created_at         timestamptz NOT NULL,
    updated_at         timestamptz NOT NULL,
    UNIQUE (user_id, item_id)
);
CREATE INDEX reviews_item ON reviews (item_id);

CREATE TABLE lists (
    id           uuid PRIMARY KEY,
    owner_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name         text NOT NULL,
    description  text,
    visibility   jsonb NOT NULL,
    created_at   timestamptz NOT NULL,
    updated_at   timestamptz NOT NULL
);
CREATE INDEX lists_owner ON lists (owner_id);

CREATE TABLE list_items (
    list_id   uuid NOT NULL REFERENCES lists (id) ON DELETE CASCADE,
    item_id   uuid NOT NULL REFERENCES catalog_items (id),
    position  integer NOT NULL,
    note      text,
    added_at  timestamptz NOT NULL,
    PRIMARY KEY (list_id, item_id)
);

CREATE TABLE sessions (
    id                uuid PRIMARY KEY,
    owner_id          uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    item_id           uuid NOT NULL REFERENCES catalog_items (id),
    started_at        timestamptz NOT NULL,
    duration_minutes  integer,
    location          text,
    notes             text,
    coop_outcome      text CHECK (coop_outcome IN ('won', 'lost')),
    incomplete        boolean NOT NULL DEFAULT false
);
CREATE INDEX sessions_owner ON sessions (owner_id, started_at DESC);

CREATE TABLE session_participants (
    session_id       uuid NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    companion_id     uuid NOT NULL REFERENCES companions (id),
    position         integer NOT NULL,
    score            double precision,
    winner           boolean NOT NULL DEFAULT false,
    rank             integer,
    team             text,
    faction          text,
    starting_player  boolean NOT NULL DEFAULT false,
    first_play       boolean NOT NULL DEFAULT false,
    PRIMARY KEY (session_id, companion_id)
);
CREATE INDEX session_participants_companion ON session_participants (companion_id);

-- Written in the same transaction as the change it describes; consumed by the search
-- indexer and other workers.
CREATE TABLE outbox (
    id            bigserial PRIMARY KEY,
    entity        text NOT NULL,
    entity_id     uuid NOT NULL,
    op            text NOT NULL CHECK (op IN ('upsert', 'delete')),
    user_id       uuid,
    created_at    timestamptz NOT NULL DEFAULT now(),
    processed_at  timestamptz
);
CREATE INDEX outbox_pending ON outbox (id) WHERE processed_at IS NULL;

-- Per-user change sequence for sync clients (changes since cursor).
CREATE TABLE user_change_seq (
    user_id   uuid PRIMARY KEY,
    last_seq  bigint NOT NULL
);

CREATE TABLE user_changes (
    user_id    uuid NOT NULL,
    seq        bigint NOT NULL,
    entity     text NOT NULL,
    entity_id  uuid NOT NULL,
    op         text NOT NULL CHECK (op IN ('upsert', 'delete')),
    at         timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, seq)
);
