-- v1-books (LB-13, LB-14, LB-15): an entry can be finished without an event that completed
-- it: a book is always completed by a status, and an import brings a status without an
-- event. The entry itself says so. `completed_at` is the day of that completion (the newest
-- step that was logged when the status was written); `completed_undated` marks one without
-- any logged step, which counts in the all-time totals and in no month, year or goal. The
-- application writes both (it knows whether a person, an import or an event moved the
-- status; the start of a rewatch run keeps the completion although the status leaves
-- Completed). The statistics statements read them instead of the status.
ALTER TABLE library_entries
    ADD COLUMN completed_at timestamptz,
    ADD COLUMN completed_undated boolean NOT NULL DEFAULT false,
    ADD CONSTRAINT library_entries_completion
        CHECK (NOT (completed_undated AND completed_at IS NOT NULL));

-- The entries that are Completed now, by the same rule: none of their effective events
-- completed them (a stop marker completes nothing), so they are finished by their status, on
-- their newest effective event or undated.
UPDATE library_entries e
SET completed_at = n.newest, completed_undated = n.newest IS NULL
FROM (
    SELECT e2.id,
           (SELECT max(x.occurred_at) FROM progress_events x
            WHERE x.user_id = e2.user_id AND x.item_id = e2.item_id AND x.undoes IS NULL
              AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = x.id)) AS newest
    FROM library_entries e2
    WHERE e2.status = 'completed'
      AND NOT EXISTS (
          SELECT 1 FROM progress_events y
          WHERE y.user_id = e2.user_id AND y.item_id = e2.item_id AND y.undoes IS NULL
            AND y.status_change ->> 'to' = 'completed'
            AND y.progress ->> 'type' <> 'rewatch_stop'
            AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = y.id))
) n
WHERE n.id = e.id;

-- The goal window and the statistics read the completions of one user.
CREATE INDEX library_entries_completed ON library_entries (user_id, completed_at)
    WHERE completed_at IS NOT NULL OR completed_undated;

-- The day of a completion never moves while the entry stays Completed, whatever writes the
-- row again (a second import, a bulk change, the status select, a stale client): the row
-- keeps the day it has. An undated completion may still get its day.
CREATE FUNCTION library_entry_completion() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status = 'completed' AND NEW.status = 'completed'
       AND OLD.completed_at IS NOT NULL THEN
        NEW.completed_at := OLD.completed_at;
        NEW.completed_undated := false;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER library_entry_completion BEFORE UPDATE ON library_entries
    FOR EACH ROW EXECUTE FUNCTION library_entry_completion();

-- An entry whose completion appears, goes, changes or is deleted marks its (user, item) pair,
-- so the per-item statistics cache follows a status set by hand. A write that changes
-- nothing about the completion marks nothing.
CREATE FUNCTION stats_entry_changed() RETURNS trigger
LANGUAGE plpgsql AS $$
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
CREATE TRIGGER stats_entry_changed AFTER INSERT OR UPDATE OR DELETE ON library_entries
    FOR EACH ROW EXECUTE FUNCTION stats_entry_changed();

-- An item whose kind or attributes change marks everybody who has events of it, and now
-- also everybody whose only trace of it is such a completion (review BOOK-20).
CREATE OR REPLACE FUNCTION stats_item_changed() RETURNS trigger
LANGUAGE plpgsql AS $$
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

-- The caches built before this migration hold none of these completions: every user's next
-- read rebuilds in full (the mechanism a changed time zone uses; review BOOK-4).
UPDATE stats_users SET zone = '';
