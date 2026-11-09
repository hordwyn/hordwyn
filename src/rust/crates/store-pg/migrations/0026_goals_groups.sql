-- WP4-6 (design.md 6.7, 9; research R-04): opt-in yearly goals, private to their owner, one per
-- year, group ('all' or a group id) and measure; game groups of the owner's people, with the
-- group a play was logged as on the session (kept when the group is removed).
CREATE TABLE goals (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    year        integer NOT NULL CHECK (year BETWEEN 2000 AND 2100),
    kind_group  text NOT NULL,
    metric      text NOT NULL
                CHECK (metric IN ('completions', 'finished', 'episodes', 'pages', 'plays', 'minutes')),
    target      integer NOT NULL CHECK (target > 0),
    created_at  timestamptz NOT NULL,
    UNIQUE (user_id, year, kind_group, metric)
);

CREATE TABLE game_groups (
    id          uuid PRIMARY KEY,
    owner_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name        text NOT NULL,
    removed_at  timestamptz,
    created_at  timestamptz NOT NULL
);
CREATE INDEX game_groups_owner ON game_groups (owner_id);

-- a person deleted for good (no history) leaves the groups; merged and removed people stay
-- as rows and are resolved when a group is read
CREATE TABLE game_group_members (
    group_id      uuid NOT NULL REFERENCES game_groups (id) ON DELETE CASCADE,
    companion_id  uuid NOT NULL REFERENCES companions (id) ON DELETE CASCADE,
    position      integer NOT NULL,
    PRIMARY KEY (group_id, companion_id)
);
CREATE INDEX game_group_members_companion ON game_group_members (companion_id);

ALTER TABLE sessions ADD COLUMN game_group_id uuid REFERENCES game_groups (id) ON DELETE SET NULL;
