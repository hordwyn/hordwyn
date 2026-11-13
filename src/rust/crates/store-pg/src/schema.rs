//! A text description of the database schema, read from the system catalogs: what the
//! migrations built, in a stable order and without anything that depends on the server
//! process (object ids, the order of creation). Two databases with the same description have
//! the same tables (with their storage parameters, row level security flags and
//! persistence), columns (with collations), defaults, constraints, indexes, triggers,
//! functions, views, sequences (with their owning column), types, domain constraints,
//! comments, policies (with their roles) and extensions in the `public` schema.
//!
//! Used by the squash of the migrations into the v1 baseline (v1-release item 4: the chain
//! of the pre-v1 migrations and the baseline describe the same schema), by the stored schema
//! of the baseline (`tests/fixtures/schema.txt`) and by `cargo xtask upgrade-test` (a
//! database of the previous release, migrated forward, describes the schema of a fresh one).
//! It needs no client binary such as `pg_dump`, so it runs wherever the tests run.
//!
//! Stable across clusters: every text sort is `COLLATE "C"` (a cluster with a glibc locale
//! would sort otherwise), and the hashes of [`contents`] are taken with fixed session
//! settings (time zone UTC, ISO dates). Not stable across Postgres major versions: the text
//! is the output of `pg_get_*def` and of the catalogs of one major version (from 18 on, NOT
//! NULL constraints are rows of `pg_constraint`), so the first section names the major and a
//! stored description is regenerated after a server upgrade.
//!
//! Not seen (none of it is used by the migrations; add a section before a migration uses
//! it): aggregates, rules, event triggers, extended statistics, operators and casts, schemas
//! other than `public`, extension versions, privileges and ownership, comments on anything
//! but tables and columns, invalid indexes.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use sqlx::PgPool;

/// Objects that belong to an extension (`pg_trgm` puts its functions and types into
/// `public`) are the extension's, not ours.
const NOT_EXTENSION: &str =
    "NOT EXISTS (SELECT 1 FROM pg_depend e WHERE e.objid = {oid} AND e.deptype = 'e')";

/// How the lines of a section are ordered.
enum Order {
    /// By the text of the line, bytewise (`COLLATE "C"`); the query has no `ORDER BY`.
    Text,
    /// By the query's own `ORDER BY` (columns in their table order).
    Query,
}

/// The sections of the description, each a query that returns one text column, one row per
/// line. `_sqlx_migrations` (sqlx's bookkeeping) is left out.
#[allow(clippy::too_many_lines)]
fn sections() -> Vec<(&'static str, Order, String)> {
    let not_ext = |oid: &str| NOT_EXTENSION.replace("{oid}", oid);
    vec![
        (
            "postgres",
            Order::Text,
            "SELECT 'major ' || (current_setting('server_version_num')::int / 10000)::text".into(),
        ),
        (
            "extensions",
            Order::Text,
            "SELECT extname::text FROM pg_extension WHERE extname <> 'plpgsql'".into(),
        ),
        (
            "types",
            Order::Text,
            format!(
                "SELECT t.typname || ' ' || t.typtype::text || COALESCE(' ' || (SELECT \
                 string_agg(e.enumlabel::text, ',' ORDER BY e.enumsortorder) FROM pg_enum e \
                 WHERE e.enumtypid = t.oid), '') \
                 || CASE WHEN t.typtype = 'd' THEN ' ' || format_type(t.typbasetype, t.typtypmod) \
                 || CASE WHEN t.typnotnull THEN ' not null' ELSE '' END \
                 || COALESCE(' default ' || t.typdefault, '') ELSE '' END FROM pg_type t \
                 WHERE t.typnamespace = 'public'::regnamespace AND t.typtype IN ('e', 'd', 'c') \
                 AND (t.typrelid = 0 OR (SELECT c.relkind FROM pg_class c WHERE c.oid = t.typrelid) = 'c') \
                 AND {}",
                not_ext("t.oid")
            ),
        ),
        (
            "domain constraints",
            Order::Text,
            format!(
                "SELECT t.typname || ' ' || con.conname || ' ' || pg_get_constraintdef(con.oid) \
                 FROM pg_constraint con JOIN pg_type t ON t.oid = con.contypid \
                 WHERE con.contypid <> 0 AND t.typnamespace = 'public'::regnamespace AND {}",
                not_ext("t.oid")
            ),
        ),
        (
            "tables",
            Order::Text,
            "SELECT c.relname || ' ' || c.relkind::text \
             || CASE c.relpersistence WHEN 'u' THEN ' unlogged' WHEN 't' THEN ' temporary' ELSE '' END \
             || CASE WHEN c.relrowsecurity THEN ' row-security' ELSE '' END \
             || CASE WHEN c.relforcerowsecurity THEN ' forced-row-security' ELSE '' END \
             || COALESCE(' with ' || array_to_string(c.reloptions, ','), '') \
             FROM pg_class c WHERE c.relnamespace = 'public'::regnamespace \
             AND c.relkind IN ('r', 'p', 'v', 'm', 'f') AND c.relname <> '_sqlx_migrations'"
                .into(),
        ),
        (
            "columns",
            Order::Query,
            "SELECT c.relname || '.' || a.attname || ' ' || format_type(a.atttypid, a.atttypmod) \
             || CASE WHEN a.attcollation <> 0 AND a.attcollation <> t.typcollation \
             THEN ' collate ' || quote_ident(co.collname) ELSE '' END \
             || CASE WHEN a.attnotnull THEN ' not null' ELSE '' END \
             || COALESCE(' default ' || pg_get_expr(d.adbin, d.adrelid), '') \
             || CASE a.attidentity WHEN 'a' THEN ' identity always' WHEN 'd' THEN ' identity by default' ELSE '' END \
             || CASE a.attgenerated WHEN 's' THEN ' generated stored' WHEN 'v' THEN ' generated virtual' ELSE '' END \
             || COALESCE(' with ' || array_to_string(a.attoptions, ','), '') \
             FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid \
             JOIN pg_type t ON t.oid = a.atttypid \
             LEFT JOIN pg_collation co ON co.oid = a.attcollation \
             LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
             WHERE c.relnamespace = 'public'::regnamespace AND c.relkind IN ('r', 'p', 'v', 'm', 'f') \
             AND c.relname <> '_sqlx_migrations' AND a.attnum > 0 AND NOT a.attisdropped \
             ORDER BY c.relname, a.attnum"
                .into(),
        ),
        (
            "constraints",
            Order::Text,
            "SELECT conrelid::regclass::text || ' ' || conname || ' ' || pg_get_constraintdef(oid) \
             FROM pg_constraint WHERE connamespace = 'public'::regnamespace AND conrelid <> 0 \
             AND conrelid::regclass::text <> '_sqlx_migrations'"
                .into(),
        ),
        (
            "indexes",
            Order::Text,
            "SELECT pg_get_indexdef(i.indexrelid) FROM pg_index i JOIN pg_class c ON c.oid = i.indrelid \
             WHERE c.relnamespace = 'public'::regnamespace AND c.relname <> '_sqlx_migrations'"
                .into(),
        ),
        (
            "triggers",
            Order::Text,
            "SELECT pg_get_triggerdef(t.oid) || CASE t.tgenabled WHEN 'O' THEN '' ELSE ' enabled=' \
             || t.tgenabled::text END FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid \
             WHERE c.relnamespace = 'public'::regnamespace AND NOT t.tgisinternal"
                .into(),
        ),
        (
            "functions",
            Order::Query,
            format!(
                "SELECT pg_get_functiondef(p.oid) FROM pg_proc p \
                 WHERE p.pronamespace = 'public'::regnamespace AND p.prokind IN ('f', 'p') AND {} \
                 ORDER BY p.proname, pg_get_function_identity_arguments(p.oid) COLLATE \"C\"",
                not_ext("p.oid")
            ),
        ),
        (
            "views",
            Order::Text,
            "SELECT c.relname || ' ' || c.relkind::text || ' ' || pg_get_viewdef(c.oid) FROM pg_class c \
             WHERE c.relnamespace = 'public'::regnamespace AND c.relkind IN ('v', 'm')"
                .into(),
        ),
        (
            "sequences",
            Order::Text,
            "SELECT s.sequencename || ' ' || s.data_type::text || ' start ' || s.start_value \
             || ' min ' || s.min_value || ' max ' || s.max_value || ' by ' || s.increment_by \
             || ' cycle ' || s.cycle || ' cache ' || s.cache_size \
             || COALESCE((SELECT ' owned by ' || d.refobjid::regclass::text || '.' || a.attname \
             FROM pg_depend d JOIN pg_attribute a ON a.attrelid = d.refobjid AND a.attnum = d.refobjsubid \
             WHERE d.classid = 'pg_class'::regclass AND d.refclassid = 'pg_class'::regclass \
             AND d.objid = format('%I.%I', s.schemaname, s.sequencename)::regclass \
             AND d.deptype IN ('a', 'i')), '') \
             FROM pg_sequences s WHERE s.schemaname = 'public'"
                .into(),
        ),
        (
            "comments",
            Order::Text,
            "SELECT c.relname || COALESCE('.' || a.attname, '') || ' ' || d.description \
             FROM pg_description d JOIN pg_class c ON c.oid = d.objoid AND d.classoid = 'pg_class'::regclass \
             LEFT JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = d.objsubid AND d.objsubid > 0 \
             WHERE c.relnamespace = 'public'::regnamespace"
                .into(),
        ),
        (
            "policies",
            Order::Text,
            "SELECT tablename || ' ' || policyname || ' ' || permissive || ' ' || cmd || ' to ' \
             || array_to_string(roles, ',') || ' ' || COALESCE(qual, '') || ' ' \
             || COALESCE(with_check, '') FROM pg_policies WHERE schemaname = 'public'"
                .into(),
        ),
    ]
}

/// The schema of the database `pool` points at, as text (see the module).
///
/// # Errors
/// Database errors.
pub async fn describe(pool: &PgPool) -> Result<String, sqlx::Error> {
    let mut out = String::new();
    for (name, order, query) in sections() {
        let query = match order {
            Order::Text => format!("SELECT l FROM ({query}) s(l) ORDER BY l COLLATE \"C\""),
            Order::Query => query,
        };
        let lines: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
            .fetch_all(pool)
            .await?;
        let _ = writeln!(out, "== {name}");
        for line in lines {
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    Ok(out)
}

/// `"name"`: an identifier from the catalog, quoted.
fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// The columns of every table of `public` except `_sqlx_migrations`, in table order.
///
/// # Errors
/// Database errors.
pub async fn table_columns(pool: &PgPool) -> Result<BTreeMap<String, Vec<String>>, sqlx::Error> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT c.relname::text, a.attname::text FROM pg_attribute a \
         JOIN pg_class c ON c.oid = a.attrelid \
         WHERE c.relnamespace = 'public'::regnamespace AND c.relkind IN ('r', 'p') \
         AND c.relname <> '_sqlx_migrations' AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY c.relname, a.attnum",
    )
    .fetch_all(pool)
    .await?;
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (table, column) in rows {
        out.entry(table).or_default().push(column);
    }
    Ok(out)
}

/// The rows of one table: how many, and the MD5 of their text over `columns`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableContent {
    pub rows: i64,
    pub md5: String,
}

/// The number of rows and a hash of their content for each table of `columns` (REL-6), over
/// the columns named there: compare a database before and after an upgrade over the columns
/// it had before. Rows are ordered by the text of their primary key (or of the whole row
/// without one), bytewise, under fixed session settings, so the hash depends on the data
/// only. A table of `columns` that no longer exists is left out of the result.
///
/// # Errors
/// Database errors, a column that no longer exists.
pub async fn table_hashes(
    pool: &PgPool,
    columns: &BTreeMap<String, Vec<String>>,
) -> Result<BTreeMap<String, TableContent>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(
        "SET LOCAL TimeZone = 'UTC'; SET LOCAL DateStyle = 'ISO, YMD'; \
         SET LOCAL IntervalStyle = 'postgres'; SET LOCAL extra_float_digits = 1; \
         SET LOCAL bytea_output = 'hex'",
    )
    .execute(&mut *tx)
    .await?;
    let keys: Vec<(String, Vec<String>)> = sqlx::query_as(
        "SELECT c.relname::text, array_agg(a.attname::text ORDER BY k.n) FROM pg_index i \
         JOIN pg_class c ON c.oid = i.indrelid \
         CROSS JOIN LATERAL unnest(i.indkey) WITH ORDINALITY k(attnum, n) \
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.attnum \
         WHERE i.indisprimary AND c.relnamespace = 'public'::regnamespace GROUP BY c.relname",
    )
    .fetch_all(&mut *tx)
    .await?;
    let keys: BTreeMap<String, Vec<String>> = keys.into_iter().collect();
    let existing = table_columns(pool).await?;
    let mut out = BTreeMap::new();
    for (table, cols) in columns {
        if !existing.contains_key(table) {
            continue;
        }
        let row = if cols.is_empty() {
            "''".to_string()
        } else {
            format!(
                "ROW({})::text",
                cols.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ")
            )
        };
        let order = match keys.get(table) {
            Some(k) if k.iter().all(|c| cols.contains(c)) => format!(
                "ROW({})::text",
                k.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ")
            ),
            _ => row.clone(),
        };
        // the names come from the catalog, quoted as identifiers
        let (rows, md5): (i64, String) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT count(*), md5(COALESCE(string_agg({row}, E'\\n' ORDER BY {order} COLLATE \"C\", \
             {row} COLLATE \"C\"), '')) FROM {}",
            ident(table)
        )))
        .fetch_one(&mut *tx)
        .await?;
        out.insert(table.clone(), TableContent { rows, md5 });
    }
    tx.commit().await?;
    Ok(out)
}

/// Every table of `public` except `_sqlx_migrations`, one line `<table> <rows> <md5>` each
/// ([`table_hashes`] over all its columns): what the migrations wrote besides the schema
/// (the one row of `instance_settings`) in a fresh database, or what a seed left in an
/// upgraded one.
///
/// # Errors
/// Database errors.
pub async fn contents(pool: &PgPool) -> Result<String, sqlx::Error> {
    let columns = table_columns(pool).await?;
    let mut out = String::new();
    for (table, c) in table_hashes(pool, &columns).await? {
        let _ = writeln!(out, "{table} {} {}", c.rows, c.md5);
    }
    Ok(out)
}
