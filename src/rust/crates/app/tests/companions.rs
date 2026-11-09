mod common;

use app::{AppError, NewSession};
use domain::session::Participant;
use testkit::TestEnv;

#[tokio::test]
async fn every_user_has_a_linked_self_companion() {
    let env = TestEnv::new();
    let (u, me) = common::user(&env, "alice").await;
    let list = env.app.companions(me).await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].is_self);
    assert_eq!(list[0].id, u.self_companion);
    assert_eq!(list[0].linked_user, Some(u.id));
    assert!(
        env.app
            .delete_companion(me, u.self_companion)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn crud_and_ownership() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    env.app
        .update_companion(
            me,
            anna.id,
            Some("Anna K.".into()),
            Some(Some("tea".into())),
        )
        .await
        .unwrap();
    assert!(matches!(
        env.app
            .update_companion(bob, anna.id, Some("x".into()), None)
            .await,
        // someone else's person reads like an unknown id (UX4-17)
        Err(AppError::NotFound(_))
    ));
    env.app.delete_companion(me, anna.id).await.unwrap();
    assert_eq!(env.app.companions(me).await.unwrap().len(), 1);
}

#[tokio::test]
async fn link_needs_acceptance_and_unlink_keeps_snapshot() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let c = env.app.create_companion(me, "Bobby").await.unwrap();
    let req = env.app.request_link(me, c.id, bob_user.id).await.unwrap();
    assert!(
        env.app
            .companion(me, c.id)
            .await
            .unwrap()
            .linked_user
            .is_none()
    );
    assert!(matches!(
        env.app.accept_link(me, req.id).await,
        Err(AppError::Forbidden)
    ));
    env.app.accept_link(bob, req.id).await.unwrap();
    assert_eq!(
        env.app.companion(me, c.id).await.unwrap().linked_user,
        Some(bob_user.id)
    );

    // the linked user can end the link (demotion); the owner keeps name + snapshot
    env.app.unlink_companion(bob, c.id).await.unwrap();
    let after = env.app.companion(me, c.id).await.unwrap();
    assert!(after.linked_user.is_none());
    assert_eq!(after.name, "Bobby");
    assert_eq!(after.snapshot.unwrap().display_name, "BOB");
}

#[tokio::test]
async fn merge_does_not_rewrite_history() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    let a1 = env.app.create_companion(me, "Anna").await.unwrap();
    let a2 = env.app.create_companion(me, "Anna (dup)").await.unwrap();
    let s = env
        .app
        .log_session(me, NewSession::new(game.id, vec![Participant::new(a2.id)]))
        .await
        .unwrap();

    env.app.merge_companions(me, a1.id, a2.id).await.unwrap();

    let stored = env.app.session(me, s.id).await.unwrap();
    assert_eq!(
        stored.participants[0].companion_id, a2.id,
        "history untouched"
    );
    assert_eq!(
        env.app.resolve_companion(me, a2.id).await.unwrap().id,
        a1.id
    );
    assert_eq!(
        env.app.companions(me).await.unwrap().len(),
        2,
        "self + Anna"
    );
    // merged companions cannot be used for new sessions
    assert!(
        env.app
            .log_session(me, NewSession::new(game.id, vec![Participant::new(a2.id)]))
            .await
            .is_err()
    );
    // a companion named in sessions is removed, not deleted: the history keeps it (WP3-6,
    // design.md 6.7 "delete keeps history")
    env.app.delete_companion(me, a1.id).await.unwrap();
    assert!(
        env.app
            .companion(me, a1.id)
            .await
            .unwrap()
            .removed_at
            .is_some()
    );
    assert_eq!(env.app.companions(me).await.unwrap().len(), 1, "self only");
}

#[tokio::test]
async fn promote_invite_creates_linked_user_who_sees_shared_sessions() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let game = common::item(&env.app, "board_game", "Catan").await;
    let c = env.app.create_companion(me, "Carl").await.unwrap();
    let s = env
        .app
        .log_session(me, NewSession::new(game.id, vec![Participant::new(c.id)]))
        .await
        .unwrap();

    let (_invite, token) = env
        .app
        .promote_companion(me, c.id, Some("carl@example.org".into()))
        .await
        .unwrap();
    let carl = env.app.accept_invite(&token, "carl", "Carl").await.unwrap();
    assert!(
        env.app
            .accept_invite(&token, "carl2", "Carl")
            .await
            .is_err(),
        "single use"
    );

    let linked = env.app.companion(me, c.id).await.unwrap();
    assert_eq!(linked.linked_user, Some(carl.id));
    let carl_actor = domain::user::Actor {
        user_id: carl.id,
        role: carl.role,
    };
    let shared = env.app.shared_sessions(carl_actor).await.unwrap();
    assert_eq!(shared.iter().map(|s| s.id).collect::<Vec<_>>(), vec![s.id]);
    // history row is unchanged
    assert_eq!(
        env.app.session(me, s.id).await.unwrap().participants[0].companion_id,
        c.id
    );
}

#[tokio::test]
async fn deleting_a_user_demotes_companions_linked_to_them() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let c = env.app.create_companion(me, "B.").await.unwrap();
    let req = env.app.request_link(me, c.id, bob_user.id).await.unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();
    env.app.delete_user(bob, bob_user.id).await.unwrap();
    let after = env.app.companion(me, c.id).await.unwrap();
    assert!(after.linked_user.is_none());
    assert_eq!(after.snapshot.unwrap().display_name, "BOB");
}

/// Alice's "Bob" linked to bob and bob's "Alice" linked to alice.
async fn linked_both_ways(
    env: &TestEnv,
) -> (
    domain::user::Actor,
    domain::user::Actor,
    domain::companion::Companion,
    domain::companion::Companion,
) {
    let (alice_user, alice) = common::user(env, "alice").await;
    let (bob_user, bob) = common::user(env, "bob").await;
    let c = env.app.create_companion(alice, "Bob").await.unwrap();
    let r = env
        .app
        .request_link(alice, c.id, bob_user.id)
        .await
        .unwrap();
    env.app.accept_link(bob, r.id).await.unwrap();
    let d = env.app.create_companion(bob, "Alice").await.unwrap();
    let r = env
        .app
        .request_link(bob, d.id, alice_user.id)
        .await
        .unwrap();
    env.app.accept_link(alice, r.id).await.unwrap();
    (alice, bob, c, d)
}

async fn linked(env: &TestEnv, id: domain::ids::CompanionId) -> bool {
    env.app
        .repos
        .companions
        .get(id)
        .await
        .unwrap()
        .is_some_and(|c| c.linked_user.is_some())
}

/// UX3-17: unlinking ends the link on both sides, whoever acts.
#[tokio::test]
async fn unlinking_ends_the_counterpart_link_too() {
    let env = TestEnv::new();
    let (alice, _bob, c, d) = linked_both_ways(&env).await;
    assert!(linked(&env, c.id).await && linked(&env, d.id).await);
    env.app.unlink_companion(alice, c.id).await.unwrap();
    assert!(!linked(&env, c.id).await);
    assert!(!linked(&env, d.id).await, "bob's Alice is unlinked as well");
}

/// UX3-17: removing a linked person (kept for history, or deleted) ends both links.
#[tokio::test]
async fn removing_a_linked_person_ends_the_counterpart_link_too() {
    let env = TestEnv::new();
    let (alice, _bob, c, d) = linked_both_ways(&env).await;
    env.app.delete_companion(alice, c.id).await.unwrap();
    assert!(
        !linked(&env, d.id).await,
        "deleted: bob's Alice is unlinked"
    );

    let env = TestEnv::new();
    let (alice, _bob, c, d) = linked_both_ways(&env).await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    env.app
        .log_session(
            alice,
            NewSession::new(game.id, vec![Participant::new(c.id)]),
        )
        .await
        .unwrap();
    env.app.delete_companion(alice, c.id).await.unwrap();
    assert!(
        !linked(&env, d.id).await,
        "kept for history: bob's Alice is unlinked"
    );
}
