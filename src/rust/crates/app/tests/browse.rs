//! Library page queries: counts, filters, sort, paging, tags; Home's continue and up next.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::ports::LibrarySort;
use app::{LibraryQuery, NewCatalogItem, UpNextReason};
use domain::catalog::ExternalId;
use domain::kind::{KindGroup, KindId, Status};
use domain::library::Rating;
use domain::progress::Progress;
use testkit::TestEnv;

async fn item_with(
    env: &TestEnv,
    kind: &str,
    title: &str,
    year: i32,
    attributes: serde_json::Value,
) -> domain::catalog::CatalogItem {
    env.app
        .upsert_catalog_item(NewCatalogItem {
            kind: KindId::new(kind).unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(year),
            description: None,
            external_ids: vec![ExternalId::new("test", title)],
            attributes,
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap()
}

fn titles(p: &app::LibraryPage) -> Vec<String> {
    p.rows.iter().map(|r| r.item.title.clone()).collect()
}

#[tokio::test]
async fn counts_per_group_kind_and_status_for_the_signed_in_user_only() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    for (kind, title, status) in [
        ("movie", "Heat", "completed"),
        ("movie", "Alien", "planned"),
        ("show", "Dark", "in_progress"),
        ("book", "Dune", "planned"),
        ("board_game", "Azul", "planned"),
    ] {
        let i = common::item(&env.app, kind, title).await;
        env.app
            .add_to_library(me, i.id, Some(Status::new(status)))
            .await
            .unwrap();
    }
    let other = common::item(&env.app, "movie", "Solaris").await;
    env.app.add_to_library(bob, other.id, None).await.unwrap();

    let c = env.app.library_counts(me).await.unwrap();
    assert_eq!(c.total, 5);
    assert_eq!(
        c.groups,
        vec![
            (KindGroup::Watch, 3),
            (KindGroup::Read, 1),
            (KindGroup::Listen, 0),
            (KindGroup::Play, 1)
        ]
    );
    assert!(c.kinds.contains(&(KindId::new("movie").unwrap(), 2)));
    assert!(c.statuses.contains(&(Status::planned(), 3)));
    let cell = c
        .cells
        .iter()
        .find(|x| x.kind.as_str() == "movie" && x.status == Status::completed())
        .unwrap();
    assert_eq!((cell.group, cell.count), (KindGroup::Watch, 1));
}

#[tokio::test]

async fn query_filters_by_group_kind_status_facet_and_sorts_and_pages() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let heat = item_with(
        &env,
        "movie",
        "Heat",
        1995,
        serde_json::json!({"genres": ["Crime"]}),
    )
    .await;
    let alien = item_with(
        &env,
        "movie",
        "Alien",
        1979,
        serde_json::json!({"genres": ["Horror"]}),
    )
    .await;
    let dark = item_with(&env, "show", "Dark", 2017, serde_json::json!({})).await;
    let dune = item_with(&env, "book", "Dune", 1965, serde_json::json!({})).await;
    for i in [&heat, &alien, &dark, &dune] {
        env.app.add_to_library(me, i.id, None).await.unwrap();
        env.clock.advance(chrono::Duration::minutes(1));
    }
    env.app
        .rate(me, alien.id, Some(Rating::new(90).unwrap()), None)
        .await
        .unwrap();
    env.app
        .log_progress(me, heat.id, Progress::Finished, None, None)
        .await
        .unwrap();

    // default: last activity first (Heat was logged last, the others by when added)
    let all = env
        .app
        .library_query(me, LibraryQuery::default())
        .await
        .unwrap();
    assert_eq!(all.total, 4);
    assert_eq!(titles(&all), ["Heat", "Dune", "Dark", "Alien"]);

    let watch = LibraryQuery {
        group: Some(KindGroup::Watch),
        sort: LibrarySort::Title,
        ..LibraryQuery::default()
    };
    assert_eq!(
        titles(&env.app.library_query(me, watch).await.unwrap()),
        ["Alien", "Dark", "Heat"]
    );

    let q = LibraryQuery {
        kinds: vec![KindId::new("movie").unwrap()],
        statuses: vec![Status::planned()],
        ..LibraryQuery::default()
    };
    assert_eq!(
        titles(&env.app.library_query(me, q).await.unwrap()),
        ["Alien"]
    );

    let q = LibraryQuery {
        facets: vec![("genres".into(), "Crime".into())],
        ..LibraryQuery::default()
    };
    assert_eq!(
        titles(&env.app.library_query(me, q).await.unwrap()),
        ["Heat"]
    );

    let q = LibraryQuery {
        facets: vec![("not_a_facet".into(), "x".into())],
        ..LibraryQuery::default()
    };
    assert!(env.app.library_query(me, q).await.is_err(), "unknown facet");

    // the second page starts after the first page's cursor (keyset paging, DEC-416)
    let q = LibraryQuery {
        sort: LibrarySort::Year,
        descending: Some(false),
        limit: 1,
        ..LibraryQuery::default()
    };
    let first = env.app.library_query(me, q.clone()).await.unwrap();
    assert_eq!(titles(&first), ["Dune"]);
    let q = LibraryQuery {
        after: first.next,
        limit: 2,
        ..q
    };
    let page = env.app.library_query(me, q).await.unwrap();
    assert_eq!(
        (titles(&page), page.total),
        (vec!["Alien".to_string(), "Heat".into()], 4)
    );

    let q = LibraryQuery {
        sort: LibrarySort::Rating,
        limit: 1,
        ..LibraryQuery::default()
    };
    assert_eq!(
        titles(&env.app.library_query(me, q).await.unwrap()),
        ["Alien"]
    );

    let q = LibraryQuery {
        items: Some(vec![dune.id]),
        ..LibraryQuery::default()
    };
    assert_eq!(
        titles(&env.app.library_query(me, q).await.unwrap()),
        ["Dune"]
    );

    // rows carry the next step
    let row = env.app.library_row(me, heat.id).await.unwrap();
    assert_eq!(row.next_step, app::NextStep::Finish { again: true });
    assert_eq!(row.group, KindGroup::Watch);
}

#[tokio::test]
async fn tags_add_remove_list_rename_and_filter() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    let heat = common::item(&env.app, "movie", "Heat").await;
    let dune = common::item(&env.app, "book", "Dune").await;
    assert_eq!(
        env.app.add_tag(me, heat.id, "  cosy ").await.unwrap(),
        vec!["cosy".to_string()]
    );
    // same tag in another case is the same tag
    env.app.add_tag(me, heat.id, "Cosy").await.unwrap();
    let tags = env.app.add_tag(me, heat.id, "with Anna").await.unwrap();
    assert_eq!(tags, ["cosy", "with Anna"]);
    env.app.add_tag(me, dune.id, "cosy").await.unwrap();
    assert!(env.app.add_tag(me, dune.id, "  ").await.is_err());
    env.app.add_tag(bob, heat.id, "bobs").await.unwrap();

    let counts = env.app.tags(me).await.unwrap();
    let pairs: Vec<(String, u64)> = counts.iter().map(|t| (t.tag.clone(), t.entries)).collect();
    assert_eq!(pairs, vec![("cosy".into(), 2), ("with Anna".into(), 1)]);

    let q = LibraryQuery {
        tags: vec!["COSY".into(), "with anna".into()],
        ..LibraryQuery::default()
    };
    let page = env.app.library_query(me, q).await.unwrap();
    assert_eq!(titles(&page), ["Heat"]);
    assert_eq!(page.rows[0].tags, ["cosy", "with Anna"]);

    assert_eq!(
        env.app.rename_tag(me, "with anna", "cosy").await.unwrap(),
        1
    );
    assert_eq!(env.app.entry_tags(me, heat.id).await.unwrap(), ["cosy"]);
    assert_eq!(env.app.rename_tag(me, "cosy", "Warm").await.unwrap(), 2);
    assert_eq!(env.app.entry_tags(me, dune.id).await.unwrap(), ["Warm"]);
    assert_eq!(
        env.app.remove_tag(me, dune.id, "warm").await.unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        env.app.entry_tags(bob, heat.id).await.unwrap(),
        ["bobs"],
        "bob's tags are his"
    );
}

#[tokio::test]
async fn continue_is_in_progress_by_last_activity_and_up_next_prefers_the_next_in_series() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let dark = common::item(&env.app, "show", "Dark").await;
    let lost = common::item(&env.app, "show", "Lost").await;
    let ep = Progress::Episode {
        season: 1,
        episode: 1,
    };
    env.app
        .log_progress(me, dark.id, ep.clone(), None, None)
        .await
        .unwrap();
    env.clock.advance(chrono::Duration::minutes(5));
    env.app
        .log_progress(me, lost.id, ep, None, None)
        .await
        .unwrap();
    let rows = env.app.continue_rows(me, 6).await.unwrap();
    let names: Vec<_> = rows.iter().map(|r| r.item.title.as_str()).collect();
    assert_eq!(names, ["Lost", "Dark"]);

    let series = |pos: f64| serde_json::json!({"series": "Dune", "series_position": pos});
    let dune1 = item_with(&env, "book", "Dune", 1965, series(1.0)).await;
    let dune2 = item_with(&env, "book", "Dune Messiah", 1969, series(2.0)).await;
    let other = item_with(&env, "book", "Emma", 1815, serde_json::json!({})).await;
    env.app
        .add_to_library(me, dune2.id, Some(Status::planned()))
        .await
        .unwrap();
    env.clock.advance(chrono::Duration::minutes(1));
    env.app
        .add_to_library(me, other.id, Some(Status::planned()))
        .await
        .unwrap();
    env.app
        .add_to_library(me, dune1.id, Some(Status::completed()))
        .await
        .unwrap();
    let next = env.app.up_next(me, 6).await.unwrap();
    let names: Vec<_> = next
        .iter()
        .map(|u| (u.row.item.title.as_str(), u.reason))
        .collect();
    assert_eq!(
        names,
        [
            ("Dune Messiah", UpNextReason::NextInSeries),
            ("Emma", UpNextReason::Planned)
        ]
    );
}

#[tokio::test]
async fn up_next_finds_the_next_in_series_behind_many_newer_planned_entries() {
    // UX1-1: the candidates are bounded, not "the newest planned rows", so an old next part
    // is still found and ranked first
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let series = |pos: f64| serde_json::json!({"series": " Dune ", "series_position": pos});
    let dune1 = item_with(&env, "book", "Dune", 1965, series(1.0)).await;
    let dune2 = item_with(&env, "book", "Dune Messiah", 1969, series(2.0)).await;
    let dune4 = item_with(&env, "book", "God Emperor", 1981, series(4.0)).await;
    env.app
        .add_to_library(me, dune1.id, Some(Status::completed()))
        .await
        .unwrap();
    env.app
        .add_to_library(me, dune2.id, Some(Status::planned()))
        .await
        .unwrap();
    env.app
        .add_to_library(me, dune4.id, Some(Status::planned()))
        .await
        .unwrap();
    for i in 0..510 {
        env.clock.advance(chrono::Duration::seconds(1));
        let m = common::item(&env.app, "movie", &format!("Movie {i}")).await;
        env.app
            .add_to_library(me, m.id, Some(Status::planned()))
            .await
            .unwrap();
    }
    let next = env.app.up_next(me, 6).await.unwrap();
    assert_eq!(next.len(), 6);
    assert_eq!(next[0].row.item.title, "Dune Messiah");
    assert_eq!(next[0].reason, UpNextReason::NextInSeries);
    let rest: Vec<_> = next[1..]
        .iter()
        .map(|u| (u.row.item.title.as_str(), u.reason))
        .collect();
    assert_eq!(
        rest,
        [
            ("Movie 509", UpNextReason::Planned),
            ("Movie 508", UpNextReason::Planned),
            ("Movie 507", UpNextReason::Planned),
            ("Movie 506", UpNextReason::Planned),
            ("Movie 505", UpNextReason::Planned),
        ]
    );
}

fn entry_ids(p: &app::LibraryPage) -> Vec<domain::ids::EntryId> {
    p.rows.iter().map(|r| r.entry.id).collect()
}

/// Keyset paging (DEC-416) on the in-memory store: walking every sort in both directions in
/// pages of 1 to 3 returns one full read's rows, ties and missing values included, and the
/// library page walks the same way; a cursor of another order is refused.
#[tokio::test]
async fn keyset_pages_walk_every_order() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "kim").await;
    let rows = [
        ("Same", Some(2000), Some(60)),
        ("same", Some(2000), Some(60)),
        ("SAME", None, None),
        ("Other", Some(1990), Some(60)),
        ("Zed", None, Some(100)),
    ];
    for (n, (title, year, rating)) in rows.into_iter().enumerate() {
        let i = env
            .app
            .upsert_catalog_item(NewCatalogItem {
                kind: KindId::new("movie").unwrap(),
                title: title.into(),
                original_title: None,
                year,
                description: None,
                external_ids: vec![ExternalId::new("test", title)],
                attributes: serde_json::json!({}),
                cover_url: None,
                popularity: None,
            })
            .await
            .unwrap();
        env.app.add_to_library(me, i.id, None).await.unwrap();
        // two entries share the time they were added
        if n != 1 {
            env.clock.advance(chrono::Duration::minutes(1));
        }
        if let Some(r) = rating {
            env.app
                .rate(me, i.id, Some(Rating::new(r).unwrap()), None)
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
            let all = entry_ids(
                &env.app
                    .library_query(me, full(sort, descending))
                    .await
                    .unwrap(),
            );
            assert_eq!(all.len(), 5);
            for limit in 1..=3 {
                let mut q = LibraryQuery {
                    limit,
                    ..full(sort, descending)
                };
                let mut walked = vec![];
                loop {
                    let p = env.app.library_query(me, q.clone()).await.unwrap();
                    assert_eq!(p.total, 5);
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
    let first = env
        .app
        .library_query(
            me,
            LibraryQuery {
                limit: 1,
                ..full(LibrarySort::Year, true)
            },
        )
        .await
        .unwrap();
    let wrong = LibraryQuery {
        after: first.next.clone(),
        ..full(LibrarySort::Rating, true)
    };
    assert!(env.app.library_query(me, wrong).await.is_err());
    assert!(
        env.app
            .library_page(me, first.next.as_ref(), 2)
            .await
            .is_err()
    );

    let mut expected = env.app.library(me).await.unwrap();
    expected.sort_by_key(|e| std::cmp::Reverse((e.added_at, e.id)));
    let expected: Vec<_> = expected.into_iter().map(|e| e.id).collect();
    let mut after = None;
    let mut walked = vec![];
    loop {
        let (entries, total, next) = env.app.library_page(me, after.as_ref(), 2).await.unwrap();
        assert_eq!(total, 5);
        walked.extend(entries.iter().map(|e| e.id));
        match next {
            Some(c) => after = Some(c),
            None => break,
        }
    }
    assert_eq!(walked, expected);
}
