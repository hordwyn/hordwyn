//! `cargo xtask upgrade-test [<previous-tag>]` (v1-release item 6): the database of the
//! previous release, with its seed data, migrated by this build.
//!
//! The migrations and the seed of the previous release come from its tag
//! (`git show <tag>:<path>`), so nothing of an old release has to be kept in the tree. The
//! work is the ignored test `the_previous_release_upgrades_to_this_one` of
//! `crates/store-pg/tests/upgrade.rs`: the tag's migrations through sqlx (as that release
//! applied them, checksums included), its seed, then the migrations of this build, a
//! read-back through the use-cases, and the schema compared with a new installation's.
//! Without a release tag below this build (before v1, or on the v1 tag itself) there is
//! nothing to upgrade from, and the command says so and succeeds.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The migrations of the store, relative to the repository root.
pub const MIGRATIONS: &str = "src/rust/crates/store-pg/migrations";
/// The seed data of this build, read from the previous release's tag by the next one.
pub const SEED: &str = "src/rust/crates/store-pg/tests/fixtures/upgrade-seed.sql";

/// A release tag: `v` and dot-separated numbers (`v1`, `v1.0.0`). The milestone tags
/// `m<NN>` and the package tags of the build (`v1-polish`, `v1-books`) are not releases.
#[must_use]
pub fn is_release_tag(tag: &str) -> bool {
    tag.strip_prefix('v').is_some_and(|rest| {
        rest.split('.')
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// The release to upgrade from: the first release tag of `merged` (the tags reachable from
/// this build, newest first) that does not point at this build itself (`at_head`).
#[must_use]
pub fn previous_release(merged: &[String], at_head: &[String]) -> Option<String> {
    merged
        .iter()
        .find(|t| is_release_tag(t) && !at_head.contains(t))
        .cloned()
}

fn git(args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(crate::repo_root())
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// The tag to upgrade from: the one given (it must exist), else [`previous_release`] of the
/// tags reachable from `HEAD`. `None`: there is no release below this build.
///
/// # Errors
/// git fails, or the given tag does not exist.
pub fn choose(given: Option<&str>) -> Result<Option<String>> {
    if let Some(tag) = given {
        git(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{tag}^{{commit}}"),
        ])
        .with_context(|| format!("there is no tag `{tag}` in this clone (fetch the tags?)"))?;
        return Ok(Some(tag.to_string()));
    }
    let merged = lines(&git(&["tag", "--merged", "HEAD", "--sort=-v:refname"])?);
    let at_head = lines(&git(&["tag", "--points-at", "HEAD"])?);
    Ok(previous_release(&merged, &at_head))
}

/// Write the migrations and the seed of `tag` under `dest`; returns the directory of the
/// migrations and the path of the seed.
///
/// # Errors
/// git fails, or the tag has no migrations or no seed (a tag before v1).
pub fn extract(tag: &str, dest: &Path) -> Result<(PathBuf, PathBuf)> {
    let dir = dest.join("migrations");
    if dest.exists() {
        std::fs::remove_dir_all(dest).with_context(|| dest.display().to_string())?;
    }
    std::fs::create_dir_all(&dir).with_context(|| dir.display().to_string())?;
    let files = lines(&git(&[
        "ls-tree",
        "--name-only",
        tag,
        &format!("{MIGRATIONS}/"),
    ])?);
    let files: Vec<&String> = files
        .iter()
        .filter(|f| {
            Path::new(f)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("sql"))
        })
        .collect();
    if files.is_empty() {
        bail!("{tag} has no migrations under {MIGRATIONS}");
    }
    for file in files {
        let name = Path::new(file).file_name().context("a file name")?;
        std::fs::write(dir.join(name), git(&["show", &format!("{tag}:{file}")])?)?;
    }
    let seed = dest.join("upgrade-seed.sql");
    let text = git(&["show", &format!("{tag}:{SEED}")])
        .with_context(|| format!("{tag} has no upgrade seed ({SEED})"))?;
    std::fs::write(&seed, text)?;
    Ok((dir, seed))
}

/// The cargo arguments of the ignored upgrade test.
#[must_use]
pub fn cargo_args() -> Vec<&'static str> {
    vec![
        "test",
        "-p",
        "store-pg",
        "--test",
        "upgrade",
        "--",
        "--ignored",
        "--exact",
        "the_previous_release_upgrades_to_this_one",
        "--nocapture",
    ]
}

/// The seed of this build from a `pg_dump --data-only --inserts --column-inserts` of the
/// database `write_the_upgrade_seed` left (REL-3), after `header`: in one transaction, the
/// migration's settings row replaced by the seed's, the accounts first (their self-companion
/// key is deferred, the companions' owner key is not), every other row in the dump's order,
/// then the sequences. psql and session commands, comments and `stats_dirty` (its rows come
/// from the statistics triggers while the seed loads) are left out; the `public.` qualifier
/// is removed from the table and sequence names only, never from the data.
#[must_use]
pub fn seed_from_dump(dump: &str, header: &str) -> String {
    let (mut users, mut rows, mut sequences) = (Vec::new(), Vec::new(), Vec::new());
    for line in statements(dump) {
        let line = line.as_str();
        if let Some(rest) = line.strip_prefix("INSERT INTO public.") {
            if rest.starts_with("stats_dirty ") {
                continue;
            }
            let insert = format!("INSERT INTO {rest}");
            if rest.starts_with("users ") {
                users.push(insert);
            } else {
                rows.push(insert);
            }
        } else if let Some(rest) = line.strip_prefix("SELECT pg_catalog.setval('public.") {
            sequences.push(format!("SELECT pg_catalog.setval('{rest}"));
        } else if line.starts_with("INSERT INTO ") {
            rows.push(line.to_string());
        }
    }
    let mut out = format!("{header}BEGIN;\nDELETE FROM instance_settings;\n");
    for line in users.iter().chain(&rows).chain(&sequences) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("COMMIT;\n");
    out
}

/// The lines of `dump`, with a statement whose string value spans several lines (a text with
/// line breaks) joined into one item: it ends where its quotes are balanced (`pg_dump` doubles
/// a quote inside a value) and the line ends with `;`.
fn statements(dump: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<String> = None;
    for line in dump.lines() {
        let text = match open.take() {
            Some(mut s) => {
                s.push('\n');
                s.push_str(line);
                s
            }
            None => line.to_string(),
        };
        let inside = text.matches('\'').count() % 2 == 1;
        if text.starts_with("INSERT INTO ") && (inside || !text.ends_with(';')) {
            open = Some(text);
        } else {
            out.push(text);
        }
    }
    out.extend(open);
    out
}

/// The header of the seed file.
pub const SEED_HEADER: &str = "\
-- Seed data of the upgrade test (v1-release item 6, REL-3; `cargo xtask upgrade-test`): a
-- small household instance on the schema of THIS release. The next release reads this file
-- from this release's tag, applies it to a database built by this release's migrations, runs
-- its own migrations, compares the content of every table and reads the data back through
-- its use-cases (crates/store-pg/tests/upgrade.rs). Every table has rows but the caches
-- listed in that test (`the_seed_has_rows_in_every_table`).
-- Generated, never edited by hand: run the ignored server test (in the dev shell, with the
-- local services)
--   KEEP_TEST_DB=1 TEST_DATABASE_URL=... cargo test -p server --features \\
--     gallery,all-connectors --test it -- --ignored --exact \\
--     upgrade_seed::write_the_upgrade_seed --nocapture
-- then `cargo xtask upgrade-seed <the database URL it printed>` (pg_dump of the same major
-- version as the server) and drop that database. Regenerate it in every change that adds a
-- migration. The secrets in it are test values encrypted with a key that no longer exists.
";

/// `pg_dump` arguments for [`seed_from_dump`].
#[must_use]
pub fn pg_dump_args(url: &str) -> Vec<String> {
    [
        "--data-only",
        "--inserts",
        "--column-inserts",
        "--no-owner",
        "--no-privileges",
        "--exclude-table-data=_sqlx_migrations",
        url,
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}
