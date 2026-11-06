-- Assets and storage-budget accounting (D-7, D-8).

CREATE TABLE assets (
    id              uuid PRIMARY KEY,
    kind            text NOT NULL CHECK (kind IN ('cover', 'backdrop', 'person', 'avatar', 'upload')),
    source_url      text UNIQUE,
    connector       text,
    hash            text UNIQUE,
    width           integer,
    height          integer,
    thumbhash       bytea,
    dominant_color  text,
    pins            integer NOT NULL DEFAULT 0 CHECK (pins >= 0),
    owner_id        uuid REFERENCES users (id) ON DELETE SET NULL,
    priority        text NOT NULL CHECK (priority IN ('visible', 'library', 'related', 'catalog')),
    failed          boolean NOT NULL DEFAULT false,
    created_at      timestamptz NOT NULL
);

-- Every stored object; the budget decides from this table, never from the backend.
CREATE TABLE stored_objects (
    key             text PRIMARY KEY,
    class           text NOT NULL CHECK (class IN ('variant', 'original', 'http_cache', 'upload')),
    tier            text NOT NULL CHECK (tier IN ('pinned', 'cache')),
    bytes           bigint NOT NULL CHECK (bytes >= 0),
    owner_id        uuid,
    asset_id        uuid,
    priority        text NOT NULL CHECK (priority IN ('visible', 'library', 'related', 'catalog')),
    last_accessed   timestamptz NOT NULL
);
CREATE INDEX stored_objects_cache_lru ON stored_objects (last_accessed) WHERE tier = 'cache';
CREATE INDEX stored_objects_asset ON stored_objects (asset_id) WHERE asset_id IS NOT NULL;
CREATE INDEX stored_objects_uploads ON stored_objects (owner_id) WHERE class = 'upload';

-- Daily S3 operation counters (UTC days).
CREATE TABLE storage_ops (
    day             date PRIMARY KEY,
    puts            bigint NOT NULL DEFAULT 0,
    gets            bigint NOT NULL DEFAULT 0,
    deletes         bigint NOT NULL DEFAULT 0,
    egress_bytes    bigint NOT NULL DEFAULT 0
);
