//! The v1 baseline and the upgrade path between releases (v1-release items 4 and 6).
//!
//! - `a_fresh_database_has_the_stored_schema` (always): the migrations of this build create
//!   the schema in `tests/fixtures/schema.txt`. A new migration changes it; regenerate it
//!   with `UPDATE_SCHEMA=1` and review the diff. A migration that is in a tag never changes.
//! - `a_chain_of_migrations_builds_the_schema_of_this_build` (ignored): the database built by
//!   the migration files in `SCHEMA_FROM_DIR` describes the same schema and rows as a fresh
//!   one. Run once for the squash of the pre-v1 migrations into `0001_v1_baseline.sql`.
//! - `the_previous_release_upgrades_to_this_one` (ignored, run by `cargo xtask upgrade-test`):
//!   the migrations of the previous release (`UPGRADE_FROM_DIR`) and its seed
//!   (`UPGRADE_SEED`), then the migrations of this build; every table keeps its rows and
//!   their content (a hash per table over the columns it had, REL-6), the data reads back
//!   through the use-cases with its values, and the schema is the one of a fresh database.
//! - `the_seed_has_rows_in_every_table` (always): the seed has rows in every table but a
//!   short list of caches (REL-3). The seed is written by the ignored server test
//!   `upgrade_seed::write_the_upgrade_seed` (`apps/server/tests/it/upgrade_seed.rs`), through
//!   the server's API and use-cases; see the header of the seed.
//! - `the_snapshot_sees_ownership_security_collation_domains_and_settings` (always): what the
//!   snapshot compares besides tables and columns (REL-9).
//!
//! The stored schema is the output of one Postgres major version (`== postgres`, its first
//! section): after a server upgrade regenerate it with `UPDATE_SCHEMA=1`.

mod common;

use std::path::Path;

use app::ports::UserRepo;
use domain::user::Actor;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

const SCHEMA_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/schema.txt");

async fn connect(url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(url)
        .await
        .expect("connect")
}

/// Schema and rows of a database, as compared here.
async fn snapshot(pool: &PgPool) -> String {
    let schema = store_pg::schema::describe(pool).await.unwrap();
    let rows = store_pg::schema::contents(pool).await.unwrap();
    format!("{schema}== rows\n{rows}")
}

/// The first line where `want` and `got` differ, with its neighbours, or `None`.
fn first_difference(want: &str, got: &str) -> Option<String> {
    let (w, g): (Vec<&str>, Vec<&str>) = (want.lines().collect(), got.lines().collect());
    let at = (0..w.len().max(g.len())).find(|&i| w.get(i) != g.get(i))?;
    let around = |v: &[&str]| {
        v.iter()
            .skip(at.saturating_sub(2))
            .take(5)
            .copied()
            .collect::<Vec<_>>()
            .join("\n")
    };
    Some(format!(
        "line {}:\n--- expected\n{}\n--- found\n{}",
        at + 1,
        around(&w),
        around(&g)
    ))
}

/// A new database built by the migration files in `dir` (sqlx records them in
/// `_sqlx_migrations` with their checksums, as the release that shipped them did).
async fn built_from(dir: &Path) -> Option<PgPool> {
    let url = store_pg::testing::empty_db_url().await?;
    let pool = connect(&url).await;
    let migrator = sqlx::migrate::Migrator::new(dir)
        .await
        .unwrap_or_else(|e| panic!("migrations in {}: {e}", dir.display()));
    migrator.run(&pool).await.expect("the earlier migrations");
    Some(pool)
}

#[tokio::test]
async fn a_fresh_database_has_the_stored_schema() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let now = snapshot(&pool).await;
    if std::env::var_os("UPDATE_SCHEMA").is_some() {
        std::fs::write(SCHEMA_FIXTURE, &now).unwrap();
        return;
    }
    let stored = std::fs::read_to_string(SCHEMA_FIXTURE).unwrap_or_default();
    if let Some(diff) = first_difference(&stored, &now) {
        panic!(
            "the migrations build another schema than tests/fixtures/schema.txt ({diff}).\n\
             After adding a migration run this test with UPDATE_SCHEMA=1 and review the diff; \
             a migration that is in a release tag must never change"
        );
    }
}

#[tokio::test]
#[ignore = "one-off proof of a squash: SCHEMA_FROM_DIR names the migration files it replaces"]
async fn a_chain_of_migrations_builds_the_schema_of_this_build() {
    let dir = std::env::var("SCHEMA_FROM_DIR").expect("SCHEMA_FROM_DIR");
    let Some(chain) = built_from(Path::new(&dir)).await else {
        return;
    };
    let fresh = store_pg::testing::fresh_db().await.unwrap();
    let (want, got) = (snapshot(&chain).await, snapshot(&fresh).await);
    if let Some(diff) = first_difference(&want, &got) {
        panic!("the chain in {dir} and this build differ ({diff})");
    }
    println!(
        "same schema and rows: {} lines compared",
        want.lines().count()
    );
}

/// What a release rewrites on purpose (REL-6), named by the release that does it: a table
/// (`"stats_users"`: its rows and content may change, a cache rebuilt by a migration) or a
/// column (`"lists.note"`: left out of the content comparison of its table, and may be
/// dropped). Everything else keeps its rows and their content over the columns it had.
const REWRITTEN: &[&str] = &[];

#[tokio::test]
#[ignore = "run by `cargo xtask upgrade-test <previous-tag>`"]
async fn the_previous_release_upgrades_to_this_one() {
    let dir = std::env::var("UPGRADE_FROM_DIR").expect("UPGRADE_FROM_DIR");
    let seed = std::env::var("UPGRADE_SEED").expect("UPGRADE_SEED");
    let Some(db) = built_from(Path::new(&dir)).await else {
        return;
    };
    let seed = std::fs::read_to_string(&seed).unwrap_or_else(|e| panic!("{seed}: {e}"));
    sqlx::raw_sql(sqlx::AssertSqlSafe(seed))
        .execute(&db)
        .await
        .expect("the seed of the previous release on its own schema");
    // the content of every table over the columns it has now, but what a release rewrites
    let mut columns = store_pg::schema::table_columns(&db).await.unwrap();
    columns.retain(|t, _| !REWRITTEN.contains(&t.as_str()));
    for (t, cols) in &mut columns {
        cols.retain(|c| !REWRITTEN.contains(&format!("{t}.{c}").as_str()));
    }
    let before = store_pg::schema::table_hashes(&db, &columns).await.unwrap();
    store_pg::migrate(&db).await.expect(
        "the migrations of this build on a database of the previous release (a migration \
         that is in a tag must never change)",
    );
    let read = read_back(&db).await;
    println!("read back: {read}");
    // the data is still there: no table or column gone, every table has its rows with the
    // same content over the columns it had (new tables and columns may have anything)
    let now = store_pg::schema::table_columns(&db).await.unwrap();
    let mut gone = Vec::new();
    for (t, cols) in &columns {
        match now.get(t) {
            None => gone.push(format!("table {t}")),
            Some(have) => gone.extend(
                cols.iter()
                    .filter(|c| !have.contains(c))
                    .map(|c| format!("column {t}.{c}")),
            ),
        }
    }
    assert!(
        gone.is_empty(),
        "the upgrade dropped {gone:?}; a release that does this on purpose lists it in REWRITTEN"
    );
    let after = store_pg::schema::table_hashes(&db, &columns).await.unwrap();
    for (t, b) in &before {
        let a = &after[t];
        assert_eq!(a.rows, b.rows, "rows of {t} after the upgrade");
        assert_eq!(a.md5, b.md5, "content of {t} after the upgrade");
    }
    // and the schema is the one a new installation gets
    let fresh = store_pg::testing::fresh_db().await.unwrap();
    let (want, got) = (
        store_pg::schema::describe(&fresh).await.unwrap(),
        store_pg::schema::describe(&db).await.unwrap(),
    );
    if let Some(diff) = first_difference(&want, &got) {
        panic!("an upgraded database differs from a new one ({diff})");
    }
}

/// Every account's library, items, progress, lists, people and plays through the use-cases
/// of this build (a row the code can no longer decode fails here), with the values every
/// seed has had since v1.0.0 (alice's rating 80, the list note "signed", the play won with
/// 61 points, the year 2001 of "Heat"). Returns a summary.
async fn read_back(pool: &PgPool) -> String {
    let env = common::env_on(pool.clone());
    let app = &env.app;
    let users = env.store.list().await.expect("users");
    assert!(users.len() >= 2, "the seed has two accounts");
    let (mut entries, mut events, mut lists, mut people, mut plays) =
        (0_usize, 0, 0_usize, 0, 0_usize);
    let (mut ratings, mut notes, mut scores, mut years) = (vec![], vec![], vec![], vec![]);
    for u in &users {
        let me = Actor {
            user_id: u.id,
            role: u.role,
        };
        for e in app.library(me).await.expect("library") {
            let item = app.catalog_item(e.item_id).await.expect("item");
            years.push((item.title, item.year));
            ratings.extend(e.rating.value.map(domain::library::Rating::value));
            events += app.progress(me, e.item_id).await.expect("progress").len();
            entries += 1;
        }
        for l in app.lists(me).await.expect("lists") {
            let list = app.list(me, l.id).await.expect("list");
            notes.extend(list.items.into_iter().filter_map(|i| i.note));
            lists += 1;
        }
        people += app.companions(me).await.expect("people").len();
        for s in app.my_sessions(me).await.expect("plays") {
            scores.extend(
                s.participants
                    .iter()
                    .filter(|p| p.winner)
                    .filter_map(|p| p.score),
            );
            plays += 1;
        }
    }
    for (n, table) in [
        (entries, "library_entries"),
        (lists, "lists"),
        (plays, "sessions"),
    ] {
        let rows = common::count(&env.store, &format!("SELECT count(*) FROM {table}")).await;
        assert_eq!(i64::try_from(n).unwrap(), rows, "{table} read back");
    }
    assert!(entries > 0 && events > 0 && lists > 0 && plays > 0);
    assert!(ratings.contains(&80), "the rating 80: {ratings:?}");
    assert!(notes.iter().any(|n| n == "signed"), "the note: {notes:?}");
    assert!(scores.contains(&61.0), "the winner's 61 points: {scores:?}");
    assert!(
        years.iter().any(|(t, y)| t == "Heat" && *y == Some(2001)),
        "the year of Heat: {years:?}"
    );
    format!(
        "{} accounts, {entries} library entries, {events} progress events, {lists} lists, \
         {people} people, {plays} plays",
        users.len()
    )
}

/// REL-9: what a migration can change without a new table or column is still seen by the
/// snapshot (the stored schema and "upgraded equals fresh"): each change below on a fresh
/// database makes its snapshot differ from the untouched one.
#[tokio::test]
async fn the_snapshot_sees_ownership_security_collation_domains_and_settings() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let base = snapshot(&pool).await;
    for (what, change, undo) in [
        (
            "sequence ownership",
            "ALTER SEQUENCE outbox_id_seq OWNED BY NONE",
            "ALTER SEQUENCE outbox_id_seq OWNED BY outbox.id",
        ),
        (
            "row level security flag",
            "ALTER TABLE lists ENABLE ROW LEVEL SECURITY",
            "ALTER TABLE lists DISABLE ROW LEVEL SECURITY",
        ),
        (
            "forced row level security",
            "ALTER TABLE lists FORCE ROW LEVEL SECURITY",
            "ALTER TABLE lists NO FORCE ROW LEVEL SECURITY",
        ),
        (
            "column collation",
            "ALTER TABLE lists ALTER COLUMN name TYPE text COLLATE \"C\"",
            "ALTER TABLE lists ALTER COLUMN name TYPE text COLLATE \"default\"",
        ),
        (
            "storage parameters",
            "ALTER TABLE lists SET (fillfactor = 70)",
            "ALTER TABLE lists RESET (fillfactor)",
        ),
        (
            "unlogged table",
            "ALTER TABLE http_cache SET UNLOGGED",
            "ALTER TABLE http_cache SET LOGGED",
        ),
        (
            "domain constraint",
            "CREATE DOMAIN rel9_pct AS integer CHECK (VALUE BETWEEN 0 AND 100)",
            "DROP DOMAIN rel9_pct",
        ),
        (
            "a value in the settings row",
            "UPDATE instance_settings SET setup_step = 'rel9'",
            "UPDATE instance_settings SET setup_step = NULL",
        ),
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(change))
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("{change}: {e}"));
        assert_ne!(snapshot(&pool).await, base, "{what} is not in the snapshot");
        sqlx::raw_sql(sqlx::AssertSqlSafe(undo))
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("{undo}: {e}"));
        assert_eq!(snapshot(&pool).await, base, "undoing {what}");
    }
    // a domain's CHECK is part of the schema, not only its name
    let mut seen = Vec::new();
    for check in ["VALUE > 0", "VALUE > 1"] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE DOMAIN rel9_pos AS integer CHECK ({check})"
        )))
        .execute(&pool)
        .await
        .unwrap();
        seen.push(snapshot(&pool).await);
        sqlx::raw_sql("DROP DOMAIN rel9_pos")
            .execute(&pool)
            .await
            .unwrap();
    }
    assert_ne!(seen[0], seen[1], "the CHECK of a domain");
    assert!(
        base.starts_with("== postgres\n"),
        "the snapshot names the Postgres major version first (the fixture is the output of \
         one major version)"
    );
}

const SEED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/upgrade-seed.sql"
);

/// Tables the upgrade seed leaves empty on purpose (REL-3): caches and short-lived guards
/// that an instance rebuilds or drops at any time. Every other table of the stored schema has
/// at least one row in the seed, so that a migration adding a NOT NULL column, a CHECK or a
/// unique index meets real rows in the next release's upgrade test.
const SEED_EMPTY_ON_PURPOSE: &[(&str, &str)] = &[
    ("http_cache", "responses of the data sources, a cache"),
    ("oidc_used_jti", "replay guard of a sign-in, minutes long"),
    (
        "oidc_used_states",
        "replay guard of a sign-in, minutes long",
    ),
    (
        "stats_dirty",
        "written by the statistics triggers while the seed loads",
    ),
];

/// REL-3: the seed covers every table of this build's schema except the short list above.
#[test]
fn the_seed_has_rows_in_every_table() {
    let schema = std::fs::read_to_string(SCHEMA_FIXTURE).unwrap();
    let seed = std::fs::read_to_string(SEED).unwrap();
    let tables: Vec<&str> = schema
        .split("== rows\n")
        .nth(1)
        .expect("a `== rows` section in schema.txt")
        .lines()
        .filter_map(|l| l.split(' ').next())
        .filter(|t| !t.is_empty())
        .collect();
    for (t, _) in SEED_EMPTY_ON_PURPOSE {
        assert!(tables.contains(t), "{t} is no table of the schema any more");
        assert!(
            !seed.contains(&format!("INSERT INTO {t} ")),
            "{t} has seed rows: take it off the list"
        );
    }
    let empty: Vec<&str> = tables
        .iter()
        .copied()
        .filter(|t| !SEED_EMPTY_ON_PURPOSE.iter().any(|(e, _)| e == t))
        .filter(|t| !seed.contains(&format!("INSERT INTO {t} ")))
        .collect();
    assert!(
        empty.is_empty(),
        "tables without a row in tests/fixtures/upgrade-seed.sql: {empty:?}. Extend \
         `write_the_upgrade_seed` (server tests, upgrade_seed.rs) and regenerate the seed, or \
         list a cache in SEED_EMPTY_ON_PURPOSE with the reason"
    );
}
