-- UX phase 3, OIDC providers (addendum-1 4.2 "Data model"; DEC-308..). Several providers,
-- configured in the admin area; the environment's `auth.oidc` is kept as a read-only row
-- (provenance `env`) so one code path serves both.

CREATE TABLE oidc_providers (
    id                   uuid PRIMARY KEY,
    slug                 text NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9][a-z0-9-]{0,31}$'),
    display_name         text NOT NULL,
    issuer               text NOT NULL,
    client_id            text NOT NULL,
    -- envelope format of `auth::secrets` (purpose `oidc_client_secret`, row = id)
    client_secret        bytea,
    scopes               text[] NOT NULL,
    enabled              boolean NOT NULL DEFAULT true,
    is_default           boolean NOT NULL DEFAULT false,
    auto_create          boolean NOT NULL DEFAULT false,
    groups_claim         text NOT NULL DEFAULT 'groups',
    allowed_groups       text[] NOT NULL DEFAULT '{}',
    admin_groups         text[] NOT NULL DEFAULT '{}',
    role_source          text NOT NULL DEFAULT 'local' CHECK (role_source IN ('local', 'provider')),
    provenance           text NOT NULL DEFAULT 'db' CHECK (provenance IN ('db', 'env')),
    -- the last discovery document and what it advertises
    discovery            jsonb,
    discovery_fetched_at timestamptz,
    capabilities         jsonb NOT NULL DEFAULT '{}',
    last_test            jsonb,
    last_test_at         timestamptz,
    last_callback_at     timestamptz,
    last_callback_result text,
    created_by           uuid REFERENCES users (id) ON DELETE SET NULL,
    updated_by           uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at           timestamptz NOT NULL,
    updated_at           timestamptz NOT NULL,
    -- OD-16: provider-managed roles only with admin groups
    CHECK (role_source = 'local' OR cardinality(admin_groups) > 0)
);
CREATE UNIQUE INDEX oidc_providers_one_default ON oidc_providers (is_default) WHERE is_default;

-- addendum-1 4.2: identities know their provider, the OP session id and when they were
-- linked; `last_groups` is what the last sign-in carried (NULL: no groups claim).
ALTER TABLE oidc_identities
    ADD COLUMN id               uuid NOT NULL DEFAULT gen_random_uuid() UNIQUE,
    ADD COLUMN provider_id      uuid REFERENCES oidc_providers (id) ON DELETE SET NULL,
    ADD COLUMN sid              text,
    ADD COLUMN linked_at        timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN last_claims_hash text,
    ADD COLUMN last_groups      text[];
CREATE INDEX oidc_identities_user ON oidc_identities (user_id);

-- back-channel logout (WP3-3) finds the sessions of an OP session
ALTER TABLE auth_sessions
    ADD COLUMN provider_id uuid REFERENCES oidc_providers (id) ON DELETE SET NULL,
    ADD COLUMN oidc_sid    text;

-- RFC 9700 4.2.4: a pending sign-in (state) and a link ticket work once, across processes.
CREATE TABLE oidc_used_states (
    state_hash text PRIMARY KEY,
    expires_at timestamptz NOT NULL
);
CREATE INDEX oidc_used_states_expiry ON oidc_used_states (expires_at);
