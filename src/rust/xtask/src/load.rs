//! `cargo xtask load [--release] [--clients N]` (WP4-1): runs the ignored load test
//! (`apps/server/tests/load.rs`) against the local services. Not part of `ci`. A release run
//! builds a second target tree, so the command refuses to start below [`MIN_FREE_BYTES`].

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// 15 GB of free disk before a load run (a release tree of the workspace is several GB).
pub const MIN_FREE_BYTES: u64 = 15 * 1_000_000_000;
/// With `target/release` already there (a second release run), the HANDOFF section 8 floor.
pub const MIN_FREE_BYTES_REBUILD: u64 = 5 * 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadArgs {
    pub release: bool,
    /// `LOAD_CLIENTS`; `None` keeps the environment's (or the test's default of 1).
    pub clients: Option<u32>,
}

/// Parse the arguments after `load`.
///
/// # Errors
/// An unknown argument, or `--clients` without a whole number of at least 1.
pub fn parse_args<I, S>(args: I) -> Result<LoadArgs>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = LoadArgs {
        release: false,
        clients: None,
    };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_ref() {
            "--release" => out.release = true,
            "--clients" => {
                let n: u32 = it
                    .next()
                    .context("--clients needs a number")?
                    .as_ref()
                    .parse()
                    .context("--clients needs a whole number")?;
                if n == 0 {
                    bail!("--clients needs at least 1");
                }
                out.clients = Some(n);
            }
            other => bail!("unknown argument `{other}` (usage: load [--release] [--clients N])"),
        }
    }
    Ok(out)
}

/// Available bytes from the output of `df -Pk <path>` (POSIX format, 1024-byte blocks).
///
/// # Errors
/// If the output does not have the POSIX layout.
pub fn available_bytes(df_output: &str) -> Result<u64> {
    let line = df_output
        .lines()
        .nth(1)
        .context("df printed no filesystem line")?;
    let kib: u64 = line
        .split_whitespace()
        .nth(3)
        .context("df line has no Available column")?
        .parse()
        .context("df Available is not a number")?;
    Ok(kib * 1024)
}

/// Free bytes on the filesystem holding `path`.
///
/// # Errors
/// If `df` cannot run or prints something unexpected.
pub fn free_bytes(path: &Path) -> Result<u64> {
    let out = Command::new("df")
        .arg("-Pk")
        .arg(path)
        .output()
        .context("running df")?;
    if !out.status.success() {
        bail!("df -Pk {} exited with {}", path.display(), out.status);
    }
    available_bytes(&String::from_utf8_lossy(&out.stdout))
}

/// The cargo arguments of the load run.
#[must_use]
pub fn cargo_args(a: LoadArgs) -> Vec<&'static str> {
    let mut args = vec!["test"];
    if a.release {
        args.push("--release");
    }
    args.extend([
        "-p",
        "server",
        "--test",
        "load",
        "--",
        "--ignored",
        "--nocapture",
    ]);
    args
}
