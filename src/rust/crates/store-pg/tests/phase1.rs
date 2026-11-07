//! UX phase 1 use-cases on Postgres: library query, counts, tags, undo, activity, stats.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::ports::LibrarySort;
use app::{ActivityQuery, Bucket, LibraryQuery, NewCatalogItem, NewSession, NextStep};
use domain::catalog::ExternalId;
use domain::kind::{KindGroup, KindId, Status};
use domain::library::Rating;
use domain::progress::Progress;
use domain::session::Participant;

async fn item(
    app: &app::App,
    kind: &str,
    title: &str,
    year: Option<i32>,
    attributes: serde_json::Value,
) -> domain::catalog::CatalogItem {
    app.upsert_catalog_item(NewCatalogItem {
        kind: KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year,
        description: None,
        external_ids: vec![ExternalId::new("test", title)],
        attributes,
        cover_url: None,
    })
    .await
    .unwrap()
}

fn titles(p: &app::LibraryPage) -> Vec<String> {
    p.rows.iter().map(|r| r.item.title.clone()).collect()
}

#[tokio::test]
async fn library_query_counts_and_tags_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "alice").await;
    let (_, bob) = common::user(app, "bob").await;
    let heat = item(
        app,
        "movie",
        "Heat",
        Some(1995),
        serde_json::json!({"genres": ["Crime"]}),
    )
    .await;
    let alien = item(
        app,
        "movie",
        "alien",
        Some(1979),
        serde_json::json!({"genres": ["Horror"]}),
    )
    .await;
    let dark = item(app, "show", "Dark", None, serde_json::json!({})).await;
    let dune = item(app, "book", "Dune", Some(1965), serde_json::json!({})).await;
    let azul = item(app, "board_game", "Azul", Some(2017), serde_json::json!({})).await;
    for i in [&heat, &alien, &dark, &dune, &azul] {
        app.add_to_library(me, i.id, None).await.unwrap();
    }
    app.add_to_library(bob, heat.id, None).await.unwrap();
    app.rate(me, alien.id, Some(Rating::new(90).unwrap()), None)
        .await
        .unwrap();
    let later = chrono::DateTime::parse_from_rfc3339("2026-02-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    app.log_progress(me, heat.id, Progress::Finished, Some(later), None)
        .await
        .unwrap();
    let me_user = app.user(me.user_id).await.unwrap();
    for _ in 0..2 {
        app.log_session(
            me,
            NewSession::new(azul.id, vec![Participant::new(me_user.self_companion)]),
        )
        .await
        .unwrap();
    }

    let c = app.library_counts(me).await.unwrap();
    assert_eq!(c.total, 5);
    assert_eq!(c.groups[0], (KindGroup::Watch, 3));
    assert!(c.statuses.contains(&(Status::completed(), 1)));

    let q = |sort, descending: Option<bool>| LibraryQuery {
        sort,
        descending,
        ..LibraryQuery::default()
    };
    assert_eq!(
        titles(
            &app.library_query(me, q(LibrarySort::Title, None))
                .await
                .unwrap()
        ),
        ["alien", "Azul", "Dark", "Dune", "Heat"]
    );
    // missing years sort last in both directions
    assert_eq!(
        titles(
            &app.library_query(me, q(LibrarySort::Year, None))
                .await
                .unwrap()
        ),
        ["Azul", "Heat", "alien", "Dune", "Dark"]
    );
    assert_eq!(
        titles(
            &app.library_query(me, q(LibrarySort::Year, Some(false)))
                .await
                .unwrap()
        ),
        ["Dune", "alien", "Heat", "Azul", "Dark"]
    );
    let rated = app
        .library_query(me, q(LibrarySort::Rating, None))
        .await
        .unwrap();
    assert_eq!(titles(&rated)[0], "alien");
    let plays = app
        .library_query(me, q(LibrarySort::Plays, None))
        .await
        .unwrap();
    assert_eq!(titles(&plays)[0], "Azul");
    assert_eq!(plays.rows[0].next_step, NextStep::Session { plays: 2 });
    let recent = app
        .library_query(me, q(LibrarySort::LastActivity, None))
        .await
        .unwrap();
    assert_eq!(titles(&recent)[0], "Heat");
    assert_eq!(recent.rows[0].last_activity, later);

    let watch = LibraryQuery {
        group: Some(KindGroup::Watch),
        statuses: vec![Status::planned()],
        facets: vec![("genres".into(), "Horror".into())],
        ..LibraryQuery::default()
    };
    assert_eq!(
        titles(&app.library_query(me, watch).await.unwrap()),
        ["alien"]
    );
    let page = LibraryQuery {
        sort: LibrarySort::Title,
        offset: 4,
        limit: 2,
        ..LibraryQuery::default()
    };
    let p = app.library_query(me, page).await.unwrap();
    assert_eq!((titles(&p), p.total), (vec!["Heat".to_string()], 5));
    let past = LibraryQuery {
        offset: 50,
        ..LibraryQuery::default()
    };
    assert_eq!(app.library_query(me, past).await.unwrap().total, 5);
    let only = LibraryQuery {
        items: Some(vec![dune.id]),
        ..LibraryQuery::default()
    };
    assert_eq!(
        titles(&app.library_query(me, only).await.unwrap()),
        ["Dune"]
    );

    // tags
    app.add_tag(me, heat.id, "cosy").await.unwrap();
    app.add_tag(me, heat.id, "With Anna").await.unwrap();
    app.add_tag(me, dune.id, "Cosy").await.unwrap();
    app.add_tag(bob, heat.id, "bobs").await.unwrap();
    let counts: Vec<_> = app
        .tags(me)
        .await
        .unwrap()
        .into_iter()
        .map(|t| (t.tag, t.entries))
        .collect();
    assert_eq!(
        counts,
        vec![("cosy".to_string(), 2), ("With Anna".into(), 1)]
    );
    let tagged = LibraryQuery {
        tags: vec!["COSY".into(), "with anna".into()],
        ..LibraryQuery::default()
    };
    let p = app.library_query(me, tagged).await.unwrap();
    assert_eq!(titles(&p), ["Heat"]);
    assert_eq!(p.rows[0].tags, ["cosy", "With Anna"]);
    assert_eq!(app.rename_tag(me, "with anna", "cosy").await.unwrap(), 1);
    assert_eq!(app.entry_tags(me, heat.id).await.unwrap(), ["cosy"]);
    assert_eq!(
        app.remove_tag(me, heat.id, "COSY").await.unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(app.entry_tags(bob, heat.id).await.unwrap(), ["bobs"]);
}

#[tokio::test]
async fn undo_activity_and_stats_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "alice").await;
    let show = item(
        app,
        "show",
        "Dark",
        None,
        serde_json::json!({"episode_runtime_minutes": 50}),
    )
    .await;
    let first = app
        .log_progress(
            me,
            show.id,
            Progress::Episode {
                season: 1,
                episode: 1,
            },
            None,
            None,
        )
        .await
        .unwrap();
    let second = app
        .log_progress(
            me,
            show.id,
            Progress::Episode {
                season: 1,
                episode: 2,
            },
            None,
            None,
        )
        .await
        .unwrap();
    let undo = app.undo_progress(me, second.id).await.unwrap();
    assert_eq!(app.undo_progress(me, second.id).await.unwrap().id, undo.id);
    assert_eq!(app.progress(me, show.id).await.unwrap().len(), 3);
    let row = app.library_row(me, show.id).await.unwrap();
    assert_eq!(
        row.next_step,
        NextStep::Episode {
            season: 1,
            episode: 2
        }
    );
    assert_eq!(app.history(me, show.id).await.unwrap(), vec![first.clone()]);

    let feed = app
        .activity_feed(me, ActivityQuery::default())
        .await
        .unwrap();
    assert_eq!(feed.rows.len(), 1);
    assert_eq!(feed.rows[0].event_id, Some(first.id));
    let none = ActivityQuery {
        group: Some(KindGroup::Read),
        ..ActivityQuery::default()
    };
    assert_eq!(app.activity_feed(me, none).await.unwrap().rows.len(), 0);

    let t = app.stats_totals(me).await.unwrap();
    let watch = &t.groups[0].totals;
    assert_eq!(
        (
            watch.amounts.episodes,
            watch.amounts.minutes,
            watch.in_progress
        ),
        (1, 50, 1)
    );
    let months = app
        .stats_series(me, Some(KindGroup::Watch), Bucket::Month)
        .await
        .unwrap();
    assert_eq!(months.len(), 1);
    assert_eq!(months[0].amounts.episodes, 1);
}

#[tokio::test]
async fn up_next_and_the_maintained_activity_columns_on_postgres() {
    // UX1-1: next in series is answered by one bounded statement, and last activity and
    // plays come from columns kept by triggers (an undo recomputes them)
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "alice").await;
    let (_, bob) = common::user(app, "bob").await;
    let series = |pos: f64| serde_json::json!({"series": "Dune ", "series_position": pos});
    let dune1 = item(app, "book", "Dune", Some(1965), series(1.0)).await;
    let dune2 = item(app, "book", "Dune Messiah", Some(1969), series(2.0)).await;
    let dune3 = item(app, "book", "Children of Dune", Some(1976), series(3.0)).await;
    let emma = item(app, "book", "Emma", Some(1815), serde_json::json!({})).await;
    app.add_to_library(me, dune1.id, Some(Status::completed()))
        .await
        .unwrap();
    for i in [&dune2, &dune3] {
        app.add_to_library(me, i.id, Some(Status::planned()))
            .await
            .unwrap();
    }
    app.add_to_library(me, emma.id, Some(Status::planned()))
        .await
        .unwrap();
    // bob finished the second part: that does not make the third his next... or mine
    app.add_to_library(bob, dune2.id, Some(Status::completed()))
        .await
        .unwrap();
    app.add_to_library(bob, dune3.id, Some(Status::planned()))
        .await
        .unwrap();
    let next = app.up_next(me, 6).await.unwrap();
    let names: Vec<_> = next
        .iter()
        .map(|u| (u.row.item.title.as_str(), u.reason))
        .collect();
    assert_eq!(
        names,
        [
            ("Dune Messiah", app::UpNextReason::NextInSeries),
            ("Emma", app::UpNextReason::Planned),
            ("Children of Dune", app::UpNextReason::Planned),
        ]
    );
    let bobs = app.up_next(bob, 6).await.unwrap();
    assert_eq!(bobs[0].row.item.title, "Children of Dune");
    assert_eq!(bobs[0].reason, app::UpNextReason::NextInSeries);

    let azul = item(app, "board_game", "Azul", Some(2017), serde_json::json!({})).await;
    let catan = item(
        app,
        "board_game",
        "Catan",
        Some(1995),
        serde_json::json!({}),
    )
    .await;
    let play = |game: domain::ids::ItemId, at: Option<chrono::DateTime<chrono::Utc>>| async move {
        app.log_progress(
            me,
            game,
            Progress::Session {
                session_id: domain::ids::SessionId(uuid::Uuid::now_v7()),
            },
            at,
            None,
        )
        .await
        .unwrap()
    };
    let a1 = play(azul.id, None).await;
    let a2 = play(azul.id, None).await;
    let tomorrow = chrono::Utc::now() + chrono::Duration::days(1);
    play(catan.id, Some(tomorrow)).await;
    let by_plays = LibraryQuery {
        sort: LibrarySort::Plays,
        group: Some(KindGroup::Play),
        ..LibraryQuery::default()
    };
    let page = app.library_query(me, by_plays.clone()).await.unwrap();
    assert_eq!(titles(&page), ["Azul", "Catan"]);
    app.undo_progress(me, a1.id).await.unwrap();
    app.undo_progress(me, a2.id).await.unwrap();
    let page = app.library_query(me, by_plays).await.unwrap();
    assert_eq!(titles(&page), ["Catan", "Azul"]);
    let azul_row = page.rows.iter().find(|r| r.item.id == azul.id).unwrap();
    assert_eq!(azul_row.last_activity, azul_row.entry.added_at);
    let catan_row = page.rows.iter().find(|r| r.item.id == catan.id).unwrap();
    assert_eq!(
        catan_row.last_activity.timestamp_micros(),
        tomorrow.timestamp_micros()
    );
}

/// Every kind of contribution: runtimes (one a float, which counts as unknown), pages forward
/// and back plus a chapter-only event, positions forward and back, a session with a duration,
/// an undone episode, a completion, two years and three months.
async fn stats_scenario(app: &app::App) -> domain::user::Actor {
    use chrono::TimeZone;
    let (user, me) = common::user(app, "alice").await;
    let at = |y, m, d| chrono::Utc.with_ymd_and_hms(y, m, d, 20, 0, 0).unwrap();
    let movie = item(
        app,
        "movie",
        "Heat",
        None,
        serde_json::json!({"runtime_minutes": 170}),
    )
    .await;
    let odd = item(
        app,
        "movie",
        "Odd",
        None,
        serde_json::json!({"runtime_minutes": 90.5}),
    )
    .await;
    let show = item(
        app,
        "show",
        "Dark",
        None,
        serde_json::json!({"episode_runtime_minutes": 50}),
    )
    .await;
    let book = item(app, "book", "Dune", None, serde_json::json!({})).await;
    let audio = item(app, "audiobook", "Emma", None, serde_json::json!({})).await;
    let game = item(app, "board_game", "Azul", None, serde_json::json!({})).await;
    for (i, p, t) in [
        (movie.id, Progress::Finished, at(2024, 12, 31)),
        (movie.id, Progress::Finished, at(2025, 3, 2)),
        (odd.id, Progress::Finished, at(2025, 3, 3)),
    ] {
        app.log_progress(me, i, p, Some(t), None).await.unwrap();
    }
    let paged = |page, chapter| Progress::Paged {
        page,
        chapter,
        volume: None,
    };
    for (p, t) in [
        (paged(Some(100), None), at(2025, 1, 10)),
        (paged(None, Some(4)), at(2025, 1, 11)),
        (paged(Some(80), None), at(2025, 1, 12)),
        (paged(Some(250), None), at(2025, 3, 2)),
    ] {
        app.log_progress(me, book.id, p, Some(t), None)
            .await
            .unwrap();
    }
    for (s, t) in [
        (600, at(2025, 1, 5)),
        (300, at(2025, 1, 6)),
        (4000, at(2025, 3, 1)),
    ] {
        let p = Progress::Position { seconds: s };
        app.log_progress(me, audio.id, p, Some(t), None)
            .await
            .unwrap();
    }
    let ep = |e| Progress::Episode {
        season: 1,
        episode: e,
    };
    app.log_progress(me, show.id, ep(1), Some(at(2025, 1, 7)), None)
        .await
        .unwrap();
    let undone = app
        .log_progress(me, show.id, ep(2), Some(at(2025, 1, 8)), None)
        .await
        .unwrap();
    app.undo_progress(me, undone.id).await.unwrap();
    let mut s = NewSession::new(game.id, vec![Participant::new(user.self_companion)]);
    s.started_at = Some(at(2025, 3, 2));
    s.duration_minutes = Some(45);
    app.log_play(me, s).await.unwrap();
    me
}

#[tokio::test]
async fn stats_on_postgres_match_the_in_memory_rules() {
    // UX1-1: the totals and series are summed in SQL; the in-memory store keeps the rules
    // in Rust, so both must agree on every amount
    let Some(env) = common::env().await else {
        return;
    };
    let mem = testkit::TestEnv::new();
    let pg_me = stats_scenario(&env.app).await;
    let mem_me = stats_scenario(&mem.app).await;
    let pg = env.app.stats_totals(pg_me).await.unwrap();
    let expected = mem.app.stats_totals(mem_me).await.unwrap();
    assert_eq!(pg, expected);
    assert_eq!(expected.overall.amounts.pages, 270, "100, back to 80, 250");
    assert_eq!(expected.overall.amounts.finished, 3);
    for group in [None, Some(KindGroup::Watch), Some(KindGroup::Read)] {
        for bucket in [Bucket::Month, Bucket::Year] {
            let pg = env.app.stats_series(pg_me, group, bucket).await.unwrap();
            let expected = mem.app.stats_series(mem_me, group, bucket).await.unwrap();
            assert_eq!(pg, expected, "{group:?} {bucket:?}");
        }
    }
}
