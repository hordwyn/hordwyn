//! WP4-6 game groups (design.md 6.7): plays logged as a group count for it, a merged member
//! stays in the group as the person it was merged into, a removed group keeps its sessions.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::{AppError, GameGroupInput, NewSession};
use chrono::{TimeZone, Utc};
use domain::session::Participant;
use testkit::TestEnv;

fn at(day: u32) -> domain::Timestamp {
    Utc.with_ymd_and_hms(2026, 3, day, 20, 0, 0).unwrap()
}

#[tokio::test]
async fn plays_logged_as_a_group_count_for_it_and_members_follow_merges() {
    let env = TestEnv::new();
    let (me_user, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    let azul = common::item(&env.app, "board_game", "Azul").await;
    let catan = common::item(&env.app, "board_game", "Harbour").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let ben = env.app.create_companion(me, "Ben").await.unwrap();
    let benny = env.app.create_companion(me, "Benny").await.unwrap();
    let theirs = env.app.create_companion(bob, "Zed").await.unwrap();

    // members must be the owner's active people, never their own entry
    let input = |members| GameGroupInput {
        name: "Thursday table".into(),
        members,
    };
    assert!(matches!(
        env.app
            .create_game_group(me, input(vec![anna.id, theirs.id]))
            .await,
        // like an unknown id: no oracle for other people's ids (UX4-17)
        Err(AppError::NotFound(_))
    ));
    assert!(
        env.app
            .create_game_group(me, input(vec![me_user.self_companion]))
            .await
            .is_err()
    );
    let group = env
        .app
        .create_game_group(me, input(vec![anna.id, ben.id]))
        .await
        .unwrap();

    let players = vec![
        Participant::new(me_user.self_companion),
        Participant::new(anna.id),
        Participant::new(ben.id),
    ];
    for (item, day) in [(azul.id, 1), (catan.id, 5)] {
        let mut s = NewSession::new(item, players.clone());
        s.started_at = Some(at(day));
        s.game_group = Some(group.id);
        env.app.log_session(me, s).await.unwrap();
    }
    // the same people without the group: not a group play
    let mut s = NewSession::new(azul.id, players.clone());
    s.started_at = Some(at(9));
    env.app.log_session(me, s).await.unwrap();
    // another user cannot log with my group
    let mut foreign = NewSession::new(azul.id, vec![]);
    foreign.game_group = Some(group.id);
    assert!(env.app.log_session(bob, foreign).await.is_err());

    let groups = env.app.game_groups(me).await.unwrap();
    assert_eq!(groups.len(), 1);
    let g = &groups[0];
    assert_eq!(g.plays, 2);
    let last = g.last.as_ref().unwrap();
    assert_eq!((last.title.as_str(), last.at), ("Harbour", at(5)));
    assert_eq!(g.group.members, vec![anna.id, ben.id]);
    assert_eq!(env.app.game_groups(bob).await.unwrap().len(), 0);

    // Ben merged into Benny: the group now has Benny in Ben's place
    env.app
        .merge_companions(me, benny.id, ben.id)
        .await
        .unwrap();
    let g = env.app.game_groups(me).await.unwrap().remove(0);
    assert_eq!(g.group.members, vec![anna.id, benny.id]);
    assert_eq!(g.plays, 2, "the group's plays are unchanged");
    // ... and back after the merge is undone
    env.app.unmerge_companion(me, ben.id).await.unwrap();
    let g = env.app.game_groups(me).await.unwrap().remove(0);
    assert_eq!(g.group.members, vec![anna.id, ben.id]);

    // rename and change members
    let edited = env
        .app
        .update_game_group(
            me,
            group.id,
            GameGroupInput {
                name: "Game night".into(),
                members: vec![ben.id],
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (edited.name.as_str(), edited.members.clone()),
        ("Game night", vec![ben.id])
    );
    assert!(
        env.app
            .update_game_group(bob, group.id, input(vec![theirs.id]))
            .await
            .is_err(),
        "not bob's group"
    );
}

#[tokio::test]
async fn a_removed_group_keeps_its_sessions() {
    let env = TestEnv::new();
    let (me_user, me) = common::user(&env, "alice").await;
    let azul = common::item(&env.app, "board_game", "Azul").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let group = env
        .app
        .create_game_group(
            me,
            GameGroupInput {
                name: "Fridays".into(),
                members: vec![anna.id],
            },
        )
        .await
        .unwrap();
    let mut s = NewSession::new(
        azul.id,
        vec![
            Participant::new(me_user.self_companion),
            Participant::new(anna.id),
        ],
    );
    s.game_group = Some(group.id);
    let session = env.app.log_session(me, s).await.unwrap();
    assert_eq!(session.game_group_id, Some(group.id));

    env.app.remove_game_group(me, group.id).await.unwrap();
    assert_eq!(env.app.game_groups(me).await.unwrap().len(), 0);
    let mine = env.app.my_sessions(me).await.unwrap();
    assert_eq!(mine.len(), 1, "the play stays in the history");
    assert_eq!(mine[0].game_group_id, Some(group.id));
    assert_eq!(mine[0].participants.len(), 2);
    // a removed group takes no new plays and cannot be removed twice
    let mut again = NewSession::new(azul.id, vec![]);
    again.game_group = Some(group.id);
    assert!(env.app.log_session(me, again).await.is_err());
    assert!(env.app.remove_game_group(me, group.id).await.is_err());
    // a member removed from People ("delete keeps history") leaves the member list
    let other = env
        .app
        .create_game_group(
            me,
            GameGroupInput {
                name: "Sundays".into(),
                members: vec![anna.id],
            },
        )
        .await
        .unwrap();
    env.app.delete_companion(me, anna.id).await.unwrap();
    let g = env.app.game_groups(me).await.unwrap();
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].group.id, other.id);
    assert_eq!(g[0].group.members.len(), 0);
}
