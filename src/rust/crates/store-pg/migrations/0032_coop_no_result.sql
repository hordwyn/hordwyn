-- OT-18: a co-op play can end without a result ("Played together"); `none` is the third
-- stored outcome. Additive: rows with `won`, `lost` or no outcome (competitive) stay as
-- they are. The check of 0001 had no name of its own, so Postgres named it after the column.
ALTER TABLE sessions DROP CONSTRAINT sessions_coop_outcome_check;
ALTER TABLE sessions ADD CONSTRAINT sessions_coop_outcome_check
    CHECK (coop_outcome IN ('won', 'lost', 'none'));
