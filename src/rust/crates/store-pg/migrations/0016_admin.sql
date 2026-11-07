-- UX phase 2 (design.md 4, addendum-1 sections 2 and 3): secrets at rest, the credential
-- model, instance settings, the admin audit log, re-authentication, recovery codes,
-- sign-up invites and disabled accounts.
--
-- Encrypted columns hold the self-describing envelope format of `auth::secrets` (format
-- byte, key version at bytes 1-2, wrapped data key, nonce, ciphertext). Plain-text values
-- that exist from before are encrypted by the server on start (it needs the key).

-- TOTP seeds: encrypted copy; the plain column is emptied by the start-up migration and
-- dropped in a later release.
ALTER TABLE credentials ADD COLUMN totp_secret_enc bytea;

-- OD-08: one-time recovery codes, argon2id hashes only.
CREATE TABLE totp_recovery_codes (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_hash   text NOT NULL,
    created_at  timestamptz NOT NULL,
    used_at     timestamptz
);
CREATE INDEX totp_recovery_codes_user ON totp_recovery_codes (user_id);

-- Notification channel secrets move out of the JSON config (DEC-140 reversed): `config`
-- keeps the redacted form, `secret` the encrypted full config.
ALTER TABLE notify_channels ADD COLUMN secret bytea;

-- DEC-191: fresh proof for sign-in related changes, kept on the session row.
ALTER TABLE auth_sessions ADD COLUMN reauth_at timestamptz;
ALTER TABLE auth_sessions ADD COLUMN reauth_method text;
ALTER TABLE auth_sessions ADD COLUMN reauth_slide_at timestamptz;

-- Accounts an admin disabled (they cannot sign in; their sessions are revoked).
CREATE TABLE user_disabled (
    user_id     uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    disabled_by uuid REFERENCES users (id) ON DELETE SET NULL,
    disabled_at timestamptz NOT NULL
);

-- Admin invites to create an account (not tied to a companion).
CREATE TABLE signup_invites (
    id          uuid PRIMARY KEY,
    token_hash  text NOT NULL UNIQUE,
    email       text,
    note        text,
    created_by  uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at  timestamptz NOT NULL,
    expires_at  timestamptz NOT NULL,
    used_by     uuid REFERENCES users (id) ON DELETE SET NULL,
    used_at     timestamptz,
    revoked_at  timestamptz
);

-- design.md 4.2: data-source credentials, instance or per user, encrypted.
CREATE TABLE connector_credentials (
    id              uuid PRIMARY KEY,
    connector_id    text NOT NULL,
    scope           text NOT NULL CHECK (scope IN ('instance', 'user')),
    user_id         uuid REFERENCES users (id) ON DELETE CASCADE,
    secret          bytea NOT NULL,
    created_by      uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at      timestamptz NOT NULL,
    last_tested_at  timestamptz,
    last_result     text,
    CHECK ((scope = 'user') = (user_id IS NOT NULL))
);
CREATE UNIQUE INDEX connector_credentials_instance ON connector_credentials (connector_id)
    WHERE scope = 'instance';
CREATE UNIQUE INDEX connector_credentials_user ON connector_credentials (connector_id, user_id)
    WHERE scope = 'user';

-- design.md 4.1: hosted mode records per source whether an agreement is on file.
CREATE TABLE source_licences (
    connector_id      text PRIMARY KEY,
    agreement_on_file boolean NOT NULL,
    recorded_by       uuid REFERENCES users (id) ON DELETE SET NULL,
    recorded_at       timestamptz NOT NULL
);

-- addendum-1 3.2: database side of the instance settings, one row. NULL means "not set
-- here": the default applies (an explicit file or environment value always wins).
CREATE TABLE instance_settings (
    id                     boolean PRIMARY KEY DEFAULT true CHECK (id),
    hosting                text CHECK (hosting IN ('home', 'hosted')),
    signup                 text CHECK (signup IN ('open', 'invite', 'closed')),
    session_days           integer CHECK (session_days > 0),
    secure_cookies         boolean,
    storage_max_total_bytes bigint CHECK (storage_max_total_bytes > 0),
    storage_high_watermark double precision,
    storage_low_watermark  double precision,
    setup_completed_at     timestamptz,
    setup_step             text,
    updated_at             timestamptz,
    updated_by             uuid REFERENCES users (id) ON DELETE SET NULL
);
INSERT INTO instance_settings (id) VALUES (true);

-- Key versions seen by this instance (OD-09 reminder: two years after first use).
CREATE TABLE secrets_keys (
    version        integer PRIMARY KEY,
    first_seen_at  timestamptz NOT NULL
);

-- addendum-1 3.4: append-only audit log of admin actions; the retention job deletes rows
-- older than 365 days (OD-13). Details never contain secret values.
CREATE TABLE admin_audit (
    id           uuid PRIMARY KEY,
    at           timestamptz NOT NULL,
    actor_id     uuid REFERENCES users (id) ON DELETE SET NULL,
    actor_kind   text NOT NULL CHECK (actor_kind IN ('user', 'system', 'cli')),
    actor_name   text,
    action       text NOT NULL,
    target_type  text,
    target_id    text,
    detail       jsonb NOT NULL DEFAULT '{}',
    client_addr  text,
    request_id   text
);
CREATE INDEX admin_audit_at ON admin_audit (at DESC, id DESC);
CREATE INDEX admin_audit_actor ON admin_audit (actor_id, at DESC);
CREATE INDEX admin_audit_action ON admin_audit (action, at DESC);
