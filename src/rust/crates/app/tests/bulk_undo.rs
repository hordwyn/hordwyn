//! UX4-5 and UX4-12: undoing a bulk op puts back only what that op changed; a change made in
//! between (on the item page, another device, an import) stays. The undo of a removal fits
//! the request body limit, and the list half of an undo cannot fail half way.

mod common;

use app::{BulkOp, MAX_UNDO_BYTES};
use domain::kind::Status;
use domain::library::Rating;
use domain::list::Visibility;
use testkit::TestEnv;

async fn status(app: &app::App, me: domain::user::Actor, item: domain::ids::ItemId) -> Status {
    app.entry(me, item).await.unwrap().unwrap().status.value
}

#[tokio::test]
async fn a_status_undo_keeps_what_changed_in_between() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let app = &env.app;
    let heat = common::item(app, "movie", "Heat").await;
    let dune = common::item(app, "book", "Dune").await;
    for i in [heat.id, dune.id] {
        app.add_to_library(me, i, None).await.unwrap();
    }
    app.rate(me, heat.id, Some(Rating::new(80).unwrap()), None)
        .await
        .unwrap();
    let out = app
        .bulk(
            me,
            &[heat.id, dune.id],
            BulkOp::SetStatus(Status::in_progress()),
        )
        .await
        .unwrap();
    // in between: heat's rating and note change, dune's status changes again
    app.rate(me, heat.id, Some(Rating::new(60).unwrap()), None)
        .await
        .unwrap();
    app.set_notes(me, heat.id, Some("later".into()), None)
        .await
        .unwrap();
    app.add_tag(me, heat.id, "late").await.unwrap();
    app.set_status(me, dune.id, Status::completed(), None)
        .await
        .unwrap();
    app.undo_bulk(me, &out.undo).await.unwrap();
    let back = app.entry(me, heat.id).await.unwrap().unwrap();
    assert_eq!(back.status.value, Status::planned());
    assert_eq!(back.rating.value, Some(Rating::new(60).unwrap()));
    assert_eq!(back.notes.value.as_deref(), Some("later"));
    assert_eq!(app.entry_tags(me, heat.id).await.unwrap(), ["late"]);
    assert_eq!(
        status(app, me, dune.id).await,
        Status::completed(),
        "newer wins"
    );
}

#[tokio::test]
async fn a_tag_undo_touches_that_tag_only() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let app = &env.app;
    let heat = common::item(app, "movie", "Heat").await;
    let dune = common::item(app, "book", "Dune").await;
    for i in [heat.id, dune.id] {
        app.add_to_library(me, i, None).await.unwrap();
    }
    app.add_tag(me, dune.id, "keep").await.unwrap();
    let added = app
        .bulk(me, &[heat.id, dune.id], BulkOp::AddTag("Cosy".into()))
        .await
        .unwrap();
    app.add_tag(me, heat.id, "late").await.unwrap();
    app.set_status(me, heat.id, Status::completed(), None)
        .await
        .unwrap();
    app.undo_bulk(me, &added.undo).await.unwrap();
    assert_eq!(app.entry_tags(me, heat.id).await.unwrap(), ["late"]);
    assert_eq!(app.entry_tags(me, dune.id).await.unwrap(), ["keep"]);
    assert_eq!(status(app, me, heat.id).await, Status::completed());

    let removed = app
        .bulk(me, &[dune.id], BulkOp::RemoveTag("keep".into()))
        .await
        .unwrap();
    app.add_tag(me, dune.id, "new").await.unwrap();
    app.undo_bulk(me, &removed.undo).await.unwrap();
    assert_eq!(app.entry_tags(me, dune.id).await.unwrap(), ["keep", "new"]);
}

#[tokio::test]
async fn a_removal_undo_leaves_an_entry_added_again_in_between() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let app = &env.app;
    let heat = common::item(app, "movie", "Heat").await;
    let dune = common::item(app, "book", "Dune").await;
    for i in [heat.id, dune.id] {
        app.add_to_library(me, i, None).await.unwrap();
    }
    app.set_notes(me, dune.id, Some("mine".into()), None)
        .await
        .unwrap();
    let out = app
        .bulk(me, &[heat.id, dune.id], BulkOp::Remove)
        .await
        .unwrap();
    app.add_to_library(me, heat.id, Some(Status::completed()))
        .await
        .unwrap();
    app.undo_bulk(me, &out.undo).await.unwrap();
    assert_eq!(status(app, me, heat.id).await, Status::completed());
    let dune_back = app.entry(me, dune.id).await.unwrap().unwrap();
    assert_eq!(dune_back.notes.value.as_deref(), Some("mine"));
}

#[tokio::test]
async fn an_add_to_list_undo_after_the_list_was_deleted_is_nothing_to_undo() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let app = &env.app;
    let heat = common::item(app, "movie", "Heat").await;
    app.add_to_library(me, heat.id, None).await.unwrap();
    let list = app
        .create_list(me, "Mix", None, Visibility::Private)
        .await
        .unwrap();
    let out = app
        .bulk(me, &[heat.id], BulkOp::AddToList(list.id))
        .await
        .unwrap();
    app.delete_list(me, list.id).await.unwrap();
    app.undo_bulk(me, &out.undo).await.unwrap();
    // nothing gained: the undo has nothing to do and names no list
    let again = app
        .create_list(me, "Other", None, Visibility::Private)
        .await
        .unwrap();
    app.add_to_list(me, again.id, heat.id, None).await.unwrap();
    let none = app
        .bulk(me, &[heat.id], BulkOp::AddToList(again.id))
        .await
        .unwrap();
    assert_eq!(none.undo, app::BulkUndo::default());
}

#[tokio::test]
async fn a_removal_too_big_to_undo_is_refused_before_anything_changes() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let app = &env.app;
    let mut ids = Vec::new();
    for n in 0..4 {
        let i = common::item(app, "movie", &format!("Long {n}")).await;
        app.add_to_library(me, i.id, None).await.unwrap();
        app.set_notes(me, i.id, Some("x".repeat(MAX_UNDO_BYTES / 3)), None)
            .await
            .unwrap();
        ids.push(i.id);
    }
    assert!(app.bulk(me, &ids, BulkOp::Remove).await.is_err());
    assert_eq!(app.library_counts(me).await.unwrap().total, 4);
    // fewer at a time fit
    let out = app.bulk(me, &ids[..2], BulkOp::Remove).await.unwrap();
    assert!(serde_json::to_vec(&out.undo).unwrap().len() <= MAX_UNDO_BYTES);
}
