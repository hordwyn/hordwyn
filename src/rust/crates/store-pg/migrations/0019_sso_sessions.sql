-- UX phase 3 WP3-3 (DEC-317..): secrets kept with one session, encrypted in the envelope
-- format of `auth::secrets` (purpose `session_secret`, row = id, field = kind): the ID token
-- of an OIDC sign-in (`id_token_hint` of RP-initiated logout) and a gated action waiting for
-- an OIDC re-authentication round trip. They go with the session (ON DELETE CASCADE).

CREATE TABLE session_secrets (
    id         uuid PRIMARY KEY,
    token_hash text NOT NULL REFERENCES auth_sessions (token_hash) ON DELETE CASCADE,
    kind       text NOT NULL CHECK (kind IN ('id_token', 'pending_action')),
    value      bytea NOT NULL,
    created_at timestamptz NOT NULL,
    UNIQUE (token_hash, kind)
);

-- back-channel logout ends the sessions of one OP session (Back-Channel Logout 1.0, 2.7)
CREATE INDEX auth_sessions_oidc_sid ON auth_sessions (provider_id, oidc_sid)
    WHERE oidc_sid IS NOT NULL;
