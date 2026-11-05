mod common;

use app::AppError;
use domain::hlc::Hlc;
use domain::kind::Status;
use domain::library::Rating;
use domain::progress::Progress;
use testkit::TestEnv;

#[tokio::test]
async fn add_to_library_is_idempotent_and_defaults_to_planned() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "The Matrix").await;
    let a = env.app.add_to_library(me, movie.id, None).await.unwrap();
    let b = env.app.add_to_library(me, movie.id, None).await.unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!(a.status.value, Status::planned());
    assert_eq!(env.app.library(me).await.unwrap().len(), 1);
}

#[tokio::test]
async fn kind_specific_status_is_validated() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Heat").await;
    let err = env
        .app
        .add_to_library(me, movie.id, Some(Status::new("endless")))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Domain(_)), "{err:?}");
    let game = common::item(&env.app, "video_game", "Tetris").await;
    env.app
        .add_to_library(me, game.id, Some(Status::new("endless")))
        .await
        .unwrap();
}

#[tokio::test]
async fn log_progress_validates_model_adds_to_library_and_is_idempotent() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Alien").await;
    let wrong = env
        .app
        .log_progress(me, movie.id, Progress::Position { seconds: 3 }, None, None)
        .await;
    assert!(matches!(wrong, Err(AppError::Domain(_))));

    let op = uuid::Uuid::from_u128(77);
    let e1 = env
        .app
        .log_progress(me, movie.id, Progress::Finished, None, Some(op))
        .await
        .unwrap();
    let e2 = env
        .app
        .log_progress(me, movie.id, Progress::Finished, None, Some(op))
        .await
        .unwrap();
    assert_eq!(e1.id, e2.id, "same client op id -> same event");
    assert_eq!(env.app.progress(me, movie.id).await.unwrap().len(), 1);
    let entry = env.app.entry(me, movie.id).await.unwrap().unwrap();
    assert_eq!(entry.status.value, Status::completed());
}

#[tokio::test]
async fn episodic_progress_moves_status_to_in_progress() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = common::item(&env.app, "show", "Dark").await;
    env.app
        .log_progress(
            me,
            show.id,
            Progress::Episode {
                season: 1,
                episode: 1,
            },
            None,
            None,
        )
        .await
        .unwrap();
    let entry = env.app.entry(me, show.id).await.unwrap().unwrap();
    assert_eq!(entry.status.value, Status::in_progress());
}

#[tokio::test]
async fn rating_uses_last_writer_wins() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Up").await;
    let newer = Hlc::new(2_000_000_000_000, 0, 7);
    let older = Hlc::new(1_000, 0, 7);
    env.app
        .rate(me, movie.id, Some(Rating::new(90).unwrap()), Some(newer))
        .await
        .unwrap();
    env.app
        .rate(me, movie.id, Some(Rating::new(10).unwrap()), Some(older))
        .await
        .unwrap();
    let entry = env.app.entry(me, movie.id).await.unwrap().unwrap();
    assert_eq!(entry.rating.value, Some(Rating::new(90).unwrap()));
}

#[tokio::test]
async fn reviews_are_one_per_user_and_item() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Arrival").await;
    let r1 = env
        .app
        .write_review(me, movie.id, "Great.", false, None)
        .await
        .unwrap();
    let r2 = env
        .app
        .write_review(me, movie.id, "Even better.", true, None)
        .await
        .unwrap();
    assert_eq!(r1.id, r2.id);
    assert_eq!(r2.body, "Even better.");
    assert!(
        env.app
            .write_review(me, movie.id, "  ", false, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn writes_publish_entity_updated() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Jaws").await;
    env.app.add_to_library(me, movie.id, None).await.unwrap();
    assert!(env.events.topics().iter().any(|t| t == "entity.updated"));
}

#[tokio::test]
async fn item_detail_returns_immediately_and_enqueues_one_refresh() {
    let env = TestEnv::new();
    let movie = common::item(&env.app, "movie", "Solaris").await;
    env.app.item_detail(movie.id).await.unwrap();
    env.app.item_detail(movie.id).await.unwrap();
    let jobs = env.jobs.jobs();
    assert_eq!(jobs.len(), 1, "deduplicated");
    assert_eq!(jobs[0].kind, app::REFRESH_JOB);
    assert_eq!(jobs[0].lane, app::ports::Lane::Interactive);
}
