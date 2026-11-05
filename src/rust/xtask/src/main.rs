//! `cargo xtask <command>`: ci | fmt | e2e | dev.
#![deny(unsafe_code)]

use std::process::Command;

use anyhow::{Context, Result, bail};

const USAGE: &str = "usage: cargo xtask <ci|fmt|e2e|dev>

  ci    everything CI runs that does not need docker: fmt check, clippy, cargo-deny, tests
  fmt   format the workspace
  e2e   run the Playwright end-to-end suite
  dev   start the development server";

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "ci" => ci(),
        "fmt" => cargo(&["fmt", "--all"]),
        "e2e" => e2e(),
        "dev" => dev(),
        _ => {
            eprintln!("{USAGE}");
            bail!("unknown command `{cmd}`")
        }
    }
}

fn ci() -> Result<()> {
    step("fmt", || cargo(&["fmt", "--all", "--check"]))?;
    step("clippy", || {
        cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
            "-W",
            "clippy::pedantic",
        ])
    })?;
    step("deny", || cargo(&["deny", "--all-features", "check"]))?;
    step("test", || cargo(&["test", "--workspace", "--all-features"]))?;
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
    cargo(&["leptos", "watch"])
}

fn step(name: &str, f: impl FnOnce() -> Result<()>) -> Result<()> {
    println!("==> {name}");
    f().with_context(|| format!("step `{name}` failed"))
}

fn cargo(args: &[&str]) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    run(Command::new(cargo)
        .args(args)
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
