-- UX phase 3, instance server (addendum-1 3.2-3.4, 4.3; DEC-239, 245, 283, 289): mail settings
-- with an encrypted password, the sign-in mode, the per-source Disabled switch, one-time
-- password reset links and the per-user time zone.

-- addendum-1 3.2: SMTP lives in the database (the environment's [notify.smtp] still wins).
-- `mail_password` holds the envelope format of `auth::secrets` (purpose `smtp_password`).
ALTER TABLE instance_settings
    ADD COLUMN mail_host             text,
    ADD COLUMN mail_port             integer CHECK (mail_port BETWEEN 1 AND 65535),
    ADD COLUMN mail_security         text CHECK (mail_security IN ('tls', 'starttls', 'none')),
    ADD COLUMN mail_username         text,
    ADD COLUMN mail_from             text,
    ADD COLUMN mail_password         bytea,
    ADD COLUMN mail_last_test_at     timestamptz,
    ADD COLUMN mail_last_test_result text,
    -- addendum-1 4.3: `page` (default) or `auto`; enforced from phase 3 WP3-3 on.
    ADD COLUMN sign_in_mode          text CHECK (sign_in_mode IN ('page', 'auto'));

-- DEC-289: an admin switches a compiled-in source off (and on again) at run time.
CREATE TABLE source_switches (
    connector_id text PRIMARY KEY,
    enabled      boolean NOT NULL,
    changed_by   uuid REFERENCES users (id) ON DELETE SET NULL,
    changed_at   timestamptz NOT NULL
);

-- addendum-1 3.3: `cli admin reset-password` prints a one-time link (1 hour, single use);
-- only the hash of the token is stored.
CREATE TABLE password_resets (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash  text NOT NULL UNIQUE,
    created_at  timestamptz NOT NULL,
    expires_at  timestamptz NOT NULL,
    used_at     timestamptz
);
CREATE INDEX password_resets_user ON password_resets (user_id);

-- DEC-239: IANA zone name (`Europe/Berlin`); NULL follows the brand default, then UTC.
ALTER TABLE users ADD COLUMN time_zone text;
