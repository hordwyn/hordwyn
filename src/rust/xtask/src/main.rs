//! `cargo xtask <command>`, see [`USAGE`].
#![deny(unsafe_code)]

use std::process::Command;

use anyhow::{Context, Result, bail};

const USAGE: &str = "usage: cargo xtask <command>

  ci                 everything CI runs that does not need docker: fmt check, clippy,
                     cargo-deny, licence notices, sqlx offline-cache check, tests
                     (starts local services when TEST_DATABASE_URL is unset)
  fmt                format the workspace
  e2e [<playwright args>]
                     run the Playwright end-to-end suite (arguments go to `playwright
                     test`, such as a spec file or `-g <title>`)
  e2e-scan <log>     fail when a server log of an e2e run holds a panic (CI job)
  dev                start the development server
  services up|down   start or stop local Postgres, Meilisearch and RustFS (S3)
  sqlx-prepare       migrate the dev database and refresh the sqlx offline cache
  upgrade-test [<previous-tag>]
                     the database of the previous release (its migrations and seed from
                     its tag), migrated by this build and read back; without a release
                     tag below this build it skips (before v1)
  upgrade-seed <database-url>
                     write the upgrade seed of this build from the database the ignored
                     server test `upgrade_seed::write_the_upgrade_seed` left (pg_dump)
  record <connector> re-record connector fixtures from the live API (needs its key)
  load [--release] [--clients N]
                     the ignored load test against the local services (not in ci;
                     needs 15 GB free disk; delete target/release after a release run)";

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "ci" => ci(),
        "fmt" => cargo(&["fmt", "--all"], &[]),
        "e2e" => e2e(&std::env::args().skip(2).collect::<Vec<_>>()),
        // the job keeps the file either way (an artifact on failure), so it never deletes it
        "e2e-scan" => match std::env::args().nth(2) {
            Some(log) => scan_server_log(std::path::Path::new(&log), false),
            None => bail!("usage: cargo xtask e2e-scan <server log>"),
        },
        "dev" => dev(),
        "services" => match std::env::args().nth(2).as_deref() {
            Some("up") => xtask::services::up()
                .and_then(|()| xtask::services::meili_up())
                .and_then(|()| xtask::services::s3_up().map(|_| ())),
            Some("down") => xtask::services::down(),
            _ => bail!("usage: cargo xtask services up|down"),
        },
        "sqlx-prepare" => sqlx_prepare(false),
        "load" => load(),
        "upgrade-test" => upgrade_test(std::env::args().nth(2).as_deref()),
        "upgrade-seed" => match std::env::args().nth(2) {
            Some(url) => upgrade_seed(&url),
            None => bail!("usage: cargo xtask upgrade-seed <database-url>"),
        },
        "record" => match std::env::args().nth(2) {
            Some(id) => xtask::record::record(&id),
            None => bail!("usage: cargo xtask record <connector>"),
        },
        _ => {
            eprintln!("{USAGE}");
            bail!("unknown command `{cmd}`")
        }
    }
}

/// Features enabled for workspace-wide native checks. `--all-features` is not usable because
/// `web`'s `ssr`, `hydrate` and `csr` are mutually exclusive.
const NATIVE_FEATURES: &str =
    "connectors/all,importers/all,integrations/all,notify/all,server/gallery,server/all-connectors";
/// Features for the browser (wasm32) build of the web app.
const WASM_FEATURES: &str = "web/hydrate,web/gallery";
const PEDANTIC: [&str; 5] = ["--", "-D", "warnings", "-W", "clippy::pedantic"];

fn ci() -> Result<()> {
    step("fmt", || cargo(&["fmt", "--all", "--check"], &[]))?;
    step("clippy", || {
        let mut args = vec![
            "clippy",
            "--workspace",
            "--all-targets",
            "--features",
            NATIVE_FEATURES,
        ];
        args.extend(PEDANTIC);
        cargo(&args, &[])
    })?;
    step("clippy-wasm", || {
        let mut args = vec![
            "clippy",
            "-p",
            "web",
            "-p",
            "ui",
            "--all-targets",
            "--target",
            "wasm32-unknown-unknown",
            "--features",
            WASM_FEATURES,
        ];
        args.extend(PEDANTIC);
        cargo(&args, &[])
    })?;
    step("deny", || cargo(&["deny", "--all-features", "check"], &[]))?;
    step("notices", || {
        let out = xtask::workspace_root().join("target/THIRD-PARTY-NOTICES.txt");
        run(Command::new("sh")
            .arg(xtask::repo_root().join("src/sh/notices.sh"))
            .arg(out))
    })?;
    step("sqlx", || sqlx_prepare(true))?;
    let test_db = if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
        url
    } else {
        step("services", xtask::services::up)?;
        xtask::services::drop_dbs("test\\_%")?;
        xtask::services::pg_admin_url()
    };
    // S3 tests run when an S3 endpoint is available, and are skipped with a message otherwise.
    let meili = if std::env::var_os("TEST_MEILI_URL").is_some() {
        None
    } else {
        step("meilisearch", xtask::services::meili_up)?;
        Some(xtask::services::meili_url())
    };
    let s3 = if std::env::var_os("TEST_S3_ENDPOINT").is_some() || !xtask::services::s3_up()? {
        None
    } else {
        Some(xtask::services::s3_url())
    };
    step("test", || {
        cargo(
            &["test", "--workspace", "--features", NATIVE_FEATURES],
            &[
                ("TEST_DATABASE_URL", test_db.as_str()),
                ("REQUIRE_SERVICES", "1"),
                ("TEST_S3_ENDPOINT", s3.as_deref().unwrap_or("")),
                ("TEST_MEILI_URL", meili.as_deref().unwrap_or("")),
                (
                    "TEST_MEILI_KEY",
                    if meili.is_some() {
                        xtask::services::MEILI_KEY
                    } else {
                        ""
                    },
                ),
            ]
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .collect::<Vec<_>>(),
        )
    })?;
    step("test-wasm", || {
        cargo(
            &["test", "-p", "ui", "--target", "wasm32-unknown-unknown"],
            &[],
        )
    })?;
    // covers and keyed rows of the web crate in a browser (OT1B-12)
    step("test-wasm-web", || {
        cargo(
            &[
                "test",
                "-p",
                "web",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "hydrate",
                "--test",
                "wasm",
            ],
            &[],
        )
    })?;
    println!("xtask ci: all green");
    Ok(())
}

/// The load test (WP4-1) against the local services; `LOAD_*` variables pass through.
fn load() -> Result<()> {
    let args = xtask::load::parse_args(std::env::args().skip(2))?;
    let root = xtask::workspace_root();
    let free = xtask::load::free_bytes(&root)?;
    // a release tree that exists already is rebuilt in place, not added
    let needed = if args.release && root.join("target/release").exists() {
        xtask::load::MIN_FREE_BYTES_REBUILD
    } else {
        xtask::load::MIN_FREE_BYTES
    };
    if free < needed {
        bail!(
            "only {} GB free disk; the load run needs {} GB (a release build is a second target tree)",
            free / 1_000_000_000,
            needed / 1_000_000_000
        );
    }
    let test_db = if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
        url
    } else {
        xtask::services::up()?;
        xtask::services::drop_dbs("test\\_%")?;
        xtask::services::pg_admin_url()
    };
    let clients = args.clients.map(|n| n.to_string());
    let mut env = vec![
        ("TEST_DATABASE_URL", test_db.as_str()),
        ("REQUIRE_SERVICES", "1"),
    ];
    if let Some(n) = clients.as_deref() {
        env.push(("LOAD_CLIENTS", n));
    }
    cargo(&xtask::load::cargo_args(args), &env)?;
    if args.release {
        println!("xtask load: delete target/release when done (it is a second target tree)");
    }
    Ok(())
}

/// `cargo xtask upgrade-seed <database-url>` (REL-3): `pg_dump` of the seed database into
/// the seed file of this build, see [`xtask::upgrade::seed_from_dump`].
fn upgrade_seed(url: &str) -> Result<()> {
    let out = std::process::Command::new("pg_dump")
        .args(xtask::upgrade::pg_dump_args(url))
        .output()
        .context("running pg_dump (in the dev shell)")?;
    if !out.status.success() {
        bail!(
            "pg_dump failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let seed = xtask::upgrade::seed_from_dump(
        &String::from_utf8_lossy(&out.stdout),
        xtask::upgrade::SEED_HEADER,
    );
    let path = xtask::repo_root().join(xtask::upgrade::SEED);
    std::fs::write(&path, &seed).with_context(|| path.display().to_string())?;
    println!(
        "upgrade-seed: {} lines in {}",
        seed.lines().count(),
        path.display()
    );
    Ok(())
}

/// `cargo xtask upgrade-test [<previous-tag>]` (v1-release item 6), see [`xtask::upgrade`].
fn upgrade_test(given: Option<&str>) -> Result<()> {
    let Some(tag) = xtask::upgrade::choose(given)? else {
        println!(
            "upgrade-test: no release tag (v<number>) below this build, so there is nothing to \
             upgrade from (before v1, or on the v1 tag itself); skipped"
        );
        return Ok(());
    };
    let dest = xtask::workspace_root()
        .join("target/upgrade-test")
        .join(&tag);
    let (migrations, seed) = xtask::upgrade::extract(&tag, &dest)?;
    println!("upgrade-test: from {tag} ({})", migrations.display());
    let test_db = if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
        url
    } else {
        xtask::services::up()?;
        xtask::services::pg_admin_url()
    };
    let (migrations, seed) = (
        migrations.to_string_lossy().into_owned(),
        seed.to_string_lossy().into_owned(),
    );
    cargo(
        &xtask::upgrade::cargo_args(),
        &[
            ("TEST_DATABASE_URL", test_db.as_str()),
            ("REQUIRE_SERVICES", "1"),
            ("UPGRADE_FROM_DIR", migrations.as_str()),
            ("UPGRADE_SEED", seed.as_str()),
        ],
    )
}

/// Build the site with the dev gallery and run the Playwright suite against it.
fn e2e(extra: &[String]) -> Result<()> {
    let web = xtask::repo_root().join("src/web");
    // a fresh database per run, unless the caller provides one (GitLab CI service)
    let db = if let Ok(url) = std::env::var("E2E_DATABASE_URL") {
        url
    } else {
        xtask::services::up()?;
        xtask::services::fresh_e2e_db()?
    };
    cargo(
        &[
            "leptos",
            "build",
            "--bin-features",
            "gallery,all-connectors",
            "--lib-features",
            "hydrate,gallery",
        ],
        &[],
    )?;
    if !web.join("node_modules").exists() {
        run(Command::new("npm")
            .args(["ci", "--no-audit", "--no-fund"])
            .current_dir(&web))?;
    }
    // the server's output goes to a file outside the repo; any panic in it fails the run
    // (UX3-25 panic guard, DEC-384)
    let log = std::env::temp_dir().join(format!(
        "e2e-server-{}.log",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    ));
    let tests = run(Command::new("npx")
        .args(["playwright", "test"])
        .args(extra)
        .env("E2E_DATABASE_URL", db)
        .env("E2E_SERVER_LOG", &log)
        .current_dir(web));
    let scan = scan_server_log(&log, tests.is_ok());
    tests?;
    scan
}

/// The UX3-25 panic guard on a server log: report its panics, keep the file (path printed)
/// when a panic was found or the tests failed, and fail when it holds a panic. Used by
/// `e2e` and, as `cargo xtask e2e-scan <log>`, by the GitLab e2e job (UX4-4).
fn scan_server_log(log: &std::path::Path, tests_passed: bool) -> Result<()> {
    let found = xtask::e2e_log::settle(log, tests_passed)
        .with_context(|| format!("checking the server log {}", log.display()))?;
    println!(
        "==> server log: {} lines, {} panic(s)",
        found.lines,
        found.panics.len()
    );
    for p in &found.panics {
        eprintln!("server panic during e2e:\n{p}\n");
    }
    if found.kept {
        println!("==> server log kept at {}", log.display());
    }
    if !found.panics.is_empty() {
        bail!(
            "the server panicked {} time(s) during the e2e run",
            found.panics.len()
        );
    }
    Ok(())
}

fn dev() -> Result<()> {
    xtask::services::up()?;
    let env = dev_env();
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    cargo(
        &[
            "leptos",
            "watch",
            "--bin-features",
            "gallery,all-connectors",
            "--lib-features",
            "hydrate,gallery",
        ],
        &env,
    )
}

/// Migrate the dev database and refresh (or, with `check`, verify) the `.sqlx/` cache.
fn sqlx_prepare(check: bool) -> Result<()> {
    let url = if let Ok(url) = std::env::var("DATABASE_URL") {
        url
    } else {
        xtask::services::up()?;
        xtask::services::pg_dev_url()
    };
    let root = xtask::workspace_root();
    run(Command::new("sqlx")
        .args([
            "migrate",
            "run",
            "--source",
            "crates/store-pg/migrations",
            "--database-url",
            &url,
        ])
        .current_dir(&root))?;
    let mut args = vec!["sqlx", "prepare", "--workspace"];
    if check {
        args.push("--check");
    }
    args.extend(["--", "--all-targets", "--features", NATIVE_FEATURES]);
    cargo(&args, &[("DATABASE_URL", &url)])
}

fn step(name: &str, f: impl FnOnce() -> Result<()>) -> Result<()> {
    println!("==> {name}");
    f().with_context(|| format!("step `{name}` failed"))
}

fn cargo(args: &[&str], env: &[(&str, &str)]) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    run(Command::new(cargo)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(xtask::workspace_root()))
}

fn run(cmd: &mut Command) -> Result<()> {
    let shown = format!("{cmd:?}");
    let status = cmd.status().with_context(|| format!("running {shown}"))?;
    if !status.success() {
        bail!("{shown} exited with {status}");
    }
    Ok(())
}

/// Settings that point the dev server at the services of `xtask services up` (the config
/// defaults assume a production layout: Postgres on 5432, Meilisearch on 7700, https
/// cookies, paths relative to the repo root). A variable the caller already set wins.
fn dev_env() -> Vec<(&'static str, String)> {
    let repo = xtask::repo_root();
    let dev = xtask::workspace_root().join(".dev");
    let wanted = [
        ("APP__DATABASE__URL", xtask::services::pg_dev_url()),
        ("APP__SEARCH__MEILI_URL", xtask::services::meili_url()),
        (
            "APP__SEARCH__MEILI_MASTER_KEY",
            xtask::services::MEILI_KEY.into(),
        ),
        ("APP__SERVER__PUBLIC_URL", "http://127.0.0.1:3000".into()),
        ("APP__AUTH__SECURE_COOKIES", "false".into()),
        (
            "APP__BRAND__DIR",
            repo.join("config/brand/default")
                .to_string_lossy()
                .into_owned(),
        ),
        (
            "APP__STORAGE__FS_ROOT",
            dev.join("blobs").to_string_lossy().into_owned(),
        ),
        // the secrets key of the dev database stays next to it, outside the repo (DEC-243)
        (
            "APP__SECRETS__KEY_FILE",
            dev.join("secrets.key").to_string_lossy().into_owned(),
        ),
    ];
    wanted
        .into_iter()
        .filter(|(key, _)| std::env::var_os(key).is_none())
        .collect()
}
