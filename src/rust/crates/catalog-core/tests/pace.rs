//! Review BOOK-8: the processes of one instance (web role, worker, shell commands) share one
//! pace per source through the database, so a source that limits per address never gets two
//! requests closer together than its interval, whoever sends them.

use std::time::{Duration, Instant};

use catalog_core::SharedPace;
use catalog_core::pg::PgPace;

#[tokio::test]
async fn two_processes_take_turns_at_one_source() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    // two clients of one database: the shell command and the server's worker
    let (cli, worker) = (PgPace::new(pool.clone()), PgPace::new(pool));
    let every = Duration::from_millis(600);
    let ms = Duration::from_millis;

    // each reservation is the slot after the one before, whoever asks
    let first = cli.reserve("openlibrary", every).await.unwrap();
    let second = worker.reserve("openlibrary", every).await.unwrap();
    let third = cli.reserve("openlibrary", every).await.unwrap();
    assert!(first < ms(200), "{first:?}");
    assert!(second > ms(300) && second <= ms(600), "{second:?}");
    assert!(third > ms(900) && third <= ms(1200), "{third:?}");
    // another source has its own pace
    let other = worker.reserve("another", every).await.unwrap();
    assert!(other < ms(200), "{other:?}");

    // a turn waits for its slot: three slots are taken, so the fourth starts 1.8 s after
    // the first
    let start = Instant::now();
    worker.turn("openlibrary", every).await;
    assert!(start.elapsed() > ms(1300), "{:?}", start.elapsed());
    // and a free source does not wait
    let start = Instant::now();
    cli.turn("idle", every).await;
    assert!(start.elapsed() < ms(300), "{:?}", start.elapsed());
}
