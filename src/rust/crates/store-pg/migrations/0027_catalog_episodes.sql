-- WP4-7: per-episode release dates of shows, from the episodic connectors (TMDB TV Seasons >
-- Details, TheTVDB series episodes). Only the episode number, the air date and the runtime;
-- no titles or overviews. A season row records what it was fetched for (episode count and
-- air date of the season list), so a refresh skips seasons that did not change.
CREATE TABLE catalog_episode_seasons (
    item_id        uuid NOT NULL REFERENCES catalog_items (id) ON DELETE CASCADE,
    season         integer NOT NULL CHECK (season > 0),
    connector      text NOT NULL,
    episode_count  integer NOT NULL CHECK (episode_count >= 0),
    air_date       date,
    fetched_at     timestamptz NOT NULL,
    PRIMARY KEY (item_id, season)
);

CREATE TABLE catalog_episodes (
    item_id          uuid NOT NULL,
    season           integer NOT NULL,
    number           integer NOT NULL CHECK (number > 0),
    air_date         date,
    runtime_minutes  integer CHECK (runtime_minutes > 0),
    PRIMARY KEY (item_id, season, number),
    FOREIGN KEY (item_id, season)
        REFERENCES catalog_episode_seasons (item_id, season) ON DELETE CASCADE
);
