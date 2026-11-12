//! Server-Sent Events in the browser: one `EventSource` on `/api/v1/events` (session cookie)
//! for the whole page. Pages react to [`LastEvent`]; nothing ever polls (D-6.1).

use leptos::prelude::*;

/// Most recent event: `(topic, JSON data)`.
#[derive(Clone, Copy)]
pub struct LastEvent(pub RwSignal<Option<(String, String)>>);

/// Topics the page listens to (browser only).
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
pub const TOPICS: [&str; 6] = [
    "entity.updated",
    "search.results",
    "job.progress",
    "import.progress",
    // a new inbox row (the notifications page refetches)
    "notification.new",
    // a call attributed to one of the user's senders (Settings > Integrations, design.md 5.9)
    "integration.call",
];

/// Counts the person's own actions that happen away from the page that shows their result
/// (the palette's Ctrl+Enter over an open Library, the Undo of its toast): such a page shows
/// them at once, they are not paced like changes from elsewhere (OT1B-9).
#[derive(Clone, Copy)]
pub struct OwnAction(pub RwSignal<u64>);

impl OwnAction {
    /// One more action went through.
    pub fn bump(self) {
        // the app root owns the signal; a caller that outlived it has nobody to tell
        let _ = self.0.try_update(|n| *n += 1);
    }
}

/// Provide [`LastEvent`] and [`OwnAction`] and, in the browser, connect the event stream.
pub fn provide_events() {
    let last = LastEvent(RwSignal::new(None));
    provide_context(last);
    provide_context(OwnAction(RwSignal::new(0)));
    #[cfg(feature = "hydrate")]
    connect(last);
}

#[cfg(feature = "hydrate")]
fn connect(last: LastEvent) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Ok(es) = web_sys::EventSource::new("/api/v1/events") else {
        return;
    };
    for topic in TOPICS {
        let cb =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                let data = ev.data().as_string().unwrap_or_default();
                last.0.set(Some((topic.to_string(), data)));
            });
        let _ = es.add_event_listener_with_callback(topic, cb.as_ref().unchecked_ref());
        // the page lives as long as the event source: keep the handler alive
        cb.forget();
    }
    // keep the connection object alive for the page's lifetime
    std::mem::forget(es);
}

/// The event signal of this page.
///
/// # Panics
/// If called outside the app (the root provides it).
#[must_use]
pub fn use_events() -> LastEvent {
    expect_context::<LastEvent>()
}

/// Does an `entity.updated` event concern `id`?
#[must_use]
pub fn mentions(data: &str, id: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v["entity"]["id"].as_str().map(|s| s == id))
        .unwrap_or(false)
}

/// When a page refetches for a run of `entity.updated` events (OT-21). The first event
/// refetches at once, so one change elsewhere shows as fast as before. Events in the
/// [`Pace::QUIET_MS`] after a refetch are answered by one refetch at the end of that time: an
/// import announces every item it writes, and a page that refetched for each of them rebuilt
/// its cards several times a second.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Pace {
    /// When the last refetch ran (ms on the caller's clock).
    ran: Option<f64>,
    /// A refetch is scheduled.
    waiting: bool,
}

/// What an event asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Refetch {
    Now,
    /// In so many ms (then call [`Pace::fired`]).
    In(f64),
    /// One is scheduled already.
    Scheduled,
}

impl Pace {
    /// One refetch per this many ms while events keep coming.
    pub const QUIET_MS: f64 = 2000.0;

    /// An event arrived at `now`.
    pub fn event(&mut self, now: f64) -> Refetch {
        if self.waiting {
            return Refetch::Scheduled;
        }
        match self.ran {
            Some(ran) if now - ran < Self::QUIET_MS => {
                self.waiting = true;
                Refetch::In(Self::QUIET_MS - (now - ran).max(0.0))
            }
            _ => {
                self.ran = Some(now);
                Refetch::Now
            }
        }
    }

    /// The scheduled refetch ran at `now`.
    pub fn fired(&mut self, now: f64) {
        self.waiting = false;
        self.ran = Some(now);
    }

    /// The page refetched at `now` for the person's own action (not paced, OT1B-9). The
    /// event that announces the same action is then answered at the end of the quiet time.
    pub fn own(&mut self, now: f64) {
        self.ran = Some(now);
    }
}

/// Run `refresh` for the `entity.updated` events of this page, paced by [`Pace`], and at
/// once for the person's own actions made elsewhere ([`OwnAction`]). For pages whose every
/// row would be looked at again (Home, Library, History, People, a person, the lists); a page
/// of one entity filters the events itself. The event that was the last one when the page
/// was built is not counted: the page has just loaded, and counting it started a quiet time
/// that made the first real change late (OT1B-9).
pub fn refresh_on_updates(refresh: Callback<()>) {
    let events = use_events();
    let pace = StoredValue::new(Pace::default());
    let alive = StoredValue::new(());
    if let Some(own) = use_context::<OwnAction>() {
        Effect::new(move |seen: Option<u64>| {
            let n = own.0.get();
            if seen.is_some_and(|s| s != n) {
                pace.update_value(|p| p.own(crate::keyboard::now_ms()));
                refresh.run(());
            }
            n
        });
    }
    Effect::new(move |built: Option<()>| {
        let event = events.0.get();
        if built.is_none() {
            return;
        }
        let Some((topic, _)) = event else {
            return;
        };
        if topic != "entity.updated" {
            return;
        }
        let mut step = Refetch::Scheduled;
        pace.update_value(|p| step = p.event(crate::keyboard::now_ms()));
        match step {
            Refetch::Now => refresh.run(()),
            Refetch::In(ms) => set_timeout(
                move || {
                    // the page may be gone by then
                    if alive.try_get_value().is_some() {
                        pace.update_value(|p| p.fired(crate::keyboard::now_ms()));
                        refresh.run(());
                    }
                },
                std::time::Duration::from_secs_f64(ms / 1000.0),
            ),
            Refetch::Scheduled => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OT-21: an import announces every item; the page refetches once per quiet time, not
    /// once per item, and still at once for a single change.
    #[test]
    fn a_burst_of_events_refetches_once_per_quiet_time() {
        let mut p = Pace::default();
        assert_eq!(
            p.event(1_000.0),
            Refetch::Now,
            "the first change shows at once"
        );
        assert_eq!(p.event(1_050.0), Refetch::In(1_950.0));
        for at in [1_100.0, 2_000.0, 2_900.0] {
            assert_eq!(p.event(at), Refetch::Scheduled, "one refetch is on its way");
        }
        p.fired(3_000.0);
        // the import goes on: again one refetch for the next quiet time
        assert_eq!(p.event(3_010.0), Refetch::In(1_990.0));
        assert_eq!(p.event(4_000.0), Refetch::Scheduled);
        p.fired(5_000.0);
        // long after the burst a change shows at once again
        assert_eq!(p.event(10_000.0), Refetch::Now);
        assert_eq!(p.event(12_000.0), Refetch::Now, "the quiet time is over");
    }

    /// OT1B-9: the page refetched for the person's own action; the event that announces
    /// the same action does not refetch at once again, it is answered once at the end of
    /// the quiet time, and nothing is lost.
    #[test]
    fn an_own_action_counts_as_a_refetch() {
        let mut p = Pace::default();
        p.own(1_000.0);
        assert_eq!(p.event(1_040.0), Refetch::In(1_960.0));
        assert_eq!(p.event(1_500.0), Refetch::Scheduled);
        // a second own action inside the quiet time: the scheduled refetch still comes
        p.own(2_000.0);
        assert_eq!(p.event(2_010.0), Refetch::Scheduled);
        p.fired(3_000.0);
        assert_eq!(p.event(6_000.0), Refetch::Now);
    }
}
