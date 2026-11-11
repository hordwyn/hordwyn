//! ot2f (OT-11 rest, OT-18): what the "Plays" tab of a game page and the Stats Play tab
//! count: plays, minutes, and the win rate of the person themselves. A play has a result
//! when a co-op game was won or lost, or a competitive play names a winner; a co-op play
//! without a result (OT-18) and a play the person did not take part in count in no rate.

mod common;

use app::{NewSession, PlayTotals};
use chrono::{TimeZone, Utc};
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

fn log(
    item: domain::ids::ItemId,
    day: u32,
    minutes: Option<u32>,
    parts: Vec<Participant>,
    coop: Option<CoopOutcome>,
) -> NewSession {
    let mut s = NewSession::new(item, parts);
    s.started_at = Some(at(day));
    s.duration_minutes = minutes;
    s.coop_outcome = coop;
    s
}

/// The plays of the first test: five of the board game, three co-op plays of the video
/// game, and one "watched with" session of the movie. `[me, anna, ben]` are the entries.
fn plays(
    [azul, lego, film]: [domain::ids::ItemId; 3],
    [me, anna, ben]: [domain::ids::CompanionId; 3],
) -> Vec<NewSession> {
    let two = |mine: bool, hers: bool| vec![player(me, mine), player(anna, hers)];
    let others = vec![player(anna, true), player(ben, false)];
    vec![
        // I win, 45 minutes
        log(azul, 1, Some(45), two(true, false), None),
        // Anna wins, no duration
        log(azul, 2, None, two(false, true), None),
        // a tie: both win
        log(azul, 3, Some(30), two(true, true), None),
        // no winner named: a play without a result
        log(azul, 4, Some(20), two(false, false), None),
        // Anna and Ben played, I logged it but did not take part: no result of mine
        log(azul, 5, Some(60), others, None),
        // co-op: won, lost, and played without a result (OT-18)
        log(lego, 6, Some(90), two(false, false), Some(CoopOutcome::Won)),
        log(
            lego,
            7,
            Some(90),
            two(false, false),
            Some(CoopOutcome::Lost),
        ),
        log(
            lego,
            8,
            Some(120),
            two(false, false),
            Some(CoopOutcome::NoResult),
        ),
        // watched together: a session, not a play
        log(film, 9, Some(100), vec![Participant::new(anna)], None),
    ]
}

#[tokio::test]
async fn play_totals_count_time_and_the_results_of_the_person() {
    let env = TestEnv::new();
    let (me_user, me) = common::user(&env, "alice").await;
    let azul = common::item(&env.app, "board_game", "Azul").await;
    let lego = common::item(&env.app, "video_game", "Brick Wizards").await;
    let film = common::item(&env.app, "movie", "Heat").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let ben = env.app.create_companion(me, "Ben").await.unwrap();
    let me_c = me_user.self_companion;
    assert_eq!(
        env.app.item_play_totals(me, azul.id).await.unwrap(),
        PlayTotals::default(),
        "nothing logged yet"
    );
    let ids = [me_c, anna.id, ben.id];
    for p in plays([azul.id, lego.id, film.id], ids) {
        env.app.log_session(me, p).await.unwrap();
    }

    let board = env.app.item_play_totals(me, azul.id).await.unwrap();
    assert_eq!(
        board,
        PlayTotals {
            plays: 5,
            minutes: 155,
            decided: 3,
            wins: 2,
        }
    );
    let video = env.app.item_play_totals(me, lego.id).await.unwrap();
    assert_eq!(
        video,
        PlayTotals {
            plays: 3,
            minutes: 300,
            decided: 2,
            wins: 1,
        }
    );
    // every game together; the movie session is not a play
    assert_eq!(
        env.app.play_totals(me).await.unwrap(),
        PlayTotals {
            plays: 8,
            minutes: 455,
            decided: 5,
            wins: 3,
        }
    );
    // another user has no plays of mine
    let (_, bob) = common::user(&env, "bob").await;
    assert_eq!(
        env.app.item_play_totals(bob, azul.id).await.unwrap(),
        PlayTotals::default()
    );
    assert_eq!(
        env.app.play_totals(bob).await.unwrap(),
        PlayTotals::default()
    );
}

/// An entry merged into the person's own entry still stands for them: its wins are theirs.
#[tokio::test]
async fn an_entry_merged_into_my_own_counts_as_me() {
    let env = TestEnv::new();
    let (me_user, me) = common::user(&env, "alice").await;
    let azul = common::item(&env.app, "board_game", "Azul").await;
    let twin = env.app.create_companion(me, "Alice B").await.unwrap();
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let mut play = NewSession::new(azul.id, vec![player(twin.id, true), player(anna.id, false)]);
    play.started_at = Some(at(1));
    env.app.log_session(me, play).await.unwrap();
    assert_eq!(
        env.app.item_play_totals(me, azul.id).await.unwrap().decided,
        0,
        "the twin is somebody else until merged"
    );
    env.app
        .merge_companions(me, me_user.self_companion, twin.id)
        .await
        .unwrap();
    let t = env.app.item_play_totals(me, azul.id).await.unwrap();
    assert_eq!((t.decided, t.wins), (1, 1));
}

/// OT-18: the co-op play without a result is stored and read back as such, and is no
/// decided play on a person's card either.
#[tokio::test]
async fn a_coop_play_without_a_result_is_kept_and_decides_nothing() {
    let env = TestEnv::new();
    let (me_user, me) = common::user(&env, "alice").await;
    let lego = common::item(&env.app, "video_game", "Brick Wizards").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let mut play = NewSession::new(
        lego.id,
        vec![
            player(me_user.self_companion, false),
            player(anna.id, false),
        ],
    );
    play.coop_outcome = Some(CoopOutcome::NoResult);
    let s = env.app.log_session(me, play).await.unwrap();
    assert_eq!(s.coop_outcome, Some(CoopOutcome::NoResult));
    let feed = env
        .app
        .activity_feed(me, app::ActivityQuery::default())
        .await
        .unwrap();
    assert_eq!(
        feed.rows[0].play.as_ref().unwrap().coop,
        Some(CoopOutcome::NoResult)
    );
    let stats = env.app.companion_stats(me).await.unwrap();
    assert_eq!((stats[0].plays, stats[0].decided, stats[0].wins), (1, 0, 0));
}

/// Review OT2F-4: a play of the person alone has no result unless a winner is marked (the
/// dialog marks nobody for one score alone), so a scored solo play is a play and is in no
/// rate; marked as won, it counts.
#[tokio::test]
async fn a_solo_play_has_a_result_only_when_it_is_marked_won() {
    let env = TestEnv::new();
    let (me_user, me) = common::user(&env, "alice").await;
    let azul = common::item(&env.app, "board_game", "Azul").await;
    let scored = |winner| Participant {
        score: Some(120.0),
        ..player(me_user.self_companion, winner)
    };
    let play = log(azul.id, 1, Some(30), vec![scored(false)], None);
    env.app.log_session(me, play).await.unwrap();
    assert_eq!(
        env.app.item_play_totals(me, azul.id).await.unwrap(),
        PlayTotals {
            plays: 1,
            minutes: 30,
            decided: 0,
            wins: 0,
        },
        "a score alone is no result"
    );
    let play = log(azul.id, 2, Some(30), vec![scored(true)], None);
    env.app.log_session(me, play).await.unwrap();
    let t = env.app.play_totals(me).await.unwrap();
    assert_eq!((t.plays, t.decided, t.wins), (2, 1, 1));
}
