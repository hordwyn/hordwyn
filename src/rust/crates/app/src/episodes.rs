//! Per-episode release dates of episodic items (WP4-7): which seasons a refresh asks a
//! connector for, storing what it answered, and the background job that does it. Only the
//! episode number, the air date and the runtime are kept (no titles or overviews).

use chrono::NaiveDate;
use domain::catalog::{CatalogEpisode, EpisodeSeason};
use domain::ids::ItemId;
use domain::kind::ProgressModel;

use crate::ports::{Lane, NewJob};
use crate::{App, AppResult};

/// Job that fetches the episode dates of one item, queued on the background lane by the
/// detail refresh.
pub const EPISODES_JOB: &str = "catalog.episodes";

/// One regular season as the item's `seasons` attribute describes it; a fetch is keyed by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonKey {
    pub number: u32,
    pub episode_count: u32,
    pub air_date: Option<NaiveDate>,
}

/// What a connector answered for one season.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonFetch {
    pub key: SeasonKey,
    pub connector: String,
    pub episodes: Vec<CatalogEpisode>,
}

/// The regular seasons (specials, season 0, left out) of an item's attributes, in order.
#[must_use]
pub fn season_keys(attributes: &serde_json::Value) -> Vec<SeasonKey> {
    let u32_of = |v: &serde_json::Value| v.as_u64().and_then(|n| u32::try_from(n).ok());
    let mut keys: Vec<SeasonKey> = attributes
        .get("seasons")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some(SeasonKey {
                number: u32_of(s.get("number")?)?,
                episode_count: s.get("episode_count").and_then(u32_of).unwrap_or(0),
                air_date: s
                    .get("air_date")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
            })
        })
        .filter(|k| k.number > 0)
        .collect();
    keys.sort_by_key(|k| k.number);
    keys.dedup_by_key(|k| k.number);
    keys
}

/// Must the season be asked for again? Yes when nothing is stored for it, when its episode
/// count or air date changed since, when fewer episodes are stored than it has, or when one of
/// them has no air date or airs today or later (dates still to come or to be corrected).
#[must_use]
pub fn season_needs_fetch(
    key: &SeasonKey,
    stored: Option<&EpisodeSeason>,
    today: NaiveDate,
) -> bool {
    let Some(s) = stored else {
        return true;
    };
    s.episode_count != key.episode_count
        || s.air_date != key.air_date
        || s.episodes.len() < key.episode_count as usize
        || s.episodes.is_empty()
        || s.episodes
            .iter()
            .any(|e| e.air_date.is_none_or(|d| d >= today))
}

/// Episodes as stored: regular numbers only, one per number, in order.
fn normalised(season: u32, episodes: &[CatalogEpisode]) -> Vec<CatalogEpisode> {
    let mut eps: Vec<CatalogEpisode> = episodes
        .iter()
        .filter(|e| e.number > 0)
        .map(|e| CatalogEpisode {
            season,
            runtime_minutes: e.runtime_minutes.filter(|r| *r > 0),
            ..e.clone()
        })
        .collect();
    eps.sort_by_key(|e| e.number);
    eps.dedup_by_key(|e| e.number);
    eps
}

impl App {
    /// The regular seasons of an episodic item that a refresh should ask for
    /// ([`season_needs_fetch`]); empty for other kinds.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn seasons_to_fetch(&self, id: ItemId) -> AppResult<Vec<SeasonKey>> {
        let item = self.catalog_item(id).await?;
        let episodic = self
            .kinds
            .get(&item.kind)
            .is_some_and(|k| k.progress_model == ProgressModel::Episodic);
        if !episodic {
            return Ok(Vec::new());
        }
        let stored = self.repos.catalog.episode_seasons(id).await?;
        let today = self.now().date_naive();
        Ok(season_keys(&item.attributes)
            .into_iter()
            .filter(|k| season_needs_fetch(k, stored.iter().find(|s| s.season == k.number), today))
            .collect())
    }

    /// Store what connectors answered for these seasons (each replaces what was stored for
    /// its season) and publish `entity.updated` once when any episode changed. Returns the
    /// number of seasons whose episodes changed.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn store_episodes(&self, id: ItemId, fetched: &[SeasonFetch]) -> AppResult<usize> {
        self.catalog_item(id).await?;
        let stored = self.repos.catalog.episode_seasons(id).await?;
        let now = self.now();
        let mut changed = 0;
        let seasons: Vec<EpisodeSeason> = fetched
            .iter()
            .map(|f| {
                let episodes = normalised(f.key.number, &f.episodes);
                let before = stored.iter().find(|s| s.season == f.key.number);
                if before.is_none_or(|b| b.episodes != episodes) {
                    changed += 1;
                }
                EpisodeSeason {
                    season: f.key.number,
                    connector: f.connector.clone(),
                    episode_count: f.key.episode_count,
                    air_date: f.key.air_date,
                    fetched_at: now,
                    episodes,
                }
            })
            .collect();
        if seasons.is_empty() {
            return Ok(0);
        }
        self.repos.catalog.put_episode_seasons(id, &seasons).await?;
        if changed > 0 {
            self.publish_item(id).await;
        }
        Ok(changed)
    }

    /// Every stored episode of an item, by season and number.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn catalog_episodes(&self, id: ItemId) -> AppResult<Vec<CatalogEpisode>> {
        Ok(self
            .repos
            .catalog
            .episode_seasons(id)
            .await?
            .into_iter()
            .flat_map(|s| s.episodes)
            .collect())
    }

    /// Queue [`EPISODES_JOB`] for the item on the background lane (deduplicated per item)
    /// when a season needs asking for. Returns whether a job was queued.
    ///
    /// # Errors
    /// Unknown item or storage errors. A failing queue is logged and not an error.
    pub async fn queue_episode_refresh(&self, id: ItemId) -> AppResult<bool> {
        if self.seasons_to_fetch(id).await?.is_empty() {
            return Ok(false);
        }
        let queued = self
            .jobs
            .enqueue(NewJob {
                lane: Lane::BackgroundRefresh,
                kind: EPISODES_JOB.into(),
                payload: serde_json::json!({ "item_id": id }),
                dedupe_key: Some(format!("{EPISODES_JOB}:{id}")),
                priority: 0,
                run_at: None,
            })
            .await;
        Ok(queued.is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn stored(count: u32, air: Option<NaiveDate>, eps: &[Option<NaiveDate>]) -> EpisodeSeason {
        EpisodeSeason {
            season: 1,
            connector: "tmdb".into(),
            episode_count: count,
            air_date: air,
            fetched_at: chrono::DateTime::UNIX_EPOCH,
            episodes: eps
                .iter()
                .zip(1..)
                .map(|(d, n)| CatalogEpisode {
                    season: 1,
                    number: n,
                    air_date: *d,
                    runtime_minutes: None,
                })
                .collect(),
        }
    }

    #[test]
    fn a_season_is_asked_for_only_when_something_can_have_changed() {
        let today = day(2026, 1, 10);
        let air = Some(day(2025, 1, 1));
        let key = SeasonKey {
            number: 1,
            episode_count: 2,
            air_date: air,
        };
        let aired = [Some(day(2025, 1, 1)), Some(day(2025, 1, 8))];
        assert!(season_needs_fetch(&key, None, today));
        assert!(!season_needs_fetch(
            &key,
            Some(&stored(2, air, &aired)),
            today
        ));
        assert!(season_needs_fetch(
            &key,
            Some(&stored(3, air, &aired)),
            today
        ));
        assert!(season_needs_fetch(
            &key,
            Some(&stored(2, None, &aired)),
            today
        ));
        assert!(season_needs_fetch(
            &key,
            Some(&stored(2, air, &aired[..1])),
            today
        ));
        assert!(season_needs_fetch(
            &key,
            Some(&stored(2, air, &[aired[0], None])),
            today
        ));
        assert!(season_needs_fetch(
            &key,
            Some(&stored(2, air, &[aired[0], Some(today)])),
            today
        ));
        // a source without episode counts (TheTVDB): unchanged once every episode aired
        let open = SeasonKey {
            episode_count: 0,
            ..key
        };
        assert!(!season_needs_fetch(
            &open,
            Some(&stored(0, air, &aired)),
            today
        ));
        assert!(season_needs_fetch(&open, Some(&stored(0, air, &[])), today));
    }
}
