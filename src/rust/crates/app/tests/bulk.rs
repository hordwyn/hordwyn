//! Remove from library (DEC-348 "delete keeps history") and bulk edits with undo
//! (design.md 3.5, 6.2; phase4-plan WP4-3).

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::{BulkFailure, BulkOp, MAX_BULK};
use domain::kind::Status;
use domain::library::Rating;
use domain::list::Visibility;
use domain::progress::Progress;
use testkit::TestEnv;

#[tokio::test]
async fn remove_keeps_history_and_undo_restores_the_entry() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let heat = common::item(&env.app, "movie", "Heat").await;
    let app = &env.app;
    app.add_to_library(me, heat.id, None).await.unwrap();
    app.log_progress(me, heat.id, Progress::Finished, None, None)
        .await
        .unwrap();
    app.rate(me, heat.id, Some(Rating::new(80).unwrap()), None)
        .await
        .unwrap();
    app.set_notes(me, heat.id, Some("great".into()), None)
        .await
        .unwrap();
    app.add_tag(me, heat.id, "Cosy").await.unwrap();
    let list = app
        .create_list(me, "Crime", None, Visibility::Private)
        .await
        .unwrap();
    app.add_to_list(me, list.id, heat.id, None).await.unwrap();
    let before = app.entry(me, heat.id).await.unwrap().unwrap();
    let before_row = app.library_row(me, heat.id).await.unwrap();
    let stats = app.stats_totals(me).await.unwrap();
    assert_eq!(stats.overall.amounts.finished, 1);

    let undo = app.remove_from_library(me, heat.id).await.unwrap();
    assert_eq!(app.entry(me, heat.id).await.unwrap(), None);
    assert_eq!(app.library_counts(me).await.unwrap().total, 0);
    // history, statistics and list membership stay
    assert_eq!(app.progress(me, heat.id).await.unwrap().len(), 1);
    let after = app.stats_totals(me).await.unwrap();
    assert_eq!(
        (after.overall.entries, after.overall.amounts.finished),
        (0, 1)
    );
    assert_eq!(app.list(me, list.id).await.unwrap().items.len(), 1);
    // removing again: not in the library
    assert!(app.remove_from_library(me, heat.id).await.is_err());

    // undo brings back status, rating, notes, tags and the place in the library
    app.undo_bulk(me, &undo).await.unwrap();
    let back = app.entry(me, heat.id).await.unwrap().unwrap();
    assert_eq!(
        (
            back.id,
            back.added_at,
            &back.status.value,
            back.rating.value,
            &back.notes.value
        ),
        (
            before.id,
            before.added_at,
            &before.status.value,
            before.rating.value,
            &before.notes.value
        )
    );
    assert_eq!(app.entry_tags(me, heat.id).await.unwrap(), ["Cosy"]);
    let row = app.library_row(me, heat.id).await.unwrap();
    assert_eq!(
        (row.summary, row.last_activity),
        (before_row.summary, before_row.last_activity)
    );

    // re-adding after a removal counts the old history again
    app.remove_from_library(me, heat.id).await.unwrap();
    let again = app
        .add_to_library(me, heat.id, Some(Status::completed()))
        .await
        .unwrap();
    assert_ne!(again.id, before.id);
    let row = app.library_row(me, heat.id).await.unwrap();
    assert_eq!(row.summary.finished, 1);
    assert_eq!(app.stats_totals(me).await.unwrap().overall.entries, 1);
}

#[tokio::test]
async fn bulk_edits_report_each_id_and_undo_in_one_step() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    let app = &env.app;
    let heat = common::item(&env.app, "movie", "Heat").await;
    let dark = common::item(&env.app, "show", "Dark").await;
    let dune = common::item(&env.app, "book", "Dune").await;
    let bobs = common::item(&env.app, "movie", "Solaris").await;
    for i in [&heat, &dark, &dune] {
        app.add_to_library(me, i.id, None).await.unwrap();
    }
    app.add_to_library(bob, bobs.id, None).await.unwrap();
    app.add_tag(me, dune.id, "keep").await.unwrap();
    let ids = [heat.id, bobs.id, dark.id, dune.id];

    // set status: the foreign id fails alone, the others change
    let out = app
        .bulk(me, &ids, BulkOp::SetStatus(Status::in_progress()))
        .await
        .unwrap();
    assert_eq!(
        out.results,
        [
            (heat.id, None),
            (bobs.id, Some(BulkFailure::NotInLibrary)),
            (dark.id, None),
            (dune.id, None)
        ]
    );
    for i in [&heat, &dark, &dune] {
        assert_eq!(
            app.entry(me, i.id).await.unwrap().unwrap().status.value,
            Status::in_progress()
        );
    }
    assert_eq!(
        app.entry(bob, bobs.id).await.unwrap().unwrap().status.value,
        Status::planned()
    );
    app.undo_bulk(me, &out.undo).await.unwrap();
    for i in [&heat, &dark, &dune] {
        assert_eq!(
            app.entry(me, i.id).await.unwrap().unwrap().status.value,
            Status::planned()
        );
    }

    // a status one kind does not allow fails for that id only
    let out = app
        .bulk(
            me,
            &[heat.id, dune.id],
            BulkOp::SetStatus(Status::new("endless")),
        )
        .await;
    assert!(out.is_ok());
    let out = out.unwrap();
    assert_eq!(
        out.results,
        [
            (heat.id, Some(BulkFailure::StatusNotAllowed)),
            (dune.id, Some(BulkFailure::StatusNotAllowed))
        ]
    );

    // tags
    let out = app
        .bulk(me, &[heat.id, dune.id], BulkOp::AddTag("Cosy".into()))
        .await
        .unwrap();
    assert!(out.results.iter().all(|(_, r)| r.is_none()));
    assert_eq!(app.entry_tags(me, heat.id).await.unwrap(), ["Cosy"]);
    assert_eq!(app.entry_tags(me, dune.id).await.unwrap(), ["Cosy", "keep"]);
    app.undo_bulk(me, &out.undo).await.unwrap();
    assert_eq!(app.entry_tags(me, heat.id).await.unwrap().len(), 0);
    assert_eq!(app.entry_tags(me, dune.id).await.unwrap(), ["keep"]);
    let out = app
        .bulk(me, &[dune.id], BulkOp::RemoveTag("KEEP".into()))
        .await
        .unwrap();
    assert_eq!(app.entry_tags(me, dune.id).await.unwrap().len(), 0);
    app.undo_bulk(me, &out.undo).await.unwrap();
    assert_eq!(app.entry_tags(me, dune.id).await.unwrap(), ["keep"]);
    assert!(
        app.bulk(me, &[dune.id], BulkOp::AddTag("  ".into()))
            .await
            .is_err()
    );

    // add to a list: only the items not there yet go, and undo takes only those out
    let list = app
        .create_list(me, "Mix", None, Visibility::Private)
        .await
        .unwrap();
    app.add_to_list(me, list.id, dark.id, None).await.unwrap();
    let out = app
        .bulk(me, &[heat.id, dark.id, bobs.id], BulkOp::AddToList(list.id))
        .await
        .unwrap();
    assert_eq!(out.results[2], (bobs.id, Some(BulkFailure::NotInLibrary)));
    let items: Vec<_> = app
        .list(me, list.id)
        .await
        .unwrap()
        .items
        .iter()
        .map(|i| i.item_id)
        .collect();
    assert_eq!(items, [dark.id, heat.id]);
    app.undo_bulk(me, &out.undo).await.unwrap();
    let items: Vec<_> = app
        .list(me, list.id)
        .await
        .unwrap()
        .items
        .iter()
        .map(|i| i.item_id)
        .collect();
    assert_eq!(items, [dark.id]);
    // someone else's list
    let theirs = app
        .create_list(bob, "Bob's", None, Visibility::Private)
        .await
        .unwrap();
    assert!(
        app.bulk(me, &[heat.id], BulkOp::AddToList(theirs.id))
            .await
            .is_err()
    );

    // remove, then undo
    let out = app
        .bulk(me, &[heat.id, dune.id, bobs.id], BulkOp::Remove)
        .await
        .unwrap();
    assert_eq!(out.results[2], (bobs.id, Some(BulkFailure::NotInLibrary)));
    assert_eq!(app.library_counts(me).await.unwrap().total, 1);
    assert!(app.entry(bob, bobs.id).await.unwrap().is_some());
    app.undo_bulk(me, &out.undo).await.unwrap();
    assert_eq!(app.library_counts(me).await.unwrap().total, 3);
    assert_eq!(app.entry_tags(me, dune.id).await.unwrap(), ["keep"]);

    // the same id twice counts once; more than MAX_BULK ids are refused
    let out = app
        .bulk(
            me,
            &[heat.id, heat.id],
            BulkOp::SetStatus(Status::completed()),
        )
        .await
        .unwrap();
    assert_eq!(out.results, [(heat.id, None)]);
    let many = vec![heat.id; MAX_BULK + 1];
    assert!(app.bulk(me, &many, BulkOp::Remove).await.is_err());
}

#[tokio::test]
async fn an_undo_token_is_checked_and_only_touches_the_callers_library() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    let app = &env.app;
    let heat = common::item(&env.app, "movie", "Heat").await;
    app.add_to_library(me, heat.id, None).await.unwrap();
    let out = app.bulk(me, &[heat.id], BulkOp::Remove).await.unwrap();
    // the JSON form survives a round trip (the server signs it into the token, api tests)
    let text = serde_json::to_string(&out.undo).unwrap();
    assert_eq!(
        serde_json::from_str::<app::BulkUndo>(&text).unwrap(),
        out.undo
    );
    // bob replaying alice's token restores into bob's library, never alice's
    app.undo_bulk(bob, &out.undo).await.unwrap();
    assert_eq!(app.entry(me, heat.id).await.unwrap(), None);
    let bobs = app.entry(bob, heat.id).await.unwrap().unwrap();
    assert_eq!(bobs.user_id, bob.user_id);
    // alice's undo still works and gets her own entry back
    app.undo_bulk(me, &out.undo).await.unwrap();
    assert!(app.entry(me, heat.id).await.unwrap().is_some());
    assert_ne!(app.entry(me, heat.id).await.unwrap().unwrap().id, bobs.id);
}
