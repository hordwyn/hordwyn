//! The offline queue (WP4-5, DEC-237, design.md 5.4, 2.4, 6.15). While the browser is offline,
//! or a call fails with a network error, the one-click logs (next step, episode marks),
//! status changes and ratings become `SyncCommandEnvelope`s in a per-person queue in this
//! browser's `localStorage`: log commands keep the client op id the online call carried
//! (the server may have stored it before the answer was lost), mutable fields carry a stamp of
//! the client's hybrid logical clock. "Saved offline, will sync" has Undo, which takes the
//! command out again. The queue is sent with the existing sync push (`POST /sync/commands`)
//! when the browser comes back online and on every page load; a refused command (the item is
//! gone, the value is invalid) is dropped with a toast naming the item, a failed request or a
//! server error keeps it for the next try. Plays and sessions stay online-only (DEC).
//!
//! The pure parts (clock, queue, results) are here and unit tested; [`provide_offline`] and
//! the browser helpers wire them to `localStorage`, the `online` event and the toasts.

use contracts::{SyncBatchDto, SyncCommandDto, SyncCommandEnvelope, SyncResultDto};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use ui::{ToastAction, ToastVariant, Toasts};

use crate::i18n::I18n;

/// The client's hybrid logical clock: the same `(wall_ms, counter, node)` ordering and text
/// form as the server's `domain::hlc::Hlc` (`{wall_ms:016x}-{counter:08x}-{node:08x}`). The
/// `web` crate may not depend on `domain` (D-3, `xtask/src/arch.rs`), so the two dozen lines of
/// `tick` live here; `apps/server/tests/it/offline.rs` checks that the server parses and orders
/// what this clock makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientClock {
    node: u32,
    wall: u64,
    counter: u32,
}

impl ClientClock {
    #[must_use]
    pub fn new(node: u32) -> Self {
        Self {
            node,
            wall: 0,
            counter: 0,
        }
    }

    /// Continue after the last stamp this browser made (kept with the queue); a stamp that
    /// does not parse starts afresh.
    #[must_use]
    pub fn resume(node: u32, last: Option<&str>) -> Self {
        let parsed = last.and_then(|s| {
            let mut parts = s.split('-');
            let wall = u64::from_str_radix(parts.next()?, 16).ok()?;
            let counter = u32::from_str_radix(parts.next()?, 16).ok()?;
            Some((wall, counter))
        });
        match parsed {
            Some((wall, counter)) => Self {
                node,
                wall,
                counter,
            },
            None => Self::new(node),
        }
    }

    /// The next stamp at the wall-clock reading `now_ms`: later than every earlier one, even
    /// when the clock went back.
    pub fn stamp(&mut self, now_ms: u64) -> String {
        if now_ms > self.wall {
            self.wall = now_ms;
            self.counter = 0;
        } else {
            self.counter = self.counter.saturating_add(1);
        }
        format!("{:016x}-{:08x}-{:08x}", self.wall, self.counter, self.node)
    }
}

/// One queued command with the words for its toasts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Queued {
    pub envelope: SyncCommandEnvelope,
    /// The item's title (the toast that says a command was refused names it).
    pub title: String,
    pub queued_at_ms: u64,
}

impl Queued {
    /// A command with a new op id.
    #[must_use]
    pub fn new(command: SyncCommandDto, title: &str, now_ms: u64) -> Self {
        Self::with_op_id(uuid::Uuid::now_v7().to_string(), command, title, now_ms)
    }

    /// A command with the op id an online attempt already carried.
    #[must_use]
    pub fn with_op_id(op_id: String, command: SyncCommandDto, title: &str, now_ms: u64) -> Self {
        Self {
            envelope: SyncCommandEnvelope { op_id, command },
            title: title.to_string(),
            queued_at_ms: now_ms,
        }
    }
}

/// Log progress (append-only, idempotent by op id); `occurred_at` is when it happened
/// offline, so History keeps the real time; `hlc` stamps the status the log implies as of
/// when it was queued (DEC-455).
#[must_use]
pub fn log_command(
    item_id: &str,
    progress: serde_json::Value,
    occurred_at: Option<String>,
    hlc: Option<&str>,
) -> SyncCommandDto {
    SyncCommandDto::LogProgress {
        item_id: item_id.to_string(),
        progress,
        occurred_at,
        hlc: hlc.map(str::to_string),
    }
}

/// A log for the queue, with its op id, its time (now when `occurred_at` is not given, so
/// History keeps the real time and the logs of a queue keep their order: the start of a
/// rewatch run before its episodes) and the queue's stamp for the status it implies
/// (DEC-455).
#[must_use]
pub fn queued_log(
    item_id: &str,
    title: &str,
    op: String,
    progress: serde_json::Value,
    occurred_at: Option<String>,
    offline: Option<Offline>,
) -> Queued {
    let at = occurred_at.or_else(now_rfc3339);
    let hlc = offline.map(Offline::stamp);
    Queued::with_op_id(
        op,
        log_command(item_id, progress, at, hlc.as_deref()),
        title,
        now_ms(),
    )
}

/// Set the status at the client stamp `hlc` (last writer wins).
#[must_use]
pub fn status_command(item_id: &str, status: &str, hlc: &str) -> SyncCommandDto {
    SyncCommandDto::SetStatus {
        item_id: item_id.to_string(),
        status: status.to_string(),
        hlc: hlc.to_string(),
    }
}

/// One change for the queue: a log and, behind it, the status that follows it ("Finished"
/// on a book offline: the last page, then Completed; review BOOK-6). The order is the order
/// the server gets them in, so the status is written after the page it should count.
#[must_use]
pub fn log_then_status(log: Queued, status: Option<SyncCommandDto>, now_ms: u64) -> Vec<Queued> {
    let title = log.title.clone();
    let mut change = vec![log];
    change.extend(status.map(|cmd| Queued::new(cmd, &title, now_ms)));
    change
}

/// Rate (0..=100, `None` clears) at the client stamp `hlc`.
#[must_use]
pub fn rate_command(item_id: &str, rating: Option<u8>, hlc: &str) -> SyncCommandDto {
    SyncCommandDto::Rate {
        item_id: item_id.to_string(),
        rating,
        hlc: hlc.to_string(),
    }
}

/// The push of a flush: every command in the order queued, with the stamps it got in the
/// queue, so an offline change counts as made when it was made: a later change from another
/// device wins over it, and the status a queued log implies (stamped when it was queued)
/// loses to a status queued after it (DEC-434, DEC-455). Replays are harmless: logs are
/// idempotent by op id, fields keep the newest stamp.
#[must_use]
pub fn flush_batch(queue: &[Queued]) -> SyncBatchDto {
    SyncBatchDto {
        commands: queue.iter().map(|q| q.envelope.clone()).collect(),
    }
}

/// What one flush did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settled {
    /// Commands the server took.
    pub synced: usize,
    /// Commands it refused (named in a toast, then forgotten).
    pub dropped: Vec<Queued>,
}

/// The stored queue of one person in this browser, with the clock's node and last stamp.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    pub node: u32,
    pub last: Option<String>,
    pub items: Vec<Queued>,
}

/// Most commands one person's queue holds in this browser (UX4-13): a bulk episode mark is one
/// command per episode, and `localStorage` is small. A change that does not fit is not queued,
/// and a toast says why.
pub const MAX_QUEUED: usize = 500;

impl Stored {
    /// Queue `q` (once per op id).
    pub fn push(&mut self, q: Queued) {
        if !self
            .items
            .iter()
            .any(|i| i.envelope.op_id == q.envelope.op_id)
        {
            self.items.push(q);
        }
    }

    /// Queue every command of one change, or none when they would not all fit under
    /// [`MAX_QUEUED`] (half a change would be worse than none); `false` then.
    pub fn push_all(&mut self, change: Vec<Queued>) -> bool {
        let new = change
            .iter()
            .filter(|q| {
                !self
                    .items
                    .iter()
                    .any(|i| i.envelope.op_id == q.envelope.op_id)
            })
            .count();
        if self.items.len() + new > MAX_QUEUED {
            return false;
        }
        for q in change {
            self.push(q);
        }
        true
    }

    /// Undo: take the command out; `false` when it was not queued (sent already).
    pub fn remove(&mut self, op_id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.envelope.op_id != op_id);
        self.items.len() != before
    }

    /// Apply the push answer: taken and refused commands leave the queue, commands that met
    /// server trouble ([`retry_later`]) or got no answer stay.
    pub fn settle(&mut self, results: &[SyncResultDto]) -> Settled {
        let mut out = Settled::default();
        for r in results {
            let Some(pos) = self.items.iter().position(|i| i.envelope.op_id == r.op_id) else {
                continue;
            };
            if r.ok {
                self.items.remove(pos);
                out.synced += 1;
            } else if !r.error.as_ref().is_some_and(|e| retry_later(&e.code)) {
                out.dropped.push(self.items.remove(pos));
            }
        }
        out
    }
}

/// A refusal that is the server's trouble, not the command's: try again later.
#[must_use]
pub fn retry_later(code: &str) -> bool {
    matches!(
        code,
        "internal" | "unavailable" | "generic" | "too_many_attempts"
    )
}

/// What a refused command was, for its "Not saved: ..." toast (UX4-13): the logged step
/// ("Watched S1 E2"), "status Paused", "rating", "notes" or "added to the library".
#[must_use]
pub fn command_what(i18n: &I18n, q: &Queued) -> String {
    match &q.envelope.command {
        SyncCommandDto::LogProgress { progress, .. } => {
            crate::widgets::progress_label(i18n, progress)
        }
        SyncCommandDto::SetStatus { status, .. } => {
            let word = i18n.t(&format!("status-{status}"));
            i18n.t_args("offline-what-status", &[("status", word.into())])
        }
        SyncCommandDto::Rate { .. } => i18n.t("offline-what-rating"),
        SyncCommandDto::SetNotes { .. } => i18n.t("offline-what-notes"),
        SyncCommandDto::Add { .. } => i18n.t("offline-what-add"),
    }
}

/// What Undo of a change saved offline did (UX4-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoOutcome {
    /// Taken out of the queue: it will not be sent.
    Undone,
    /// A flush is sending the queue right now: the server applies it anyway, so nothing is
    /// taken out and the toast says so.
    Sending,
    /// Already sent: nothing left in the queue to take out.
    Sent,
}

/// Undo of the change with `ops`: refused while `flushing` (the push already carries it),
/// else taken out when it is still queued.
pub fn undo_queued(s: &mut Stored, ops: &[String], flushing: bool) -> UndoOutcome {
    if flushing
        && ops
            .iter()
            .any(|op| s.items.iter().any(|i| i.envelope.op_id == *op))
    {
        return UndoOutcome::Sending;
    }
    let mut removed = false;
    for op in ops {
        removed |= s.remove(op);
    }
    if removed {
        UndoOutcome::Undone
    } else {
        UndoOutcome::Sent
    }
}

/// `localStorage` key of a person's queue (several people may share a browser).
#[must_use]
pub fn storage_key(username: &str) -> String {
    format!("app-offline-{username}")
}

// ---------------------------------------------------------------- in the browser

/// Did a server function fail because the request never reached the server (offline, the
/// connection dropped)? Server answers, even errors, are not network failures.
#[must_use]
pub fn network_error(e: &ServerFnError) -> bool {
    matches!(e, ServerFnError::Request(_))
}

/// The queue in context (the shell provides it for a signed-in person).
#[derive(Clone, Copy)]
pub struct Offline {
    /// Commands waiting to be sent (the banner's count).
    pub waiting: RwSignal<usize>,
    user: StoredValue<String>,
    flushing: RwSignal<bool>,
}

/// What a queued change needs to say so: the messages and the toasts, taken while the
/// component is built (handlers run outside the reactive owner).
#[derive(Clone)]
pub struct OfflineCtx {
    pub offline: Offline,
    pub i18n: std::sync::Arc<I18n>,
    pub toasts: Toasts,
}

impl OfflineCtx {
    /// `None` outside the signed-in shell.
    #[must_use]
    pub fn current() -> Option<Self> {
        Some(Self {
            offline: use_context::<Offline>()?,
            i18n: crate::i18n::use_i18n(),
            toasts: ui::use_toasts(),
        })
    }
}

/// Milliseconds since the Unix epoch (browser clock; 0 on the server).
#[must_use]
pub fn now_ms() -> u64 {
    #[cfg(feature = "hydrate")]
    {
        // `Date.now()` is a whole, non-negative number of milliseconds
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ms = js_sys::Date::now() as u64;
        ms
    }
    #[cfg(not(feature = "hydrate"))]
    0
}

/// Now as RFC 3339 (`occurred_at` of an offline log).
#[must_use]
pub fn now_rfc3339() -> Option<String> {
    #[cfg(feature = "hydrate")]
    {
        js_sys::Date::new_0().to_iso_string().as_string()
    }
    #[cfg(not(feature = "hydrate"))]
    None
}

#[cfg(feature = "hydrate")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// The person's stored queue; a new clock node when there is none yet.
fn load(user: &str) -> Stored {
    #[cfg(feature = "hydrate")]
    {
        let mut s: Stored = storage()
            .and_then(|st| st.get_item(&storage_key(user)).ok().flatten())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if s.node == 0 {
            // a random clock node per browser and person (ties between devices)
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let node = (js_sys::Math::random() * f64::from(u32::MAX)) as u32;
            s.node = node.max(1);
        }
        s
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = user;
        Stored::default()
    }
}

fn save(user: &str, s: &Stored) {
    #[cfg(feature = "hydrate")]
    if let Some(st) = storage() {
        let key = storage_key(user);
        let _ = if s.items.is_empty() {
            st.remove_item(&key)
        } else {
            st.set_item(&key, &serde_json::to_string(s).unwrap_or_default())
        };
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (user, s);
}

impl Offline {
    /// Change the stored queue and update the count.
    fn with_stored<T>(self, f: impl FnOnce(&mut Stored) -> T) -> T {
        let user = self.user.get_value();
        let mut s = load(&user);
        let out = f(&mut s);
        save(&user, &s);
        self.waiting.set(s.items.len());
        out
    }

    /// A client stamp for a status or rating change queued now.
    pub fn stamp(self) -> String {
        self.with_stored(|s| {
            let mut clock = ClientClock::resume(s.node, s.last.as_deref());
            let h = clock.stamp(now_ms());
            s.last = Some(h.clone());
            h
        })
    }

    /// Send the queue (one push in the order queued, with the stamps from the queue), drop what
    /// the server refuses with a toast naming the item and the change, keep the rest for the
    /// next try.
    pub fn flush(self, i18n: std::sync::Arc<I18n>, toasts: Toasts) {
        if self.flushing.get_untracked() || self.with_stored(|s| s.items.is_empty()) {
            return;
        }
        leptos::task::spawn_local(async move {
            self.send(&i18n, toasts).await;
        });
    }

    /// One flush, awaited: `None` while another flush runs, else the number of commands still
    /// waiting afterwards (sign-out waits for it, UX4-6).
    pub async fn send(self, i18n: &I18n, toasts: Toasts) -> Option<usize> {
        if self.flushing.get_untracked() {
            return None;
        }
        self.flushing.set(true);
        let batch = self.with_stored(|s| flush_batch(&s.items));
        let mut dropped = Vec::new();
        push(self, &batch, &mut dropped).await;
        for q in dropped {
            let what = command_what(i18n, &q);
            toasts.push(
                i18n.t_args("offline-dropped", &[("what", what.into())]),
                Some(q.title),
                ToastVariant::Destructive,
            );
        }
        self.flushing.set(false);
        Some(self.with_stored(|s| s.items.len()))
    }

    /// Forget this person's queue in this browser (sign-out after "discard", UX4-6).
    pub fn discard(self) {
        self.with_stored(|s| s.items.clear());
    }
}

/// One push; a failed request keeps the commands for the next flush.
async fn push(o: Offline, b: &SyncBatchDto, dropped: &mut Vec<Queued>) {
    if b.commands.is_empty() {
        return;
    }
    let body = serde_json::to_value(b).unwrap_or_default();
    if let Ok(Some(results)) =
        crate::rest::call::<Vec<SyncResultDto>>("POST", "/sync/commands", Some(body)).await
    {
        let settled = o.with_stored(|s| s.settle(&results));
        dropped.extend(settled.dropped);
    }
}

/// Is the browser offline now? Sheets and the bulk bar disable their actions then: plays,
/// sessions and bulk edits stay online-only (DEC).
#[must_use]
pub fn is_offline() -> Signal<bool> {
    let online = use_context::<crate::shell::Online>().map(|o| o.0);
    Signal::derive(move || online.is_some_and(|o| !o.get()))
}

/// `refresh` for a server event, held back while the browser is offline: the event stream can
/// outlive the network (a half-open connection; Playwright's offline mode keeps it open), and a
/// refetch then can only fail, which replaced a loaded page with its error state. A refresh
/// held back runs once when the browser is back online (DEC-486).
pub fn refresh_when_online(refresh: Callback<()>) -> Callback<()> {
    let online = use_context::<crate::shell::Online>().map(|o| o.0);
    let missed = StoredValue::new(false);
    if let Some(online) = online {
        Effect::new(move |_| {
            if online.get() && missed.get_value() {
                missed.set_value(false);
                refresh.run(());
            }
        });
    }
    Callback::new(move |()| {
        if online.is_some_and(|o| !o.get_untracked()) {
            missed.set_value(true);
        } else {
            refresh.run(());
        }
    })
}

/// Provide the queue for `username`: count it, send it on page load and whenever the browser
/// comes back online.
pub fn provide_offline(username: String, online: RwSignal<bool>) -> Offline {
    let o = Offline {
        waiting: RwSignal::new(0),
        user: StoredValue::new(username),
        flushing: RwSignal::new(false),
    };
    provide_context(o);
    let (i18n, toasts) = (crate::i18n::use_i18n(), ui::use_toasts());
    Effect::new(move |prev: Option<bool>| {
        let on = online.get();
        // the first run is the page load (browser only), later ones a change of `online`
        if on && prev != Some(true) {
            o.flush(i18n.clone(), toasts);
        } else if prev.is_none() {
            o.with_stored(|_| ());
        }
        on
    });
    o
}

/// Queue `commands` (one change the person made, e.g. a bulk episode mark) and say "Saved
/// offline, will sync" with Undo, which takes them out of the queue again (or says honestly
/// that it is being sent or was sent, UX4-13); `false` when the queue is full (a toast says so).
pub fn save_offline(
    ctx: &OfflineCtx,
    title: String,
    commands: Vec<Queued>,
    on_undo: Option<Callback<()>>,
) -> bool {
    let OfflineCtx {
        offline,
        i18n,
        toasts,
    } = ctx.clone();
    let ops: Vec<String> = commands.iter().map(|q| q.envelope.op_id.clone()).collect();
    if !offline.with_stored(|s| s.push_all(commands)) {
        toasts.push(
            i18n.t_args("offline-full", &[("count", MAX_QUEUED.into())]),
            Some(title),
            ToastVariant::Destructive,
        );
        return false;
    }
    let words = [
        i18n.t("library-undone"),
        i18n.t("offline-undo-sending"),
        i18n.t("offline-undo-sent"),
    ];
    let undo = ToastAction::new(
        i18n.t("library-undo"),
        Callback::new(move |()| {
            let flushing = offline.flushing.get_untracked();
            let outcome = offline.with_stored(|s| undo_queued(s, &ops, flushing));
            let (text, variant) = match outcome {
                UndoOutcome::Undone => (words[0].clone(), ToastVariant::Default),
                UndoOutcome::Sending => (words[1].clone(), ToastVariant::Destructive),
                UndoOutcome::Sent => (words[2].clone(), ToastVariant::Destructive),
            };
            toasts.push(text, None, variant);
            if outcome == UndoOutcome::Undone
                && let Some(cb) = on_undo
            {
                cb.run(());
            }
        }),
    );
    toasts.push_action(i18n.t("offline-saved"), Some(title), undo);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ErrorDto;

    fn q(op: &str, title: &str) -> Queued {
        Queued::with_op_id(
            op.into(),
            log_command("i1", serde_json::json!({"type": "finished"}), None, None),
            title,
            5,
        )
    }

    fn result(op: &str, error: Option<&str>) -> SyncResultDto {
        SyncResultDto {
            op_id: op.into(),
            ok: error.is_none(),
            error: error.map(|c| ErrorDto {
                code: c.into(),
                message: String::new(),
                retry_after_secs: None,
                field: None,
            }),
        }
    }

    /// Review BOOK-6: "Finished" on a book while offline queues the last page and, behind
    /// it, Completed with a later stamp; a plain log queues itself alone.
    #[test]
    fn finished_offline_queues_the_page_and_then_the_completion() {
        let mut clock = ClientClock::new(7);
        let page = serde_json::json!({"type": "paged", "page": 412});
        let log_stamp = clock.stamp(1_000);
        let log = Queued::with_op_id(
            "op-1".into(),
            log_command("i1", page, None, Some(&log_stamp)),
            "Dune",
            1_000,
        );
        let status_stamp = clock.stamp(1_000);
        let status = status_command("i1", "completed", &status_stamp);
        let change = log_then_status(log.clone(), Some(status), 1_000);
        assert_eq!(change.len(), 2);
        assert_eq!(change[0], log);
        assert_eq!(change[1].title, "Dune");
        assert_ne!(change[1].envelope.op_id, "op-1");
        match &change[1].envelope.command {
            SyncCommandDto::SetStatus {
                item_id,
                status,
                hlc,
            } => {
                assert_eq!((item_id.as_str(), status.as_str()), ("i1", "completed"));
                assert!(*hlc > log_stamp, "the status is stamped after the log");
            }
            other => panic!("{other:?}"),
        }
        // both are one change of the queue and are sent in this order
        let mut stored = Stored::default();
        assert!(stored.push_all(change));
        let sent = flush_batch(&stored.items).commands;
        assert!(matches!(
            sent[0].command,
            SyncCommandDto::LogProgress { .. }
        ));
        assert!(matches!(sent[1].command, SyncCommandDto::SetStatus { .. }));
        assert_eq!(log_then_status(log, None, 1_000).len(), 1);
    }

    /// Review OT2E-7 b: what the web queues when the start or the stop of a rewatch run
    /// meets a network failure, and when "Log N again" does (the start, then the episodes,
    /// in that order): `log_progress` commands that keep their op ids and their dates.
    #[test]
    fn a_rewatch_run_is_queued_as_logs_in_their_order() {
        let ep = serde_json::json!({"type": "episode", "season": 1, "episode": 1});
        let queue = [
            queued_log(
                "i1",
                "Dark",
                "op-1".into(),
                crate::next_step::rewatch_start(),
                None,
                None,
            ),
            queued_log("i1", "Dark", "op-2".into(), ep.clone(), None, None),
            queued_log(
                "i1",
                "Dark",
                "op-3".into(),
                crate::next_step::rewatch_stop(),
                None,
                None,
            ),
        ];
        let sent: Vec<(String, serde_json::Value)> = queue
            .iter()
            .map(|q| {
                let SyncCommandDto::LogProgress {
                    item_id, progress, ..
                } = &q.envelope.command
                else {
                    panic!("not a log: {q:?}");
                };
                assert_eq!((item_id.as_str(), q.title.as_str()), ("i1", "Dark"));
                (q.envelope.op_id.clone(), progress.clone())
            })
            .collect();
        assert_eq!(
            sent,
            [
                ("op-1".to_string(), serde_json::json!({"type": "rewatch"})),
                ("op-2".to_string(), ep.clone()),
                (
                    "op-3".to_string(),
                    serde_json::json!({"type": "rewatch_stop"})
                ),
            ]
        );
        // a backfilled date is kept as it is
        let dated = queued_log(
            "i1",
            "Dark",
            "op-4".into(),
            ep,
            Some("2026-03-09T12:00:00Z".into()),
            None,
        );
        let SyncCommandDto::LogProgress {
            occurred_at, hlc, ..
        } = dated.envelope.command
        else {
            panic!("not a log");
        };
        assert_eq!(occurred_at.as_deref(), Some("2026-03-09T12:00:00Z"));
        assert_eq!(hlc, None, "no queue, no stamp");
    }

    #[test]
    fn stamps_are_the_server_format_and_always_grow() {
        let mut c = ClientClock::new(0x2a);
        assert_eq!(c.stamp(0x10), "0000000000000010-00000000-0000002a");
        assert_eq!(c.stamp(0x10), "0000000000000010-00000001-0000002a");
        assert_eq!(c.stamp(0x0f), "0000000000000010-00000002-0000002a");
        assert_eq!(c.stamp(0x11), "0000000000000011-00000000-0000002a");
        let mut r = ClientClock::resume(0x2a, Some("0000000000000011-00000000-0000002a"));
        assert_eq!(r.stamp(0x11), "0000000000000011-00000001-0000002a");
        let mut bad = ClientClock::resume(1, Some("not a stamp"));
        assert_eq!(bad.stamp(3), "0000000000000003-00000000-00000001");
    }

    #[test]
    fn commands_keep_the_op_id_and_carry_the_stamp() {
        assert_eq!(
            status_command("i1", "paused", "h"),
            SyncCommandDto::SetStatus {
                item_id: "i1".into(),
                status: "paused".into(),
                hlc: "h".into()
            }
        );
        assert_eq!(
            rate_command("i1", Some(80), "h"),
            SyncCommandDto::Rate {
                item_id: "i1".into(),
                rating: Some(80),
                hlc: "h".into()
            }
        );
        let a = Queued::new(status_command("i1", "paused", "h"), "T", 1);
        let b = Queued::new(status_command("i1", "paused", "h"), "T", 1);
        assert_eq!(a.envelope.op_id.len(), 36, "a UUID");
        assert_ne!(a.envelope.op_id, b.envelope.op_id);
        let rated = Queued::with_op_id("op-3".into(), rate_command("i1", None, "old"), "A", 1);
        let queue = [q("op-1", "A"), rated, q("op-2", "B")];
        let sent = flush_batch(&queue);
        let ids: Vec<&str> = sent.commands.iter().map(|c| c.op_id.as_str()).collect();
        assert_eq!(
            ids,
            ["op-1", "op-3", "op-2"],
            "one push, in the order queued"
        );
        assert_eq!(
            sent.commands[1].command,
            rate_command("i1", None, "old"),
            "with the stamp from the queue"
        );
        assert_eq!(
            log_command(
                "i1",
                serde_json::json!({"type": "finished"}),
                None,
                Some("h")
            ),
            SyncCommandDto::LogProgress {
                item_id: "i1".into(),
                progress: serde_json::json!({"type": "finished"}),
                occurred_at: None,
                hlc: Some("h".into()),
            },
            "a log carries the stamp for the status it implies"
        );
    }

    #[test]
    fn undo_takes_one_command_out() {
        let mut s = Stored::default();
        s.push(q("op-1", "A"));
        s.push(q("op-2", "B"));
        s.push(q("op-1", "A"));
        assert_eq!(s.items.len(), 2, "the same op id is queued once");
        assert!(s.remove("op-1"));
        assert!(!s.remove("op-1"));
        assert_eq!(s.items, vec![q("op-2", "B")]);
    }

    #[test]
    fn a_flush_drops_refusals_and_keeps_server_trouble() {
        let mut s = Stored::default();
        for (op, t) in [("op-1", "A"), ("op-2", "B"), ("op-3", "C"), ("op-4", "D")] {
            s.push(q(op, t));
        }
        let settled = s.settle(&[
            result("op-1", None),
            result("op-2", Some("not_found")),
            result("op-3", Some("internal")),
        ]);
        assert_eq!(settled.synced, 1);
        assert_eq!(settled.dropped, vec![q("op-2", "B")], "named in a toast");
        // a server error stays for the next flush, an unanswered command too
        assert_eq!(s.items, vec![q("op-3", "C"), q("op-4", "D")]);
        assert!(retry_later("internal") && retry_later("unavailable") && retry_later("generic"));
        assert!(
            !retry_later("not_found") && !retry_later("bad_request") && !retry_later("invalid")
        );
    }

    #[test]
    fn the_queue_is_kept_per_person() {
        assert_eq!(storage_key("anna"), "app-offline-anna");
        let mut s = Stored {
            node: 9,
            ..Stored::default()
        };
        s.push(q("op-1", "A"));
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Stored>(&text).unwrap(), s);
    }

    #[test]
    fn the_queue_takes_a_whole_change_or_none_up_to_its_cap() {
        // UX4-13
        let mut s = Stored::default();
        let many: Vec<Queued> = (0..MAX_QUEUED - 1)
            .map(|i| q(&format!("op-{i}"), "A"))
            .collect();
        assert!(s.push_all(many));
        assert!(
            !s.push_all(vec![q("x-1", "B"), q("x-2", "B")]),
            "two do not fit"
        );
        assert_eq!(s.items.len(), MAX_QUEUED - 1, "nothing of it was queued");
        assert!(
            s.push_all(vec![q("op-0", "A"), q("x-1", "B")]),
            "a queued op id takes no room"
        );
        assert_eq!(s.items.len(), MAX_QUEUED);
        assert!(!s.push_all(vec![q("x-3", "C")]));
    }

    #[test]
    fn a_refusal_names_the_change() {
        let en = I18n::new("en", std::collections::BTreeMap::new());
        let status = Queued::new(status_command("i1", "paused", "h"), "Dune", 1);
        assert_eq!(command_what(&en, &status), "status Paused");
        assert_eq!(
            command_what(&en, &Queued::new(rate_command("i1", None, "h"), "Dune", 1)),
            "rating"
        );
        let watched = Queued::new(
            log_command(
                "i1",
                serde_json::json!({"type": "episode", "season": 1, "episode": 2}),
                None,
                None,
            ),
            "Dune",
            1,
        );
        assert_eq!(command_what(&en, &watched), "Season 1, episode 2");
        assert_eq!(
            en.t_args("offline-dropped", &[("what", "status Paused".into())]),
            "Not saved: status Paused - the server refused this change made offline"
        );
    }

    #[test]
    fn undo_is_refused_while_the_change_is_being_sent() {
        let mut s = Stored::default();
        s.push(q("op-1", "A"));
        let ops = vec!["op-1".to_string()];
        assert_eq!(undo_queued(&mut s, &ops, true), UndoOutcome::Sending);
        assert_eq!(s.items.len(), 1, "the push carries it: nothing taken out");
        assert_eq!(undo_queued(&mut s, &ops, false), UndoOutcome::Undone);
        assert_eq!(
            undo_queued(&mut s, &ops, false),
            UndoOutcome::Sent,
            "already gone"
        );
        assert_eq!(undo_queued(&mut s, &ops, true), UndoOutcome::Sent);
    }
}
