-- UX1-1: keep the Library query and Home bounded at 50 000 entries.

-- Home's "next in series" and the counts read the user's entries by kind and status.
CREATE INDEX library_entries_kind_status ON library_entries (user_id, kind, status);

-- The latest effective progress event and the number of effective session events of the
-- entry's item, maintained by the triggers below, so the library query sorts without a
-- per-entry aggregate over `progress_events`. NULL / 0 while there is no effective event.
ALTER TABLE library_entries ADD COLUMN last_event_at timestamptz;
ALTER TABLE library_entries ADD COLUMN plays integer NOT NULL DEFAULT 0;

-- Effective events: not an undo and not undone (see `domain::progress::effective`).
CREATE FUNCTION library_activity(p_user uuid, p_item uuid, OUT last_at timestamptz,
                                 OUT plays integer)
LANGUAGE sql STABLE AS $$
    SELECT max(x.occurred_at),
           (COUNT(*) FILTER (WHERE x.progress ->> 'type' = 'session'))::integer
    FROM progress_events x
    WHERE x.user_id = p_user AND x.item_id = p_item AND x.undoes IS NULL
      AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = x.id)
$$;

-- Every appended event (including an undo) recomputes the entry of its item.
CREATE FUNCTION progress_events_touch_entry() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    UPDATE library_entries e SET (last_event_at, plays) =
        (SELECT a.last_at, a.plays FROM library_activity(NEW.user_id, NEW.item_id) a)
    WHERE e.user_id = NEW.user_id AND e.item_id = NEW.item_id;
    RETURN NULL;
END
$$;
CREATE TRIGGER progress_events_touch_entry AFTER INSERT ON progress_events
    FOR EACH ROW EXECUTE FUNCTION progress_events_touch_entry();

-- A new entry picks up events logged before it existed.
CREATE FUNCTION library_entries_init_activity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    SELECT a.last_at, a.plays INTO NEW.last_event_at, NEW.plays
    FROM library_activity(NEW.user_id, NEW.item_id) a;
    RETURN NEW;
END
$$;
CREATE TRIGGER library_entries_init_activity BEFORE INSERT ON library_entries
    FOR EACH ROW EXECUTE FUNCTION library_entries_init_activity();

UPDATE library_entries e SET (last_event_at, plays) =
    (SELECT a.last_at, a.plays FROM library_activity(e.user_id, e.item_id) a);

-- A JSON whole number >= 0 as `serde_json::Value::as_u64` reads it (`170`, not `170.0`
-- or `90.5`), else NULL. The statistics read runtimes with it.
CREATE FUNCTION jsonb_u64(j jsonb) RETURNS bigint
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN jsonb_typeof(j) = 'number' AND (j #>> '{}') ~ '^[0-9]{1,18}$'
                THEN (j #>> '{}')::bigint END
$$;
