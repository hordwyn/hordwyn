-- Job queue (D-3 jobs, D-6.4 lanes). Lanes: 0 interactive, 1 user_import,
-- 2 background_refresh, 3 prefetch. Lower lane number wins, then higher priority.

CREATE TABLE jobs (
    id            uuid PRIMARY KEY,
    lane          smallint NOT NULL CHECK (lane BETWEEN 0 AND 3),
    kind          text NOT NULL,
    payload       jsonb NOT NULL DEFAULT 'null'::jsonb,
    dedupe_key    text,
    priority      smallint NOT NULL DEFAULT 0,
    state         text NOT NULL DEFAULT 'queued'
                  CHECK (state IN ('queued', 'running', 'done', 'dead')),
    run_at        timestamptz NOT NULL DEFAULT now(),
    attempts      integer NOT NULL DEFAULT 0,
    max_attempts  integer NOT NULL DEFAULT 5,
    locked_until  timestamptz,
    last_error    text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX jobs_dedupe ON jobs (dedupe_key) WHERE state IN ('queued', 'running');
CREATE INDEX jobs_ready ON jobs (lane, priority DESC, run_at) WHERE state = 'queued';
CREATE INDEX jobs_running ON jobs (locked_until) WHERE state = 'running';

CREATE TABLE cron_schedules (
    name         text PRIMARY KEY,
    schedule     text NOT NULL,
    lane         smallint NOT NULL CHECK (lane BETWEEN 0 AND 3),
    kind         text NOT NULL,
    payload      jsonb NOT NULL DEFAULT 'null'::jsonb,
    next_run_at  timestamptz NOT NULL
);
