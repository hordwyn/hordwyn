//! Local development services started as plain processes (KICKOFF Q1): Postgres now,
//! Meilisearch from M08. Data lives in `src/rust/.dev/` (git-ignored).

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub const PG_PORT: u16 = 55432;

fn dev_dir() -> PathBuf {
    crate::workspace_root().join(".dev")
}

/// URL of the maintenance database; tests create their own databases from it.
#[must_use]
pub fn pg_admin_url() -> String {
    format!("postgres://app@127.0.0.1:{PG_PORT}/postgres")
}

/// URL of the development database (`cargo sqlx prepare`, `cargo xtask dev`).
#[must_use]
pub fn pg_dev_url() -> String {
    format!("postgres://app@127.0.0.1:{PG_PORT}/app_dev")
}

fn run(cmd: &mut Command) -> Result<()> {
    let shown = format!("{cmd:?}");
    let out = cmd.output().with_context(|| format!("running {shown}"))?;
    if !out.status.success() {
        bail!(
            "{shown} failed: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

fn pg_running(data: &std::path::Path) -> bool {
    Command::new("pg_ctl")
        .arg("-D")
        .arg(data)
        .arg("status")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Start Postgres (initialising the cluster on first use) and create `app_dev`.
///
/// # Errors
/// If the Postgres binaries are missing or fail.
pub fn up() -> Result<()> {
    let dir = dev_dir();
    let data = dir.join("pg");
    std::fs::create_dir_all(&dir)?;
    if !data.join("PG_VERSION").exists() {
        run(Command::new("initdb").arg("-D").arg(&data).args([
            "-U",
            "app",
            "--auth=trust",
            "--encoding=UTF8",
            "--no-locale",
        ]))?;
    }
    if !pg_running(&data) {
        let opts = format!(
            "-p {PG_PORT} -k {} -c listen_addresses=127.0.0.1 -c fsync=off -c max_connections=300",
            dir.display()
        );
        run(Command::new("pg_ctl")
            .arg("-D")
            .arg(&data)
            .args(["-o", &opts, "-l"])
            .arg(dir.join("pg.log"))
            .args(["-w", "start"]))?;
    }
    let exists = Command::new("psql")
        .args([
            &pg_admin_url(),
            "-tAc",
            "SELECT 1 FROM pg_database WHERE datname='app_dev'",
        ])
        .output()
        .context("psql")?;
    if String::from_utf8_lossy(&exists.stdout).trim() != "1" {
        run(Command::new("psql").args([&pg_admin_url(), "-c", "CREATE DATABASE app_dev"]))?;
    }
    println!("postgres: {}", pg_admin_url());
    Ok(())
}

/// Stop the local services.
///
/// # Errors
/// If `pg_ctl` fails.
pub fn down() -> Result<()> {
    let data = dev_dir().join("pg");
    if pg_running(&data) {
        run(Command::new("pg_ctl")
            .arg("-D")
            .arg(&data)
            .args(["-m", "fast", "stop"]))?;
    }
    Ok(())
}
