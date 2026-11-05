//! Fresh, migrated test databases on the server named by `TEST_DATABASE_URL`.
//!
//! Without `TEST_DATABASE_URL` tests are skipped with a message, unless
//! `REQUIRE_SERVICES=1` (set by `cargo xtask ci` and GitLab CI), which turns a missing
//! database into a failure.

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Create a new database with all migrations applied. Returns `None` when tests should skip.
///
/// # Panics
/// If the server is unreachable, or no URL is set while `REQUIRE_SERVICES=1`.
pub async fn fresh_db() -> Option<PgPool> {
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
    let url = replace_db_name(&admin_url, &name);
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&url)
        .await
        .expect("connect to test database");
    crate::migrate(&pool).await.expect("migrations");
    Some(pool)
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
