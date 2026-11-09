//! DEC-247/326 (WP4-2): the counters of the governed client are shared by every process.
//! A web and a worker role each keep their own [`SourceStats`]; both flush into one database,
//! and each reads the database plus its own unflushed deltas.

use catalog_core::SourceStats;
use catalog_core::pg::PgSourceStats;

#[tokio::test]
async fn the_web_role_sees_the_workers_429_after_a_flush() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let store = PgSourceStats::new(pool.clone());
    let web = SourceStats::default();
    let worker = SourceStats::default();

    // the worker's background refresh hits a rate limit; the web role answers a search
    worker.record_call("tmdb");
    worker.record_status("tmdb", 429);
    web.record_call("tmdb");
    web.record_status("tmdb", 200);
    // before any flush each role only knows its own calls
    assert_eq!(web.counters("tmdb").rate_limited_24h, 0);
    assert_eq!(web.counters("tmdb").calls_24h, 1);

    worker.sync(&store).await.unwrap();
    web.sync(&store).await.unwrap();
    let c = web.counters("tmdb");
    assert_eq!((c.calls_24h, c.rate_limited_24h), (2, 1), "{c:?}");
    assert!(c.last_rate_limited_at.is_some(), "{c:?}");
    assert!(c.last_ok_at.is_some(), "{c:?}");

    // unflushed deltas count on top of the database, and nothing is counted twice
    web.record_call("tmdb");
    assert_eq!(web.counters("tmdb").calls_24h, 3);
    web.sync(&store).await.unwrap();
    worker.sync(&store).await.unwrap();
    assert_eq!(web.counters("tmdb").calls_24h, 3);
    assert_eq!(worker.counters("tmdb").calls_24h, 3);

    // a rejection seen by the worker marks the key on the web role; a new key clears it
    // everywhere
    worker.record_status("tvdb", 401);
    worker.sync(&store).await.unwrap();
    web.sync(&store).await.unwrap();
    assert!(web.rejected("tvdb"));
    web.key_changed("tvdb");
    assert!(!web.rejected("tvdb"));
    web.sync(&store).await.unwrap();
    worker.sync(&store).await.unwrap();
    assert!(
        !worker.rejected("tvdb"),
        "the key change reached the worker"
    );

    // an open circuit elsewhere is reported until a later success
    worker.record_circuit_open("tvdb", std::time::Duration::from_secs(300));
    worker.sync(&store).await.unwrap();
    web.sync(&store).await.unwrap();
    assert!(web.counters("tvdb").circuit_open_until.is_some());
    web.record_status("tvdb", 200);
    assert!(web.counters("tvdb").circuit_open_until.is_none());
    pool.close().await;
}
