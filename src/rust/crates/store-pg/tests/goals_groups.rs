//! WP4-6 on Postgres (migration 0026): goals stay one per year, group and measure; game
//! groups keep their members in order; a play keeps its group after the group is removed and
//! after a member is merged; deleting a person without history drops the membership.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::{AppError, GameGroupInput, NewGoal, NewSession};
use domain::goal::GoalMetric;
use domain::kind::KindGroup;
use domain::session::Participant;

#[tokio::test]
async fn goals_and_game_groups_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (me_user, me) = common::user(app, "alice").await;
    let (_, bob) = common::user(app, "bob").await;
    let goal = NewGoal {
        year: 2026,
        group: Some(KindGroup::Play),
        metric: GoalMetric::Plays,
        target: 12,
    };
    let g = app.set_goal(me, goal).await.unwrap();
    assert!(matches!(
        app.set_goal(me, goal).await,
        Err(AppError::Conflict(_))
    ));
    let all = NewGoal {
        group: None,
        ..goal
    };
    app.set_goal(me, all).await.unwrap();
    assert_eq!(app.goals(me).await.unwrap().len(), 2);
    assert_eq!(app.goals(bob).await.unwrap().len(), 0);
    let changed = app
        .update_goal(me, g.goal.id, NewGoal { target: 20, ..goal })
        .await
        .unwrap();
    assert_eq!(changed.goal.target, 20);
    assert!(app.delete_goal(bob, g.goal.id).await.is_err());
    app.delete_goal(me, g.goal.id).await.unwrap();
    assert_eq!(app.goals(me).await.unwrap().len(), 1);

    let azul = common::item(app, "board_game", "Azul").await;
    let anna = app.create_companion(me, "Anna").await.unwrap();
    let ben = app.create_companion(me, "Ben").await.unwrap();
    let cleo = app.create_companion(me, "Cleo").await.unwrap();
    let dora = app.create_companion(me, "Dora").await.unwrap();
    let group = app
        .create_game_group(
            me,
            GameGroupInput {
                name: "Thursday".into(),
                members: vec![cleo.id, anna.id, ben.id, dora.id],
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
    app.log_play(me, s).await.unwrap();
    let read = app.game_groups(me).await.unwrap().remove(0);
    assert_eq!(read.group.members, vec![cleo.id, anna.id, ben.id, dora.id]);
    assert_eq!(read.plays, 1);
    assert_eq!(read.last.as_ref().map(|l| l.title.as_str()), Some("Azul"));
    // Ben merged into Anna: Anna once, in her place; Dora has no history and is deleted
    app.merge_companions(me, anna.id, ben.id).await.unwrap();
    app.delete_companion(me, dora.id).await.unwrap();
    let read = app.game_groups(me).await.unwrap().remove(0);
    assert_eq!(read.group.members, vec![cleo.id, anna.id]);
    app.remove_game_group(me, group.id).await.unwrap();
    assert_eq!(app.game_groups(me).await.unwrap().len(), 0);
    let mine = app.my_sessions(me).await.unwrap();
    assert_eq!(mine[0].game_group_id, Some(group.id));
    assert_eq!(
        common::count(&env.store, "SELECT COUNT(*) FROM game_group_members").await,
        3,
        "Cleo, Anna and the merged Ben stay stored; Dora went with her row"
    );
}
