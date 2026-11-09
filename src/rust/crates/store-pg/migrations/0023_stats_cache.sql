-- WP4-1 (DEC-235): incremental statistics. The amounts per user, item and month of the
-- user's zone are kept in `stats_item_months` and summed per kind and month in
-- `stats_months` (what a read returns); every write that can change them marks the
-- (user, item) pair in `stats_dirty`, and a statistics read recomputes only the marked items
-- with the full statement restricted to them (`ProgressRepo::month_amounts`). The zone the
-- cache was built in is stamped per user in `stats_users`; another zone rebuilds it all.

-- No foreign key on the user: a rebuild writes tens of thousands of rows, and a per-row
-- check doubled its time; the rows go with the user's `stats_users` row (trigger below).
CREATE TABLE stats_item_months (
    user_id      uuid NOT NULL,
    item_id      uuid NOT NULL,
    kind         text NOT NULL,
    -- `2026-03` in the user's zone
    month        text NOT NULL,
    events       bigint NOT NULL,
    completions  bigint NOT NULL,
    finished     bigint NOT NULL,
    episodes     bigint NOT NULL,
    pages        bigint NOT NULL,
    plays        bigint NOT NULL,
    seconds      bigint NOT NULL,
    PRIMARY KEY (user_id, item_id, month)
);

CREATE TABLE stats_months (
    user_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind         text NOT NULL,
    month        text NOT NULL,
    events       bigint NOT NULL,
    completions  bigint NOT NULL,
    finished     bigint NOT NULL,
    episodes     bigint NOT NULL,
    pages        bigint NOT NULL,
    plays        bigint NOT NULL,
    seconds      bigint NOT NULL,
    PRIMARY KEY (user_id, kind, month)
);

CREATE TABLE stats_dirty (
    user_id  uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    item_id  uuid NOT NULL,
    PRIMARY KEY (user_id, item_id)
);

-- The zone name the cached months of the user were built in ('' before the first build),
-- and the zone Postgres buckets in for it (UTC when its tzdata lacks the name, UX3-20;
-- checking that reads the tz directory, about 50 ms, so it is done once per zone).
CREATE TABLE stats_users (
    user_id      uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    zone         text NOT NULL,
    bucket_zone  text NOT NULL DEFAULT 'UTC'
);

CREATE FUNCTION stats_users_deleted() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM stats_item_months WHERE user_id = OLD.user_id;
    RETURN NULL;
END
$$;
CREATE TRIGGER stats_users_deleted AFTER DELETE ON stats_users
    FOR EACH ROW EXECUTE FUNCTION stats_users_deleted();

-- A runtime change marks every user with events on the item.
CREATE INDEX progress_events_item ON progress_events (item_id);

-- Appended, changed or removed events (an undo marks the undone event's item as well).
-- Statement-level with transition tables, so a bulk insert marks each pair once.
CREATE FUNCTION stats_events_inserted() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT p.user_id, p.item_id FROM (
        SELECT n.user_id, n.item_id FROM new_rows n
        UNION ALL
        SELECT n.user_id, t.item_id FROM new_rows n JOIN progress_events t ON t.id = n.undoes
    ) p
    ORDER BY 1, 2
    ON CONFLICT DO NOTHING;
    RETURN NULL;
END
$$;
CREATE TRIGGER stats_events_inserted AFTER INSERT ON progress_events
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stats_events_inserted();

CREATE FUNCTION stats_events_changed() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT p.user_id, p.item_id FROM (
        SELECT o.user_id, o.item_id FROM old_rows o
        UNION ALL
        SELECT n.user_id, n.item_id FROM new_rows n
    ) p
    ORDER BY 1, 2
    ON CONFLICT DO NOTHING;
    RETURN NULL;
END
$$;
CREATE TRIGGER stats_events_updated AFTER UPDATE ON progress_events
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stats_events_changed();

CREATE FUNCTION stats_events_deleted() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT o.user_id, o.item_id FROM old_rows o
    WHERE EXISTS (SELECT 1 FROM users u WHERE u.id = o.user_id)
    ORDER BY 1, 2
    ON CONFLICT DO NOTHING;
    RETURN NULL;
END
$$;
CREATE TRIGGER stats_events_deleted AFTER DELETE ON progress_events
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stats_events_deleted();

-- A session's duration counts for its owner's session events on its item.
CREATE FUNCTION stats_sessions_changed() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP IN ('UPDATE', 'DELETE') AND EXISTS (SELECT 1 FROM users u WHERE u.id = OLD.owner_id) THEN
        INSERT INTO stats_dirty (user_id, item_id) VALUES (OLD.owner_id, OLD.item_id)
        ON CONFLICT DO NOTHING;
    END IF;
    IF TG_OP IN ('INSERT', 'UPDATE') THEN
        INSERT INTO stats_dirty (user_id, item_id) VALUES (NEW.owner_id, NEW.item_id)
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN NULL;
END
$$;
CREATE TRIGGER stats_sessions_inserted AFTER INSERT ON sessions
    FOR EACH ROW EXECUTE FUNCTION stats_sessions_changed();
CREATE TRIGGER stats_sessions_updated AFTER UPDATE ON sessions
    FOR EACH ROW
    WHEN (OLD.duration_minutes IS DISTINCT FROM NEW.duration_minutes
          OR OLD.item_id IS DISTINCT FROM NEW.item_id
          OR OLD.owner_id IS DISTINCT FROM NEW.owner_id)
    EXECUTE FUNCTION stats_sessions_changed();
CREATE TRIGGER stats_sessions_deleted AFTER DELETE ON sessions
    FOR EACH ROW EXECUTE FUNCTION stats_sessions_changed();

-- Runtimes and the kind come from the catalog item: a change marks everyone who logged it.
CREATE FUNCTION stats_item_changed() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT x.user_id, NEW.id FROM progress_events x WHERE x.item_id = NEW.id
    ORDER BY 1, 2
    ON CONFLICT DO NOTHING;
    RETURN NULL;
END
$$;
CREATE TRIGGER stats_item_changed AFTER UPDATE ON catalog_items
    FOR EACH ROW
    WHEN (OLD.kind IS DISTINCT FROM NEW.kind
          OR OLD.attributes -> 'runtime_minutes' IS DISTINCT FROM NEW.attributes -> 'runtime_minutes'
          OR OLD.attributes -> 'episode_runtime_minutes'
             IS DISTINCT FROM NEW.attributes -> 'episode_runtime_minutes')
    EXECUTE FUNCTION stats_item_changed();

-- The activity feed reads the owner's sessions in windows by (started_at, id) (DEC-235).
CREATE INDEX sessions_owner_recent ON sessions (owner_id, started_at DESC, id DESC);
DROP INDEX sessions_owner;
