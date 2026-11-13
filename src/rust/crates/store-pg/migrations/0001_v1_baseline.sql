-- v1 baseline: the whole schema of the first release in one migration (v1-release item 4).
-- It replaces the pre-v1 migrations 0001_core .. 0037_catalog_title_lookup, which were
-- squashed before the v1 tag: generated with pg_dump --schema-only from a database built by
-- that chain, and proven equal to it (crates/store-pg/tests/upgrade.rs; the stored schema is
-- tests/fixtures/schema.txt). From v1 on this file never changes; every change of the schema
-- is a new migration 0002_..., 0003_..., with its upgrade notes in the release notes.

-- SQL functions below name tables that are created after them.
SET LOCAL check_function_bodies = false;

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE FUNCTION jsonb_u64(j jsonb) RETURNS bigint
    LANGUAGE sql IMMUTABLE
    AS $_$
    SELECT CASE WHEN jsonb_typeof(j) = 'number' AND (j #>> '{}') ~ '^[0-9]{1,18}$'
                THEN (j #>> '{}')::bigint END
$_$;

CREATE FUNCTION library_activity(p_user uuid, p_item uuid, OUT last_at timestamp with time zone, OUT plays integer) RETURNS record
    LANGUAGE sql STABLE
    AS $$
    SELECT max(x.occurred_at),
           (COUNT(*) FILTER (WHERE x.progress ->> 'type' = 'session'))::integer
    FROM progress_events x
    WHERE x.user_id = p_user AND x.item_id = p_item AND x.undoes IS NULL
      AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = x.id)
$$;

CREATE FUNCTION library_entries_init_activity() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    SELECT a.last_at, a.plays INTO NEW.last_event_at, NEW.plays
    FROM library_activity(NEW.user_id, NEW.item_id) a;
    RETURN NEW;
END
$$;

CREATE FUNCTION library_entry_completion() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.status = 'completed' AND NEW.status = 'completed'
       AND OLD.completed_at IS NOT NULL THEN
        NEW.completed_at := OLD.completed_at;
        NEW.completed_undated := false;
    END IF;
    RETURN NEW;
END
$$;

CREATE FUNCTION progress_events_touch_entry() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    UPDATE library_entries e SET (last_event_at, plays) =
        (SELECT a.last_at, a.plays FROM library_activity(NEW.user_id, NEW.item_id) a)
    WHERE e.user_id = NEW.user_id AND e.item_id = NEW.item_id;
    RETURN NULL;
END
$$;

CREATE FUNCTION stats_entry_changed() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        IF (OLD.completed_at IS NOT NULL OR OLD.completed_undated)
           AND EXISTS (SELECT 1 FROM users u WHERE u.id = OLD.user_id) THEN
            INSERT INTO stats_dirty (user_id, item_id) VALUES (OLD.user_id, OLD.item_id)
            ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
        END IF;
    ELSIF TG_OP = 'INSERT' THEN
        IF NEW.completed_at IS NOT NULL OR NEW.completed_undated THEN
            INSERT INTO stats_dirty (user_id, item_id) VALUES (NEW.user_id, NEW.item_id)
            ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
        END IF;
    ELSIF OLD.completed_at IS DISTINCT FROM NEW.completed_at
          OR OLD.completed_undated IS DISTINCT FROM NEW.completed_undated THEN
        INSERT INTO stats_dirty (user_id, item_id) VALUES (NEW.user_id, NEW.item_id)
        ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    END IF;
    RETURN NULL;
END
$$;

CREATE FUNCTION stats_events_changed() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT p.user_id, p.item_id FROM (
        SELECT o.user_id, o.item_id FROM old_rows o
        UNION ALL
        SELECT n.user_id, n.item_id FROM new_rows n
    ) p
    ORDER BY 1, 2
    ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    RETURN NULL;
END
$$;

CREATE FUNCTION stats_events_deleted() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT o.user_id, o.item_id FROM old_rows o
    WHERE EXISTS (SELECT 1 FROM users u WHERE u.id = o.user_id)
    ORDER BY 1, 2
    ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    RETURN NULL;
END
$$;

CREATE FUNCTION stats_events_inserted() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT p.user_id, p.item_id FROM (
        SELECT n.user_id, n.item_id FROM new_rows n
        UNION ALL
        SELECT n.user_id, t.item_id FROM new_rows n JOIN progress_events t ON t.id = n.undoes
    ) p
    ORDER BY 1, 2
    ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    RETURN NULL;
END
$$;

CREATE FUNCTION stats_item_changed() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT p.user_id, NEW.id FROM (
        SELECT x.user_id FROM progress_events x WHERE x.item_id = NEW.id
        UNION
        SELECT e.user_id FROM library_entries e
        WHERE e.item_id = NEW.id AND (e.completed_at IS NOT NULL OR e.completed_undated)
    ) p
    ORDER BY 1, 2
    ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    RETURN NULL;
END
$$;

CREATE FUNCTION stats_sessions_changed() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP IN ('UPDATE', 'DELETE') AND EXISTS (SELECT 1 FROM users u WHERE u.id = OLD.owner_id) THEN
        INSERT INTO stats_dirty (user_id, item_id) VALUES (OLD.owner_id, OLD.item_id)
        ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    END IF;
    IF TG_OP IN ('INSERT', 'UPDATE') THEN
        INSERT INTO stats_dirty (user_id, item_id) VALUES (NEW.owner_id, NEW.item_id)
        ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    END IF;
    RETURN NULL;
END
$$;

CREATE FUNCTION stats_users_deleted() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    DELETE FROM stats_item_months WHERE user_id = OLD.user_id;
    RETURN NULL;
END
$$;

CREATE TABLE admin_audit (
    id uuid NOT NULL,
    at timestamp with time zone NOT NULL,
    actor_id uuid,
    actor_kind text NOT NULL,
    actor_name text,
    action text NOT NULL,
    target_type text,
    target_id text,
    detail jsonb DEFAULT '{}'::jsonb NOT NULL,
    client_addr text,
    request_id text,
    CONSTRAINT admin_audit_actor_kind_check CHECK ((actor_kind = ANY (ARRAY['user'::text, 'system'::text, 'cli'::text])))
);

CREATE TABLE assets (
    id uuid NOT NULL,
    kind text NOT NULL,
    source_url text,
    connector text,
    hash text,
    width integer,
    height integer,
    thumbhash bytea,
    dominant_color text,
    pins integer DEFAULT 0 NOT NULL,
    owner_id uuid,
    priority text NOT NULL,
    failed boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone NOT NULL,
    CONSTRAINT assets_kind_check CHECK ((kind = ANY (ARRAY['cover'::text, 'backdrop'::text, 'person'::text, 'avatar'::text, 'upload'::text]))),
    CONSTRAINT assets_pins_check CHECK ((pins >= 0)),
    CONSTRAINT assets_priority_check CHECK ((priority = ANY (ARRAY['visible'::text, 'library'::text, 'related'::text, 'catalog'::text])))
);

CREATE TABLE auth_sessions (
    token_hash text NOT NULL,
    user_id uuid NOT NULL,
    created_at timestamp with time zone NOT NULL,
    last_seen_at timestamp with time zone NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    reauth_at timestamp with time zone,
    reauth_method text,
    reauth_slide_at timestamp with time zone,
    provider_id uuid,
    oidc_sid text
);

CREATE TABLE catalog_episode_seasons (
    item_id uuid NOT NULL,
    season integer NOT NULL,
    connector text NOT NULL,
    episode_count integer NOT NULL,
    air_date date,
    fetched_at timestamp with time zone NOT NULL,
    CONSTRAINT catalog_episode_seasons_episode_count_check CHECK ((episode_count >= 0)),
    CONSTRAINT catalog_episode_seasons_season_check CHECK ((season > 0))
);

CREATE TABLE catalog_episodes (
    item_id uuid NOT NULL,
    season integer NOT NULL,
    number integer NOT NULL,
    air_date date,
    runtime_minutes integer,
    CONSTRAINT catalog_episodes_number_check CHECK ((number > 0)),
    CONSTRAINT catalog_episodes_runtime_minutes_check CHECK ((runtime_minutes > 0))
);

CREATE TABLE catalog_items (
    id uuid NOT NULL,
    kind text NOT NULL,
    title text NOT NULL,
    original_title text,
    year integer,
    description text,
    cover_asset text,
    attributes jsonb DEFAULT '{}'::jsonb NOT NULL,
    state text NOT NULL,
    fetched_at timestamp with time zone,
    fresh_until timestamp with time zone,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    cover_url text,
    provenance jsonb DEFAULT '{}'::jsonb NOT NULL,
    provider_data_at timestamp with time zone,
    provider_refetch_after timestamp with time zone,
    popularity double precision,
    CONSTRAINT catalog_items_state_check CHECK ((state = ANY (ARRAY['stub'::text, 'partial'::text, 'full'::text, 'failed'::text])))
);

CREATE TABLE catalog_match_log (
    id bigint NOT NULL,
    matched_at timestamp with time zone DEFAULT now() NOT NULL,
    item_id uuid NOT NULL,
    source text NOT NULL,
    external_id text NOT NULL,
    before jsonb NOT NULL
);

ALTER TABLE catalog_match_log ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME catalog_match_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

CREATE TABLE catalog_watchers (
    item_id uuid NOT NULL,
    user_id uuid NOT NULL,
    until timestamp with time zone NOT NULL
);

CREATE TABLE companions (
    id uuid NOT NULL,
    owner_id uuid NOT NULL,
    name text NOT NULL,
    avatar_asset text,
    notes text,
    linked_user uuid,
    snapshot jsonb,
    is_self boolean DEFAULT false NOT NULL,
    merged_into uuid,
    created_at timestamp with time zone NOT NULL,
    merge_record jsonb,
    removed_at timestamp with time zone
);

CREATE TABLE connector_credentials (
    id uuid NOT NULL,
    connector_id text NOT NULL,
    scope text NOT NULL,
    user_id uuid,
    secret bytea NOT NULL,
    created_by uuid,
    created_at timestamp with time zone NOT NULL,
    last_tested_at timestamp with time zone,
    last_result text,
    CONSTRAINT connector_credentials_check CHECK (((scope = 'user'::text) = (user_id IS NOT NULL))),
    CONSTRAINT connector_credentials_scope_check CHECK ((scope = ANY (ARRAY['instance'::text, 'user'::text])))
);

CREATE TABLE credentials (
    user_id uuid NOT NULL,
    password_hash text,
    totp_secret bytea,
    totp_enabled boolean DEFAULT false NOT NULL,
    totp_last_step bigint,
    totp_secret_enc bytea
);

CREATE TABLE cron_schedules (
    name text NOT NULL,
    schedule text NOT NULL,
    lane smallint NOT NULL,
    kind text NOT NULL,
    payload jsonb DEFAULT 'null'::jsonb NOT NULL,
    next_run_at timestamp with time zone NOT NULL,
    CONSTRAINT cron_schedules_lane_check CHECK (((lane >= 0) AND (lane <= 3)))
);

CREATE TABLE entry_tags (
    entry_id uuid NOT NULL,
    user_id uuid NOT NULL,
    tag text NOT NULL,
    tag_key text NOT NULL
);

CREATE TABLE external_ids (
    item_id uuid NOT NULL,
    kind text NOT NULL,
    namespace text NOT NULL,
    value text NOT NULL
);

CREATE TABLE game_group_members (
    group_id uuid NOT NULL,
    companion_id uuid NOT NULL,
    "position" integer NOT NULL
);

CREATE TABLE game_groups (
    id uuid NOT NULL,
    owner_id uuid NOT NULL,
    name text NOT NULL,
    removed_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL
);

CREATE TABLE goals (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    year integer NOT NULL,
    kind_group text NOT NULL,
    metric text NOT NULL,
    target integer NOT NULL,
    created_at timestamp with time zone NOT NULL,
    CONSTRAINT goals_metric_check CHECK ((metric = ANY (ARRAY['completions'::text, 'finished'::text, 'episodes'::text, 'pages'::text, 'plays'::text, 'minutes'::text]))),
    CONSTRAINT goals_target_check CHECK ((target > 0)),
    CONSTRAINT goals_year_check CHECK (((year >= 2000) AND (year <= 2100)))
);

CREATE TABLE health_probes (
    id text NOT NULL,
    status text NOT NULL,
    detail text NOT NULL,
    args jsonb DEFAULT '{}'::jsonb NOT NULL,
    checked_at timestamp with time zone NOT NULL
);

CREATE TABLE http_cache (
    key text NOT NULL,
    entry jsonb NOT NULL,
    fresh_until timestamp with time zone NOT NULL
);

CREATE TABLE import_rows (
    user_id uuid NOT NULL,
    format text NOT NULL,
    source_id text NOT NULL,
    item_id uuid NOT NULL
);

CREATE TABLE imports (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    record jsonb NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    cancel_requested boolean DEFAULT false NOT NULL
);

CREATE TABLE instance_settings (
    id boolean DEFAULT true NOT NULL,
    hosting text,
    signup text,
    session_days integer,
    secure_cookies boolean,
    storage_max_total_bytes bigint,
    storage_high_watermark double precision,
    storage_low_watermark double precision,
    setup_completed_at timestamp with time zone,
    setup_step text,
    updated_at timestamp with time zone,
    updated_by uuid,
    mail_host text,
    mail_port integer,
    mail_security text,
    mail_username text,
    mail_from text,
    mail_password bytea,
    mail_last_test_at timestamp with time zone,
    mail_last_test_result text,
    sign_in_mode text,
    usage_funnel boolean,
    operator_contact text,
    CONSTRAINT instance_settings_hosting_check CHECK ((hosting = ANY (ARRAY['home'::text, 'hosted'::text]))),
    CONSTRAINT instance_settings_id_check CHECK (id),
    CONSTRAINT instance_settings_mail_port_check CHECK (((mail_port >= 1) AND (mail_port <= 65535))),
    CONSTRAINT instance_settings_mail_security_check CHECK ((mail_security = ANY (ARRAY['tls'::text, 'starttls'::text, 'none'::text]))),
    CONSTRAINT instance_settings_operator_contact_check CHECK (((operator_contact IS NULL) OR ((char_length(operator_contact) >= 1) AND (char_length(operator_contact) <= 200)))),
    CONSTRAINT instance_settings_session_days_check CHECK ((session_days > 0)),
    CONSTRAINT instance_settings_sign_in_mode_check CHECK ((sign_in_mode = ANY (ARRAY['page'::text, 'auto'::text]))),
    CONSTRAINT instance_settings_signup_check CHECK ((signup = ANY (ARRAY['open'::text, 'invite'::text, 'closed'::text]))),
    CONSTRAINT instance_settings_storage_max_total_bytes_check CHECK ((storage_max_total_bytes > 0))
);

CREATE TABLE integrations (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    name text NOT NULL,
    token_hash text NOT NULL,
    created_at timestamp with time zone NOT NULL,
    last_seen_at timestamp with time zone,
    last_error text,
    last_status smallint,
    last_error_field text,
    last_payload text,
    last_item_id uuid,
    last_item_title text,
    CONSTRAINT integrations_last_payload_check CHECK ((octet_length(last_payload) <= 4096))
);

CREATE TABLE invites (
    id uuid NOT NULL,
    companion_id uuid NOT NULL,
    invited_by uuid NOT NULL,
    email text,
    token_hash text NOT NULL,
    state text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone NOT NULL,
    CONSTRAINT invites_state_check CHECK ((state = ANY (ARRAY['pending'::text, 'accepted'::text, 'declined'::text, 'cancelled'::text])))
);

CREATE TABLE jobs (
    id uuid NOT NULL,
    lane smallint NOT NULL,
    kind text NOT NULL,
    payload jsonb DEFAULT 'null'::jsonb NOT NULL,
    dedupe_key text,
    priority smallint DEFAULT 0 NOT NULL,
    state text DEFAULT 'queued'::text NOT NULL,
    run_at timestamp with time zone DEFAULT now() NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 5 NOT NULL,
    locked_until timestamp with time zone,
    last_error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT jobs_lane_check CHECK (((lane >= 0) AND (lane <= 3))),
    CONSTRAINT jobs_state_check CHECK ((state = ANY (ARRAY['queued'::text, 'running'::text, 'done'::text, 'dead'::text])))
);

CREATE TABLE library_entries (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    item_id uuid NOT NULL,
    kind text NOT NULL,
    status text NOT NULL,
    status_hlc text NOT NULL,
    rating smallint,
    rating_hlc text NOT NULL,
    notes text,
    notes_hlc text NOT NULL,
    added_at timestamp with time zone NOT NULL,
    last_event_at timestamp with time zone,
    plays integer DEFAULT 0 NOT NULL,
    completed_at timestamp with time zone,
    completed_undated boolean DEFAULT false NOT NULL,
    CONSTRAINT library_entries_completion CHECK ((NOT (completed_undated AND (completed_at IS NOT NULL)))),
    CONSTRAINT library_entries_rating_check CHECK (((rating >= 0) AND (rating <= 100)))
);

CREATE TABLE link_requests (
    id uuid NOT NULL,
    companion_id uuid NOT NULL,
    from_user uuid NOT NULL,
    to_user uuid NOT NULL,
    state text NOT NULL,
    created_at timestamp with time zone NOT NULL,
    CONSTRAINT link_requests_state_check CHECK ((state = ANY (ARRAY['pending'::text, 'accepted'::text, 'declined'::text, 'cancelled'::text])))
);

CREATE TABLE list_items (
    list_id uuid NOT NULL,
    item_id uuid NOT NULL,
    "position" integer NOT NULL,
    note text,
    added_at timestamp with time zone NOT NULL
);

CREATE TABLE lists (
    id uuid NOT NULL,
    owner_id uuid NOT NULL,
    name text NOT NULL,
    description text,
    visibility jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);

CREATE TABLE notify_channels (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    name text NOT NULL,
    config jsonb NOT NULL,
    enabled boolean NOT NULL,
    created_at timestamp with time zone NOT NULL,
    secret bytea
);

CREATE TABLE notify_deliveries (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    channel_id uuid NOT NULL,
    event text NOT NULL,
    dedupe_key text NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    link text,
    state text NOT NULL,
    attempts integer NOT NULL,
    last_error text,
    due_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone NOT NULL,
    sent_at timestamp with time zone
);

CREATE TABLE notify_inbox (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    event text NOT NULL,
    dedupe_key text NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    link text,
    created_at timestamp with time zone NOT NULL,
    read_at timestamp with time zone
);

CREATE TABLE notify_subscriptions (
    id uuid NOT NULL,
    user_id uuid,
    event text NOT NULL,
    filter jsonb NOT NULL,
    targets jsonb NOT NULL,
    mode jsonb NOT NULL,
    quiet_hours jsonb,
    timezone text NOT NULL,
    rate_limit_per_hour integer,
    enabled boolean NOT NULL,
    locked boolean NOT NULL
);

CREATE TABLE notify_templates (
    locale text NOT NULL,
    key text NOT NULL,
    text text NOT NULL
);

CREATE TABLE oidc_identities (
    issuer text NOT NULL,
    subject text NOT NULL,
    user_id uuid NOT NULL,
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid,
    sid text,
    linked_at timestamp with time zone DEFAULT now() NOT NULL,
    last_claims_hash text,
    last_groups text[]
);

CREATE TABLE oidc_providers (
    id uuid NOT NULL,
    slug text NOT NULL,
    display_name text NOT NULL,
    issuer text NOT NULL,
    client_id text NOT NULL,
    client_secret bytea,
    scopes text[] NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    is_default boolean DEFAULT false NOT NULL,
    auto_create boolean DEFAULT false NOT NULL,
    groups_claim text DEFAULT 'groups'::text NOT NULL,
    allowed_groups text[] DEFAULT '{}'::text[] NOT NULL,
    admin_groups text[] DEFAULT '{}'::text[] NOT NULL,
    role_source text DEFAULT 'local'::text NOT NULL,
    provenance text DEFAULT 'db'::text NOT NULL,
    discovery jsonb,
    discovery_fetched_at timestamp with time zone,
    capabilities jsonb DEFAULT '{}'::jsonb NOT NULL,
    last_test jsonb,
    last_test_at timestamp with time zone,
    last_callback_at timestamp with time zone,
    last_callback_result text,
    created_by uuid,
    updated_by uuid,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    CONSTRAINT oidc_providers_check CHECK (((role_source = 'local'::text) OR (cardinality(admin_groups) > 0))),
    CONSTRAINT oidc_providers_provenance_check CHECK ((provenance = ANY (ARRAY['db'::text, 'env'::text]))),
    CONSTRAINT oidc_providers_role_source_check CHECK ((role_source = ANY (ARRAY['local'::text, 'provider'::text]))),
    CONSTRAINT oidc_providers_slug_check CHECK ((slug ~ '^[a-z0-9][a-z0-9-]{0,31}$'::text))
);

CREATE TABLE oidc_used_jti (
    jti_key text NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

CREATE TABLE oidc_used_states (
    state_hash text NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

CREATE TABLE outbox (
    id bigint NOT NULL,
    entity text NOT NULL,
    entity_id uuid NOT NULL,
    op text NOT NULL,
    user_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    processed_at timestamp with time zone,
    CONSTRAINT outbox_op_check CHECK ((op = ANY (ARRAY['upsert'::text, 'delete'::text])))
);

CREATE SEQUENCE outbox_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE outbox_id_seq OWNED BY outbox.id;

CREATE TABLE password_resets (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    token_hash text NOT NULL,
    created_at timestamp with time zone NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used_at timestamp with time zone,
    purpose text DEFAULT 'password'::text NOT NULL,
    binding text
);

CREATE TABLE progress_events (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    item_id uuid NOT NULL,
    occurred_at timestamp with time zone NOT NULL,
    progress jsonb NOT NULL,
    client_op_id uuid,
    recorded_at timestamp with time zone DEFAULT now() NOT NULL,
    status_change jsonb,
    undoes uuid
);

CREATE TABLE reviews (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    item_id uuid NOT NULL,
    body text NOT NULL,
    contains_spoilers boolean NOT NULL,
    visibility jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);

CREATE TABLE search_docs (
    index_name text NOT NULL,
    id text NOT NULL,
    owner_id uuid,
    fields jsonb NOT NULL,
    body text NOT NULL,
    tsv tsvector NOT NULL
);

CREATE TABLE secrets_keys (
    version integer NOT NULL,
    first_seen_at timestamp with time zone NOT NULL,
    backup_confirmed_at timestamp with time zone
);

CREATE TABLE session_participants (
    session_id uuid NOT NULL,
    companion_id uuid NOT NULL,
    "position" integer NOT NULL,
    score double precision,
    winner boolean DEFAULT false NOT NULL,
    rank integer,
    team text,
    faction text,
    starting_player boolean DEFAULT false NOT NULL,
    first_play boolean DEFAULT false NOT NULL
);

CREATE TABLE session_secrets (
    id uuid NOT NULL,
    token_hash text NOT NULL,
    kind text NOT NULL,
    value bytea NOT NULL,
    created_at timestamp with time zone NOT NULL,
    CONSTRAINT session_secrets_kind_check CHECK ((kind = ANY (ARRAY['id_token'::text, 'pending_action'::text])))
);

CREATE TABLE sessions (
    id uuid NOT NULL,
    owner_id uuid NOT NULL,
    item_id uuid NOT NULL,
    started_at timestamp with time zone NOT NULL,
    duration_minutes integer,
    location text,
    notes text,
    coop_outcome text,
    incomplete boolean DEFAULT false NOT NULL,
    game_group_id uuid,
    CONSTRAINT sessions_coop_outcome_check CHECK ((coop_outcome = ANY (ARRAY['won'::text, 'lost'::text, 'none'::text])))
);

CREATE TABLE signup_invites (
    id uuid NOT NULL,
    token_hash text NOT NULL,
    email text,
    note text,
    created_by uuid,
    created_at timestamp with time zone NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used_by uuid,
    used_at timestamp with time zone,
    revoked_at timestamp with time zone
);

CREATE TABLE source_licences (
    connector_id text NOT NULL,
    agreement_on_file boolean NOT NULL,
    recorded_by uuid,
    recorded_at timestamp with time zone NOT NULL
);

CREATE TABLE source_pace (
    source text NOT NULL,
    next_free timestamp with time zone NOT NULL
);

CREATE TABLE source_stats (
    connector text NOT NULL,
    last_ok_at timestamp with time zone,
    last_error_at timestamp with time zone,
    last_error text,
    last_auth_status smallint,
    last_auth_failure_at timestamp with time zone,
    last_rate_limited_at timestamp with time zone,
    circuit_opened_at timestamp with time zone,
    circuit_open_until timestamp with time zone,
    key_changed_at timestamp with time zone
);

CREATE TABLE source_stats_hours (
    connector text NOT NULL,
    hour bigint NOT NULL,
    calls bigint DEFAULT 0 NOT NULL,
    rate_limited bigint DEFAULT 0 NOT NULL
);

CREATE TABLE source_switches (
    connector_id text NOT NULL,
    enabled boolean NOT NULL,
    changed_by uuid,
    changed_at timestamp with time zone NOT NULL
);

CREATE SEQUENCE stats_dirty_gen
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE TABLE stats_dirty (
    user_id uuid NOT NULL,
    item_id uuid NOT NULL,
    gen bigint DEFAULT nextval('stats_dirty_gen'::regclass) NOT NULL
);

CREATE TABLE stats_item_months (
    user_id uuid NOT NULL,
    item_id uuid NOT NULL,
    kind text NOT NULL,
    month text NOT NULL,
    events bigint NOT NULL,
    completions bigint NOT NULL,
    finished bigint NOT NULL,
    episodes bigint NOT NULL,
    pages bigint NOT NULL,
    plays bigint NOT NULL,
    seconds bigint NOT NULL
);

CREATE TABLE stats_months (
    user_id uuid NOT NULL,
    kind text NOT NULL,
    month text NOT NULL,
    events bigint NOT NULL,
    completions bigint NOT NULL,
    finished bigint NOT NULL,
    episodes bigint NOT NULL,
    pages bigint NOT NULL,
    plays bigint NOT NULL,
    seconds bigint NOT NULL
);

CREATE TABLE stats_users (
    user_id uuid NOT NULL,
    zone text NOT NULL,
    bucket_zone text DEFAULT 'UTC'::text NOT NULL
);

CREATE TABLE storage_ops (
    day date NOT NULL,
    puts bigint DEFAULT 0 NOT NULL,
    gets bigint DEFAULT 0 NOT NULL,
    deletes bigint DEFAULT 0 NOT NULL,
    egress_bytes bigint DEFAULT 0 NOT NULL
);

CREATE TABLE stored_objects (
    key text NOT NULL,
    class text NOT NULL,
    tier text NOT NULL,
    bytes bigint NOT NULL,
    owner_id uuid,
    asset_id uuid,
    priority text NOT NULL,
    last_accessed timestamp with time zone NOT NULL,
    CONSTRAINT stored_objects_bytes_check CHECK ((bytes >= 0)),
    CONSTRAINT stored_objects_class_check CHECK ((class = ANY (ARRAY['variant'::text, 'original'::text, 'http_cache'::text, 'upload'::text]))),
    CONSTRAINT stored_objects_priority_check CHECK ((priority = ANY (ARRAY['visible'::text, 'library'::text, 'related'::text, 'catalog'::text]))),
    CONSTRAINT stored_objects_tier_check CHECK ((tier = ANY (ARRAY['pinned'::text, 'cache'::text])))
);

CREATE TABLE totp_recovery_codes (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    code_hash text NOT NULL,
    created_at timestamp with time zone NOT NULL,
    used_at timestamp with time zone
);

CREATE TABLE usage_funnel_daily (
    day date NOT NULL,
    flow text NOT NULL,
    subject text DEFAULT ''::text NOT NULL,
    outcome text NOT NULL,
    taps_bucket text DEFAULT ''::text NOT NULL,
    ms_bucket text DEFAULT ''::text NOT NULL,
    n bigint DEFAULT 0 NOT NULL,
    CONSTRAINT usage_funnel_daily_n_check CHECK ((n >= 0))
);

CREATE TABLE user_change_seq (
    user_id uuid NOT NULL,
    last_seq bigint NOT NULL
);

CREATE TABLE user_changes (
    user_id uuid NOT NULL,
    seq bigint NOT NULL,
    entity text NOT NULL,
    entity_id uuid NOT NULL,
    op text NOT NULL,
    at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT user_changes_op_check CHECK ((op = ANY (ARRAY['upsert'::text, 'delete'::text])))
);

CREATE TABLE user_disabled (
    user_id uuid NOT NULL,
    disabled_by uuid,
    disabled_at timestamp with time zone NOT NULL
);

CREATE TABLE users (
    id uuid NOT NULL,
    username text NOT NULL,
    display_name text NOT NULL,
    email text,
    role text NOT NULL,
    self_companion uuid NOT NULL,
    created_at timestamp with time zone NOT NULL,
    share_sessions boolean DEFAULT true NOT NULL,
    locale text,
    time_zone text,
    CONSTRAINT users_role_check CHECK ((role = ANY (ARRAY['admin'::text, 'member'::text])))
);

ALTER TABLE ONLY outbox ALTER COLUMN id SET DEFAULT nextval('outbox_id_seq'::regclass);

ALTER TABLE ONLY admin_audit
    ADD CONSTRAINT admin_audit_pkey PRIMARY KEY (id);

ALTER TABLE ONLY assets
    ADD CONSTRAINT assets_hash_key UNIQUE (hash);

ALTER TABLE ONLY assets
    ADD CONSTRAINT assets_pkey PRIMARY KEY (id);

ALTER TABLE ONLY assets
    ADD CONSTRAINT assets_source_url_key UNIQUE (source_url);

ALTER TABLE ONLY auth_sessions
    ADD CONSTRAINT auth_sessions_pkey PRIMARY KEY (token_hash);

ALTER TABLE ONLY catalog_episode_seasons
    ADD CONSTRAINT catalog_episode_seasons_pkey PRIMARY KEY (item_id, season);

ALTER TABLE ONLY catalog_episodes
    ADD CONSTRAINT catalog_episodes_pkey PRIMARY KEY (item_id, season, number);

ALTER TABLE ONLY catalog_items
    ADD CONSTRAINT catalog_items_pkey PRIMARY KEY (id);

ALTER TABLE ONLY catalog_match_log
    ADD CONSTRAINT catalog_match_log_pkey PRIMARY KEY (id);

ALTER TABLE ONLY catalog_watchers
    ADD CONSTRAINT catalog_watchers_pkey PRIMARY KEY (item_id, user_id);

ALTER TABLE ONLY companions
    ADD CONSTRAINT companions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY connector_credentials
    ADD CONSTRAINT connector_credentials_pkey PRIMARY KEY (id);

ALTER TABLE ONLY credentials
    ADD CONSTRAINT credentials_pkey PRIMARY KEY (user_id);

ALTER TABLE ONLY cron_schedules
    ADD CONSTRAINT cron_schedules_pkey PRIMARY KEY (name);

ALTER TABLE ONLY entry_tags
    ADD CONSTRAINT entry_tags_pkey PRIMARY KEY (entry_id, tag_key);

ALTER TABLE ONLY external_ids
    ADD CONSTRAINT external_ids_pkey PRIMARY KEY (kind, namespace, value);

ALTER TABLE ONLY game_group_members
    ADD CONSTRAINT game_group_members_pkey PRIMARY KEY (group_id, companion_id);

ALTER TABLE ONLY game_groups
    ADD CONSTRAINT game_groups_pkey PRIMARY KEY (id);

ALTER TABLE ONLY goals
    ADD CONSTRAINT goals_pkey PRIMARY KEY (id);

ALTER TABLE ONLY goals
    ADD CONSTRAINT goals_user_id_year_kind_group_metric_key UNIQUE (user_id, year, kind_group, metric);

ALTER TABLE ONLY health_probes
    ADD CONSTRAINT health_probes_pkey PRIMARY KEY (id);

ALTER TABLE ONLY http_cache
    ADD CONSTRAINT http_cache_pkey PRIMARY KEY (key);

ALTER TABLE ONLY import_rows
    ADD CONSTRAINT import_rows_pkey PRIMARY KEY (user_id, format, source_id);

ALTER TABLE ONLY imports
    ADD CONSTRAINT imports_pkey PRIMARY KEY (id);

ALTER TABLE ONLY instance_settings
    ADD CONSTRAINT instance_settings_pkey PRIMARY KEY (id);

ALTER TABLE ONLY integrations
    ADD CONSTRAINT integrations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY integrations
    ADD CONSTRAINT integrations_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY invites
    ADD CONSTRAINT invites_pkey PRIMARY KEY (id);

ALTER TABLE ONLY invites
    ADD CONSTRAINT invites_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY jobs
    ADD CONSTRAINT jobs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY library_entries
    ADD CONSTRAINT library_entries_pkey PRIMARY KEY (id);

ALTER TABLE ONLY library_entries
    ADD CONSTRAINT library_entries_user_id_item_id_key UNIQUE (user_id, item_id);

ALTER TABLE ONLY link_requests
    ADD CONSTRAINT link_requests_pkey PRIMARY KEY (id);

ALTER TABLE ONLY list_items
    ADD CONSTRAINT list_items_pkey PRIMARY KEY (list_id, item_id);

ALTER TABLE ONLY lists
    ADD CONSTRAINT lists_pkey PRIMARY KEY (id);

ALTER TABLE ONLY notify_channels
    ADD CONSTRAINT notify_channels_pkey PRIMARY KEY (id);

ALTER TABLE ONLY notify_deliveries
    ADD CONSTRAINT notify_deliveries_channel_id_event_dedupe_key_key UNIQUE (channel_id, event, dedupe_key);

ALTER TABLE ONLY notify_deliveries
    ADD CONSTRAINT notify_deliveries_pkey PRIMARY KEY (id);

ALTER TABLE ONLY notify_inbox
    ADD CONSTRAINT notify_inbox_pkey PRIMARY KEY (id);

ALTER TABLE ONLY notify_inbox
    ADD CONSTRAINT notify_inbox_user_id_event_dedupe_key_key UNIQUE (user_id, event, dedupe_key);

ALTER TABLE ONLY notify_subscriptions
    ADD CONSTRAINT notify_subscriptions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY notify_templates
    ADD CONSTRAINT notify_templates_pkey PRIMARY KEY (locale, key);

ALTER TABLE ONLY oidc_identities
    ADD CONSTRAINT oidc_identities_id_key UNIQUE (id);

ALTER TABLE ONLY oidc_identities
    ADD CONSTRAINT oidc_identities_pkey PRIMARY KEY (issuer, subject);

ALTER TABLE ONLY oidc_providers
    ADD CONSTRAINT oidc_providers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY oidc_providers
    ADD CONSTRAINT oidc_providers_slug_key UNIQUE (slug);

ALTER TABLE ONLY oidc_used_jti
    ADD CONSTRAINT oidc_used_jti_pkey PRIMARY KEY (jti_key);

ALTER TABLE ONLY oidc_used_states
    ADD CONSTRAINT oidc_used_states_pkey PRIMARY KEY (state_hash);

ALTER TABLE ONLY outbox
    ADD CONSTRAINT outbox_pkey PRIMARY KEY (id);

ALTER TABLE ONLY password_resets
    ADD CONSTRAINT password_resets_pkey PRIMARY KEY (id);

ALTER TABLE ONLY password_resets
    ADD CONSTRAINT password_resets_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY progress_events
    ADD CONSTRAINT progress_events_pkey PRIMARY KEY (id);

ALTER TABLE ONLY progress_events
    ADD CONSTRAINT progress_events_user_id_client_op_id_key UNIQUE (user_id, client_op_id);

ALTER TABLE ONLY reviews
    ADD CONSTRAINT reviews_pkey PRIMARY KEY (id);

ALTER TABLE ONLY reviews
    ADD CONSTRAINT reviews_user_id_item_id_key UNIQUE (user_id, item_id);

ALTER TABLE ONLY search_docs
    ADD CONSTRAINT search_docs_pkey PRIMARY KEY (index_name, id);

ALTER TABLE ONLY secrets_keys
    ADD CONSTRAINT secrets_keys_pkey PRIMARY KEY (version);

ALTER TABLE ONLY session_participants
    ADD CONSTRAINT session_participants_pkey PRIMARY KEY (session_id, companion_id);

ALTER TABLE ONLY session_secrets
    ADD CONSTRAINT session_secrets_pkey PRIMARY KEY (id);

ALTER TABLE ONLY session_secrets
    ADD CONSTRAINT session_secrets_token_hash_kind_key UNIQUE (token_hash, kind);

ALTER TABLE ONLY sessions
    ADD CONSTRAINT sessions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY signup_invites
    ADD CONSTRAINT signup_invites_pkey PRIMARY KEY (id);

ALTER TABLE ONLY signup_invites
    ADD CONSTRAINT signup_invites_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY source_licences
    ADD CONSTRAINT source_licences_pkey PRIMARY KEY (connector_id);

ALTER TABLE ONLY source_pace
    ADD CONSTRAINT source_pace_pkey PRIMARY KEY (source);

ALTER TABLE ONLY source_stats_hours
    ADD CONSTRAINT source_stats_hours_pkey PRIMARY KEY (connector, hour);

ALTER TABLE ONLY source_stats
    ADD CONSTRAINT source_stats_pkey PRIMARY KEY (connector);

ALTER TABLE ONLY source_switches
    ADD CONSTRAINT source_switches_pkey PRIMARY KEY (connector_id);

ALTER TABLE ONLY stats_dirty
    ADD CONSTRAINT stats_dirty_pkey PRIMARY KEY (user_id, item_id);

ALTER TABLE ONLY stats_item_months
    ADD CONSTRAINT stats_item_months_pkey PRIMARY KEY (user_id, item_id, month);

ALTER TABLE ONLY stats_months
    ADD CONSTRAINT stats_months_pkey PRIMARY KEY (user_id, kind, month);

ALTER TABLE ONLY stats_users
    ADD CONSTRAINT stats_users_pkey PRIMARY KEY (user_id);

ALTER TABLE ONLY storage_ops
    ADD CONSTRAINT storage_ops_pkey PRIMARY KEY (day);

ALTER TABLE ONLY stored_objects
    ADD CONSTRAINT stored_objects_pkey PRIMARY KEY (key);

ALTER TABLE ONLY totp_recovery_codes
    ADD CONSTRAINT totp_recovery_codes_pkey PRIMARY KEY (id);

ALTER TABLE ONLY usage_funnel_daily
    ADD CONSTRAINT usage_funnel_daily_pkey PRIMARY KEY (day, flow, subject, outcome, taps_bucket, ms_bucket);

ALTER TABLE ONLY user_change_seq
    ADD CONSTRAINT user_change_seq_pkey PRIMARY KEY (user_id);

ALTER TABLE ONLY user_changes
    ADD CONSTRAINT user_changes_pkey PRIMARY KEY (user_id, seq);

ALTER TABLE ONLY user_disabled
    ADD CONSTRAINT user_disabled_pkey PRIMARY KEY (user_id);

ALTER TABLE ONLY users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

ALTER TABLE ONLY users
    ADD CONSTRAINT users_username_key UNIQUE (username);

CREATE INDEX admin_audit_action ON admin_audit USING btree (action, at DESC);

CREATE INDEX admin_audit_actor ON admin_audit USING btree (actor_id, at DESC);

CREATE INDEX admin_audit_at ON admin_audit USING btree (at DESC, id DESC);

CREATE INDEX auth_sessions_expiry ON auth_sessions USING btree (expires_at);

CREATE INDEX auth_sessions_oidc_sid ON auth_sessions USING btree (provider_id, oidc_sid) WHERE (oidc_sid IS NOT NULL);

CREATE INDEX auth_sessions_user ON auth_sessions USING btree (user_id);

CREATE INDEX catalog_items_kind ON catalog_items USING btree (kind);

CREATE INDEX catalog_items_kind_title ON catalog_items USING btree (kind, lower(btrim(title)));

CREATE INDEX catalog_items_provider_data_at ON catalog_items USING btree (provider_data_at) WHERE (provider_data_at IS NOT NULL);

CREATE INDEX catalog_items_provider_refetch_after ON catalog_items USING btree (provider_refetch_after) WHERE (provider_refetch_after IS NOT NULL);

CREATE INDEX catalog_watchers_user ON catalog_watchers USING btree (user_id, until);

CREATE INDEX companions_linked ON companions USING btree (linked_user) WHERE (linked_user IS NOT NULL);

CREATE UNIQUE INDEX companions_one_self ON companions USING btree (owner_id) WHERE is_self;

CREATE INDEX companions_owner ON companions USING btree (owner_id);

CREATE UNIQUE INDEX connector_credentials_instance ON connector_credentials USING btree (connector_id) WHERE (scope = 'instance'::text);

CREATE UNIQUE INDEX connector_credentials_user ON connector_credentials USING btree (connector_id, user_id) WHERE (scope = 'user'::text);

CREATE INDEX entry_tags_user ON entry_tags USING btree (user_id, tag_key);

CREATE INDEX external_ids_item ON external_ids USING btree (item_id);

CREATE INDEX game_group_members_companion ON game_group_members USING btree (companion_id);

CREATE INDEX game_groups_owner ON game_groups USING btree (owner_id);

CREATE INDEX http_cache_fresh ON http_cache USING btree (fresh_until);

CREATE INDEX imports_user ON imports USING btree (user_id);

CREATE INDEX imports_user_newest ON imports USING btree (user_id, id DESC);

CREATE INDEX integrations_user ON integrations USING btree (user_id);

CREATE UNIQUE INDEX jobs_dedupe ON jobs USING btree (dedupe_key) WHERE (state = ANY (ARRAY['queued'::text, 'running'::text]));

CREATE INDEX jobs_ready ON jobs USING btree (lane, priority DESC, run_at) WHERE (state = 'queued'::text);

CREATE INDEX jobs_running ON jobs USING btree (locked_until) WHERE (state = 'running'::text);

CREATE INDEX library_entries_completed ON library_entries USING btree (user_id, completed_at) WHERE ((completed_at IS NOT NULL) OR completed_undated);

CREATE INDEX library_entries_item ON library_entries USING btree (item_id);

CREATE INDEX library_entries_kind_status ON library_entries USING btree (user_id, kind, status);

CREATE INDEX library_entries_recent ON library_entries USING btree (user_id, added_at DESC, id DESC);

CREATE INDEX link_requests_from ON link_requests USING btree (from_user) WHERE (state = 'pending'::text);

CREATE INDEX link_requests_to ON link_requests USING btree (to_user) WHERE (state = 'pending'::text);

CREATE INDEX list_items_item ON list_items USING btree (item_id);

CREATE INDEX lists_instance ON lists USING btree (updated_at DESC) WHERE ((visibility ->> 'type'::text) = 'instance'::text);

CREATE INDEX lists_owner ON lists USING btree (owner_id);

CREATE UNIQUE INDEX lists_share_token ON lists USING btree (((visibility ->> 'token'::text))) WHERE ((visibility ->> 'type'::text) = 'link'::text);

CREATE INDEX notify_channels_user ON notify_channels USING btree (user_id);

CREATE INDEX notify_deliveries_digest ON notify_deliveries USING btree (channel_id, state, due_at);

CREATE INDEX notify_deliveries_recent ON notify_deliveries USING btree (user_id, created_at DESC);

CREATE INDEX notify_inbox_recent ON notify_inbox USING btree (user_id, created_at DESC);

CREATE INDEX notify_subscriptions_user ON notify_subscriptions USING btree (user_id);

CREATE INDEX oidc_identities_user ON oidc_identities USING btree (user_id);

CREATE UNIQUE INDEX oidc_providers_one_default ON oidc_providers USING btree (is_default) WHERE is_default;

CREATE INDEX oidc_used_jti_expiry ON oidc_used_jti USING btree (expires_at);

CREATE INDEX oidc_used_states_expiry ON oidc_used_states USING btree (expires_at);

CREATE INDEX outbox_pending ON outbox USING btree (id) WHERE (processed_at IS NULL);

CREATE INDEX password_resets_user ON password_resets USING btree (user_id);

CREATE INDEX progress_events_item ON progress_events USING btree (item_id);

CREATE UNIQUE INDEX progress_events_undoes ON progress_events USING btree (undoes) WHERE (undoes IS NOT NULL);

CREATE INDEX progress_events_user_item ON progress_events USING btree (user_id, item_id, occurred_at);

CREATE INDEX progress_events_user_recent ON progress_events USING btree (user_id, occurred_at DESC, id DESC);

CREATE INDEX reviews_item ON reviews USING btree (item_id);

CREATE INDEX search_docs_owner ON search_docs USING btree (index_name, owner_id);

CREATE INDEX search_docs_trgm ON search_docs USING gin (body gin_trgm_ops);

CREATE INDEX search_docs_tsv ON search_docs USING gin (tsv);

CREATE INDEX session_participants_companion ON session_participants USING btree (companion_id);

CREATE INDEX sessions_item ON sessions USING btree (item_id);

CREATE INDEX sessions_owner_recent ON sessions USING btree (owner_id, started_at DESC, id DESC);

CREATE INDEX stored_objects_asset ON stored_objects USING btree (asset_id) WHERE (asset_id IS NOT NULL);

CREATE INDEX stored_objects_cache_lru ON stored_objects USING btree (last_accessed) WHERE (tier = 'cache'::text);

CREATE INDEX stored_objects_uploads ON stored_objects USING btree (owner_id) WHERE (class = 'upload'::text);

CREATE INDEX totp_recovery_codes_user ON totp_recovery_codes USING btree (user_id);

CREATE TRIGGER library_entries_init_activity BEFORE INSERT ON library_entries FOR EACH ROW EXECUTE FUNCTION library_entries_init_activity();

CREATE TRIGGER library_entry_completion BEFORE UPDATE ON library_entries FOR EACH ROW EXECUTE FUNCTION library_entry_completion();

CREATE TRIGGER progress_events_touch_entry AFTER INSERT ON progress_events FOR EACH ROW EXECUTE FUNCTION progress_events_touch_entry();

CREATE TRIGGER stats_entry_changed AFTER INSERT OR DELETE OR UPDATE ON library_entries FOR EACH ROW EXECUTE FUNCTION stats_entry_changed();

CREATE TRIGGER stats_events_deleted AFTER DELETE ON progress_events REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION stats_events_deleted();

CREATE TRIGGER stats_events_inserted AFTER INSERT ON progress_events REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION stats_events_inserted();

CREATE TRIGGER stats_events_updated AFTER UPDATE ON progress_events REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION stats_events_changed();

CREATE TRIGGER stats_item_changed AFTER UPDATE ON catalog_items FOR EACH ROW WHEN (((old.kind IS DISTINCT FROM new.kind) OR ((old.attributes -> 'runtime_minutes'::text) IS DISTINCT FROM (new.attributes -> 'runtime_minutes'::text)) OR ((old.attributes -> 'episode_runtime_minutes'::text) IS DISTINCT FROM (new.attributes -> 'episode_runtime_minutes'::text)))) EXECUTE FUNCTION stats_item_changed();

CREATE TRIGGER stats_sessions_deleted AFTER DELETE ON sessions FOR EACH ROW EXECUTE FUNCTION stats_sessions_changed();

CREATE TRIGGER stats_sessions_inserted AFTER INSERT ON sessions FOR EACH ROW EXECUTE FUNCTION stats_sessions_changed();

CREATE TRIGGER stats_sessions_updated AFTER UPDATE ON sessions FOR EACH ROW WHEN (((old.duration_minutes IS DISTINCT FROM new.duration_minutes) OR (old.item_id IS DISTINCT FROM new.item_id) OR (old.owner_id IS DISTINCT FROM new.owner_id))) EXECUTE FUNCTION stats_sessions_changed();

CREATE TRIGGER stats_users_deleted AFTER DELETE ON stats_users FOR EACH ROW EXECUTE FUNCTION stats_users_deleted();

ALTER TABLE ONLY admin_audit
    ADD CONSTRAINT admin_audit_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY assets
    ADD CONSTRAINT assets_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY auth_sessions
    ADD CONSTRAINT auth_sessions_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES oidc_providers(id) ON DELETE SET NULL;

ALTER TABLE ONLY auth_sessions
    ADD CONSTRAINT auth_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY catalog_episode_seasons
    ADD CONSTRAINT catalog_episode_seasons_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id) ON DELETE CASCADE;

ALTER TABLE ONLY catalog_episodes
    ADD CONSTRAINT catalog_episodes_item_id_season_fkey FOREIGN KEY (item_id, season) REFERENCES catalog_episode_seasons(item_id, season) ON DELETE CASCADE;

ALTER TABLE ONLY catalog_watchers
    ADD CONSTRAINT catalog_watchers_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id) ON DELETE CASCADE;

ALTER TABLE ONLY catalog_watchers
    ADD CONSTRAINT catalog_watchers_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY companions
    ADD CONSTRAINT companions_linked_user_fkey FOREIGN KEY (linked_user) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY companions
    ADD CONSTRAINT companions_merged_into_fkey FOREIGN KEY (merged_into) REFERENCES companions(id) ON DELETE SET NULL;

ALTER TABLE ONLY companions
    ADD CONSTRAINT companions_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY connector_credentials
    ADD CONSTRAINT connector_credentials_created_by_fkey FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY connector_credentials
    ADD CONSTRAINT connector_credentials_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY credentials
    ADD CONSTRAINT credentials_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY entry_tags
    ADD CONSTRAINT entry_tags_entry_id_fkey FOREIGN KEY (entry_id) REFERENCES library_entries(id) ON DELETE CASCADE;

ALTER TABLE ONLY entry_tags
    ADD CONSTRAINT entry_tags_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY external_ids
    ADD CONSTRAINT external_ids_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id) ON DELETE CASCADE;

ALTER TABLE ONLY game_group_members
    ADD CONSTRAINT game_group_members_companion_id_fkey FOREIGN KEY (companion_id) REFERENCES companions(id) ON DELETE CASCADE;

ALTER TABLE ONLY game_group_members
    ADD CONSTRAINT game_group_members_group_id_fkey FOREIGN KEY (group_id) REFERENCES game_groups(id) ON DELETE CASCADE;

ALTER TABLE ONLY game_groups
    ADD CONSTRAINT game_groups_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY goals
    ADD CONSTRAINT goals_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY import_rows
    ADD CONSTRAINT import_rows_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id) ON DELETE CASCADE;

ALTER TABLE ONLY import_rows
    ADD CONSTRAINT import_rows_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY imports
    ADD CONSTRAINT imports_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY instance_settings
    ADD CONSTRAINT instance_settings_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY integrations
    ADD CONSTRAINT integrations_last_item_id_fkey FOREIGN KEY (last_item_id) REFERENCES catalog_items(id) ON DELETE SET NULL;

ALTER TABLE ONLY integrations
    ADD CONSTRAINT integrations_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY invites
    ADD CONSTRAINT invites_companion_id_fkey FOREIGN KEY (companion_id) REFERENCES companions(id) ON DELETE CASCADE;

ALTER TABLE ONLY invites
    ADD CONSTRAINT invites_invited_by_fkey FOREIGN KEY (invited_by) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY library_entries
    ADD CONSTRAINT library_entries_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id);

ALTER TABLE ONLY library_entries
    ADD CONSTRAINT library_entries_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY link_requests
    ADD CONSTRAINT link_requests_companion_id_fkey FOREIGN KEY (companion_id) REFERENCES companions(id) ON DELETE CASCADE;

ALTER TABLE ONLY link_requests
    ADD CONSTRAINT link_requests_from_user_fkey FOREIGN KEY (from_user) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY link_requests
    ADD CONSTRAINT link_requests_to_user_fkey FOREIGN KEY (to_user) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY list_items
    ADD CONSTRAINT list_items_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id);

ALTER TABLE ONLY list_items
    ADD CONSTRAINT list_items_list_id_fkey FOREIGN KEY (list_id) REFERENCES lists(id) ON DELETE CASCADE;

ALTER TABLE ONLY lists
    ADD CONSTRAINT lists_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY notify_channels
    ADD CONSTRAINT notify_channels_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY notify_deliveries
    ADD CONSTRAINT notify_deliveries_channel_id_fkey FOREIGN KEY (channel_id) REFERENCES notify_channels(id) ON DELETE CASCADE;

ALTER TABLE ONLY notify_deliveries
    ADD CONSTRAINT notify_deliveries_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY notify_inbox
    ADD CONSTRAINT notify_inbox_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY notify_subscriptions
    ADD CONSTRAINT notify_subscriptions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY oidc_identities
    ADD CONSTRAINT oidc_identities_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES oidc_providers(id) ON DELETE SET NULL;

ALTER TABLE ONLY oidc_identities
    ADD CONSTRAINT oidc_identities_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY oidc_providers
    ADD CONSTRAINT oidc_providers_created_by_fkey FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY oidc_providers
    ADD CONSTRAINT oidc_providers_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY password_resets
    ADD CONSTRAINT password_resets_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY progress_events
    ADD CONSTRAINT progress_events_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id);

ALTER TABLE ONLY progress_events
    ADD CONSTRAINT progress_events_undoes_fkey FOREIGN KEY (undoes) REFERENCES progress_events(id);

ALTER TABLE ONLY progress_events
    ADD CONSTRAINT progress_events_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY reviews
    ADD CONSTRAINT reviews_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id);

ALTER TABLE ONLY reviews
    ADD CONSTRAINT reviews_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY session_participants
    ADD CONSTRAINT session_participants_companion_id_fkey FOREIGN KEY (companion_id) REFERENCES companions(id);

ALTER TABLE ONLY session_participants
    ADD CONSTRAINT session_participants_session_id_fkey FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE;

ALTER TABLE ONLY session_secrets
    ADD CONSTRAINT session_secrets_token_hash_fkey FOREIGN KEY (token_hash) REFERENCES auth_sessions(token_hash) ON DELETE CASCADE;

ALTER TABLE ONLY sessions
    ADD CONSTRAINT sessions_game_group_id_fkey FOREIGN KEY (game_group_id) REFERENCES game_groups(id) ON DELETE SET NULL;

ALTER TABLE ONLY sessions
    ADD CONSTRAINT sessions_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog_items(id);

ALTER TABLE ONLY sessions
    ADD CONSTRAINT sessions_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY signup_invites
    ADD CONSTRAINT signup_invites_created_by_fkey FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY signup_invites
    ADD CONSTRAINT signup_invites_used_by_fkey FOREIGN KEY (used_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY source_licences
    ADD CONSTRAINT source_licences_recorded_by_fkey FOREIGN KEY (recorded_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY source_switches
    ADD CONSTRAINT source_switches_changed_by_fkey FOREIGN KEY (changed_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY stats_dirty
    ADD CONSTRAINT stats_dirty_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY stats_months
    ADD CONSTRAINT stats_months_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY stats_users
    ADD CONSTRAINT stats_users_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY totp_recovery_codes
    ADD CONSTRAINT totp_recovery_codes_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY user_disabled
    ADD CONSTRAINT user_disabled_disabled_by_fkey FOREIGN KEY (disabled_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE ONLY user_disabled
    ADD CONSTRAINT user_disabled_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE ONLY users
    ADD CONSTRAINT users_self_companion_fk FOREIGN KEY (self_companion) REFERENCES companions(id) DEFERRABLE INITIALLY DEFERRED;


-- the one row of the instance settings (defaults everywhere)
INSERT INTO instance_settings (id) VALUES (true);
