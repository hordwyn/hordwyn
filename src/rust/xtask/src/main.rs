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
  services up|down   start or stop local Postgres (and later Meilisearch)
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
            Some("up") => xtask::services::up(),
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

fn ci() -> Result<()> {
    step("fmt", || cargo(&["fmt", "--all", "--check"], &[]))?;
    step("clippy", || {
        cargo(
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
                "-W",
                "clippy::pedantic",
            ],
            &[],
        )
    })?;
    step("deny", || cargo(&["deny", "--all-features", "check"], &[]))?;
    step("sqlx", || sqlx_prepare(true))?;
    let test_db = if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
        url
    } else {
        step("services", xtask::services::up)?;
        xtask::services::pg_admin_url()
    };
    step("test", || {
        cargo(
            &["test", "--workspace", "--all-features"],
            &[("TEST_DATABASE_URL", &test_db), ("REQUIRE_SERVICES", "1")],
        )
    })?;
    println!("xtask ci: all green");
    Ok(())
}

fn e2e() -> Result<()> {
    let web = xtask::repo_root().join("src/web");
    if !web.join("playwright.config.ts").exists() {
        println!("xtask e2e: no Playwright suite yet");
        return Ok(());
    }
    run(Command::new("npx")
        .args(["playwright", "test"])
        .current_dir(web))
}

fn dev() -> Result<()> {
    let manifest = std::fs::read_to_string(xtask::workspace_root().join("Cargo.toml"))?;
    if !manifest.contains("[[workspace.metadata.leptos]]") {
        println!("xtask dev: no Leptos app configured yet");
        return Ok(());
    }
    xtask::services::up()?;
    cargo(&["leptos", "watch"], &[])
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
    args.extend(["--", "--all-targets", "--all-features"]);
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
