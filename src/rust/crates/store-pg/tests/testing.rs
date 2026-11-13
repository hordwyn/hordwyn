//! `store_pg::testing` drops the `test_<uuid>` database a test created when the test ends,
//! on success; on failure (a panic of the test's thread) the database stays for a look
//! (v1-release item 3, plan addendum 4). Each "test" here is a thread of its own, the way the
//! test harness runs every test.

use std::sync::{Arc, Mutex};

use sqlx::postgres::PgPoolOptions;

/// Run a test of its own: a new thread with its own runtime. Returns the name of
/// the database `fresh_db` created there, and whether the thread panicked.
fn as_a_test(fail: bool) -> (String, bool) {
    let name = Arc::new(Mutex::new(None));
    let seen = Arc::clone(&name);
    let outcome = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let pool = store_pg::testing::fresh_db().await.unwrap();
            let name: String = sqlx::query_scalar("SELECT current_database()")
                .fetch_one(&pool)
                .await
                .unwrap();
            *seen.lock().unwrap() = Some(name);
            // connections that are still open when the test ends do not stop the drop
            std::mem::forget(pool);
        });
        assert!(!fail, "this test fails on purpose");
    })
    .join();
    let name = name.lock().unwrap().clone().unwrap();
    (name, outcome.is_err())
}

async fn exists(admin: &sqlx::PgPool, name: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(name)
        .fetch_one(admin)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_passing_test_drops_its_database_and_a_failing_one_keeps_it() {
    let Ok(admin_url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    if store_pg::testing::keep_requested() {
        eprintln!("skipping: KEEP_TEST_DB is set, every test database is kept");
        return;
    }
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();

    let (passed, panicked) = tokio::task::spawn_blocking(|| as_a_test(false))
        .await
        .unwrap();
    assert!(!panicked);
    assert!(passed.starts_with("test_"), "{passed}");
    assert!(
        !exists(&admin, &passed).await,
        "{passed} is left after a passing test"
    );

    let (failed, panicked) = tokio::task::spawn_blocking(|| as_a_test(true))
        .await
        .unwrap();
    assert!(panicked);
    let kept = exists(&admin, &failed).await;
    // clean up the kept one before the assertion can fail
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {failed} WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .unwrap();
    assert!(kept, "{failed} was dropped although its test failed");
}

/// `KEEP_TEST_DB=1` keeps every database; any other value (or none) does not.
#[test]
fn keep_test_db_is_read_as_a_flag() {
    use store_pg::testing::keep_flag;
    assert!(keep_flag(Some("1")));
    assert!(keep_flag(Some("true")));
    assert!(!keep_flag(Some("0")));
    assert!(!keep_flag(Some("")));
    assert!(!keep_flag(None));
}
