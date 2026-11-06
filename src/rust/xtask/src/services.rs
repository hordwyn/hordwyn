//! Local development services started as plain processes (KICKOFF Q1): Postgres,
//! Meilisearch and `RustFS` (S3). Data lives in `src/rust/.dev/` (git-ignored).

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
    s3_down()?;
    meili_down()
}

// ---------------------------------------------------------------- S3 (RustFS)

pub const S3_PORT: u16 = 55433;
pub const S3_ACCESS_KEY: &str = "test";
pub const S3_SECRET_KEY: &str = "testsecret";
pub const S3_BUCKET: &str = "assets";
pub const S3_REGION: &str = "us-east-1";

/// Endpoint of the local S3-compatible server.
#[must_use]
pub fn s3_url() -> String {
    format!("http://127.0.0.1:{S3_PORT}")
}

fn port_open(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

fn on_path(bin: &str) -> bool {
    Command::new(bin).arg("--version").output().is_ok()
}

/// HTTP status code of a curl call (`000` when the connection failed).
fn curl_status(args: &[&str]) -> Result<String> {
    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}"])
        .args(args)
        .output()
        .context("curl")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Create the bucket with a SigV4-signed `PUT /<bucket>` and return whether it exists now.
/// `RustFS` 1.0.4 answers 200 also when the bucket already exists; 409
/// (`BucketAlreadyOwnedByYou`) is accepted as well. Right after `/health` turns 200 a first
/// start can still answer 503 for a moment; that means "not yet", not an error.
fn s3_create_bucket() -> Result<bool> {
    let sigv4 = format!("aws:amz:{S3_REGION}:s3");
    let user = format!("{S3_ACCESS_KEY}:{S3_SECRET_KEY}");
    let url = format!("{}/{S3_BUCKET}", s3_url());
    match curl_status(&["--aws-sigv4", &sigv4, "--user", &user, "-X", "PUT", &url])?.as_str() {
        "200" | "409" => Ok(true),
        "000" | "503" => Ok(false),
        code => bail!("s3: creating bucket {S3_BUCKET} answered HTTP {code}"),
    }
}

/// Wait until `/health` answers 200 and the bucket exists. The first start formats the
/// volume, so allow more time than for a plain port bind.
fn s3_wait_ready() -> Result<()> {
    let health = format!("{}/health", s3_url());
    for _ in 0..300 {
        if curl_status(&[&health])? == "200" && s3_create_bucket()? {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    bail!(
        "rustfs did not become ready, see {}",
        dev_dir().join("s3").join("s3.log").display()
    )
}

/// Start `RustFS` (Apache-2.0, single node single disk) if it is installed, and create the
/// bucket. Returns whether an S3 endpoint is available; S3 tests are skipped otherwise.
///
/// # Errors
/// If `RustFS` is installed but does not come up or the bucket cannot be created.
pub fn s3_up() -> Result<bool> {
    if port_open(S3_PORT) {
        return s3_wait_ready().map(|()| true);
    }
    if !on_path("rustfs") || !on_path("curl") {
        println!("s3: rustfs or curl not installed, S3 tests will be skipped");
        return Ok(false);
    }
    let dir = dev_dir().join("s3");
    // A versitygw posix tree (the S3 server before DEC-008 was reconsidered) is not a
    // RustFS volume: wipe it once.
    if dir.join("iam").exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let data = dir.join("data");
    std::fs::create_dir_all(&data)?;
    let log = std::fs::File::create(dir.join("s3.log"))?;
    let child = Command::new("rustfs")
        .env("RUSTFS_VOLUMES", &data)
        .env("RUSTFS_ADDRESS", format!("127.0.0.1:{S3_PORT}"))
        .env("RUSTFS_ACCESS_KEY", S3_ACCESS_KEY)
        .env("RUSTFS_SECRET_KEY", S3_SECRET_KEY)
        .env("RUSTFS_CONSOLE_ENABLE", "false")
        .env("RUSTFS_REGION", S3_REGION)
        .env("RUSTFS_SCANNER_SPEED", "slowest")
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("start rustfs")?;
    std::fs::write(dir.join("pid"), child.id().to_string())?;
    s3_wait_ready()?;
    println!("s3: {}", s3_url());
    Ok(true)
}

fn s3_down() -> Result<()> {
    let pid_file = dev_dir().join("s3").join("pid");
    if let Ok(pid) = std::fs::read_to_string(&pid_file) {
        let _ = Command::new("kill").arg(pid.trim()).status();
        std::fs::remove_file(pid_file)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- Meilisearch

pub const MEILI_PORT: u16 = 55434;
/// Local development/test master key only (never used in deployments).
pub const MEILI_KEY: &str = "dev-only-master-key-0123456789";

#[must_use]
pub fn meili_url() -> String {
    format!("http://127.0.0.1:{MEILI_PORT}")
}

/// Start Meilisearch (from the dev shell) as a local process.
///
/// # Errors
/// If the binary is missing or the server does not come up.
pub fn meili_up() -> Result<()> {
    if port_open(MEILI_PORT) {
        return Ok(());
    }
    if !on_path("meilisearch") {
        bail!("meilisearch not found; run inside `nix develop`");
    }
    let dir = dev_dir().join("meili");
    std::fs::create_dir_all(&dir)?;
    let log = std::fs::File::create(dir.join("meili.log"))?;
    let child = Command::new("meilisearch")
        .arg("--db-path")
        .arg(dir.join("data.ms"))
        .args([
            "--http-addr",
            &format!("127.0.0.1:{MEILI_PORT}"),
            "--master-key",
            MEILI_KEY,
            "--env",
            "development",
            "--no-analytics",
            "--max-indexing-memory",
            "256MiB",
        ])
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("start meilisearch")?;
    std::fs::write(dir.join("pid"), child.id().to_string())?;
    for _ in 0..150 {
        if port_open(MEILI_PORT) {
            println!("meilisearch: {}", meili_url());
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    bail!(
        "meilisearch did not start, see {}",
        dir.join("meili.log").display()
    )
}

fn meili_down() -> Result<()> {
    let pid_file = dev_dir().join("meili").join("pid");
    if let Ok(pid) = std::fs::read_to_string(&pid_file) {
        let _ = Command::new("kill").arg(pid.trim()).status();
        std::fs::remove_file(pid_file)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- databases

/// Create a new, empty database for an e2e run (the server migrates it) and return its URL.
///
/// # Errors
/// If `psql` fails.
pub fn fresh_e2e_db() -> Result<String> {
    let name = format!(
        "e2e_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    );
    drop_dbs("e2e\\_%")?;
    run(Command::new("psql").args([&pg_admin_url(), "-c", &format!("CREATE DATABASE {name}")]))?;
    Ok(format!("postgres://app@127.0.0.1:{PG_PORT}/{name}"))
}

/// Drop leftover databases whose name matches the `LIKE` pattern (`test\_%` from test runs,
/// `e2e\_%` from end-to-end runs), so local runs do not fill the disk.
///
/// # Errors
/// If `psql` fails.
pub fn drop_dbs(like: &str) -> Result<()> {
    let out = Command::new("psql")
        .args([
            &pg_admin_url(),
            "-tAc",
            &format!("SELECT datname FROM pg_database WHERE datname LIKE '{like}'"),
        ])
        .output()
        .context("psql")?;
    for name in String::from_utf8_lossy(&out.stdout).lines() {
        let name = name.trim();
        if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !name.is_empty() {
            run(Command::new("psql").args([
                &pg_admin_url(),
                "-q",
                "-c",
                &format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"),
            ]))?;
        }
    }
    Ok(())
}
