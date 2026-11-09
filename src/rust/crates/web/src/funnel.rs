//! The logging funnel (WP4-5, design.md 9 "Instrumentation", research R-54, addendum-1 D16).
//! The browser follows a flow from its intent (palette opened, next step clicked, sheet
//! opened, episode marked, first row selected for a bulk action) to `logged` or `abandoned`,
//! counting taps (clicks while the flow runs, the intent included) and the time it took, and
//! keeps only fixed buckets of both. The disclosures of `ui` add how often each section
//! (`page/section` of its storage key) was shown, opened, opened within 2 s and open when the
//! page saved. Finished records go out together in one beacon to `POST /usage/funnel` (signed
//! in only, never an item id, a title or a name), at most every few seconds and when the page
//! is hidden; the server keeps daily totals without any user (DEC).
//!
//! A second intent while a flow runs belongs to that flow (palette -> item page -> next
//! step is one palette flow); a flow left for two minutes is abandoned.

use contracts::usage::{
    DISCLOSURE_FLOW, DISCLOSURE_SUBJECTS, FunnelEventDto, MAX_BEACON_EVENTS, UsageBeaconDto,
};
use leptos::prelude::*;

/// The flows (`contracts::usage::FUNNEL_FLOWS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Palette,
    NextStep,
    Sheet,
    EpisodeMark,
    Bulk,
}

impl Flow {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Palette => "palette",
            Self::NextStep => "next_step",
            Self::Sheet => "sheet",
            Self::EpisodeMark => "episode_mark",
            Self::Bulk => "bulk",
        }
    }
}

/// How a flow ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Logged,
    Abandoned,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Logged => "logged",
            Self::Abandoned => "abandoned",
        }
    }
}

/// A flow untouched this long is over (abandoned).
pub const FLOW_EXPIRY_MS: u64 = 120_000;

/// `1` to `4`, then `5+` (the intent is the first tap, so there is no `0`).
#[must_use]
pub fn taps_bucket(taps: u32) -> &'static str {
    match taps {
        0 | 1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        _ => "5+",
    }
}

/// Under 2, 5, 10, 30, 60 s, then `60s+`.
#[must_use]
pub fn time_bucket(ms: u64) -> &'static str {
    match ms {
        0..2_000 => "lt2s",
        2_000..5_000 => "lt5s",
        5_000..10_000 => "lt10s",
        10_000..30_000 => "lt30s",
        30_000..60_000 => "lt60s",
        _ => "60s+",
    }
}

/// `page/section` of a disclosure storage key (`app-disclosure:item:log-more` ->
/// `item/log-more`); `None` for any other key and for a part that is not plain lower-case
/// words (the server refuses those too, so an id can never become a subject).
#[must_use]
pub fn subject_of(storage_key: &str) -> Option<String> {
    let rest = storage_key.strip_prefix("app-disclosure:")?;
    let (page, section) = rest.split_once(':')?;
    let word = |w: &str| {
        (1..=32).contains(&w.len())
            && w.starts_with(|c: char| c.is_ascii_lowercase())
            && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
    };
    let subject = format!("{page}/{section}");
    // only the sections the server counts (UX4-14); the dev gallery's are not
    (word(page) && word(section) && DISCLOSURE_SUBJECTS.contains(&subject.as_str()))
        .then_some(subject)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Active {
    flow: Flow,
    started_ms: u64,
    last_ms: u64,
    taps: u32,
}

/// The running flow and the records waiting for the next beacon.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tracker {
    active: Option<Active>,
    pending: Vec<FunnelEventDto>,
}

impl Tracker {
    /// An intent: starts `flow`, or counts as a tap of the flow already running.
    pub fn intent(&mut self, flow: Flow, now_ms: u64) {
        self.expire(now_ms);
        match &mut self.active {
            Some(a) => {
                a.taps = a.taps.saturating_add(1);
                a.last_ms = now_ms;
            }
            None => {
                self.active = Some(Active {
                    flow,
                    started_ms: now_ms,
                    last_ms: now_ms,
                    taps: 1,
                });
            }
        }
    }

    /// A click while a flow runs.
    pub fn tap(&mut self) {
        if let Some(a) = &mut self.active {
            a.taps = a.taps.saturating_add(1);
        }
    }

    /// End the running flow (nothing when none runs).
    pub fn end(&mut self, outcome: Outcome, now_ms: u64) {
        self.expire(now_ms);
        if let Some(a) = self.active.take() {
            self.record(a, outcome, now_ms.saturating_sub(a.started_ms));
        }
    }

    /// A flow left for [`FLOW_EXPIRY_MS`] is abandoned.
    pub fn expire(&mut self, now_ms: u64) {
        if let Some(a) = self.active
            && now_ms.saturating_sub(a.last_ms) >= FLOW_EXPIRY_MS
        {
            self.active = None;
            self.record(a, Outcome::Abandoned, now_ms.saturating_sub(a.started_ms));
        }
    }

    fn record(&mut self, a: Active, outcome: Outcome, ms: u64) {
        self.pending.push(FunnelEventDto {
            flow: a.flow.as_str().into(),
            outcome: outcome.as_str().into(),
            subject: None,
            taps: Some(taps_bucket(a.taps).into()),
            time: Some(time_bucket(ms).into()),
        });
    }

    /// A disclosure number (D16) for `subject` (`page/section`).
    pub fn disclosure(&mut self, subject: &str, outcome: &str) {
        self.pending.push(FunnelEventDto {
            flow: DISCLOSURE_FLOW.into(),
            outcome: outcome.into(),
            subject: Some(subject.into()),
            taps: None,
            time: None,
        });
    }

    /// The running flow.
    #[must_use]
    pub fn active(&self) -> Option<Flow> {
        self.active.map(|a| a.flow)
    }

    /// Are records waiting?
    #[must_use]
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The records for one beacon (at most `MAX_BEACON_EVENTS`, oldest first).
    pub fn take(&mut self) -> Vec<FunnelEventDto> {
        let n = self.pending.len().min(MAX_BEACON_EVENTS);
        self.pending.drain(..n).collect()
    }
}

// ---------------------------------------------------------------- in the browser

/// A beacon goes out this long after the last finished record (or when the page is hidden).
pub const BEACON_DELAY_MS: u64 = 5_000;

/// The funnel in context: the shell provides it; disabled (every call a no-op) when the
/// instance does not count (`usage.funnel` off) or nobody is signed in.
#[derive(Clone, Copy)]
pub struct Funnel {
    tracker: StoredValue<Tracker>,
    enabled: bool,
    scheduled: StoredValue<bool>,
    watch: Option<ui::DisclosureWatch>,
}

fn clock_ms() -> u64 {
    crate::offline::now_ms()
}

impl Funnel {
    /// The funnel of this page (a disabled one outside the shell).
    #[must_use]
    pub fn current() -> Self {
        use_context::<Self>().unwrap_or(Self {
            tracker: StoredValue::new(Tracker::default()),
            enabled: false,
            scheduled: StoredValue::new(false),
            watch: None,
        })
    }

    pub fn intent(self, flow: Flow) {
        if self.enabled {
            self.tracker
                .try_update_value(|t| t.intent(flow, clock_ms()));
            self.schedule();
        }
    }

    /// The running flow ended with a saved log.
    pub fn logged(self) {
        self.finish(None, Outcome::Logged);
    }

    /// End the running flow as logged when it is `flow` (a bulk edit).
    pub fn logged_if(self, flow: Flow) {
        self.finish(Some(flow), Outcome::Logged);
    }

    /// End the running flow as abandoned when it is `flow` (a sheet closed unsaved, the
    /// palette closed without a choice).
    pub fn abandoned_if(self, flow: Flow) {
        self.finish(Some(flow), Outcome::Abandoned);
    }

    fn finish(self, only: Option<Flow>, outcome: Outcome) {
        if !self.enabled {
            return;
        }
        self.tracker.try_update_value(|t| {
            if only.is_none_or(|f| t.active() == Some(f)) {
                t.end(outcome, clock_ms());
            }
        });
        self.schedule();
    }

    /// A page saved: every remembered disclosure open now counts as open at a save (D16).
    pub fn saved(self) {
        if let (true, Some(w)) = (self.enabled, self.watch) {
            for key in w.open_keys() {
                if let Some(subject) = subject_of(&key) {
                    self.tracker
                        .try_update_value(|t| t.disclosure(&subject, "open_at_save"));
                }
            }
            self.schedule();
        }
    }

    // only the browser build counts clicks (`listen`)
    #[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
    fn tap(self) {
        if self.enabled {
            self.tracker.try_update_value(Tracker::tap);
        }
    }

    fn disclosure(self, key: &str, e: ui::DisclosureEvent) {
        let Some(subject) = subject_of(key) else {
            return;
        };
        let outcome = match e {
            ui::DisclosureEvent::Shown => "shown",
            ui::DisclosureEvent::Opened => "opened",
            ui::DisclosureEvent::OpenedEarly => "opened_early",
        };
        self.tracker
            .try_update_value(|t| t.disclosure(&subject, outcome));
        self.schedule();
    }

    /// Send what waits in [`BEACON_DELAY_MS`] (once).
    fn schedule(self) {
        if self.scheduled.try_get_value().unwrap_or(true)
            || !self
                .tracker
                .try_with_value(Tracker::has_pending)
                .unwrap_or(false)
        {
            return;
        }
        self.scheduled.try_set_value(true);
        set_timeout(
            move || {
                self.scheduled.try_set_value(false);
                self.send();
            },
            std::time::Duration::from_millis(BEACON_DELAY_MS),
        );
    }

    /// One beacon with what waits (also when the page is hidden; a flow left for two minutes
    /// is abandoned first).
    pub fn send(self) {
        if !self.enabled {
            return;
        }
        let events = self.tracker.try_update_value(|t| {
            t.expire(clock_ms());
            t.take()
        });
        if let Some(events) = events.filter(|e| !e.is_empty()) {
            beacon(&UsageBeaconDto { events });
        }
        if self
            .tracker
            .try_with_value(Tracker::has_pending)
            .unwrap_or(false)
        {
            self.schedule();
        }
    }
}

/// `POST /api/v1/usage/funnel` with `keepalive`, so it also leaves a closing page; the answer
/// is not read.
fn beacon(b: &UsageBeaconDto) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsValue;
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_credentials(web_sys::RequestCredentials::SameOrigin);
        // `keepalive` (Fetch standard) has no setter in web-sys 0.3.107
        let _ = js_sys::Reflect::set(&init, &JsValue::from_str("keepalive"), &JsValue::TRUE);
        let Ok(headers) = web_sys::Headers::new() else {
            return;
        };
        let _ = headers.set("content-type", "application/json");
        init.set_headers(&headers);
        init.set_body(&JsValue::from_str(
            &serde_json::to_string(b).unwrap_or_default(),
        ));
        if let (Some(w), Ok(req)) = (
            web_sys::window(),
            web_sys::Request::new_with_str_and_init("/api/v1/usage/funnel", &init),
        ) {
            // the request starts now; its promise is not awaited
            let _ = w.fetch_with_request(&req);
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = b;
}

/// Count clicks while a flow runs (capture phase, so the intent's own click is not counted
/// twice) and send the beacon when the page is hidden.
#[cfg(feature = "hydrate")]
fn listen(f: Funnel) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let tap = Closure::<dyn Fn()>::new(move || f.tap());
    let _ =
        doc.add_event_listener_with_callback_and_bool("click", tap.as_ref().unchecked_ref(), true);
    let hidden = Closure::<dyn Fn()>::new(move || {
        let hidden = web_sys::window()
            .and_then(|w| w.document())
            .is_some_and(|d| d.visibility_state() == web_sys::VisibilityState::Hidden);
        if hidden {
            f.send();
        }
    });
    let _ =
        doc.add_event_listener_with_callback("visibilitychange", hidden.as_ref().unchecked_ref());
    // the shell lives as long as the page: the listeners stay
    tap.forget();
    hidden.forget();
}

/// A sheet as a funnel flow: opening it is the intent, closing it unsaved abandons it (a
/// save ends it as logged first: [`Funnel::saved`], [`Funnel::logged`]).
pub fn sheet_flow(open: RwSignal<bool>) {
    let funnel = Funnel::current();
    Effect::new(move |prev: Option<bool>| {
        let now = open.get();
        if now && prev != Some(true) {
            funnel.intent(Flow::Sheet);
        } else if !now && prev == Some(true) {
            funnel.abandoned_if(Flow::Sheet);
        }
        now
    });
}

/// Provide the funnel (and the disclosure watcher) for the shell.
pub fn provide_funnel(enabled: bool) -> Funnel {
    let tracker = StoredValue::new(Tracker::default());
    let scheduled = StoredValue::new(false);
    let mut f = Funnel {
        tracker,
        enabled,
        scheduled,
        watch: None,
    };
    if enabled {
        let report = Callback::new(move |(key, e): (String, ui::DisclosureEvent)| {
            Funnel {
                tracker,
                enabled,
                scheduled,
                watch: None,
            }
            .disclosure(&key, e);
        });
        let watch = ui::DisclosureWatch::new(report);
        provide_context(watch);
        f.watch = Some(watch);
        #[cfg(feature = "hydrate")]
        Effect::new(move |prev: Option<()>| {
            if prev.is_none() {
                listen(f);
            }
        });
    }
    provide_context(f);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(flow: &str, outcome: &str, taps: &str, time: &str) -> FunnelEventDto {
        FunnelEventDto {
            flow: flow.into(),
            outcome: outcome.into(),
            subject: None,
            taps: Some(taps.into()),
            time: Some(time.into()),
        }
    }

    #[test]
    fn taps_and_time_fall_into_the_fixed_buckets() {
        let taps: Vec<&str> = [0, 1, 2, 3, 4, 5, 9].into_iter().map(taps_bucket).collect();
        assert_eq!(taps, ["1", "1", "2", "3", "4", "5+", "5+"]);
        let times: Vec<&str> = [
            0, 1_999, 2_000, 4_999, 5_000, 9_999, 10_000, 29_999, 30_000, 59_999, 60_000, 600_000,
        ]
        .into_iter()
        .map(time_bucket)
        .collect();
        assert_eq!(
            times,
            [
                "lt2s", "lt2s", "lt5s", "lt5s", "lt10s", "lt10s", "lt30s", "lt30s", "lt60s",
                "lt60s", "60s+", "60s+"
            ]
        );
        for b in taps {
            assert!(contracts::usage::TAPS_BUCKETS.contains(&b));
        }
        for b in times {
            assert!(contracts::usage::TIME_BUCKETS.contains(&b));
        }
        for f in [
            Flow::Palette,
            Flow::NextStep,
            Flow::Sheet,
            Flow::EpisodeMark,
            Flow::Bulk,
        ] {
            assert!(
                contracts::usage::FUNNEL_FLOWS.contains(&f.as_str()),
                "{f:?}"
            );
        }
    }

    #[test]
    fn a_flow_runs_from_the_intent_to_its_end() {
        let mut t = Tracker::default();
        t.intent(Flow::NextStep, 1_000);
        t.end(Outcome::Logged, 1_400);
        t.intent(Flow::Sheet, 2_000);
        t.tap();
        t.tap();
        t.end(Outcome::Abandoned, 9_000);
        // an end without a flow counts nothing
        t.end(Outcome::Logged, 9_500);
        assert_eq!(
            t.take(),
            vec![
                ev("next_step", "logged", "1", "lt2s"),
                ev("sheet", "abandoned", "3", "lt10s")
            ]
        );
        assert_eq!(t.take().len(), 0);
    }

    #[test]
    fn a_second_intent_continues_the_first_flow() {
        // palette open, result chosen, then the next step on the item page: one palette flow
        let mut t = Tracker::default();
        t.intent(Flow::Palette, 0);
        t.intent(Flow::NextStep, 3_000);
        assert_eq!(t.active(), Some(Flow::Palette));
        t.end(Outcome::Logged, 3_500);
        assert_eq!(t.take(), vec![ev("palette", "logged", "2", "lt5s")]);
        // after two minutes a flow is over: abandoned, and the new intent starts its own
        t.intent(Flow::Palette, 10_000);
        t.intent(Flow::Bulk, 140_000);
        assert_eq!(t.active(), Some(Flow::Bulk));
        assert_eq!(t.take(), vec![ev("palette", "abandoned", "1", "60s+")]);
    }

    #[test]
    fn disclosures_report_by_page_and_section_only() {
        assert_eq!(
            subject_of("app-disclosure:item:log-more").as_deref(),
            Some("item/log-more")
        );
        assert_eq!(subject_of("app-theme"), None);
        assert_eq!(
            subject_of("app-disclosure:item:0190a1b2-c3d4"),
            None,
            "never an id"
        );
        let mut t = Tracker::default();
        t.disclosure("item/log-more", "open_at_save");
        assert_eq!(
            t.take(),
            vec![FunnelEventDto {
                flow: "disclosure".into(),
                outcome: "open_at_save".into(),
                subject: Some("item/log-more".into()),
                taps: None,
                time: None,
            }]
        );
        // a beacon carries at most 50 events; the rest wait for the next one
        for _ in 0..60 {
            t.disclosure("item/log-more", "shown");
        }
        assert_eq!(t.take().len(), 50);
        assert_eq!(t.take().len(), 10);
    }

    /// Every storage key call with two literal words in the pages, read from the sources.
    fn keys_in(dir: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                keys_in(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                for call in text.split("disclosure_storage_key(\"").skip(1) {
                    let mut parts = call.split('"');
                    let page = parts.next().unwrap_or_default();
                    let section = parts.nth(1).unwrap_or_default();
                    out.push(format!("{page}/{section}"));
                }
            }
        }
    }

    #[test]
    fn every_measured_section_is_on_the_servers_list() {
        // UX4-14: the server takes the listed subjects only; a section missing there would
        // refuse the whole beacon, so the browser sends only listed ones
        let mut used = Vec::new();
        keys_in(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut used,
        );
        used.retain(|k| !k.starts_with("gallery/"));
        used.sort();
        used.dedup();
        let mut listed: Vec<String> = DISCLOSURE_SUBJECTS
            .iter()
            .map(ToString::to_string)
            .collect();
        listed.sort();
        assert_eq!(
            used, listed,
            "the pages' sections and contracts::usage::DISCLOSURE_SUBJECTS"
        );
        assert_eq!(
            subject_of("app-disclosure:gallery:mail"),
            None,
            "the dev gallery is not measured"
        );
        assert_eq!(subject_of("app-disclosure:made:up"), None);
    }
}
