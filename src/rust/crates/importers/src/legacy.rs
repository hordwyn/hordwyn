//! Legacy export of the owner's previous tracker (D-16, KICKOFF Q4).
//!
//! The format is derived only from the owner's own export file (rule 6): the anonymised
//! fixture `crates/testkit/fixtures/import/legacy/export.json` and, for a local ignored test,
//! the full file in the git-ignored `owner-inbox/legacy/`. Nothing else describes it here.
//!
//! What the file holds and how it maps (DEC-354, DEC-355):
//! - top-level `metadata` (one entry per item), `collections` (definitions) and the sections
//!   `people`, `workouts`, `exercises`, `measurements`, `metadata_groups`,
//!   `workout_templates`, which are not imported (reported with their size);
//! - per item `lot` -> kind (`anime`, `book`, `movie`, `show`, `video_game`; any other lot is
//!   a row error), `source` + `identifier` -> one external id (`tmdb`, `igdb`,
//!   `openlibrary`, `google_books`, `myanimelist`, `anilist`; any other source leaves the
//!   row without an id, so it is unmatched), `source_id` -> the title;
//! - `seen_history` (newest first): a `completed` entry with `ended_on` becomes a history
//!   event (movies finished, shows `show_season_number`/`show_episode_number`, anime
//!   `anime_episode_number` in season 1); books and video games keep the status only (their
//!   progress models have no "finished" event); an entry without a date is logged on the
//!   day the item entered `Completed` (else `In Progress`), reported as an estimate, and
//!   left out when the item is in neither;
//!   other states set the status (`dropped`, `on_a_hold` -> paused) and lose their
//!   percentage;
//! - built-in collections `Completed`, `In Progress`, `Watchlist` -> the status (in
//!   progress wins over completed: a rewatch), `Reminders` and `Custom` are not imported,
//!   every other collection (built-in `Monitoring`, `Owned` and the user's own) -> a list.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::row::{HistoryEntry, ImportRow, RowError};
use crate::{Mapping, Parsed, lost_list};

pub const FORMAT: &str = "legacy-json";

/// Collections that set the status instead of becoming a list.
const STATUS_COLLECTIONS: [(&str, &str); 3] = [
    ("In Progress", "in_progress"),
    ("Completed", "completed"),
    ("Watchlist", "planned"),
];
/// Built-in collections that are not imported.
const SKIPPED_COLLECTIONS: [&str; 2] = ["Reminders", "Custom"];
/// Sections of the file that are not imported.
const SECTIONS: [&str; 6] = [
    "people",
    "workouts",
    "exercises",
    "measurements",
    "metadata_groups",
    "workout_templates",
];

#[derive(Deserialize)]
struct Item {
    lot: String,
    source: String,
    identifier: String,
    /// The title (despite its name).
    source_id: String,
    #[serde(default)]
    collections: Vec<Membership>,
    #[serde(default)]
    reviews: Vec<serde_json::Value>,
    #[serde(default)]
    seen_history: Vec<Seen>,
}

#[derive(Deserialize)]
struct Membership {
    collection_name: String,
    /// When the item entered the collection.
    #[serde(default)]
    created_on: Option<String>,
}

#[derive(Deserialize)]
struct Seen {
    state: String,
    #[serde(default)]
    ended_on: Option<String>,
    #[serde(default)]
    started_on: Option<String>,
    #[serde(default)]
    show_season_number: Option<u32>,
    #[serde(default)]
    show_episode_number: Option<u32>,
    #[serde(default)]
    anime_episode_number: Option<u32>,
    #[serde(default)]
    providers_consumed_on: Vec<String>,
}

/// What the preview says maps to what.
#[must_use]
pub fn mapping() -> Vec<Mapping> {
    [
        ("lot", "kind"),
        ("source + identifier", "external_ids"),
        ("source_id", "title"),
        ("seen_history", "history"),
        ("collections: Completed, In Progress, Watchlist", "status"),
        ("collections: other", "lists"),
    ]
    .into_iter()
    .map(|(from, to)| Mapping::new(from, to))
    .collect()
}

fn kind_of(lot: &str) -> Option<&'static str> {
    Some(match lot {
        "anime" => "anime",
        "book" => "book",
        "movie" => "movie",
        "show" => "show",
        "video_game" => "video_game",
        _ => return None,
    })
}

fn namespace_of(source: &str) -> Option<&'static str> {
    Some(match source {
        "tmdb" => "tmdb",
        "igdb" => "igdb",
        "openlibrary" => "openlibrary",
        "google_books" => "google_books",
        "myanimelist" => "myanimelist",
        "anilist" => "anilist",
        _ => return None,
    })
}

/// Parse a whole export: one row per item, plus what is lost.
#[must_use]
pub fn parse(bytes: &[u8]) -> Parsed {
    let doc: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => return Parsed::error(format!("not a JSON export: {e}")),
    };
    let Some(items) = doc.get("metadata").and_then(serde_json::Value::as_array) else {
        return Parsed::error("not a legacy export: `metadata` is missing".into());
    };
    let mut lost = BTreeMap::new();
    for s in SECTIONS {
        let n = doc
            .get(s)
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        *lost.entry(s).or_insert(0) += n as u64;
    }
    let rows = items
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let line = i as u64 + 1;
            let item = Item::deserialize(v).map_err(|e| RowError {
                line,
                source_id: None,
                message: format!("item {line}: {e}"),
            })?;
            row(&item, line, &mut lost)
        })
        .collect();
    Parsed {
        rows,
        lost: lost_list(lost),
    }
}

fn row(
    item: &Item,
    line: u64,
    lost: &mut BTreeMap<&'static str, u64>,
) -> Result<ImportRow, RowError> {
    let source_id = format!("{}:{}:{}", item.lot, item.source, item.identifier);
    let kind = kind_of(&item.lot).ok_or_else(|| RowError {
        line,
        source_id: Some(source_id.clone()),
        message: format!("the kind `{}` is not imported", item.lot),
    })?;
    let external_ids = namespace_of(&item.source)
        .map(|ns| BTreeMap::from([(ns.to_string(), item.identifier.clone())]))
        .unwrap_or_default();
    *lost.entry("reviews").or_insert(0) += item.reviews.len() as u64;
    // views without a date are logged on the day the item entered Completed (else In
    // Progress): an estimate, reported as such, rather than losing the watched episodes
    let estimate = ["Completed", "In Progress"].iter().find_map(|c| {
        item.collections
            .iter()
            .find(|m| m.collection_name == *c)
            .and_then(|m| m.created_on.clone())
    });
    let history = history(kind, &item.seen_history, estimate.as_deref(), lost);
    let mut lists = Vec::new();
    for m in &item.collections {
        let name = m.collection_name.as_str();
        if SKIPPED_COLLECTIONS.contains(&name) {
            *lost.entry("skipped_collections").or_insert(0) += 1;
        } else if !STATUS_COLLECTIONS.iter().any(|(c, _)| *c == name)
            && !lists.iter().any(|l: &String| l == name)
        {
            lists.push(name.to_string());
        }
    }
    Ok(ImportRow {
        source_id,
        kind: kind.to_string(),
        title: item.source_id.clone(),
        year: None,
        external_ids,
        status: status(item).map(String::from),
        rating: None,
        notes: None,
        history,
        lists,
    })
}

/// Dropped or on hold (the newest entry) first, then the status collections, then the
/// newest entry's own state.
fn status(item: &Item) -> Option<&'static str> {
    let newest = item.seen_history.first().map(|s| s.state.as_str());
    match newest {
        Some("dropped") => return Some("dropped"),
        Some("on_a_hold") => return Some("paused"),
        _ => {}
    }
    for (name, status) in STATUS_COLLECTIONS {
        if item.collections.iter().any(|m| m.collection_name == name) {
            return Some(status);
        }
    }
    match newest {
        Some("in_progress") => Some("in_progress"),
        Some("completed") => Some("completed"),
        _ => None,
    }
}

fn history(
    kind: &str,
    seen: &[Seen],
    estimate: Option<&str>,
    lost: &mut BTreeMap<&'static str, u64>,
) -> Vec<HistoryEntry> {
    let mut out = Vec::new();
    for s in seen {
        if !s.providers_consumed_on.is_empty() {
            *lost.entry("watched_on").or_insert(0) += 1;
        }
        if s.state != "completed" {
            *lost.entry("partial_progress").or_insert(0) += 1;
            continue;
        }
        let progress = match kind {
            "movie" => serde_json::json!({"type": "finished"}),
            "show" | "anime" => {
                // anime numbers its episodes in one run: season 1
                let numbers = if kind == "show" {
                    s.show_season_number.zip(s.show_episode_number)
                } else {
                    s.anime_episode_number.map(|e| (1, e))
                };
                let Some((season, episode)) = numbers else {
                    *lost.entry("views_without_episode").or_insert(0) += 1;
                    continue;
                };
                serde_json::json!({"type": "episode", "season": season, "episode": episode})
            }
            // books and games: no "finished" event in their progress models
            _ => {
                *lost.entry("finish_dates").or_insert(0) += 1;
                continue;
            }
        };
        let date = match (&s.ended_on, estimate) {
            (Some(d), _) => d.clone(),
            (None, Some(d)) => {
                *lost.entry("estimated_dates").or_insert(0) += 1;
                d.to_string()
            }
            (None, None) => {
                *lost.entry("undated_history").or_insert(0) += 1;
                continue;
            }
        };
        if s.started_on.is_some() {
            *lost.entry("start_dates").or_insert(0) += 1;
        }
        out.push(HistoryEntry {
            date,
            progress: Some(progress),
            event_id: None,
        });
    }
    stable_ids(&mut out);
    out
}

/// The file has no event ids and lists views newest first, so a position would shift with
/// every new view (UX3-13). The id is the view's own content instead: date, progress and
/// how often that pair occurred before it in the row, counted from the oldest.
fn stable_ids(out: &mut [HistoryEntry]) {
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    for h in out.iter_mut().rev() {
        let progress = h
            .progress
            .as_ref()
            .map(Value::to_string)
            .unwrap_or_default();
        let key = format!("{}|{progress}", h.date);
        let n = seen.entry(key.clone()).or_insert(0);
        h.event_id = Some(format!("{key}|{n}"));
        *n += 1;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unknown_lots_are_row_errors() {
        let doc = serde_json::json!({"metadata": [
            {"lot": "podcast", "source": "x", "identifier": "1", "source_id": "Pod"}
        ]});
        let p = super::parse(doc.to_string().as_bytes());
        assert!(p.rows[0].as_ref().unwrap_err().message.contains("podcast"));
    }
}
