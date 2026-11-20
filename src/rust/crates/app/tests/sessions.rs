mod common;

use app::{AppError, NewSession};
use domain::progress::Progress;
use domain::session::Participant;
use testkit::TestEnv;

#[tokio::test]
async fn session_on_sessions_kind_appends_progress_event() {
    let env = TestEnv::new();
    let (u, me) = common::user(&env, "alice").await;
    let game = common::item(&env.app, "board_game", "Brass").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let mut p_me = Participant::new(u.self_companion);
    p_me.score = Some(152.0);
    p_me.winner = true;
    let mut p_anna = Participant::new(anna.id);
    p_anna.score = Some(140.0);
    let s = env
        .app
        .log_session(me, NewSession::new(game.id, vec![p_me, p_anna]))
        .await
        .unwrap();
    let events = env.app.progress(me, game.id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].progress, Progress::Session { session_id: s.id });
    assert!(env.app.entry(me, game.id).await.unwrap().is_some());
}

#[tokio::test]
async fn watched_with_reuses_sessions_for_other_kinds() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Coco").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    env.app
        .log_session(
            me,
            NewSession::new(movie.id, vec![Participant::new(anna.id)]),
        )
        .await
        .unwrap();
    assert_eq!(env.app.progress(me, movie.id).await.unwrap().len(), 0);
}

#[tokio::test]
async fn participants_must_be_own_companions_and_unique() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    let game = common::item(&env.app, "board_game", "Root").await;
    let bobs = env.app.create_companion(bob, "Zed").await.unwrap();
    assert!(matches!(
        env.app
            .log_session(
                me,
                NewSession::new(game.id, vec![Participant::new(bobs.id)])
            )
            .await,
        Err(AppError::Forbidden)
    ));
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    assert!(
        env.app
            .log_session(
                me,
                NewSession::new(
                    game.id,
                    vec![Participant::new(anna.id), Participant::new(anna.id)]
                )
            )
            .await
            .is_err()
    );
}

/// Alice logs a play with her companion "B." who is linked to Bob.
async fn shared_play(
    env: &TestEnv,
    kind: &str,
) -> (
    domain::user::Actor,
    domain::user::Actor,
    domain::session::Session,
) {
    let (_, alice) = common::user(env, "alice").await;
    let (bob_user, bob) = common::user(env, "bob").await;
    let item = common::item(&env.app, kind, "Azul").await;
    let c = env.app.create_companion(alice, "B.").await.unwrap();
    let req = env
        .app
        .request_link(alice, c.id, bob_user.id)
        .await
        .unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();
    let mut p = Participant::new(c.id);
    p.score = Some(61.0);
    p.winner = true;
    let mut new = NewSession::new(item.id, vec![p]);
    new.notes = Some("alice's private note".into());
    let s = env.app.log_session(alice, new).await.unwrap();
    (alice, bob, s)
}

#[tokio::test]
async fn claiming_a_shared_session_copies_it_once() {
    let env = TestEnv::new();
    let (alice, bob, s) = shared_play(&env, "board_game").await;
    let copy = env.app.claim_session(bob, s.id).await.unwrap();
    assert_ne!(copy.id, s.id);
    assert_eq!(copy.owner, bob.user_id);
    assert_eq!(copy.started_at, s.started_at);
    assert_eq!(copy.notes, None, "notes stay with the owner");
    let me = env.app.user(bob.user_id).await.unwrap();
    assert_eq!(copy.participants.len(), 1);
    assert_eq!(copy.participants[0].companion_id, me.self_companion);
    assert_eq!(copy.participants[0].score, Some(61.0));
    assert!(copy.participants[0].winner);
    let events = env.app.progress(bob, s.item_id).await.unwrap();
    assert_eq!(events.len(), 1);

    // claiming again is a no-op
    let again = env.app.claim_session(bob, s.id).await.unwrap();
    assert_eq!(again.id, copy.id);
    assert_eq!(env.app.progress(bob, s.item_id).await.unwrap().len(), 1);
    // the owner's history is untouched, and the owner cannot claim their own
    assert_eq!(env.app.session(alice, s.id).await.unwrap(), s);
    assert!(matches!(
        env.app.claim_session(alice, s.id).await,
        Err(AppError::Conflict(_))
    ));
    // the copy is not shared back as "with me" to anyone
    assert_eq!(env.app.shared_sessions(alice).await.unwrap().len(), 0);
}

#[tokio::test]
async fn claiming_a_watched_with_session_logs_a_finish() {
    let env = TestEnv::new();
    let (_, bob, s) = shared_play(&env, "movie").await;
    env.app.claim_session(bob, s.id).await.unwrap();
    let events = env.app.progress(bob, s.item_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].progress, Progress::Finished);
    assert_eq!(events[0].occurred_at, s.started_at);
}

#[tokio::test]
async fn strangers_cannot_claim() {
    let env = TestEnv::new();
    let (_, _, s) = shared_play(&env, "board_game").await;
    let (_, eve) = common::user(&env, "eve").await;
    assert!(matches!(
        env.app.claim_session(eve, s.id).await,
        Err(AppError::Forbidden)
    ));
}

#[tokio::test]
async fn owners_can_stop_sharing_sessions() {
    let env = TestEnv::new();
    let (alice, bob, s) = shared_play(&env, "board_game").await;
    assert_eq!(env.app.shared_sessions(bob).await.unwrap().len(), 1);
    let u = env.app.set_privacy(alice, false).await.unwrap();
    assert!(!u.share_sessions);
    assert_eq!(env.app.shared_sessions(bob).await.unwrap().len(), 0);
    assert!(matches!(
        env.app.session(bob, s.id).await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        env.app.claim_session(bob, s.id).await,
        Err(AppError::Forbidden)
    ));
    env.app.set_privacy(alice, true).await.unwrap();
    assert_eq!(env.app.shared_sessions(bob).await.unwrap().len(), 1);
}

#[tokio::test]
async fn undo_of_a_view_with_a_person_deletes_its_session_and_leftovers_are_found() {
    // OT-22 (#1)
    let env = TestEnv::new();
    let (u, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Coco").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let op = uuid::Uuid::from_u128(0xc1);
    let e = env
        .app
        .log_view_with(
            me,
            movie.id,
            Progress::Finished,
            None,
            Some(op),
            vec![anna.id],
        )
        .await
        .unwrap();
    let id = app::view_session_id(u.id, op);
    let session = env.app.repos.sessions.get(id).await.unwrap().unwrap();
    assert_eq!(session.item_id, movie.id);
    env.app.undo_progress(me, e.id).await.unwrap();
    assert!(env.app.repos.sessions.get(id).await.unwrap().is_none());
    assert_eq!(env.app.orphaned_view_sessions(false).await.unwrap(), vec![]);

    // what an Undo before the fix left behind, beside a view with Anna that stays
    env.app.repos.sessions.insert(&session).await.unwrap();
    let kept = uuid::Uuid::from_u128(0xc2);
    env.app
        .log_view_with(
            me,
            movie.id,
            Progress::Finished,
            None,
            Some(kept),
            vec![anna.id],
        )
        .await
        .unwrap();
    let found = env.app.orphaned_view_sessions(false).await.unwrap();
    assert_eq!(found.iter().map(|s| s.id).collect::<Vec<_>>(), vec![id]);
    assert!(
        env.app.repos.sessions.get(id).await.unwrap().is_some(),
        "a dry run"
    );
    assert_eq!(env.app.orphaned_view_sessions(true).await.unwrap().len(), 1);
    assert!(env.app.repos.sessions.get(id).await.unwrap().is_none());
    let live = app::view_session_id(u.id, kept);
    assert!(env.app.repos.sessions.get(live).await.unwrap().is_some());
    assert_eq!(env.app.orphaned_view_sessions(false).await.unwrap(), vec![]);
}

/// Bob links his own entry "Alice" to alice (the way back of the link, #15).
async fn link_back(
    env: &TestEnv,
    alice: domain::user::Actor,
    bob: domain::user::Actor,
) -> domain::ids::CompanionId {
    let c = env.app.create_companion(bob, "Alice").await.unwrap();
    let req = env
        .app
        .request_link(bob, c.id, alice.user_id)
        .await
        .unwrap();
    env.app.accept_link(alice, req.id).await.unwrap();
    c.id
}

#[tokio::test]
async fn a_copy_is_with_the_owner_when_the_copier_has_them_in_people() {
    // #35: the copy carries "with <owner>"
    let env = TestEnv::new();
    let (alice, bob, s) = shared_play(&env, "movie").await;
    let alice_entry = link_back(&env, alice, bob).await;
    let copy = env.app.claim_session(bob, s.id).await.unwrap();
    let ids: Vec<_> = copy.participants.iter().map(|p| p.companion_id).collect();
    let me = env.app.user(bob.user_id).await.unwrap();
    assert_eq!(ids, vec![me.self_companion, alice_entry]);
    // the copy names alice, but it is not shared back to her as a play of bob's
    assert_eq!(env.app.shared_sessions(alice).await.unwrap().len(), 0);
    let q = app::ActivityQuery {
        shared_only: true,
        ..Default::default()
    };
    let feed = env.app.activity_feed(alice, q).await.unwrap();
    assert_eq!(feed.rows.len(), 0);
    assert!(matches!(
        env.app.claim_session(alice, copy.id).await,
        Err(AppError::Conflict(_))
    ));
    // a play bob logs himself with his entry for alice is still shared with her
    let own = env
        .app
        .log_session(
            bob,
            NewSession::new(s.item_id, vec![Participant::new(alice_entry)]),
        )
        .await
        .unwrap();
    let theirs = env.app.shared_sessions(alice).await.unwrap();
    assert_eq!(
        theirs.iter().map(|x| x.id).collect::<Vec<_>>(),
        vec![own.id]
    );
}

#[tokio::test]
async fn a_copy_takes_the_owners_score_onto_the_copiers_entry_for_them() {
    // #35: alice played too; her score goes with her into bob's copy
    let env = TestEnv::new();
    let (_, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    let c = env.app.create_companion(alice, "B.").await.unwrap();
    let req = env
        .app
        .request_link(alice, c.id, bob_user.id)
        .await
        .unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();
    let alice_entry = link_back(&env, alice, bob).await;
    let a = env.app.user(alice.user_id).await.unwrap();
    let mut p_alice = Participant::new(a.self_companion);
    p_alice.score = Some(70.0);
    p_alice.winner = true;
    let mut p_bob = Participant::new(c.id);
    p_bob.score = Some(61.0);
    let s = env
        .app
        .log_session(alice, NewSession::new(game.id, vec![p_alice, p_bob]))
        .await
        .unwrap();
    let copy = env.app.claim_session(bob, s.id).await.unwrap();
    assert_eq!(copy.participants.len(), 2);
    let theirs = copy
        .participants
        .iter()
        .find(|p| p.companion_id == alice_entry)
        .unwrap();
    assert_eq!(theirs.score, Some(70.0));
    assert!(theirs.winner);
}

#[tokio::test]
async fn a_shared_row_says_when_it_was_copied() {
    // #35: the feed knows a shared row is in the viewer's log, without a migration
    let env = TestEnv::new();
    let (_, bob, s) = shared_play(&env, "movie").await;
    let q = || app::ActivityQuery {
        shared_only: true,
        ..Default::default()
    };
    let before = env.app.activity_feed(bob, q()).await.unwrap();
    assert_eq!(before.rows.len(), 1);
    assert!(!before.rows[0].copied);
    env.app.claim_session(bob, s.id).await.unwrap();
    let after = env.app.activity_feed(bob, q()).await.unwrap();
    assert_eq!(after.rows.len(), 1);
    assert!(after.rows[0].shared && after.rows[0].copied);
}
