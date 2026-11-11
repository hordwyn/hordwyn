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
    // keyset paging (DEC-416): the third page of two by title is the last row
    let mut page = LibraryQuery {
        sort: LibrarySort::Title,
        limit: 2,
        ..LibraryQuery::default()
    };
    let mut seen = vec![];
    loop {
        let p = app.library_query(me, page.clone()).await.unwrap();
        assert_eq!(p.total, 5);
        seen.push(titles(&p));
        match p.next {
            Some(c) => page.after = Some(c),
            None => break,
        }
    }
    assert_eq!(
        seen,
        [vec!["alien", "Azul"], vec!["Dark", "Dune"], vec!["Heat"]]
    );
    // the rows after a cursor were removed meanwhile: an empty page still counts
    let mut gone = page.clone();
    gone.after = Some(app::ports::LibraryCursor {
        key: app::ports::SortValue::Text("zzz".into()),
        ..page.after.clone().unwrap()
    });
    let p = app.library_query(me, gone).await.unwrap();
    assert_eq!((p.rows.len(), p.total, p.next), (0, 5, None));
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
    // a show completed by hand, then a rewatch run that is stopped (its marker gives
    // Completed back and is no completion, DEC-590) and one that is finished (one completion)
    let again = item(
        app,
        "show",
        "Lost",
        None,
        serde_json::json!({"seasons": [{"number": 1, "episode_count": 2}]}),
    )
    .await;
    for (p, t) in [(ep(1), at(2025, 2, 1)), (ep(2), at(2025, 2, 2))] {
        app.log_progress(me, again.id, p, Some(t), None)
            .await
            .unwrap();
    }
    app.set_status(me, again.id, Status::completed(), None)
        .await
        .unwrap();
    for (p, t) in [
        (Progress::Rewatch, at(2025, 4, 1)),
        (ep(1), at(2025, 4, 2)),
        (Progress::RewatchStop, at(2025, 4, 3)),
        (Progress::Rewatch, at(2025, 5, 1)),
        (ep(1), at(2025, 5, 2)),
        (ep(2), at(2025, 5, 3)),
    ] {
        let e = app
            .log_progress(me, again.id, p, Some(t), None)
            .await
            .unwrap();
        assert_eq!(e.occurred_at, t);
    }
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
    let years = mem.app.stats_series(mem_me, None, Bucket::Year).await;
    let completions: u64 = years.unwrap().iter().map(|p| p.amounts.completions).sum();
    assert_eq!(completions, 3, "two movies and the finished rewatch run");
    for group in [None, Some(KindGroup::Watch), Some(KindGroup::Read)] {
        for bucket in [Bucket::Month, Bucket::Year] {
            let pg = env.app.stats_series(pg_me, group, bucket).await.unwrap();
            let expected = mem.app.stats_series(mem_me, group, bucket).await.unwrap();
            assert_eq!(pg, expected, "{group:?} {bucket:?}");
        }
    }
    // UX3-20: a zone this Postgres does not know (tzdata older than chrono-tz) buckets in
    // UTC instead of failing the Stats page
    let progress = &env.app.repos.progress;
    let unknown = progress
        .month_amounts(pg_me.user_id, "Mars/Olympus")
        .await
        .unwrap();
    let utc = progress.month_amounts(pg_me.user_id, "UTC").await.unwrap();
    assert_eq!(unknown, utc);
}

/// The statistics rules in Rust over what Postgres holds for `user` (events, items, the
/// user's own session durations).
async fn rust_rules(
    env: &common::PgEnv,
    user: domain::ids::UserId,
    zone: app::time_zone::Tz,
) -> Vec<app::MonthAmounts> {
    use app::ports::{CatalogRepo, ProgressRepo, SessionRepo};
    let events = ProgressRepo::by_user(&env.store, user).await.unwrap();
    let mut ids: Vec<_> = events.iter().map(|e| e.item_id).collect();
    ids.sort();
    ids.dedup();
    let items = CatalogRepo::get_many(&env.store, &ids)
        .await
        .unwrap()
        .into_iter()
        .map(|i| (i.id, i))
        .collect::<std::collections::HashMap<_, _>>();
    let durations = SessionRepo::by_owner(&env.store, user)
        .await
        .unwrap()
        .into_iter()
        .map(|s| (s.id, u64::from(s.duration_minutes.unwrap_or(0))))
        .collect::<std::collections::HashMap<_, _>>();
    app::month_amounts(&items, &events, &durations, zone)
}

/// The cached read equals the full statement and the Rust rules, twice (the second read
/// takes the clean path).
async fn same_stats(env: &common::PgEnv, user: domain::ids::UserId, step: &str) {
    use app::ports::ProgressRepo;
    let zone = env.app.user_zone(user).await.unwrap();
    let full = env
        .store
        .month_amounts_full(user, zone.name())
        .await
        .unwrap();
    let rules = rust_rules(env, user, zone).await;
    assert_eq!(full, rules, "{step}: full statement vs Rust rules");
    for read in ["first", "second"] {
        let cached = ProgressRepo::month_amounts(&env.store, user, zone.name())
            .await
            .unwrap();
        assert_eq!(cached, full, "{step}: {read} cached read");
    }
}

#[tokio::test]
async fn cached_stats_equal_the_full_statement_after_every_step() {
    // WP4-1 (DEC-235): the per-item month cache answers exactly what the full statement
    // and `app::month_amounts` answer, whatever changed in between
    use app::ports::SessionRepo;
    use chrono::TimeZone;
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (alice, me) = common::user(app, "alice").await;
    let (_, bob) = common::user(app, "bob").await;
    let uid = me.user_id;
    let at = |y, m, d| chrono::Utc.with_ymd_and_hms(y, m, d, 20, 0, 0).unwrap();
    let movie_attrs = |minutes: u32| serde_json::json!({"runtime_minutes": minutes});
    let movie = item(app, "movie", "Heat", None, movie_attrs(170)).await;
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
    same_stats(&env, uid, "nothing logged").await;

    // logs of every kind, one on a month end that moves with the zone
    let ep = |e| Progress::Episode {
        season: 1,
        episode: e,
    };
    let paged = |page| Progress::Paged {
        page: Some(page),
        chapter: None,
        volume: None,
    };
    app.log_progress(
        me,
        movie.id,
        Progress::Finished,
        Some(at(2025, 1, 31)),
        None,
    )
    .await
    .unwrap();
    app.log_progress(me, show.id, ep(1), Some(at(2025, 2, 1)), None)
        .await
        .unwrap();
    let second = app
        .log_progress(me, show.id, ep(2), Some(at(2025, 2, 2)), None)
        .await
        .unwrap();
    app.log_progress(me, book.id, paged(100), Some(at(2025, 2, 3)), None)
        .await
        .unwrap();
    app.log_progress(
        me,
        audio.id,
        Progress::Position { seconds: 900 },
        Some(at(2025, 2, 4)),
        None,
    )
    .await
    .unwrap();
    let mut play = NewSession::new(game.id, vec![Participant::new(alice.self_companion)]);
    play.started_at = Some(at(2025, 3, 1));
    play.duration_minutes = Some(45);
    let (session, _) = app.log_play(me, play).await.unwrap();
    app.log_progress(
        bob,
        movie.id,
        Progress::Finished,
        Some(at(2025, 1, 5)),
        None,
    )
    .await
    .unwrap();
    same_stats(&env, uid, "logs").await;
    same_stats(&env, bob.user_id, "bob's own").await;

    app.undo_progress(me, second.id).await.unwrap();
    same_stats(&env, uid, "undo").await;

    for (page, d) in [(80, 5), (250, 6)] {
        app.log_progress(me, book.id, paged(page), Some(at(2025, 2, d)), None)
            .await
            .unwrap();
    }
    // a page logged earlier than the others moves the forward moves after it
    app.log_progress(me, book.id, paged(120), Some(at(2025, 2, 2)), None)
        .await
        .unwrap();
    same_stats(&env, uid, "pages back and forth").await;

    let mut edited = SessionRepo::get(&env.store, session.id)
        .await
        .unwrap()
        .unwrap();
    edited.duration_minutes = Some(90);
    SessionRepo::update(&env.store, &edited).await.unwrap();
    same_stats(&env, uid, "session duration edited").await;

    // a refresh brings a new runtime
    item(app, "movie", "Heat", None, movie_attrs(160)).await;
    same_stats(&env, uid, "runtime changed").await;

    app.set_time_zone(me, app::time_zone::parse_zone("Pacific/Auckland"))
        .await
        .unwrap();
    same_stats(&env, uid, "zone changed").await;
    app.set_time_zone(me, None).await.unwrap();
    same_stats(&env, uid, "zone back to UTC").await;

    sqlx::query("DELETE FROM library_entries WHERE user_id = $1 AND item_id = $2")
        .bind(uid.0)
        .bind(movie.id.0)
        .execute(env.store.pool())
        .await
        .unwrap();
    same_stats(&env, uid, "entry removed").await;
    app.add_to_library(me, movie.id, None).await.unwrap();
    same_stats(&env, uid, "entry added again").await;

    // an import batch: client op ids (a replay adds nothing) and rows written in one statement
    for round in 0..2 {
        for d in 10..14 {
            let op = uuid::Uuid::from_u128(u128::from(d));
            app.log_progress(me, show.id, ep(d), Some(at(2025, 4, d)), Some(op))
                .await
                .unwrap();
        }
        same_stats(&env, uid, &format!("import round {round}")).await;
    }
    sqlx::query(
        "INSERT INTO progress_events (id, user_id, item_id, occurred_at, progress, recorded_at)
         SELECT gen_random_uuid(), $1, $2, $3 + g * interval '1 day',
                jsonb_build_object('type', 'finished'), now()
         FROM generate_series(1, 5) AS g",
    )
    .bind(uid.0)
    .bind(movie.id.0)
    .bind(at(2025, 5, 1))
    .execute(env.store.pool())
    .await
    .unwrap();
    same_stats(&env, uid, "rows written in one statement").await;

    SessionRepo::delete(&env.store, session.id).await.unwrap();
    same_stats(&env, uid, "session deleted").await;
    same_stats(&env, bob.user_id, "bob untouched").await;
}

fn entry_ids(p: &app::LibraryPage) -> Vec<domain::ids::EntryId> {
    p.rows.iter().map(|r| r.entry.id).collect()
}

/// Keyset paging (DEC-416): walking every sort in both directions in pages of 1 to 3 returns
/// exactly one full read's rows, ties and missing values included; a cursor of another order
/// is refused; the plain library page walks the same way.
#[tokio::test]
async fn keyset_pages_walk_every_order_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "kim").await;
    let at = chrono::DateTime::parse_from_rfc3339("2026-01-10T10:00:00Z")
        .unwrap()
        .to_utc();
    // equal titles (but for case), years and ratings, missing years and ratings, and equal
    // last activity on every other row; all plays are 0
    let rows = [
        ("Same", Some(2000), Some(60)),
        ("same", Some(2000), Some(60)),
        ("SAME", None, None),
        ("Other", Some(1990), Some(60)),
        ("Zed", None, Some(100)),
        ("alpha", Some(2000), None),
        ("Beta", Some(1990), Some(20)),
    ];
    for (n, (title, year, rating)) in rows.into_iter().enumerate() {
        let i = item(app, "movie", title, year, serde_json::json!({})).await;
        app.add_to_library(me, i.id, None).await.unwrap();
        if let Some(r) = rating {
            app.rate(me, i.id, Some(Rating::new(r).unwrap()), None)
                .await
                .unwrap();
        }
        if n % 2 == 0 {
            app.log_progress(me, i.id, Progress::Finished, Some(at), None)
                .await
                .unwrap();
        }
    }
    let full = |sort, descending| LibraryQuery {
        sort,
        descending: Some(descending),
        limit: 500,
        ..LibraryQuery::default()
    };
    for sort in [
        LibrarySort::LastActivity,
        LibrarySort::Title,
        LibrarySort::Added,
        LibrarySort::Year,
        LibrarySort::Rating,
        LibrarySort::Plays,
    ] {
        for descending in [false, true] {
            let all = entry_ids(&app.library_query(me, full(sort, descending)).await.unwrap());
            assert_eq!(all.len(), 7);
            for limit in 1..=3 {
                let mut q = LibraryQuery {
                    limit,
                    ..full(sort, descending)
                };
                let mut walked = vec![];
                loop {
                    let p = app.library_query(me, q.clone()).await.unwrap();
                    assert_eq!(p.total, 7);
                    walked.extend(entry_ids(&p));
                    match p.next {
                        Some(c) => q.after = Some(c),
                        None => break,
                    }
                }
                assert_eq!(
                    walked, all,
                    "{sort:?} descending={descending} limit={limit}"
                );
            }
        }
    }
    // a cursor made for another order
    let first = app
        .library_query(
            me,
            LibraryQuery {
                limit: 1,
                ..full(LibrarySort::Title, false)
            },
        )
        .await
        .unwrap();
    let wrong = LibraryQuery {
        after: first.next,
        ..full(LibrarySort::Title, true)
    };
    assert!(app.library_query(me, wrong).await.is_err());

    // the library page, most recently added first
    let mut expected = app.library(me).await.unwrap();
    expected.sort_by_key(|e| std::cmp::Reverse((e.added_at, e.id)));
    let expected: Vec<_> = expected.into_iter().map(|e| e.id).collect();
    let mut after = None;
    let mut walked = vec![];
    loop {
        let (entries, total, next) = app.library_page(me, after.as_ref(), 2).await.unwrap();
        assert_eq!(total, 7);
        walked.extend(entries.iter().map(|e| e.id));
        match next {
            Some(c) => after = Some(c),
            None => break,
        }
    }
    assert_eq!(walked, expected);
}

/// Remove from library on Postgres (WP4-3, DEC-348 "delete keeps history"): the entry and its
/// tags go, events, plays, list items and the statistics stay; undo restores the entry with
/// its id, tags and place; a bulk status change writes every entry and undoes in one step.
#[tokio::test]
async fn remove_keeps_history_and_bulk_undo_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "rem").await;
    let azul = item(app, "board_game", "Azul", Some(2017), serde_json::json!({})).await;
    let heat = item(app, "movie", "Heat", Some(1995), serde_json::json!({})).await;
    app.add_to_library(me, heat.id, None).await.unwrap();
    app.log_progress(me, heat.id, Progress::Finished, None, None)
        .await
        .unwrap();
    let me_user = app.user(me.user_id).await.unwrap();
    app.log_session(
        me,
        NewSession::new(azul.id, vec![Participant::new(me_user.self_companion)]),
    )
    .await
    .unwrap();
    app.add_tag(me, azul.id, "Family").await.unwrap();
    let list = app
        .create_list(me, "Games", None, domain::list::Visibility::Private)
        .await
        .unwrap();
    app.add_to_list(me, list.id, azul.id, None).await.unwrap();
    let before = app.entry(me, azul.id).await.unwrap().unwrap();
    let totals = app.stats_totals(me).await.unwrap().overall;

    let undo = app.remove_from_library(me, azul.id).await.unwrap();
    assert_eq!(app.entry(me, azul.id).await.unwrap(), None);
    assert_eq!(app.tags(me).await.unwrap().len(), 0);
    let after = app.stats_totals(me).await.unwrap().overall;
    assert_eq!(
        (after.entries, after.amounts.plays, after.amounts.finished),
        (1, totals.amounts.plays, totals.amounts.finished)
    );
    assert_eq!(app.list(me, list.id).await.unwrap().items.len(), 1);
    assert_eq!(app.my_sessions(me).await.unwrap().len(), 1);

    app.undo_bulk(me, &undo).await.unwrap();
    let back = app.entry(me, azul.id).await.unwrap().unwrap();
    assert_eq!((back.id, back.added_at), (before.id, before.added_at));
    assert_eq!(app.entry_tags(me, azul.id).await.unwrap(), ["Family"]);
    // the play still counts on the restored entry
    let row = app.library_row(me, azul.id).await.unwrap();
    assert_eq!(row.summary.sessions, 1);

    let out = app
        .bulk(
            me,
            &[heat.id, azul.id],
            app::BulkOp::SetStatus(Status::new("paused")),
        )
        .await
        .unwrap();
    assert!(out.results.iter().all(|(_, r)| r.is_none()), "{out:?}");
    assert_eq!(
        app.library_counts(me).await.unwrap().statuses,
        [(Status::new("paused"), 2)]
    );
    app.undo_bulk(me, &out.undo).await.unwrap();
    assert_eq!(
        app.entry(me, heat.id).await.unwrap().unwrap().status.value,
        Status::completed()
    );
}

#[tokio::test]
async fn window_amounts_and_the_year_comparison_on_postgres_match_the_in_memory_rules() {
    // WP4-6: "this year so far vs the same period last year" adds the current month from
    // the events in a window; Postgres and the Rust rules must agree on every window edge
    use app::ports::ProgressRepo;
    use chrono::TimeZone;
    let Some(mut env) = common::env().await else {
        return;
    };
    let mem = testkit::TestEnv::new();
    let pg_me = stats_scenario(&env.app).await;
    let mem_me = stats_scenario(&mem.app).await;
    let t = |y, m, d, h| chrono::Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap();
    for (from, to) in [
        (t(2025, 1, 11, 0), t(2025, 3, 2, 21)),
        (t(2025, 1, 1, 0), t(2025, 2, 1, 0)),
        // the start is inside, the end is not
        (t(2024, 12, 31, 20), t(2025, 1, 10, 20)),
        (t(2025, 3, 2, 20), t(2025, 3, 3, 20)),
        (t(2020, 1, 1, 0), t(2030, 1, 1, 0)),
    ] {
        let pg = env
            .store
            .window_amounts(pg_me.user_id, from, to)
            .await
            .unwrap();
        let expected = mem
            .store
            .window_amounts(mem_me.user_id, from, to)
            .await
            .unwrap();
        assert_eq!(pg, expected, "{from} .. {to}");
    }
    let all = mem
        .store
        .window_amounts(mem_me.user_id, t(2025, 1, 11, 0), t(2025, 3, 2, 21))
        .await
        .unwrap();
    let book = all.iter().find(|k| k.kind.as_str() == "book").unwrap();
    assert_eq!(
        book.amounts.pages, 170,
        "back to 80 counts nothing, then 80 -> 250"
    );
    // the comparison on 2026-03-02: last year's period ends with 2025-03-02
    let clock = std::sync::Arc::new(testkit::fakes::FixedClock::default());
    clock.set(t(2026, 3, 2, 23));
    env.app.clock = clock.clone();
    mem.clock.set(t(2026, 3, 2, 23));
    let pg = env.app.year_compare(pg_me).await.unwrap();
    let expected = mem.app.year_compare(mem_me).await.unwrap();
    assert_eq!(pg, expected);
    assert_eq!(expected.overall.last.plays, 1);
    assert_eq!(expected.overall.this, app::Amounts::default());
}

/// OT1A-3: the plays of an item page are read in one statement; the answer keeps the order
/// asked for (the statement itself orders by start) and leaves out an unknown id.
#[tokio::test]
async fn sessions_are_read_by_ids_in_the_order_asked_on_postgres() {
    use app::ports::SessionRepo;

    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "many").await;
    let azul = item(app, "board_game", "Azul", Some(2017), serde_json::json!({})).await;
    let me_user = app.user(me.user_id).await.unwrap();
    let mut ids = Vec::new();
    for minutes in [30, 45, 60] {
        let mut play = NewSession::new(azul.id, vec![Participant::new(me_user.self_companion)]);
        play.duration_minutes = Some(minutes);
        ids.push(app.log_session(me, play).await.unwrap().id);
    }
    let unknown = domain::ids::SessionId(uuid::Uuid::now_v7());
    for asked in [
        vec![ids[0], ids[1], ids[2]],
        vec![ids[2], unknown, ids[0]],
        vec![ids[1]],
    ] {
        let got = SessionRepo::get_many(&env.store, &asked).await.unwrap();
        let expected: Vec<_> = asked.iter().copied().filter(|id| *id != unknown).collect();
        assert_eq!(got.iter().map(|s| s.id).collect::<Vec<_>>(), expected);
        assert_eq!(
            got.iter().filter(|s| s.participants.len() == 1).count(),
            expected.len()
        );
    }
    assert_eq!(
        SessionRepo::get_many(&env.store, &[]).await.unwrap().len(),
        0
    );
}
