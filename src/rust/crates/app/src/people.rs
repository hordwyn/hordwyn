//! People (design.md 3.6, 6.7): what a person card answers (together, plays, win rate, last
//! time, most played game) and the duplicates worth merging.

use std::collections::{HashMap, HashSet};

use domain::Timestamp;
use domain::companion::{Companion, name_key};
use domain::ids::{CompanionId, ItemId, UserId};
use domain::kind::ProgressModel;
use domain::session::{CoopOutcome, Participant, Session};
use domain::user::Actor;

use crate::{App, AppResult};

/// An item and when it happened (the card's "Last: Harbour Masters, Sat").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatItem {
    pub item_id: ItemId,
    pub title: String,
    pub at: Timestamp,
}

/// The game played most often together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MostPlayed {
    pub item_id: ItemId,
    pub title: String,
    pub plays: u32,
}

/// One person's numbers, from the actor's log and the sessions their linked account shared
/// with the actor. A play is a session of a kind with the `Sessions` progress model; it is
/// decided when someone won (or a cooperative outcome was set). A tie names several
/// winners, and each of them counts the play as a win.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionStats {
    pub companion_id: CompanionId,
    /// Sessions together (plays and "watched with").
    pub together: u32,
    pub plays: u32,
    pub wins: u32,
    pub decided: u32,
    pub last: Option<StatItem>,
    pub most_played: Option<MostPlayed>,
}

impl CompanionStats {
    /// Wins per decided play; `None` before the first decided play.
    #[must_use]
    pub fn win_rate(&self) -> Option<f64> {
        (self.decided > 0).then(|| f64::from(self.wins) / f64::from(self.decided))
    }
}

/// Two entries whose names are equal after normalising ([`name_key`]): fold `remove` into
/// `keep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeSuggestion {
    pub keep: CompanionId,
    pub remove: CompanionId,
}

#[derive(Default)]
struct Acc {
    together: u32,
    plays: u32,
    wins: u32,
    decided: u32,
    last: Option<(Timestamp, ItemId)>,
    per_game: HashMap<ItemId, (u32, Timestamp)>,
}

impl Acc {
    fn add(&mut self, s: &Session, play: bool, won: bool) {
        self.together += 1;
        if self.last.is_none_or(|(t, _)| s.started_at > t) {
            self.last = Some((s.started_at, s.item_id));
        }
        if !play {
            return;
        }
        self.plays += 1;
        let game = self.per_game.entry(s.item_id).or_insert((0, s.started_at));
        game.0 += 1;
        game.1 = game.1.max(s.started_at);
        if decided(s) {
            self.decided += 1;
            self.wins += u32::from(won);
        }
    }
}

fn decided(s: &Session) -> bool {
    s.coop_outcome.is_some() || s.participants.iter().any(|p| p.winner)
}

fn won(s: &Session, p: Option<&Participant>) -> bool {
    match s.coop_outcome {
        Some(CoopOutcome::Won) => true,
        Some(CoopOutcome::Lost) => false,
        None => p.is_some_and(|p| p.winner),
    }
}

/// The active companion each owned companion stands for now (merge chains followed).
pub(crate) fn roots(owned: &[Companion]) -> HashMap<CompanionId, CompanionId> {
    let by_id: HashMap<CompanionId, &Companion> = owned.iter().map(|c| (c.id, c)).collect();
    let mut out = HashMap::new();
    for c in owned {
        let mut at = c;
        // merge chains are short; the bound protects against corrupt cycles
        for _ in 0..32 {
            match at.merged_into.and_then(|n| by_id.get(&n)) {
                Some(next) => at = next,
                None => break,
            }
        }
        if at.is_active() {
            out.insert(c.id, at.id);
        }
    }
    out
}

/// The merge suggestions among `people` (active, not the self entry): per normalised name,
/// the entry with the most sessions together (then the older) is kept.
#[must_use]
pub fn suggest_merges(people: &[Companion], stats: &[CompanionStats]) -> Vec<MergeSuggestion> {
    let together: HashMap<CompanionId, u32> =
        stats.iter().map(|s| (s.companion_id, s.together)).collect();
    let mut groups: HashMap<String, Vec<&Companion>> = HashMap::new();
    for c in people.iter().filter(|c| c.is_active() && !c.is_self) {
        let key = name_key(&c.name);
        if !key.is_empty() {
            groups.entry(key).or_default().push(c);
        }
    }
    let mut out = Vec::new();
    for mut group in groups.into_values().filter(|g| g.len() > 1) {
        group.sort_by_key(|c| {
            (
                std::cmp::Reverse(together.get(&c.id).copied().unwrap_or(0)),
                c.created_at,
                c.id.0,
            )
        });
        let keep = group[0];
        for r in &group[1..] {
            // two different accounts are two people
            if let (Some(a), Some(b)) = (keep.linked_user, r.linked_user)
                && a != b
            {
                continue;
            }
            out.push(MergeSuggestion {
                keep: keep.id,
                remove: r.id,
            });
        }
    }
    out.sort_by_key(|s| (s.keep.0, s.remove.0));
    out
}

impl App {
    /// Stats for every active companion of the actor except the self entry, in the order of
    /// [`App::companions`].
    ///
    /// # Errors
    /// Storage errors.
    pub async fn companion_stats(&self, actor: Actor) -> AppResult<Vec<CompanionStats>> {
        let owned = self.repos.companions.by_owner(actor.user_id).await?;
        let root = roots(&owned);
        let people: Vec<CompanionId> = self
            .companions(actor)
            .await?
            .into_iter()
            .filter(|c| !c.is_self)
            .map(|c| c.id)
            .collect();
        let by_user: HashMap<UserId, CompanionId> = owned
            .iter()
            .filter(|c| c.is_active() && !c.is_self)
            .filter_map(|c| c.linked_user.map(|u| (u, c.id)))
            .collect();
        let mine = self.repos.sessions.by_owner(actor.user_id).await?;
        let shared = self.shared_sessions(actor).await?;
        let plays = self.play_items(mine.iter().chain(&shared)).await?;
        let mut acc: HashMap<CompanionId, Acc> = HashMap::new();
        for s in &mine {
            let play = plays.contains(&s.item_id);
            // entries merged into one person count once, with a win if any of them won
            // (UX3-26: a twin listed first must not hide the other's win)
            let mut per: Vec<(CompanionId, bool)> = Vec::new();
            for p in &s.participants {
                if let Some(r) = root.get(&p.companion_id)
                    && people.contains(r)
                {
                    match per.iter_mut().find(|(x, _)| x == r) {
                        Some((_, w)) => *w |= won(s, Some(p)),
                        None => per.push((*r, won(s, Some(p)))),
                    }
                }
            }
            for (r, w) in per {
                acc.entry(r).or_default().add(s, play, w);
            }
        }
        for s in &shared {
            let Some(c) = by_user.get(&s.owner) else {
                continue;
            };
            let owner_self = self.user(s.owner).await?.self_companion;
            let p = s.participants.iter().find(|p| p.companion_id == owner_self);
            let play = plays.contains(&s.item_id);
            acc.entry(*c).or_default().add(s, play, won(s, p));
        }
        self.finish_stats(people, acc).await
    }

    /// The items among these sessions whose kind is played (`Sessions` progress model).
    async fn play_items(
        &self,
        sessions: impl Iterator<Item = &Session>,
    ) -> AppResult<HashSet<ItemId>> {
        let ids: Vec<ItemId> = sessions
            .map(|s| s.item_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        Ok(self
            .repos
            .catalog
            .get_many(&ids)
            .await?
            .into_iter()
            .filter(|i| {
                self.kind(&i.kind)
                    .is_ok_and(|k| k.progress_model == ProgressModel::Sessions)
            })
            .map(|i| i.id)
            .collect())
    }

    async fn finish_stats(
        &self,
        people: Vec<CompanionId>,
        mut acc: HashMap<CompanionId, Acc>,
    ) -> AppResult<Vec<CompanionStats>> {
        let mut wanted: Vec<ItemId> = Vec::new();
        let mut out = Vec::with_capacity(people.len());
        for id in people {
            let a = acc.remove(&id).unwrap_or_default();
            // most plays, then the latest, then the item id: stable between renders
            let most = a
                .per_game
                .iter()
                .max_by_key(|(item, (n, last))| (*n, *last, std::cmp::Reverse(item.0)))
                .map(|(item, (n, _))| (*item, *n));
            wanted.extend(a.last.map(|(_, i)| i));
            wanted.extend(most.map(|(i, _)| i));
            out.push((id, a, most));
        }
        let titles: HashMap<ItemId, String> = self
            .repos
            .catalog
            .get_many(&wanted)
            .await?
            .into_iter()
            .map(|i| (i.id, i.title))
            .collect();
        let title = |i: &ItemId| titles.get(i).cloned().unwrap_or_default();
        Ok(out
            .into_iter()
            .map(|(id, a, most)| CompanionStats {
                companion_id: id,
                together: a.together,
                plays: a.plays,
                wins: a.wins,
                decided: a.decided,
                last: a.last.map(|(at, item_id)| StatItem {
                    item_id,
                    title: title(&item_id),
                    at,
                }),
                most_played: most.map(|(item_id, plays)| MostPlayed {
                    item_id,
                    title: title(&item_id),
                    plays,
                }),
            })
            .collect())
    }

    /// Duplicates worth merging among the actor's people ([`suggest_merges`]).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn merge_suggestions(&self, actor: Actor) -> AppResult<Vec<MergeSuggestion>> {
        let people = self.companions(actor).await?;
        let stats = self.companion_stats(actor).await?;
        Ok(suggest_merges(&people, &stats))
    }
}
