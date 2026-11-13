//! OPS-6: the migration dry run lists what `migrate` would apply and changes nothing.

use std::fmt::Write as _;

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

    // a database behind this build: the newest (at most two) are pending, oldest first (with
    // the v1 baseline alone that is the baseline itself)
    let behind = total.min(2);
    let newest: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version DESC LIMIT $1")
            .bind(behind)
            .fetch_all(&pool)
            .await
            .unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ANY($1)")
        .bind(&newest)
        .execute(&pool)
        .await
        .unwrap();
    let pending = pending_migrations(&pool).await.unwrap();
    let mut oldest_first = newest.clone();
    oldest_first.reverse();
    assert_eq!(
        pending.iter().map(|m| m.version).collect::<Vec<_>>(),
        oldest_first
    );
    let last = MIGRATOR.iter().last().unwrap();
    assert_eq!(pending.last().unwrap().description, last.description);
    assert_ne!(pending.last().unwrap().description, "");
    assert_eq!(applied(&pool).await, total - behind, "nothing was applied");

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

/// REL-13: the dry run also compares what the database applied with this build's files. A
/// changed released migration (another checksum) or a version this build does not have
/// (a database of an unrelated build) is named, so the dry run does not promise a start
/// that `MIGRATOR.run` would refuse.
#[tokio::test]
async fn the_dry_run_names_applied_migrations_that_differ_from_this_build() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    assert_eq!(
        store_pg::mismatched_migrations(&pool).await.unwrap(),
        vec![],
        "a database of this build matches"
    );
    let first = MIGRATOR.iter().next().unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum = '\\x00'::bytea WHERE version = $1")
        .bind(first.version)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (9999, 'from another build', true, '\\x00'::bytea, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let found = store_pg::mismatched_migrations(&pool).await.unwrap();
    assert_eq!(
        found,
        vec![
            store_pg::MigrationMismatch::Changed {
                version: first.version,
                description: first.description.to_string(),
            },
            store_pg::MigrationMismatch::Unknown {
                version: 9999,
                description: "from another build".into(),
            },
        ]
    );
    assert!(found[0].to_string().contains("changed"), "{}", found[0]);
    // a database that was never migrated has nothing applied, so nothing differs
    sqlx::query("DROP TABLE _sqlx_migrations")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store_pg::mismatched_migrations(&pool).await.unwrap(),
        vec![]
    );
}

const RELEASED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/released-migrations.txt"
);

/// REL-5: a migration that is in a release tag never changes, not even a comment (sqlx
/// compares the checksum of the file and refuses to start). The checksums of the released
/// files are stored in `tests/fixtures/released-migrations.txt` and compared with the
/// embedded set in every test run, without git history or a database.
#[test]
fn released_migrations_have_not_changed() {
    let text = std::fs::read_to_string(RELEASED).unwrap_or_else(|e| panic!("{RELEASED}: {e}"));
    let hex = |b: &[u8]| {
        b.iter().fold(String::new(), |mut s, x| {
            let _ = write!(s, "{x:02x}");
            s
        })
    };
    let mut listed = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [version, sum, file] = parts[..] else {
            panic!("released-migrations.txt: `{line}` is not `<version> <sha384> <file>`");
        };
        let version: i64 = version.parse().unwrap();
        let m = MIGRATOR
            .iter()
            .find(|m| m.version == version)
            .unwrap_or_else(|| panic!("released migration {file} is gone from this build"));
        assert_eq!(
            hex(&m.checksum),
            sum,
            "{file} is in a release and has changed: restore the released file and put the \
             change into a new migration"
        );
        listed.push(version);
    }
    let newest = listed.iter().max().copied().unwrap_or(0);
    let missing: Vec<String> = MIGRATOR
        .iter()
        .filter(|m| m.version <= newest && !listed.contains(&m.version))
        .map(|m| format!("{:04} {}", m.version, m.description))
        .collect();
    assert!(
        missing.is_empty(),
        "migrations older than the newest released one, but not released: {missing:?}"
    );
    for m in MIGRATOR.iter().filter(|m| m.version > newest) {
        println!(
            "not released yet (append at the release): {:04} {} {}",
            m.version,
            hex(&m.checksum),
            m.description
        );
    }
}
