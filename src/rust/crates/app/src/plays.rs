//! What a game page's "Plays" tab and the Stats Play tab count (ot2f, OT-11): plays, time
//! and the person's own win rate, from one grouped read of their sessions.

use domain::ids::{CompanionId, ItemId};
use domain::kind::ProgressModel;
use domain::user::Actor;

use crate::ports::ItemPlayTotals;
use crate::{App, AppResult};

/// Plays, their logged time, and the results of the person themselves: `decided` plays
/// had a result for them ([`domain::session::Session::result_for`]), `wins` of those were
/// theirs; the rate is `wins / decided`, and the one place that rounds it is the client
/// (review OT2F-15). A co-op play without a result, a play without a winner and a play
/// they logged without taking part are plays but not decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayTotals {
    pub plays: u32,
    pub minutes: u64,
    pub decided: u32,
    pub wins: u32,
}

impl App {
    /// The entries that stand for the actor in their own log: their own entry and every
    /// entry merged into it (merge chains followed).
    async fn own_entries(&self, actor: Actor) -> AppResult<Vec<CompanionId>> {
        let me = self.user(actor.user_id).await?.self_companion;
        let owned = self.repos.companions.by_owner(actor.user_id).await?;
        let mut mine: Vec<CompanionId> = crate::people::roots(&owned)
            .into_iter()
            .filter(|(_, root)| *root == me)
            .map(|(id, _)| id)
            .collect();
        if !mine.contains(&me) {
            mine.push(me);
        }
        Ok(mine)
    }

    /// The actor's plays of one item.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn item_play_totals(&self, actor: Actor, item: ItemId) -> AppResult<PlayTotals> {
        let mine = self.own_entries(actor).await?;
        let rows = self
            .repos
            .sessions
            .play_totals(actor.user_id, &mine, Some(item))
            .await?;
        Ok(sum(rows.iter()))
    }

    /// The actor's plays of every item whose kind is played (`Sessions` progress model);
    /// "watched with" sessions of other kinds are not plays.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn play_totals(&self, actor: Actor) -> AppResult<PlayTotals> {
        let mine = self.own_entries(actor).await?;
        let rows = self
            .repos
            .sessions
            .play_totals(actor.user_id, &mine, None)
            .await?;
        if rows.is_empty() {
            return Ok(PlayTotals::default());
        }
        let ids: Vec<ItemId> = rows.iter().map(|r| r.item_id).collect();
        let played: std::collections::HashSet<ItemId> = self
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
            .collect();
        Ok(sum(rows.iter().filter(|r| played.contains(&r.item_id))))
    }
}

fn sum<'a>(rows: impl Iterator<Item = &'a ItemPlayTotals>) -> PlayTotals {
    let mut out = PlayTotals::default();
    for r in rows {
        out.plays += r.plays;
        out.minutes += r.minutes;
        out.decided += r.decided;
        out.wins += r.wins;
    }
    out
}
