-- UX4-1: a dirty mark could be lost. The marking triggers inserted with ON CONFLICT DO
-- NOTHING, which takes no row lock: a writer whose trigger found a committed mark and that
-- committed after a reader's `DELETE ... RETURNING` and recompute left no mark behind, and the
-- statistics stayed stale. Now every mark carries a generation from a sequence and a write
-- that finds a mark moves it to a new generation (and so holds the row lock until it commits).
-- A refresh reads the marks without taking them, recomputes, and at the end deletes only the
-- marks that still have the generation it read and that no open writer holds (SKIP LOCKED):
-- a mark a writer touched during the refresh survives for the next read, and the reader never
-- waits for a writer (nor a writer for the recompute).

CREATE SEQUENCE stats_dirty_gen;
ALTER TABLE stats_dirty ADD COLUMN gen bigint NOT NULL DEFAULT nextval('stats_dirty_gen');

CREATE OR REPLACE FUNCTION stats_events_inserted() RETURNS trigger
LANGUAGE plpgsql AS $$
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

CREATE OR REPLACE FUNCTION stats_events_changed() RETURNS trigger
LANGUAGE plpgsql AS $$
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

CREATE OR REPLACE FUNCTION stats_events_deleted() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT o.user_id, o.item_id FROM old_rows o
    WHERE EXISTS (SELECT 1 FROM users u WHERE u.id = o.user_id)
    ORDER BY 1, 2
    ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    RETURN NULL;
END
$$;

CREATE OR REPLACE FUNCTION stats_sessions_changed() RETURNS trigger
LANGUAGE plpgsql AS $$
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

CREATE OR REPLACE FUNCTION stats_item_changed() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO stats_dirty (user_id, item_id)
    SELECT DISTINCT x.user_id, NEW.id FROM progress_events x WHERE x.item_id = NEW.id
    ORDER BY 1, 2
    ON CONFLICT (user_id, item_id) DO UPDATE SET gen = EXCLUDED.gen;
    RETURN NULL;
END
$$;

-- Note (UX4-21): the amounts start from `progress_events`, so a session without a `session`
-- progress event is not counted; `month_amounts_full` and `app::month_amounts` do the same.
