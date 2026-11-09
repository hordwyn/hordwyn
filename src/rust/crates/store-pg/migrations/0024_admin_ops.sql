-- WP4-2 admin-ops (phase 4 plan): health probes read from a table, data-source counters
-- shared by every process, the back-channel `jti` replay cache across processes and the
-- emailed one-time link for password-less accounts.

-- addendum-1 3.4 (DEC-285): the `admin.probes` job writes one row per Overview health row
-- every 60 s; the Overview reads them and never probes in the request.
CREATE TABLE health_probes (
    id         text PRIMARY KEY,
    status     text NOT NULL,
    detail     text NOT NULL,
    args       jsonb NOT NULL DEFAULT '{}'::jsonb,
    checked_at timestamptz NOT NULL
);

-- DEC-247/326: the counters of the governed client, flushed by every process (web, worker)
-- every 60 s and added up here. The per-connector marks keep the newest value of each field.
CREATE TABLE source_stats (
    connector            text PRIMARY KEY,
    last_ok_at           timestamptz,
    last_error_at        timestamptz,
    last_error           text,
    last_auth_status     smallint,
    last_auth_failure_at timestamptz,
    last_rate_limited_at timestamptz,
    circuit_opened_at    timestamptz,
    circuit_open_until   timestamptz,
    key_changed_at       timestamptz
);

-- calls and 429s per hour (hours since the Unix epoch); rows older than two days are dropped
-- by the flush
CREATE TABLE source_stats_hours (
    connector    text NOT NULL,
    hour         bigint NOT NULL,
    calls        bigint NOT NULL DEFAULT 0,
    rate_limited bigint NOT NULL DEFAULT 0,
    PRIMARY KEY (connector, hour)
);

-- Back-Channel Logout 1.0 2.6 step 7: a logout token's `jti` is accepted once per provider
-- across every process (was in memory per process, DEC-326).
CREATE TABLE oidc_used_jti (
    jti_key    text PRIMARY KEY,
    expires_at timestamptz NOT NULL
);
CREATE INDEX oidc_used_jti_expiry ON oidc_used_jti (expires_at);

-- DEC-312/316 (addendum-1 4.2 step 3): the emailed one-time link shares the reset table.
-- `purpose` keeps a link from ever working as a password reset; `binding` is the hash of the
-- link ticket the browser holds, so the link only works in the browser that asked for it.
ALTER TABLE password_resets
    ADD COLUMN purpose text NOT NULL DEFAULT 'password',
    ADD COLUMN binding text;
