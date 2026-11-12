//! OPS-6: the migration dry run lists what `migrate` would apply and changes nothing.

use store_pg::{MIGRATOR, pending_migrations};

async fn applied(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_dry_run_lists_the_pending_migrations_and_changes_nothing() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let total = i64::try_from(MIGRATOR.iter().count()).unwrap();
    assert_eq!(pending_migrations(&pool).await.unwrap().len(), 0);
    assert_eq!(applied(&pool).await, total);

    // a database two releases behind: the two newest are pending, oldest first
    let newest: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version DESC LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ANY($1)")
        .bind(&newest)
        .execute(&pool)
        .await
        .unwrap();
    let pending = pending_migrations(&pool).await.unwrap();
    assert_eq!(
        pending.iter().map(|m| m.version).collect::<Vec<_>>(),
        vec![newest[1], newest[0]]
    );
    let last = MIGRATOR.iter().last().unwrap();
    assert_eq!(pending[1].description, last.description);
    assert_ne!(pending[1].description, "");
    assert_eq!(applied(&pool).await, total - 2, "nothing was applied");

    // a database that was never migrated: everything is pending, and the dry run does not
    // even create the bookkeeping table
    sqlx::query("DROP TABLE _sqlx_migrations")
        .execute(&pool)
        .await
        .unwrap();
    let pending = pending_migrations(&pool).await.unwrap();
    assert_eq!(i64::try_from(pending.len()).unwrap(), total);
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!exists, "the dry run created the bookkeeping table");
}
