-- Credentials, sessions and OIDC identities (auth crate). Users stay in `users`.

CREATE TABLE credentials (
    user_id         uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    password_hash   text,
    totp_secret     bytea,
    totp_enabled    boolean NOT NULL DEFAULT false,
    totp_last_step  bigint
);

CREATE TABLE auth_sessions (
    token_hash      text PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at      timestamptz NOT NULL,
    last_seen_at    timestamptz NOT NULL,
    expires_at      timestamptz NOT NULL
);
CREATE INDEX auth_sessions_user ON auth_sessions (user_id);
CREATE INDEX auth_sessions_expiry ON auth_sessions (expires_at);

CREATE TABLE oidc_identities (
    issuer          text NOT NULL,
    subject         text NOT NULL,
    user_id         uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    PRIMARY KEY (issuer, subject)
);
