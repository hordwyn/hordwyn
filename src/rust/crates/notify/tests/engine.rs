//! M13 acceptance: dedupe (the same episode released twice -> one notification), digests on a
//! fixed clock, quiet hours, rate limits, instance defaults, retries and gone targets.

use std::sync::{Arc, Mutex};

use app::ports::{Audience, Clock, JobQueue, Notification, Notifications, Subject};
use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveTime, Utc};
use domain::ids::UserId;
use notify::model::{
    Channel, ChannelConfig, DeliveryMode, DeliveryState, Filter, QuietHours, Subscription, Targets,
};
use notify::{
    DELIVER_JOB, DELIVERY_TOPIC, DIGEST_JOB, Deliverer, Engine, MemNotifyStore, Message, Notifier,
    NotifyStore, SendError,
};
use testkit::{FixedClock, MemJobs, RecordingEvents, SeqIds};

/// Records messages; answers with the queued results first, then `Ok`.
#[derive(Default)]
struct FakeChannel {
    sent: Mutex<Vec<Message>>,
    answers: Mutex<Vec<Result<(), SendError>>>,
    /// Sending takes a while (another run can start meanwhile).
    slow: std::sync::atomic::AtomicBool,
}

#[async_trait]
impl Notifier for FakeChannel {
    async fn send(&self, _c: &ChannelConfig, m: &Message) -> Result<(), SendError> {
        self.sent.lock().unwrap().push(m.clone());
        if self.slow.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let mut a = self.answers.lock().unwrap();
        if a.is_empty() { Ok(()) } else { a.remove(0) }
    }
}

struct Env {
    store: Arc<MemNotifyStore>,
    jobs: Arc<MemJobs>,
    clock: Arc<FixedClock>,
    events: Arc<RecordingEvents>,
    engine: Arc<Engine>,
    deliverer: Deliverer,
    channel: Arc<FakeChannel>,
    user: UserId,
}

fn env() -> Env {
    let store = Arc::new(MemNotifyStore::default());
    let jobs = Arc::new(MemJobs::default());
    let clock = Arc::new(FixedClock::default());
    let events = Arc::new(RecordingEvents::default());
    let engine = Arc::new(Engine::new(
        store.clone(),
        jobs.clone(),
        events.clone(),
        clock.clone(),
        Arc::new(SeqIds::default()),
        "Shelf",
        "en",
        notify::template::Overrides::new(),
    ));
    let channel = Arc::new(FakeChannel::default());
    let deliverer = Deliverer::new(
        engine.clone(),
        channel.clone(),
        clock.clone(),
        "https://shelf.example.org",
        3,
    );
    Env {
        store,
        jobs,
        clock,
        events,
        engine,
        deliverer,
        channel,
        user: UserId(uuid::Uuid::from_u128(7)),
    }
}

impl Env {
    async fn channel(&self, name: &str) -> uuid::Uuid {
        let c = Channel {
            id: uuid::Uuid::now_v7(),
            user: self.user,
            name: name.into(),
            config: ChannelConfig::Ntfy {
                server: "https://ntfy.example.org".into(),
                topic: name.into(),
                token: None,
            },
            enabled: true,
            created_at: self.clock.now(),
        };
        self.store.put_channel(&c).await.unwrap();
        c.id
    }

    async fn subscribe(&self, event: &str, mode: DeliveryMode, f: impl FnOnce(&mut Subscription)) {
        let mut s = Subscription {
            id: uuid::Uuid::now_v7(),
            user: Some(self.user),
            event: event.into(),
            filter: Filter::default(),
            targets: Targets::All,
            mode,
            quiet_hours: None,
            timezone: "Europe/Berlin".into(),
            rate_limit_per_hour: None,
            enabled: true,
            locked: false,
        };
        f(&mut s);
        self.store.put_subscription(&s).await.unwrap();
    }

    fn episode(&self, n: u32) -> Notification {
        Notification::new(
            "release.episode",
            Audience::Users(vec![self.user]),
            format!("episode:show-1:1x{n:02}"),
        )
        .arg("title", "Severance")
        .arg("season", 1)
        .arg("episode", n)
        .subject(Subject {
            kind: Some("tv".into()),
            ..Subject::default()
        })
        .link("/items/show-1")
    }

    /// Run every job that is due now, like a worker; later jobs stay queued.
    async fn run_due(&self) -> Vec<Result<(), String>> {
        let now = self.clock.now();
        let mut results = vec![];
        for job in self.jobs.drain() {
            if job.run_at.is_some_and(|t| t > now) {
                self.jobs.enqueue(job).await.unwrap();
                continue;
            }
            let r = match job.kind.as_str() {
                DELIVER_JOB => {
                    let id = job.payload["delivery_id"]
                        .as_str()
                        .unwrap()
                        .parse()
                        .unwrap();
                    self.deliverer.deliver(id).await
                }
                DIGEST_JOB => {
                    let id = job.payload["channel_id"].as_str().unwrap().parse().unwrap();
                    let until: DateTime<Utc> =
                        serde_json::from_value(job.payload["until"].clone()).unwrap();
                    self.deliverer.digest(id, until).await
                }
                k => panic!("unexpected job {k}"),
            };
            results.push(r);
        }
        results
    }

    fn sent(&self) -> Vec<Message> {
        self.channel.sent.lock().unwrap().clone()
    }
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

#[tokio::test]
async fn the_same_episode_released_twice_is_one_notification() {
    let e = env();
    e.channel("phone").await;
    e.subscribe("release.*", DeliveryMode::Immediate, |_| {})
        .await;
    e.engine.emit(e.episode(2)).await;
    e.engine.emit(e.episode(2)).await;
    e.run_due().await;
    let inbox = e.store.inbox(e.user, 10).await.unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].title, "New episode of Severance");
    assert_eq!(e.store.count_unread(e.user).await.unwrap(), 1);
    let sent = e.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].body, "Season 1, episode 2 of Severance is out.");
    assert_eq!(
        sent[0].link.as_deref(),
        Some("https://shelf.example.org/items/show-1")
    );
    // a different episode is a new notification
    e.engine.emit(e.episode(3)).await;
    e.run_due().await;
    assert_eq!(e.sent().len(), 2);
    let log = e.store.deliveries(e.user, 10).await.unwrap();
    assert!(log.iter().all(|d| d.state == DeliveryState::Sent));
}

#[tokio::test]
async fn digest_collects_until_the_schedule_on_a_fixed_clock() {
    let e = env();
    e.clock.set(at("2026-01-10T10:00:00Z"));
    let ch = e.channel("mail").await;
    // every morning at 08:00 Berlin time = 07:00 UTC in January
    e.subscribe(
        "release.episode",
        DeliveryMode::Digest {
            cron: "0 8 * * *".into(),
        },
        |_| {},
    )
    .await;
    for n in 1..=3 {
        e.engine.emit(e.episode(n)).await;
        e.clock.advance(Duration::hours(1));
    }
    // one digest job for the channel, due at the next 08:00
    let jobs = e.jobs.jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].run_at, Some(at("2026-01-11T07:00:00Z")));
    assert_eq!(e.run_due().await.len(), 0, "nothing due yet");
    assert_eq!(e.sent().len(), 0);

    e.clock.set(at("2026-01-11T07:00:00Z"));
    assert_eq!(e.run_due().await, vec![Ok(())]);
    let sent = e.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].title, "3 notifications");
    assert_eq!(sent[0].body.lines().count(), 3);
    assert!(
        sent[0]
            .body
            .starts_with("- New episode of Severance: Season 1, episode 1")
    );
    let now = e.clock.now();
    assert_eq!(
        e.store
            .claim_digest(ch, now, now, now - Duration::minutes(15))
            .await
            .unwrap()
            .len(),
        0
    );
    // the next notification starts the next digest
    e.engine.emit(e.episode(4)).await;
    assert_eq!(e.jobs.jobs()[0].run_at, Some(at("2026-01-12T07:00:00Z")));
}

#[tokio::test]
async fn quiet_hours_delay_immediate_deliveries() {
    let e = env();
    // 23:30 in Berlin
    e.clock.set(at("2026-01-10T22:30:00Z"));
    e.channel("phone").await;
    e.subscribe("release.*", DeliveryMode::Immediate, |s| {
        s.quiet_hours = Some(QuietHours {
            start: NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        });
    })
    .await;
    e.engine.emit(e.episode(1)).await;
    assert_eq!(e.jobs.jobs()[0].run_at, Some(at("2026-01-11T06:00:00Z")));
    e.run_due().await;
    assert_eq!(e.sent().len(), 0);
    // but the inbox has it right away
    assert_eq!(e.store.inbox(e.user, 10).await.unwrap().len(), 1);
    e.clock.set(at("2026-01-11T06:00:00Z"));
    e.run_due().await;
    assert_eq!(e.sent().len(), 1);
}

#[tokio::test]
async fn rate_limits_suppress_but_keep_the_inbox() {
    let e = env();
    e.channel("phone").await;
    e.subscribe("release.*", DeliveryMode::Immediate, |s| {
        s.rate_limit_per_hour = Some(2);
    })
    .await;
    for n in 1..=4 {
        e.engine.emit(e.episode(n)).await;
    }
    e.run_due().await;
    assert_eq!(e.sent().len(), 2);
    let log = e.store.deliveries(e.user, 10).await.unwrap();
    assert_eq!(
        log.iter()
            .filter(|d| d.state == DeliveryState::Suppressed)
            .count(),
        2
    );
    assert_eq!(e.store.inbox(e.user, 10).await.unwrap().len(), 4);
    // an hour later the limit resets
    e.clock.advance(Duration::minutes(61));
    e.engine.emit(e.episode(5)).await;
    e.run_due().await;
    assert_eq!(e.sent().len(), 3);
}

#[tokio::test]
async fn filters_targets_and_instance_defaults() {
    let e = env();
    let phone = e.channel("phone").await;
    let _mail = e.channel("mail").await;
    // only TV releases, only to the phone
    e.subscribe("release.*", DeliveryMode::Immediate, |s| {
        s.filter.kinds = vec!["tv".into()];
        s.targets = Targets::Only(vec![phone]);
    })
    .await;
    // an admin default for social events to every channel, muted by the user
    let mut default = Subscription {
        id: uuid::Uuid::now_v7(),
        user: None,
        event: "social.*".into(),
        filter: Filter::default(),
        targets: Targets::All,
        mode: DeliveryMode::Immediate,
        quiet_hours: None,
        timezone: "UTC".into(),
        rate_limit_per_hour: None,
        enabled: true,
        locked: false,
    };
    e.store.put_subscription(&default).await.unwrap();
    e.subscribe("social.*", DeliveryMode::Immediate, |s| s.enabled = false)
        .await;

    let mut book = e.episode(1);
    book.subject.kind = Some("book".into());
    book.dedupe_key = "book".into();
    e.engine.emit(book).await;
    e.engine.emit(e.episode(1)).await;
    let social = Notification::new("social.link_request", Audience::Users(vec![e.user]), "r1")
        .arg("from", "Anna")
        .arg("companion", "B.");
    e.engine.emit(social.clone()).await;
    e.run_due().await;
    let log = e.store.deliveries(e.user, 10).await.unwrap();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(log[0].channel, phone);
    assert_eq!(e.store.inbox(e.user, 10).await.unwrap().len(), 3);

    // a locked default cannot be muted
    default.locked = true;
    e.store.put_subscription(&default).await.unwrap();
    let mut again = social;
    again.dedupe_key = "r2".into();
    e.engine.emit(again).await;
    e.run_due().await;
    assert_eq!(e.store.deliveries(e.user, 10).await.unwrap().len(), 3);
}

#[tokio::test]
async fn admins_receive_admin_events() {
    let e = env();
    e.store.add_admin(e.user);
    e.engine
        .emit(
            Notification::new("admin.signup", Audience::Admins, "user:1")
                .arg("username", "carol")
                .arg("name", "Carol"),
        )
        .await;
    let inbox = e.store.inbox(e.user, 10).await.unwrap();
    assert_eq!(inbox[0].title, "New account: carol");
    assert_eq!(inbox[0].body, "Carol (carol) created an account on Shelf.");
}

#[tokio::test]
async fn temporary_failures_retry_and_gone_targets_disable_the_channel() {
    let e = env();
    let ch = e.channel("phone").await;
    e.subscribe("release.*", DeliveryMode::Immediate, |_| {})
        .await;
    e.channel
        .answers
        .lock()
        .unwrap()
        .extend([Err(SendError::Retry("503".into())), Ok(())]);
    e.engine.emit(e.episode(1)).await;
    let job = e.jobs.jobs()[0].clone();
    let id = job.payload["delivery_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(e.deliverer.deliver(id).await.is_err(), "the job retries");
    assert_eq!(
        e.store.delivery(id).await.unwrap().unwrap().state,
        DeliveryState::Queued
    );
    e.deliverer.deliver(id).await.unwrap();
    let d = e.store.delivery(id).await.unwrap().unwrap();
    assert_eq!((d.state, d.attempts), (DeliveryState::Sent, 2));
    // a retried job after success sends nothing again
    e.deliverer.deliver(id).await.unwrap();
    assert_eq!(e.sent().len(), 2);

    e.channel
        .answers
        .lock()
        .unwrap()
        .push(Err(SendError::Gone("410".into())));
    e.engine.emit(e.episode(2)).await;
    e.run_due().await;
    assert!(!e.store.channel(ch).await.unwrap().unwrap().enabled);
    let log = e.store.deliveries(e.user, 1).await.unwrap();
    assert_eq!(log[0].state, DeliveryState::Failed);
    // disabled channels get no new deliveries
    e.engine.emit(e.episode(3)).await;
    assert_eq!(e.store.deliveries(e.user, 10).await.unwrap().len(), 2);
}

/// Per-user language (DEC-201): texts follow the recipient's choice, else the instance
/// language; numbers in texts follow the locale.
#[tokio::test]
async fn texts_follow_the_recipients_language() {
    let e = env();
    let ch = e.channel("phone").await;
    e.subscribe("release.*", DeliveryMode::Immediate, |_| {})
        .await;
    e.store.set_locale(e.user, "de");
    e.engine.emit(e.episode(2)).await;
    e.run_due().await;
    let inbox = e.store.inbox(e.user, 10).await.unwrap();
    assert_eq!(inbox[0].title, "Neue Folge von Severance");
    assert_eq!(e.sent()[0].title, "Neue Folge von Severance");
    let c = e.store.channel(ch).await.unwrap().unwrap();
    e.deliverer.test(&c).await.unwrap();
    assert!(
        e.sent()[1].title.starts_with("Test von"),
        "{:?}",
        e.sent()[1]
    );
    // another user without a choice gets the instance language
    let other = UserId(uuid::Uuid::from_u128(8));
    e.engine
        .emit(
            Notification::new("import.finished", Audience::Users(vec![other]), "i:1")
                .arg("imported", 1200)
                .arg("errors", 0),
        )
        .await;
    let inbox = e.store.inbox(other, 10).await.unwrap();
    assert_eq!(
        inbox[0].body,
        "1,200 entries imported, 0 rows with problems."
    );
}

/// What the outcome of a delivery changes on an open page (its log row, the health of its
/// channel) is announced to the channel's owner when it is written: every attempt of a single
/// delivery, a digest once for all it sent.
#[tokio::test]
async fn a_written_outcome_is_announced_to_the_channels_owner() {
    let e = env();
    let ch = e.channel("phone").await;
    e.subscribe("release.*", DeliveryMode::Immediate, |_| {})
        .await;
    e.channel.answers.lock().unwrap().extend([
        Err(SendError::Retry("503".into())),
        Err(SendError::Permanent("refused".into())),
    ]);
    e.engine.emit(e.episode(1)).await;
    let announced = || {
        e.events
            .all()
            .into_iter()
            .filter(|ev| ev.topic == DELIVERY_TOPIC)
            .collect::<Vec<_>>()
    };
    assert_eq!(announced().len(), 0, "nothing was tried yet");
    let id: uuid::Uuid = e.jobs.jobs()[0].payload["delivery_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(e.deliverer.deliver(id).await.is_err(), "the job retries");
    assert_eq!(announced().len(), 1, "the failed attempt shows in the log");
    e.deliverer.deliver(id).await.unwrap();
    let seen = announced();
    assert_eq!(seen.len(), 2, "failed for good");
    assert_eq!(seen[1].audience, vec![e.user]);
    assert_eq!(seen[1].payload["channel_id"], ch.to_string());
    assert_eq!(
        e.store.delivery(id).await.unwrap().unwrap().state,
        DeliveryState::Failed
    );
    // already settled: nothing is written, nothing announced
    e.deliverer.deliver(id).await.unwrap();
    assert_eq!(announced().len(), 2);

    // a digest of three is one announcement
    let digest = env();
    digest.channel("mail").await;
    digest
        .subscribe(
            "release.episode",
            DeliveryMode::Digest {
                cron: "0 8 * * *".into(),
            },
            |_| {},
        )
        .await;
    for n in 1..=3 {
        digest.engine.emit(digest.episode(n)).await;
    }
    digest.clock.advance(Duration::days(2));
    digest.run_due().await;
    assert_eq!(digest.sent().len(), 1, "one digest message");
    let topics = digest.events.topics();
    assert_eq!(
        topics.iter().filter(|t| *t == DELIVERY_TOPIC).count(),
        1,
        "{topics:?}"
    );
}

/// OT-37 (#6): two runs of one delivery (two workers, or a lease that lapsed) send it once;
/// the one that finds it claimed sends nothing. A claim whose run died is taken over after
/// `SENDING_STALE_MINUTES`.
#[tokio::test]
async fn two_runs_of_a_delivery_send_it_once() {
    let e = env();
    e.channel("phone").await;
    e.subscribe("release.*", DeliveryMode::Immediate, |_| {})
        .await;
    e.channel
        .slow
        .store(true, std::sync::atomic::Ordering::SeqCst);
    e.engine.emit(e.episode(1)).await;
    let job = e.jobs.jobs()[0].clone();
    let id: uuid::Uuid = job.payload["delivery_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let (a, b) = tokio::join!(e.deliverer.deliver(id), e.deliverer.deliver(id));
    assert_eq!((a, b), (Ok(()), Ok(())));
    assert_eq!(e.sent().len(), 1, "sent once");
    let d = e.store.delivery(id).await.unwrap().unwrap();
    assert_eq!((d.state, d.attempts), (DeliveryState::Sent, 1));

    // a run that claimed the next one and died: nothing is sent until the claim is stale
    e.engine.emit(e.episode(2)).await;
    let id: uuid::Uuid = e.jobs.jobs()[1].payload["delivery_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let now = e.clock.now();
    let claimed = e
        .store
        .claim_delivery(id, now, now - Duration::minutes(15))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.state, DeliveryState::Sending);
    e.deliverer.deliver(id).await.unwrap();
    assert_eq!(e.sent().len(), 1, "another run holds it");
    e.clock
        .advance(Duration::minutes(notify::SENDING_STALE_MINUTES + 1));
    e.deliverer.deliver(id).await.unwrap();
    assert_eq!(e.sent().len(), 2, "the stale claim is taken over");
    let d = e.store.delivery(id).await.unwrap().unwrap();
    assert_eq!(d.state, DeliveryState::Sent);
}

/// OT-37 (#6): two runs of one digest send it once; a retried digest goes back to waiting.
#[tokio::test]
async fn two_runs_of_a_digest_send_it_once() {
    let e = env();
    e.clock.set(at("2026-01-10T10:00:00Z"));
    let ch = e.channel("mail").await;
    e.subscribe(
        "release.episode",
        DeliveryMode::Digest {
            cron: "0 8 * * *".into(),
        },
        |_| {},
    )
    .await;
    for n in 1..=2 {
        e.engine.emit(e.episode(n)).await;
    }
    e.clock.set(at("2026-01-11T07:00:00Z"));
    let until = e.clock.now();
    e.channel
        .answers
        .lock()
        .unwrap()
        .push(Err(SendError::Retry("503".into())));
    assert!(
        e.deliverer.digest(ch, until).await.is_err(),
        "the job retries"
    );
    let log = e.store.deliveries(e.user, 10).await.unwrap();
    assert!(
        log.iter().all(|d| d.state == DeliveryState::Digest),
        "{log:?}"
    );
    e.channel
        .slow
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (a, b) = tokio::join!(e.deliverer.digest(ch, until), e.deliverer.digest(ch, until));
    assert_eq!((a, b), (Ok(()), Ok(())));
    assert_eq!(e.sent().len(), 2, "the failed attempt and one digest");
    let log = e.store.deliveries(e.user, 10).await.unwrap();
    assert!(
        log.iter().all(|d| d.state == DeliveryState::Sent),
        "{log:?}"
    );
}
