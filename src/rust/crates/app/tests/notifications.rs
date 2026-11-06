//! Use-cases emit the notification events of the catalogue (D-17); rendering, subscriptions
//! and delivery are the `notify` adapter's job.
mod common;

use app::NewSession;
use app::ports::Audience;
use domain::session::Participant;
use testkit::TestEnv;

#[tokio::test]
async fn social_events_reach_the_right_people() {
    let env = TestEnv::new();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let c = env.app.create_companion(alice, "B.").await.unwrap();
    let req = env
        .app
        .request_link(alice, c.id, bob_user.id)
        .await
        .unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();
    let game = common::item(&env.app, "board_game", "Azul").await;
    let s = env
        .app
        .log_session(
            alice,
            NewSession::new(game.id, vec![Participant::new(c.id)]),
        )
        .await
        .unwrap();

    let all = env.notifications.all();
    let find = |e: &str| all.iter().find(|n| n.event == e).unwrap().clone();
    let asked = find("social.link_request");
    assert_eq!(asked.audience, Audience::Users(vec![bob_user.id]));
    assert_eq!(asked.args["from"], "ALICE");
    assert_eq!(asked.args["companion"], "B.");
    assert_eq!(asked.link.as_deref(), Some("/people"));
    let accepted = find("social.link_accepted");
    assert_eq!(accepted.audience, Audience::Users(vec![alice_user.id]));
    assert_eq!(accepted.args["user"], "BOB");
    let shared = find("social.session_shared");
    assert_eq!(shared.audience, Audience::Users(vec![bob_user.id]));
    assert_eq!(shared.dedupe_key, format!("session:{}", s.id.0));
    assert_eq!(shared.args["title"], "Azul");
    assert_eq!(shared.subject.item, Some(game.id));
    assert_eq!(shared.subject.kind.as_deref(), Some("board_game"));
}

#[tokio::test]
async fn private_owners_do_not_announce_sessions() {
    let env = TestEnv::new();
    let (_, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let c = env.app.create_companion(alice, "B.").await.unwrap();
    let req = env
        .app
        .request_link(alice, c.id, bob_user.id)
        .await
        .unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();
    env.app.set_privacy(alice, false).await.unwrap();
    let game = common::item(&env.app, "board_game", "Azul").await;
    env.app
        .log_session(
            alice,
            NewSession::new(game.id, vec![Participant::new(c.id)]),
        )
        .await
        .unwrap();
    assert!(
        !env.notifications
            .events()
            .contains(&"social.session_shared".to_string())
    );
    // and the update event goes to the owner only (G2-9)
    let last = env
        .events
        .all()
        .into_iter()
        .rev()
        .find(|e| e.entity.as_ref().is_some_and(|r| r.entity == "session"))
        .unwrap();
    assert_eq!(last.audience, vec![alice.user_id]);
}

#[tokio::test]
async fn sign_ups_are_announced_to_admins() {
    let env = TestEnv::new();
    let (u, _) = common::user(&env, "carol").await;
    let n = env.notifications.all().pop().unwrap();
    assert_eq!(n.event, "admin.signup");
    assert_eq!(n.audience, Audience::Admins);
    assert_eq!(n.dedupe_key, format!("user:{}", u.id.0));
    assert_eq!(n.args["username"], "carol");
}
