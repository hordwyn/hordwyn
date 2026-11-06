//! The app use-cases running on Postgres instead of the in-memory fakes.

mod common;

use app::{AppError, NewSession};
use domain::hlc::Hlc;
use domain::kind::Status;
use domain::library::Rating;
use domain::list::Visibility;
use domain::progress::Progress;
use domain::session::Participant;

#[tokio::test]
async fn library_progress_rating_review_roundtrip() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "alice").await;
    let movie = common::item(app, "movie", "Heat").await;
    assert_eq!(app.catalog_item(movie.id).await.unwrap(), movie);

    let again = common::item(app, "movie", "Heat").await;
    assert_eq!(again.id, movie.id, "dedupe by external id");

    app.add_to_library(me, movie.id, None).await.unwrap();
    let op = uuid::Uuid::from_u128(999);
    let e1 = app
        .log_progress(me, movie.id, Progress::Finished, None, Some(op))
        .await
        .unwrap();
    let e2 = app
        .log_progress(me, movie.id, Progress::Finished, None, Some(op))
        .await
        .unwrap();
    assert_eq!(e1.id, e2.id);
    assert_eq!(app.progress(me, movie.id).await.unwrap(), vec![e1]);

    // LWW on the stamp, not on arrival order: a newer server write, then an older offline
    // edit that arrives later and loses
    app.rate(me, movie.id, Some(Rating::new(80).unwrap()), None)
        .await
        .unwrap();
    app.rate(
        me,
        movie.id,
        Some(Rating::new(5).unwrap()),
        Some(Hlc::new(1, 0, 1)),
    )
    .await
    .unwrap();
    let entry = app.entry(me, movie.id).await.unwrap().unwrap();
    assert_eq!(entry.rating.value, Some(Rating::new(80).unwrap()));
    assert_eq!(entry.status.value, Status::completed());
    assert_eq!(app.library(me).await.unwrap(), vec![entry]);

    let r = app
        .write_review(me, movie.id, "Tense.", true, None)
        .await
        .unwrap();
    let r2 = app
        .write_review(me, movie.id, "Very tense.", false, None)
        .await
        .unwrap();
    assert_eq!(r.id, r2.id);
}

#[tokio::test]
async fn lists_keep_order() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "alice").await;
    let a = common::item(app, "movie", "A").await;
    let b = common::item(app, "book", "B").await;
    let l = app
        .create_list(me, "Mix", None, Visibility::Private)
        .await
        .unwrap();
    app.add_to_list(me, l.id, a.id, None).await.unwrap();
    app.add_to_list(me, l.id, b.id, Some("note".into()))
        .await
        .unwrap();
    app.move_in_list(me, l.id, b.id, 0).await.unwrap();
    let got = app.list(me, l.id).await.unwrap();
    assert_eq!(
        got.items.iter().map(|i| i.item_id).collect::<Vec<_>>(),
        vec![b.id, a.id]
    );
    assert_eq!(got.items[0].note.as_deref(), Some("note"));
    assert_eq!(app.lists(me).await.unwrap().len(), 1);
    app.delete_list(me, l.id).await.unwrap();
    assert!(matches!(
        app.list(me, l.id).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn companion_lifecycle_without_rewriting_history() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (u, me) = common::user(app, "alice").await;
    let (bob_user, bob) = common::user(app, "bob").await;
    let game = common::item(app, "board_game", "Azul").await;

    let anna = app.create_companion(me, "Anna").await.unwrap();
    let dup = app.create_companion(me, "Anna 2").await.unwrap();
    let carl = app.create_companion(me, "Carl").await.unwrap();
    let b = app.create_companion(me, "Bob").await.unwrap();
    let mut winner = Participant::new(u.self_companion);
    winner.winner = true;
    winner.score = Some(77.5);
    let s = app
        .log_session(
            me,
            NewSession::new(
                game.id,
                vec![
                    winner,
                    Participant::new(dup.id),
                    Participant::new(carl.id),
                    Participant::new(b.id),
                ],
            ),
        )
        .await
        .unwrap();
    let stored = app.session(me, s.id).await.unwrap();
    assert_eq!(stored, s);

    // merge
    app.merge_companions(me, anna.id, dup.id).await.unwrap();
    assert_eq!(app.resolve_companion(me, dup.id).await.unwrap().id, anna.id);
    assert_eq!(
        app.session(me, s.id).await.unwrap().participants[1].companion_id,
        dup.id
    );

    // link + shared view
    let req = app.request_link(me, b.id, bob_user.id).await.unwrap();
    assert_eq!(app.link_requests(bob).await.unwrap().len(), 1);
    app.accept_link(bob, req.id).await.unwrap();
    assert_eq!(app.shared_sessions(bob).await.unwrap().len(), 1);

    // promote
    let (_, token) = app.promote_companion(me, carl.id, None).await.unwrap();
    let carl_user = app.accept_invite(&token, "carl", "Carl").await.unwrap();
    let carl_actor = domain::user::Actor {
        user_id: carl_user.id,
        role: carl_user.role,
    };
    assert_eq!(app.shared_sessions(carl_actor).await.unwrap()[0].id, s.id);

    // demote by deleting bob's account
    app.delete_user(bob, bob_user.id).await.unwrap();
    let after = app.companion(me, b.id).await.unwrap();
    assert!(after.linked_user.is_none());
    assert_eq!(after.snapshot.unwrap().display_name, "BOB");
    assert_eq!(app.session(me, s.id).await.unwrap(), s, "history unchanged");
}

#[tokio::test]
async fn duplicate_username_is_a_conflict() {
    let Some(env) = common::env().await else {
        return;
    };
    common::user(&env.app, "alice").await;
    let err = env
        .app
        .register_user("alice", "A", None, domain::user::Role::Member)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Conflict(_)));
}
