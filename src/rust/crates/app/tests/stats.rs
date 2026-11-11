//! All-time totals per group (Home) and time series per group (Stats page).

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::ports::KindCatalog;
use app::{Bucket, NewCatalogItem, NewSession};
use chrono::{TimeZone, Utc};
use domain::catalog::ExternalId;
use domain::kind::{KindGroup, KindId, Status};
use domain::progress::Progress;
use domain::session::Participant;
use testkit::{StaticKinds, TestEnv};

#[tokio::test]
async fn totals_per_group_and_series_per_month_and_year() {
    let env = TestEnv::new();
    let (user, me) = common::user(&env, "alice").await;
    let movie = env
        .app
        .upsert_catalog_item(NewCatalogItem {
            kind: KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: Some(1995),
            description: None,
            external_ids: vec![ExternalId::new("test", "heat")],
            attributes: serde_json::json!({"runtime_minutes": 170}),
            cover_url: None,
        })
        .await
        .unwrap();
    let jan = Utc.with_ymd_and_hms(2025, 1, 10, 20, 0, 0).unwrap();
    let mar = Utc.with_ymd_and_hms(2025, 3, 2, 20, 0, 0).unwrap();
    env.app
        .log_progress(me, movie.id, Progress::Finished, Some(jan), None)
        .await
        .unwrap();
    env.app
        .log_progress(me, movie.id, Progress::Finished, Some(mar), None)
        .await
        .unwrap();
    let book = common::item(&env.app, "book", "Dune").await;
    for (p, t) in [(100, jan), (250, mar)] {
        let page = Progress::Paged {
            page: Some(p),
            chapter: None,
            volume: None,
        };
        env.app
            .log_progress(me, book.id, page, Some(t), None)
            .await
            .unwrap();
    }
    let game = common::item(&env.app, "board_game", "Azul").await;
    let mut s = NewSession::new(game.id, vec![Participant::new(user.self_companion)]);
    s.started_at = Some(mar);
    s.duration_minutes = Some(45);
    env.app.log_session(me, s).await.unwrap();
    let planned = common::item(&env.app, "show", "Dark").await;
    env.app
        .add_to_library(me, planned.id, Some(Status::planned()))
        .await
        .unwrap();

    let t = env.app.stats_totals(me).await.unwrap();
    assert_eq!(t.overall.entries, 4);
    let watch = &t
        .groups
        .iter()
        .find(|g| g.group == KindGroup::Watch)
        .unwrap()
        .totals;
    assert_eq!((watch.entries, watch.completed, watch.planned), (2, 1, 1));
    assert_eq!((watch.amounts.finished, watch.amounts.minutes), (2, 340));
    let read = &t
        .groups
        .iter()
        .find(|g| g.group == KindGroup::Read)
        .unwrap()
        .totals;
    assert_eq!((read.in_progress, read.amounts.pages), (1, 250));
    let play = &t
        .groups
        .iter()
        .find(|g| g.group == KindGroup::Play)
        .unwrap()
        .totals;
    assert_eq!((play.amounts.plays, play.amounts.minutes), (1, 45));
    assert_eq!(t.overall.amounts.minutes, 385);

    let months = env.app.stats_series(me, None, Bucket::Month).await.unwrap();
    let keys: Vec<_> = months.iter().map(|p| p.bucket.as_str()).collect();
    assert_eq!(keys, ["2025-01", "2025-03"]);
    assert_eq!(months[0].amounts.pages, 100);
    assert_eq!(months[1].amounts.pages, 150);
    assert_eq!(
        months[0].amounts.completions, 1,
        "the first Finished completed Heat"
    );
    let read_years = env
        .app
        .stats_series(me, Some(KindGroup::Read), Bucket::Year)
        .await
        .unwrap();
    assert_eq!(read_years.len(), 1);
    assert_eq!(
        (
            read_years[0].bucket.as_str(),
            read_years[0].amounts.pages,
            read_years[0].amounts.minutes
        ),
        ("2025", 250, 0)
    );
}

/// OT-17 (DEC-542): the time tiles need the totals per kind. The kinds add up to their
/// group and to the overall figure (the hero tile), minutes included; a kind without any
/// time reports none; a kind without activity is left out.
#[tokio::test]
async fn totals_per_kind_add_up_to_the_hero_figure() {
    // the small test catalog has one timed kind; a second one in the same group
    let mut catalog = StaticKinds::default().all();
    let mut music = catalog
        .iter()
        .find(|k| k.id.as_str() == "audiobook")
        .cloned()
        .unwrap();
    music.id = KindId::new("music").unwrap();
    catalog.push(music);
    let env = TestEnv::with_kinds(std::sync::Arc::new(StaticKinds::new(catalog)));
    let (user, me) = common::user(&env, "alice").await;
    let at = Utc.with_ymd_and_hms(2025, 1, 10, 20, 0, 0).unwrap();
    let timed = |kind: &str, title: &str, attributes: serde_json::Value| NewCatalogItem {
        kind: KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: None,
        description: None,
        external_ids: vec![ExternalId::new("test", title)],
        attributes,
        cover_url: None,
    };
    let movie = env
        .app
        .upsert_catalog_item(timed(
            "movie",
            "Heat",
            serde_json::json!({"runtime_minutes": 170}),
        ))
        .await
        .unwrap();
    let show = env
        .app
        .upsert_catalog_item(timed(
            "show",
            "Dark",
            serde_json::json!({"episode_runtime_minutes": 50}),
        ))
        .await
        .unwrap();
    // a movie without a known length counts, but adds no time
    let untimed = common::item(&env.app, "movie", "Stub").await;
    for id in [movie.id, untimed.id] {
        env.app
            .log_progress(me, id, Progress::Finished, Some(at), None)
            .await
            .unwrap();
    }
    for episode in 1..=3 {
        let p = Progress::Episode { season: 1, episode };
        env.app
            .log_progress(me, show.id, p, Some(at), None)
            .await
            .unwrap();
    }
    let book = common::item(&env.app, "book", "Dune").await;
    let page = Progress::Paged {
        page: Some(120),
        chapter: None,
        volume: None,
    };
    env.app
        .log_progress(me, book.id, page, Some(at), None)
        .await
        .unwrap();
    // two listening kinds with seconds that do not fill a minute: 90 s and 45 s
    for (kind, title, seconds) in [("audiobook", "Emma", 90), ("music", "Radio", 45)] {
        let item = common::item(&env.app, kind, title).await;
        env.app
            .log_progress(me, item.id, Progress::Position { seconds }, Some(at), None)
            .await
            .unwrap();
    }
    let game = common::item(&env.app, "board_game", "Azul").await;
    let mut s = NewSession::new(game.id, vec![Participant::new(user.self_companion)]);
    s.started_at = Some(at);
    s.duration_minutes = Some(45);
    env.app.log_session(me, s).await.unwrap();
    // in the library, never logged: no kind row
    let planned = common::item(&env.app, "video_game", "Celeste").await;
    env.app
        .add_to_library(me, planned.id, Some(Status::planned()))
        .await
        .unwrap();

    let t = env.app.stats_totals(me).await.unwrap();
    let kinds: Vec<(&str, u64)> = t
        .kinds
        .iter()
        .map(|k| (k.kind.as_str(), k.amounts.minutes))
        .collect();
    assert_eq!(
        kinds,
        [
            ("audiobook", 1),
            ("board_game", 45),
            ("book", 0),
            ("movie", 170),
            ("music", 0),
            ("show", 150),
        ],
        "sorted by kind; whole minutes per kind; no time: none"
    );
    let of = |kind: &str| t.kinds.iter().find(|k| k.kind.as_str() == kind).unwrap();
    assert_eq!(of("movie").amounts.finished, 2);
    assert_eq!(of("show").amounts.episodes, 3);
    assert_eq!(of("book").amounts.pages, 120);
    assert_eq!(of("board_game").amounts.plays, 1);
    let sum: u64 = t.kinds.iter().map(|k| k.amounts.minutes).sum();
    assert_eq!(sum, 366);
    assert_eq!(t.overall.amounts.minutes, sum, "the hero figure");
    let events: u64 = t.kinds.iter().map(|k| k.amounts.events).sum();
    assert_eq!(t.overall.amounts.events, events);
    let group = |g: KindGroup| t.groups.iter().find(|x| x.group == g).unwrap().totals;
    assert_eq!(group(KindGroup::Watch).amounts.minutes, 320);
    assert_eq!(
        group(KindGroup::Listen).amounts.minutes,
        1,
        "the group is the sum of its kinds (90 s + 45 s as one sum would be 2)"
    );
    assert_eq!(group(KindGroup::Play).entries, 2);
}
