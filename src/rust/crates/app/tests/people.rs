//! WP3-6 People (design.md 3.6, 6.7, 5.11): stats per person (together, plays, win rate with
//! ties, last time, most played game), merge suggestions for names equal after normalising,
//! undoing a merge from its record, removing a person who has history, outgoing requests.

mod common;

use app::{AppError, NewSession};
use chrono::{Duration, TimeZone, Utc};
use domain::session::{CoopOutcome, Participant};
use testkit::TestEnv;

fn at(day: u32) -> domain::Timestamp {
    Utc.with_ymd_and_hms(2026, 3, day, 20, 0, 0).unwrap()
}

fn player(id: domain::ids::CompanionId, winner: bool) -> Participant {
    Participant {
        winner,
        ..Participant::new(id)
    }
}

#[tokio::test]
async fn companion_stats_count_plays_wins_ties_and_the_most_played_game() {
    let env = TestEnv::new();
    let (me_user, me) = common::user(&env, "alice").await;
    let azul = common::item(&env.app, "board_game", "Azul").await;
    let catan = common::item(&env.app, "board_game", "Catan").await;
    let film = common::item(&env.app, "movie", "Heat").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let ben = env.app.create_companion(me, "Ben").await.unwrap();
    let me_c = me_user.self_companion;

    let log = |item, day, parts: Vec<Participant>, coop: Option<CoopOutcome>| {
        let mut s = NewSession::new(item, parts);
        s.started_at = Some(at(day));
        s.coop_outcome = coop;
        s
    };
    let plays = vec![
        // Anna wins
        log(
            azul.id,
            1,
            vec![player(me_c, false), player(anna.id, true)],
            None,
        ),
        // a tie: Anna and I both win
        log(
            azul.id,
            2,
            vec![player(me_c, true), player(anna.id, true)],
            None,
        ),
        // I win
        log(
            catan.id,
            3,
            vec![player(me_c, true), player(anna.id, false)],
            None,
        ),
        // nobody recorded as the winner: not decided
        log(
            azul.id,
            4,
            vec![player(me_c, false), player(anna.id, false)],
            None,
        ),
        // a cooperative loss is decided and no win
        log(
            catan.id,
            5,
            vec![player(me_c, false), player(anna.id, false)],
            Some(CoopOutcome::Lost),
        ),
        // watched together: together, not a play
        log(film.id, 6, vec![Participant::new(anna.id)], None),
    ];
    for p in plays {
        env.app.log_session(me, p).await.unwrap();
    }

    let stats = env.app.companion_stats(me).await.unwrap();
    assert_eq!(stats.len(), 2, "Anna and Ben, never the self entry");
    let a = stats.iter().find(|s| s.companion_id == anna.id).unwrap();
    assert_eq!(a.together, 6);
    assert_eq!(a.plays, 5);
    assert_eq!((a.wins, a.decided), (2, 4), "the tie counts as a win");
    assert_eq!(a.win_rate(), Some(0.5));
    let last = a.last.as_ref().unwrap();
    assert_eq!(
        (last.item_id, last.title.as_str(), last.at),
        (film.id, "Heat", at(6))
    );
    let most = a.most_played.as_ref().unwrap();
    assert_eq!(
        (most.item_id, most.title.as_str(), most.plays),
        (azul.id, "Azul", 3)
    );

    let b = stats.iter().find(|s| s.companion_id == ben.id).unwrap();
    assert_eq!((b.together, b.plays, b.wins, b.decided), (0, 0, 0, 0));
    assert_eq!(b.win_rate(), None);
    assert!(b.last.is_none() && b.most_played.is_none());
}

#[tokio::test]
async fn stats_follow_merges_and_include_sessions_a_linked_person_shared() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let azul = common::item(&env.app, "board_game", "Azul").await;
    let b1 = env.app.create_companion(me, "Bob").await.unwrap();
    let b2 = env.app.create_companion(me, "bob ").await.unwrap();
    let mut s = NewSession::new(azul.id, vec![player(b2.id, true)]);
    s.started_at = Some(at(1));
    env.app.log_session(me, s).await.unwrap();
    env.app.merge_companions(me, b1.id, b2.id).await.unwrap();

    // Bob links b1 to himself and logs a play with Alice, which he wins
    let req = env.app.request_link(me, b1.id, bob_user.id).await.unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();
    let alice_there = env.app.create_companion(bob, "Alice").await.unwrap();
    let alice_user = env.app.user(me.user_id).await.unwrap();
    let req = env
        .app
        .request_link(bob, alice_there.id, alice_user.id)
        .await
        .unwrap();
    env.app.accept_link(me, req.id).await.unwrap();
    let mut s = NewSession::new(
        azul.id,
        vec![
            player(bob_user.self_companion, true),
            player(alice_there.id, false),
        ],
    );
    s.started_at = Some(at(2));
    env.app.log_session(bob, s).await.unwrap();

    let stats = env.app.companion_stats(me).await.unwrap();
    assert_eq!(stats.len(), 1, "the merged entry is folded in");
    let st = &stats[0];
    assert_eq!(st.companion_id, b1.id);
    assert_eq!((st.together, st.plays), (2, 2));
    assert_eq!((st.wins, st.decided), (2, 2));
    assert_eq!(st.last.as_ref().unwrap().at, at(2));
}

#[tokio::test]
async fn merge_suggestions_pair_names_equal_after_normalising() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let lee = env.app.create_companion(me, "Lee").await.unwrap();
    let lee2 = env.app.create_companion(me, " lee.").await.unwrap();
    env.app.create_companion(me, "Lee K.").await.unwrap();
    env.app.create_companion(me, "Anna").await.unwrap();
    let suggestions = env.app.merge_suggestions(me).await.unwrap();
    assert_eq!(suggestions.len(), 1, "{suggestions:?}");
    // the older entry is kept when neither has more history
    assert_eq!(
        (suggestions[0].keep, suggestions[0].remove),
        (lee.id, lee2.id)
    );
    assert_eq!(
        domain::companion::name_key("  Lee-K. "),
        domain::companion::name_key("leek")
    );
}

#[tokio::test]
async fn a_merge_is_undone_from_its_record() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let keep = env.app.create_companion(me, "Bob").await.unwrap();
    let dup = env.app.create_companion(me, "Bobby").await.unwrap();
    env.app
        .update_companion(me, dup.id, None, Some(Some("brings snacks".into())))
        .await
        .unwrap();
    let req = env.app.request_link(me, dup.id, bob_user.id).await.unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();

    let merged = env.app.merge_companions(me, keep.id, dup.id).await.unwrap();
    assert_eq!(merged.linked_user, Some(bob_user.id), "the link moved");
    assert_eq!(merged.notes.as_deref(), Some("brings snacks"));
    assert_eq!(env.app.merged_into(me, keep.id).await.unwrap().len(), 1);

    let back = env.app.unmerge_companion(me, dup.id).await.unwrap();
    assert!(back.merged_into.is_none());
    assert_eq!(back.linked_user, Some(bob_user.id), "the link went back");
    assert_eq!(back.notes.as_deref(), Some("brings snacks"));
    let kept = env.app.companion(me, keep.id).await.unwrap();
    assert!(kept.linked_user.is_none() && kept.notes.is_none());
    assert_eq!(env.app.companions(me).await.unwrap().len(), 3);
    // only a merged entry can be unmerged
    assert!(matches!(
        env.app.unmerge_companion(me, dup.id).await,
        Err(AppError::Conflict(_))
    ));
    // UX3-21: through a chain (the kept one was merged on) it is the same conflict (409)
    let third = env.app.create_companion(me, "Robert").await.unwrap();
    env.app.merge_companions(me, keep.id, dup.id).await.unwrap();
    env.app
        .merge_companions(me, third.id, keep.id)
        .await
        .unwrap();
    assert!(matches!(
        env.app.unmerge_companion(me, dup.id).await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn removing_a_person_with_history_keeps_the_history() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let s = env
        .app
        .log_session(
            me,
            NewSession::new(game.id, vec![Participant::new(anna.id)]),
        )
        .await
        .unwrap();
    env.app.delete_companion(me, anna.id).await.unwrap();
    // gone from People and the pickers, still named in the history
    assert_eq!(env.app.companions(me).await.unwrap().len(), 1);
    assert_eq!(
        env.app.participant_names(me, s.id).await.unwrap(),
        vec![("Anna".to_string(), false)]
    );
    assert!(
        env.app
            .log_session(
                me,
                NewSession::new(game.id, vec![Participant::new(anna.id)])
            )
            .await
            .is_err(),
        "a removed person cannot be used again"
    );
    // an unused person is deleted outright
    let ben = env.app.create_companion(me, "Ben").await.unwrap();
    env.app.delete_companion(me, ben.id).await.unwrap();
    assert!(matches!(
        env.app.companion(me, ben.id).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn outgoing_link_requests_are_listed_until_answered() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let c = env.app.create_companion(me, "Bob").await.unwrap();
    let req = env.app.request_link(me, c.id, bob_user.id).await.unwrap();
    let out = env.app.sent_link_requests(me).await.unwrap();
    assert_eq!(
        out.iter().map(|r| r.companion_id).collect::<Vec<_>>(),
        vec![c.id]
    );
    env.clock.advance(Duration::minutes(1));
    env.app.decline_link(bob, req.id).await.unwrap();
    assert_eq!(env.app.sent_link_requests(me).await.unwrap().len(), 0);
}

/// UX3-26: two entries of one session that were later merged into one person count one
/// play, and a win if either of them won (the twin listed first lost here).
#[tokio::test]
async fn merged_twins_in_one_session_count_the_win() {
    let env = TestEnv::new();
    let (me_user, me) = common::user(&env, "alice").await;
    let azul = common::item(&env.app, "board_game", "Azul").await;
    let bob = env.app.create_companion(me, "Bob").await.unwrap();
    let twin = env.app.create_companion(me, "Bobby").await.unwrap();
    env.app
        .log_session(
            me,
            NewSession::new(
                azul.id,
                vec![
                    player(me_user.self_companion, false),
                    player(twin.id, false),
                    player(bob.id, true),
                ],
            ),
        )
        .await
        .unwrap();
    env.app.merge_companions(me, bob.id, twin.id).await.unwrap();
    let stats = env.app.companion_stats(me).await.unwrap();
    let s = stats.iter().find(|s| s.companion_id == bob.id).unwrap();
    assert_eq!((s.plays, s.wins, s.decided), (1, 1, 1));
}
