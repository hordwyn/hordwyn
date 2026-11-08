//! The user's own Trakt data as JSON files (docs/connectors/trakt.md, DEC-297): the documented
//! response shapes of `/sync/history`, `/sync/ratings` and `/sync/watchlist`. No network
//! call, no API app, no images; Trakt's `title`/`year` snapshots are used for matching and
//! display only.
//!
//! A file is a JSON array; each item's kind of list is told by its fields (`watched_at` ->
//! history, `rated_at` + `rating` -> ratings, `listed_at` -> watchlist), so the three files
//! can be uploaded in any order or as one array. Items are grouped into one row per movie or
//! show (source id `movie:<trakt id>` / `show:<trakt id>`): history events become history
//! entries keyed by their Trakt event id (re-importing a newer file adds only new events),
//! ratings 1..10 become 0..100 (DEC-018), the watchlist sets `planned` (when the file has no
//! history for the row) and the note. External ids are TMDB, `IMDb` and TVDB only: a row
//! without one is unmatched (Trakt's own ids are not catalog ids).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::row::{HistoryEntry, ImportRow, RowError};
use crate::{Mapping, Parsed, lost_list};

pub const FORMAT: &str = "trakt-json";

/// What the preview says maps to what.
#[must_use]
pub fn mapping() -> Vec<Mapping> {
    [
        ("type + ids (tmdb, imdb, tvdb)", "external_ids"),
        ("title, year", "title"),
        ("history: watched_at", "history"),
        ("ratings: rating", "rating"),
        ("watchlist", "status"),
        ("watchlist: notes", "notes"),
    ]
    .into_iter()
    .map(|(from, to)| Mapping::new(from, to))
    .collect()
}

#[derive(Default)]
struct Acc {
    kind: &'static str,
    title: String,
    year: Option<i32>,
    external_ids: BTreeMap<String, String>,
    history: Vec<HistoryEntry>,
    rating: Option<u8>,
    planned: bool,
    notes: Option<String>,
}

impl Acc {
    fn into_row(self, source_id: String) -> ImportRow {
        let status = if !self.history.is_empty() {
            Some(if self.kind == "movie" {
                "completed"
            } else {
                "in_progress"
            })
        } else if self.planned {
            Some("planned")
        } else {
            None
        };
        ImportRow {
            source_id,
            kind: self.kind.to_string(),
            title: self.title,
            year: self.year,
            external_ids: self.external_ids,
            status: status.map(String::from),
            rating: self.rating,
            notes: self.notes,
            history: self.history,
            lists: Vec::new(),
        }
    }
}

fn id_text(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// The movie or show an item is about, its kind and its row key.
fn subject(item: &Value) -> Option<(&'static str, &Value, String)> {
    let (kind, media) = match item["type"].as_str()? {
        "movie" => ("movie", &item["movie"]),
        "show" | "season" | "episode" => ("show", &item["show"]),
        _ => return None,
    };
    if !media.is_object() {
        return None;
    }
    let ids = &media["ids"];
    let key = id_text(&ids["trakt"])
        .or_else(|| id_text(&ids["slug"]).map(|s| format!("slug:{s}")))
        .unwrap_or_else(|| {
            format!(
                "title:{}:{}",
                media["title"].as_str().unwrap_or_default(),
                media["year"]
            )
        });
    Some((kind, media, format!("{kind}:{key}")))
}

/// Parse one file.
#[must_use]
pub fn parse(bytes: &[u8]) -> Parsed {
    let items: Vec<Value> = match serde_json::from_slice(bytes) {
        Ok(Value::Array(a)) => a,
        Ok(_) => return Parsed::error("not a Trakt file: expected a JSON array".into()),
        Err(e) => return Parsed::error(format!("not a JSON file: {e}")),
    };
    let mut order: Vec<String> = Vec::new();
    let mut rows: BTreeMap<String, Acc> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut lost = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        let line = i as u64 + 1;
        let fail = |message: String| RowError {
            line,
            source_id: None,
            message,
        };
        let Some((kind, media, key)) = subject(item) else {
            errors.push(fail(format!("item {line}: no movie or show")));
            continue;
        };
        let sub = item["type"].as_str().unwrap_or_default();
        let history = item["watched_at"].as_str();
        let rated = item["rated_at"].is_string() && item["rating"].is_number();
        let listed = item["listed_at"].is_string();
        if history.is_none() && !rated && !listed {
            errors.push(fail(format!(
                "item {line}: not a history, rating or watchlist entry"
            )));
            continue;
        }
        if rated && matches!(sub, "season" | "episode") {
            *lost.entry("season_episode_ratings").or_insert(0) += 1;
            continue;
        }
        let entry = rows.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Acc {
                kind,
                title: media["title"].as_str().unwrap_or_default().to_string(),
                year: media["year"].as_i64().and_then(|y| i32::try_from(y).ok()),
                external_ids: ["tmdb", "imdb", "tvdb"]
                    .into_iter()
                    .filter_map(|ns| id_text(&media["ids"][ns]).map(|v| (ns.to_string(), v)))
                    .collect(),
                ..Acc::default()
            }
        });
        if let Some(date) = history {
            if let Err(message) = add_history(entry, item, sub, date) {
                errors.push(fail(format!("item {line}: {message}")));
                continue;
            }
            if item.get("action").is_some() {
                *lost.entry("action").or_insert(0) += 1;
            }
        } else if rated {
            let r = item["rating"].as_u64().unwrap_or(0);
            if !(1..=10).contains(&r) {
                errors.push(fail(format!("item {line}: rating {r} is not 1..10")));
                continue;
            }
            entry.rating = u8::try_from(r * 10).ok();
            *lost.entry("rated_at").or_insert(0) += 1;
        } else {
            entry.planned = true;
            if let Some(n) = item["notes"].as_str().filter(|n| !n.is_empty()) {
                entry.notes = Some(n.to_string());
            }
            *lost.entry("listed_at").or_insert(0) += 1;
        }
    }
    let mut out: Vec<Result<ImportRow, RowError>> = errors.into_iter().map(Err).collect();
    for key in order {
        if let Some(acc) = rows.remove(&key) {
            out.push(Ok(acc.into_row(key)));
        }
    }
    Parsed {
        rows: out,
        lost: lost_list(lost),
    }
}

fn add_history(acc: &mut Acc, item: &Value, sub: &str, date: &str) -> Result<(), String> {
    let progress = match sub {
        "movie" => serde_json::json!({"type": "finished"}),
        "episode" => {
            let ep = &item["episode"];
            match (ep["season"].as_u64(), ep["number"].as_u64()) {
                (Some(season), Some(episode)) => {
                    serde_json::json!({"type": "episode", "season": season, "episode": episode})
                }
                _ => return Err("an episode without season and number".into()),
            }
        }
        other => return Err(format!("history of type `{other}`")),
    };
    acc.history.push(HistoryEntry {
        date: date.to_string(),
        progress: Some(progress),
        event_id: id_text(&item["id"]),
    });
    Ok(())
}
