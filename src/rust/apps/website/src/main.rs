//! `website build --css <file> --out <dir> [--strict]` writes the static site;
//! `website check [--strict] [--report <file>]` lists what still blocks publishing.
//! Common options: `[--content <dir>] [--brand <dir>] [--assets <dir>] [--docs-playbook <file>]`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail};
use website::content::{Site, open_slots};
use website::gate::publish_blockers;

const USAGE: &str = "usage:
  website build --css <file> --out <dir> [--strict] [options]
  website check [--strict] [--report <file>] [options]

build   render every page into <dir>
check   list what blocks publishing: open owner slots (lines marked PLACEHOLDER), an invalid
        security.txt, a missing source address, claims marked shipped that release.toml does
        not confirm, documentation that is not ready
--strict            fail while anything blocks publishing
--report <file>     also write the blockers to <file>, one per line (empty file: none)
options: --content <dir>  --brand <dir>  --assets <dir>  --docs-playbook <file>";

#[derive(Default)]
struct Args {
    css: Option<PathBuf>,
    out: Option<PathBuf>,
    content: Option<PathBuf>,
    brand: Option<PathBuf>,
    assets: Option<PathBuf>,
    playbook: Option<PathBuf>,
    report: Option<PathBuf>,
    strict: bool,
}

fn parse(mut rest: impl Iterator<Item = String>) -> Result<Args> {
    let mut args = Args::default();
    while let Some(flag) = rest.next() {
        if flag == "--strict" {
            args.strict = true;
            continue;
        }
        let value = rest
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("{flag} needs a value"))?;
        match flag.as_str() {
            "--css" => args.css = Some(value),
            "--out" => args.out = Some(value),
            "--content" => args.content = Some(value),
            "--brand" => args.brand = Some(value),
            "--assets" => args.assets = Some(value),
            "--docs-playbook" => args.playbook = Some(value),
            "--report" => args.report = Some(value),
            other => bail!("unknown option {other}\n{USAGE}"),
        }
    }
    Ok(args)
}

/// The blockers of the content directory (the gate itself is `website::gate`), printed to
/// stderr.
fn blockers(args: &Args, content: &Path) -> Result<Vec<String>> {
    let site = Site::load(content)?;
    let slots = open_slots(content)?;
    let playbook = args
        .playbook
        .clone()
        .unwrap_or_else(|| website::crate_dir().join("../../../../docs/antora-playbook.yml"));
    let playbook = std::fs::read_to_string(playbook).ok();
    let found = publish_blockers(&site, &slots, playbook.as_deref(), SystemTime::now().into());
    for line in &found {
        eprintln!("blocks publishing: {line}");
    }
    eprintln!("{} thing(s) block publishing", found.len());
    Ok(found)
}

fn run() -> Result<ExitCode> {
    let mut words = std::env::args().skip(1);
    let command = words.next().unwrap_or_default();
    let args = parse(words)?;
    let content = args
        .content
        .clone()
        .unwrap_or_else(|| website::crate_dir().join("content"));
    if !matches!(command.as_str(), "check" | "build") {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    }
    let found = blockers(&args, &content)?;
    if let Some(report) = &args.report {
        let mut text = found.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        std::fs::write(report, text).with_context(|| format!("writing {}", report.display()))?;
    }
    if args.strict && !found.is_empty() {
        bail!(
            "{} thing(s) block publishing: settle them first (or run without --strict)",
            found.len()
        );
    }
    if command == "build" {
        let css = args
            .css
            .ok_or_else(|| anyhow!("--css is required\n{USAGE}"))?;
        let out = args
            .out
            .ok_or_else(|| anyhow!("--out is required\n{USAGE}"))?;
        let mut options = website::default_options(css, out);
        options.content = content;
        if let Some(brand) = args.brand {
            options.brand = brand;
        }
        if let Some(assets) = args.assets {
            options.assets = assets;
        }
        let files = website::generate::build(&options)?;
        eprintln!("{files} files written to {}", options.out.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
