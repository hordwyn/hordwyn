//! `cargo xtask <command>`, see [`USAGE`].
#![deny(unsafe_code)]

use std::process::Command;

use anyhow::{Context, Result, bail};

const USAGE: &str = "usage: cargo xtask <command>

  ci                 everything CI runs that does not need docker: fmt check, clippy,
                     cargo-deny, sqlx offline-cache check, tests (starts local services
                     when TEST_DATABASE_URL is unset)
  fmt                format the workspace
  e2e                run the Playwright end-to-end suite
  dev                start the development server
  services up|down   start or stop local Postgres and versitygw (S3)
  sqlx-prepare       migrate the dev database and refresh the sqlx offline cache
  record <connector> re-record connector fixtures from the live API (needs its key)";

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "ci" => ci(),
        "fmt" => cargo(&["fmt", "--all"], &[]),
        "e2e" => e2e(),
        "dev" => dev(),
        "services" => match std::env::args().nth(2).as_deref() {
            Some("up") => xtask::services::up()
                .and_then(|()| xtask::services::meili_up())
                .and_then(|()| xtask::services::s3_up().map(|_| ())),
            Some("down") => xtask::services::down(),
            _ => bail!("usage: cargo xtask services up|down"),
        },
        "sqlx-prepare" => sqlx_prepare(false),
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
const NATIVE_FEATURES: &str = "connectors/all,importers/all,server/gallery,server/all-connectors";
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
    println!("xtask ci: all green");
    Ok(())
}

/// Build the site with the dev gallery and run the Playwright suite against it.
fn e2e() -> Result<()> {
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
    run(Command::new("npx")
        .args(["playwright", "test"])
        .env("E2E_DATABASE_URL", db)
        .current_dir(web))
}

fn dev() -> Result<()> {
    xtask::services::up()?;
    cargo(
        &[
            "leptos",
            "watch",
            "--bin-features",
            "gallery,all-connectors",
            "--lib-features",
            "hydrate,gallery",
        ],
        &[],
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
