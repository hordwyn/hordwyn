//! All-time totals per group (Home) and time series per group (Stats page).

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::{Bucket, NewCatalogItem, NewSession};
use chrono::{TimeZone, Utc};
use domain::catalog::ExternalId;
use domain::kind::{KindGroup, KindId, Status};
use domain::progress::Progress;
use domain::session::Participant;
use testkit::TestEnv;

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
