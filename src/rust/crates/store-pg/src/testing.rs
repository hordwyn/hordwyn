//! Fresh, migrated test databases on the server named by `TEST_DATABASE_URL`.
//!
//! Without `TEST_DATABASE_URL` tests are skipped with a message, unless
//! `REQUIRE_SERVICES=1` (set by `cargo xtask ci` and GitLab CI), which turns a missing
//! database into a failure.
//!
//! Every `test_<uuid>` database is dropped when the test that created it ends, on success
//! (v1-release item 3): the names are kept per thread (the test harness runs each test on a
//! thread of its own and waits for its end), and the end of the thread drops them with
//! `WITH (FORCE)`, so connections still open in the test do not stop it. A test that failed
//! (its thread panicked) keeps its databases and prints their names; `KEEP_TEST_DB=1` keeps
//! every one. Not told apart: a test that fails by returning an `Err` without a panic loses
//! its database, and a `#[should_panic]` test keeps it.
//!
//! The thread that ends is the one that called [`fresh_db_url`]: a database created inside
//! `tokio::task::spawn_blocking` belongs to a thread of the blocking pool, which ends after
//! 10 s without work, so a longer test loses its database in the middle ("database ...
//! does not exist", a terminated connection); create test databases in the test's body.
//! Kept databases (failed runs, `KEEP_TEST_DB`) are not swept automatically: parallel
//! checkouts share the server, and a kept database is kept for a look. The name is
//! `test_<UUIDv7>`, so its age is in the name; to drop the ones of your own runs older than a
//! day, see HANDOFF.md ("Kept test databases").

use std::cell::{Cell, RefCell};
use std::sync::Once;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Create a new database with all migrations applied. Returns `None` when tests should skip.
///
/// # Panics
/// If the server is unreachable, or no URL is set while `REQUIRE_SERVICES=1`.
pub async fn fresh_db() -> Option<PgPool> {
    let url = fresh_db_url().await?;
    Some(
        PgPoolOptions::new()
            .max_connections(20)
            .connect(&url)
            .await
            .expect("connect to test database"),
    )
}

/// Like [`fresh_db`], but returns the URL of the new, migrated database.
///
/// # Panics
/// If the server is unreachable, or no URL is set while `REQUIRE_SERVICES=1`.
pub async fn fresh_db_url() -> Option<String> {
    let url = empty_db_url().await?;
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&url)
        .await
        .expect("connect to test database");
    crate::migrate(&pool).await.expect("migrations");
    pool.close().await;
    Some(url)
}

/// A new database without any migration (for building the schema of an earlier release,
/// `tests/upgrade.rs`); dropped like the others when the test ends.
///
/// # Panics
/// If the server is unreachable, or no URL is set while `REQUIRE_SERVICES=1`.
pub async fn empty_db_url() -> Option<String> {
    let Ok(admin_url) = std::env::var("TEST_DATABASE_URL") else {
        assert!(
            std::env::var_os("REQUIRE_SERVICES").is_none(),
            "REQUIRE_SERVICES is set but TEST_DATABASE_URL is missing"
        );
        eprintln!("skipping: TEST_DATABASE_URL not set (run `cargo xtask services up`)");
        return None;
    };
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("connect to TEST_DATABASE_URL");
    let name = format!("test_{}", uuid::Uuid::now_v7().simple());
    // name is generated above from a UUID, not user input
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;
    remember(&admin_url, &name);
    Some(replace_db_name(&admin_url, &name))
}

/// Whether `KEEP_TEST_DB` asks to keep every test database.
#[must_use]
pub fn keep_requested() -> bool {
    keep_flag(std::env::var("KEEP_TEST_DB").ok().as_deref())
}

/// `1` or `true` (any case) keep the databases; anything else, or no value, does not.
#[must_use]
pub fn keep_flag(value: Option<&str>) -> bool {
    value.is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// The databases this thread created, dropped when the thread ends.
struct Created {
    admin_url: String,
    names: Vec<String>,
}

impl Drop for Created {
    fn drop(&mut self) {
        let names = std::mem::take(&mut self.names);
        if names.is_empty() {
            return;
        }
        // unknown (the flag is gone already) counts as failed: keeping is the safe side
        let failed = PANICKED.try_with(Cell::get).unwrap_or(true);
        if failed || keep_requested() {
            eprintln!("kept test databases: {}", names.join(", "));
            return;
        }
        let admin_url = std::mem::take(&mut self.admin_url);
        // a thread of its own: this runs while the test's thread ends, outside any runtime
        match std::thread::spawn(move || drop_databases(&admin_url, &names)).join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("dropping test databases: {e}"),
            Err(_) => eprintln!("dropping test databases: the thread panicked"),
        }
    }
}

thread_local! {
    static CREATED: RefCell<Option<Created>> = const { RefCell::new(None) };
    static PANICKED: Cell<bool> = const { Cell::new(false) };
}

static PANIC_HOOK: Once = Once::new();

/// Note `name` for the end of this thread, and mark a thread that panics (the hook calls the
/// one installed before, which prints the message).
fn remember(admin_url: &str, name: &str) {
    PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = PANICKED.try_with(|p| p.set(true));
            previous(info);
        }));
    });
    // a thread that is already ending cannot take the name; the database stays then
    let _ = CREATED.try_with(|c| {
        c.borrow_mut()
            .get_or_insert_with(|| Created {
                admin_url: admin_url.to_string(),
                names: Vec::new(),
            })
            .names
            .push(name.to_string());
    });
}

fn drop_databases(admin_url: &str, names: &[String]) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(admin_url)
            .await
            .map_err(|e| e.to_string())?;
        for name in names {
            // the names were generated by `fresh_db_url` from a UUID, not user input
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE IF EXISTS {name} WITH (FORCE)"
            )))
            .execute(&admin)
            .await
            .map_err(|e| format!("{name}: {e}"))?;
        }
        admin.close().await;
        Ok(())
    })
}

fn replace_db_name(url: &str, name: &str) -> String {
    let (base, query) = url
        .split_once('?')
        .map_or((url, None), |(b, q)| (b, Some(q)));
    let base = base.rsplit_once('/').map_or(base, |(b, _)| b);
    match query {
        Some(q) => format!("{base}/{name}?{q}"),
        None => format!("{base}/{name}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn replaces_database_name() {
        assert_eq!(
            super::replace_db_name("postgres://u@h:1/postgres?sslmode=disable", "x"),
            "postgres://u@h:1/x?sslmode=disable"
        );
        assert_eq!(
            super::replace_db_name("postgres://u@h/db", "x"),
            "postgres://u@h/x"
        );
    }
}
