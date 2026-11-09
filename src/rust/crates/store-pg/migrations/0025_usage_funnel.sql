-- WP4-5 logging funnel (design.md 9, research R-54, addendum-1 D16): daily totals of the
-- browser's beacons. No user and no item column on purpose: a row says how often a flow ended
-- with an outcome in a taps and a time bucket on a day (UTC), nothing about who or what.
-- `subject` is the `page/section` of a disclosure and empty for the other flows. Rows older
-- than 180 days are deleted by the daily job `usage.funnel_daily`.
CREATE TABLE usage_funnel_daily (
    day         date NOT NULL,
    flow        text NOT NULL,
    subject     text NOT NULL DEFAULT '',
    outcome     text NOT NULL,
    taps_bucket text NOT NULL DEFAULT '',
    ms_bucket   text NOT NULL DEFAULT '',
    n           bigint NOT NULL DEFAULT 0 CHECK (n >= 0),
    PRIMARY KEY (day, flow, subject, outcome, taps_bucket, ms_bucket)
);

-- the instance setting `usage.funnel` (NULL: the configured default, on)
ALTER TABLE instance_settings ADD COLUMN usage_funnel boolean;
