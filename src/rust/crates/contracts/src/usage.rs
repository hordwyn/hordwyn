//! The logging funnel (WP4-5, design.md 9, research R-54, addendum-1 D16): what the browser
//! sends in its beacon (`POST /usage/funnel`) and what Admin > Storage and jobs shows
//! (`GET /admin/usage`). Nothing here names a person or an item: the beacon carries fixed
//! words and buckets only, and the server keeps daily totals.

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// Flows that run from an intent to `logged` or `abandoned` with taps and time.
pub const FUNNEL_FLOWS: [&str; 5] = ["palette", "next_step", "sheet", "episode_mark", "bulk"];
/// Outcomes of those flows.
pub const FUNNEL_OUTCOMES: [&str; 2] = ["logged", "abandoned"];
/// The flow of the disclosure numbers (D16), keyed by page and section.
pub const DISCLOSURE_FLOW: &str = "disclosure";
/// What a disclosure reports: shown on a visit, opened, opened within 2 s of the page
/// showing, open when the page saved.
pub const DISCLOSURE_OUTCOMES: [&str; 4] = ["shown", "opened", "opened_early", "open_at_save"];
/// The collapsible sections whose numbers are counted, as `page/section` (UX4-14: a fixed
/// list, so a beacon cannot add rows without bound). Each is a `disclosure_storage_key(page,
/// section)` of the web pages; a web test checks that every key the pages use is here, and
/// Settings > Privacy names each of them. The dev gallery is not measured.
pub const DISCLOSURE_SUBJECTS: [&str; 10] = [
    "library/filters",
    "item/log-more",
    "item/play-more",
    "stats/goal",
    "account/signin",
    "users/invites",
    "instance/mail",
    "instance/secrets",
    "instance/advanced",
    "storage/usage",
];
/// Tap buckets: 1 to 4, then 5 and more.
pub const TAPS_BUCKETS: [&str; 5] = ["1", "2", "3", "4", "5+"];
/// Time buckets: under 2, 5, 10, 30 and 60 seconds, then 60 seconds and over.
pub const TIME_BUCKETS: [&str; 6] = ["lt2s", "lt5s", "lt10s", "lt30s", "lt60s", "60s+"];
/// Most events in one beacon.
pub const MAX_BEACON_EVENTS: usize = 50;
/// Days the daily totals are kept.
pub const RETENTION_DAYS: u32 = 180;

/// One finished flow, or one disclosure number. Unknown fields are refused, so an item id
/// cannot ride along.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(deny_unknown_fields)]
pub struct FunnelEventDto {
    pub flow: String,
    pub outcome: String,
    /// `page/section` of a disclosure (`item/log-more`); absent for the other flows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// A [`TAPS_BUCKETS`] value; absent for disclosures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taps: Option<String>,
    /// A [`TIME_BUCKETS`] value; absent for disclosures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
}

/// Body of `POST /usage/funnel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(deny_unknown_fields)]
pub struct UsageBeaconDto {
    pub events: Vec<FunnelEventDto>,
}

/// One daily total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct UsageRowDto {
    /// `2026-11-08` (UTC).
    pub day: String,
    pub flow: String,
    /// Empty except for disclosures.
    pub subject: String,
    pub outcome: String,
    pub taps_bucket: String,
    pub ms_bucket: String,
    pub n: u64,
}

/// `GET /admin/usage`: whether counting is on and the totals of the last days, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct UsageReportDto {
    pub enabled: bool,
    pub retention_days: u32,
    /// Days covered by `rows`.
    pub days: u32,
    pub rows: Vec<UsageRowDto>,
}
