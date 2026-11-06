-- HTTP cache index (D-6.5): validators and freshness per request key; bodies are
-- content-addressed blobs (`http/<blake3>`) in the budgeted blob store.
CREATE TABLE http_cache (
    key          text PRIMARY KEY,
    entry        jsonb NOT NULL,
    fresh_until  timestamptz NOT NULL
);
CREATE INDEX http_cache_fresh ON http_cache (fresh_until);
