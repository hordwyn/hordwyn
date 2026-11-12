//! Build-free checks of the deployment files (package v1-ops): the image contains the admin
//! CLI and keeps the secrets key out of the blob directory (OPS-1, OPS-3), the compose files
//! ship no default password and publish on loopback (SEC-6, OPS-7), and
//! `config/.env.example` lists every key of a first run (OPS-2).
//!
//! The build host has no container runtime, so these read the files as text. They prove what
//! the files say, not that an image builds or starts; the image build itself runs
//! `cli help` (a `RUN` step of the runtime stage), which the `docker-image` CI job executes.

use std::collections::BTreeSet;

/// The keys a first run behind a reverse proxy needs besides the compose passwords.
pub const FIRST_RUN_KEYS: [&str; 3] = [
    "APP__SERVER__PUBLIC_URL",
    "APP__SERVER__TRUSTED_PROXIES",
    "APP__AUTH__SECURE_COOKIES",
];

/// One Dockerfile instruction with the stage it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    /// Name of the stage (`FROM ... AS <name>`), or its index when unnamed.
    pub stage: String,
    /// `RUN`, `COPY`, `ENV`, ... in upper case.
    pub word: String,
    /// The rest, continuation lines joined with one space.
    pub args: String,
}

/// Split a Dockerfile into instructions (comments dropped, `\` continuations joined).
#[must_use]
pub fn dockerfile_instructions(text: &str) -> Vec<Instruction> {
    let mut out = Vec::new();
    let mut stage = String::new();
    let mut stages = 0_usize;
    let mut pending = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') || (line.is_empty() && pending.is_empty()) {
            continue;
        }
        if let Some(head) = line.strip_suffix('\\') {
            pending.push_str(head.trim());
            pending.push(' ');
            continue;
        }
        pending.push_str(line);
        let full = std::mem::take(&mut pending);
        let (word, args) = full.split_once(char::is_whitespace).unwrap_or((&full, ""));
        let word = word.to_ascii_uppercase();
        let args = args.split_whitespace().collect::<Vec<_>>().join(" ");
        if word == "FROM" {
            let words: Vec<&str> = args.split(' ').collect();
            stage = match words.as_slice() {
                [_, kw, name] if kw.eq_ignore_ascii_case("as") => (*name).to_string(),
                _ => stages.to_string(),
            };
            stages += 1;
        }
        out.push(Instruction {
            stage: stage.clone(),
            word,
            args,
        });
    }
    out
}

/// `KEY=value` pairs of the `ENV` instructions of one stage.
fn env_of(ins: &[Instruction], stage: &str) -> Vec<(String, String)> {
    ins.iter()
        .filter(|i| i.stage == stage && i.word == "ENV")
        .flat_map(|i| i.args.split(' '))
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_string(), v.trim_matches('"').to_string()))
        .collect()
}

/// Is `path` the directory `dir` or below it?
fn inside(path: &str, dir: &str) -> bool {
    let dir = dir.trim_end_matches('/');
    path == dir || path.strip_prefix(dir).is_some_and(|r| r.starts_with('/'))
}

/// What is wrong with the image definition; empty when it holds.
#[must_use]
pub fn dockerfile_problems(text: &str) -> Vec<String> {
    let ins = dockerfile_instructions(text);
    let mut out = Vec::new();
    let Some(last) = ins.last().map(|i| i.stage.clone()) else {
        return vec!["the Dockerfile has no instruction".into()];
    };
    let builds_cli = ins.iter().any(|i| {
        i.word == "RUN"
            && i.stage != last
            && i.args.contains("cargo build")
            && i.args.contains("--release")
            && i.args.contains("-p cli")
    });
    if !builds_cli {
        out.push("no build stage runs `cargo build --release ... -p cli` (OPS-1)".into());
    }
    let copies = |bin: &str| {
        ins.iter().any(|i| {
            i.word == "COPY"
                && i.stage == last
                && i.args.contains("--from=")
                && i.args.split(' ').any(|a| a.ends_with(&format!("/{bin}")))
                && i.args
                    .split(' ')
                    .next_back()
                    .is_some_and(|d| d == "/usr/local/bin/" || d == format!("/usr/local/bin/{bin}"))
        })
    };
    for bin in ["server", "cli"] {
        if !copies(bin) {
            out.push(format!(
                "the runtime stage does not copy `{bin}` to /usr/local/bin (OPS-1)"
            ));
        }
    }
    let checked = ins.iter().any(|i| {
        i.word == "RUN"
            && i.stage == last
            && i.args.contains("/usr/local/bin/cli")
            && i.args.contains("help")
    });
    if !checked {
        out.push(
            "the runtime stage does not run `/usr/local/bin/cli help` (the image build is the \
             check that the binary is there and starts)"
                .into(),
        );
    }
    let env = env_of(&ins, &last);
    let get = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    match (get("APP__STORAGE__FS_ROOT"), get("APP__SECRETS__KEY_FILE")) {
        (Some(root), Some(key)) => {
            if inside(key, root) {
                out.push(format!(
                    "the secrets key file {key} is inside the blob directory {root} (OPS-3)"
                ));
            }
        }
        _ => out.push(
            "the runtime stage sets no APP__STORAGE__FS_ROOT or no APP__SECRETS__KEY_FILE".into(),
        ),
    }
    out
}

/// One service of a compose file: its name and its lines (without the name line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub lines: Vec<String>,
}

/// The services of a compose file. A reader for the files of this repository (two-space
/// indentation, block style), not a YAML parser.
#[must_use]
pub fn compose_services(text: &str) -> Vec<Service> {
    let mut out: Vec<Service> = Vec::new();
    let mut in_services = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            in_services = line == "services:";
            continue;
        }
        if !in_services {
            continue;
        }
        if indent == 2 && line.ends_with(':') {
            out.push(Service {
                name: line.trim().trim_end_matches(':').to_string(),
                lines: Vec::new(),
            });
        } else if let Some(s) = out.last_mut() {
            s.lines.push(line.to_string());
        }
    }
    out
}

/// `${NAME...}` references of a text: the name and what follows it up to the closing brace.
fn variables(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("${") {
        let after = &rest[at + 2..];
        let Some(end) = after.find('}') else {
            break;
        };
        let inner = &after[..end];
        let split = inner
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(inner.len());
        out.push((inner[..split].to_string(), inner[split..].to_string()));
        rest = &after[end + 1..];
    }
    out
}

/// Is this a variable that holds a credential?
fn is_secret(name: &str) -> bool {
    ["PASSWORD", "SECRET", "KEY"]
        .iter()
        .any(|w| name.contains(w))
}

/// The entries of a service's `ports:` list.
fn ports(s: &Service) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for l in &s.lines {
        let t = l.trim();
        if t.starts_with("ports:") {
            inside = true;
        } else if inside && t.starts_with("- ") {
            out.push(t[2..].trim().trim_matches('"').to_string());
        } else {
            inside = false;
        }
    }
    out
}

/// Does a published port bind to loopback unless the operator says otherwise?
fn loopback(port: &str) -> bool {
    if port.starts_with("127.0.0.1:") {
        return true;
    }
    variables(port)
        .first()
        .is_some_and(|(_, rest)| rest == ":-127.0.0.1" && port.starts_with("${"))
}

/// What is wrong with a compose file; empty when it holds. The healthcheck and the stop grace
/// period are asked of services the file defines (`image:` or `build:`), not of the
/// fragments of an override file.
#[must_use]
pub fn compose_problems(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (name, rest) in variables(text) {
        if is_secret(&name) && !rest.starts_with(":?") {
            out.push(format!(
                "${{{name}{rest}}}: a credential needs `:?` (required, no default; SEC-6)"
            ));
        }
    }
    let services = compose_services(text);
    for s in &services {
        for p in ports(s) {
            if !loopback(&p) {
                out.push(format!(
                    "service {}: port `{p}` is not published on 127.0.0.1 by default (SEC-6)",
                    s.name
                ));
            }
        }
        let has = |key: &str| s.lines.iter().any(|l| l.trim().starts_with(key));
        let one_shot = s.lines.iter().any(|l| l.trim() == "restart: \"no\"");
        let defined = has("image:") || has("build:");
        if defined && !one_shot && !has("healthcheck:") {
            out.push(format!("service {} has no healthcheck", s.name));
        }
        if defined && s.name == "app" && !has("stop_grace_period: 30s") {
            out.push(
                "service app: `stop_grace_period: 30s` is missing (the worker waits \
                 jobs.shutdown_grace_secs = 20 s for running jobs; OPS-7)"
                    .into(),
            );
        }
    }
    out
}

/// Every variable a compose file reads from the environment.
#[must_use]
pub fn compose_variables(text: &str) -> BTreeSet<String> {
    let uncommented: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    variables(&uncommented)
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

/// `NAME=value` lines of an env file (comments and blank lines skipped).
#[must_use]
pub fn env_entries(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

/// `APP__...` names written in a text (docs pages).
#[must_use]
pub fn app_keys(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(at) = rest.find("APP__") {
        let tail = &rest[at..];
        let end = tail
            .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
            .unwrap_or(tail.len());
        let name = tail[..end].trim_end_matches('_');
        if name.len() > "APP__".len() {
            out.insert(name.to_string());
        }
        rest = &tail[end.max(1)..];
    }
    out
}

/// What `config/.env.example` lacks for a first run; empty when it holds. `compose` are the
/// compose files, `pages` the operator pages that walk through a first run (every `APP__*`
/// key they name must be listed), `reference` the generated configuration reference (the
/// first-run keys must be real settings).
#[must_use]
pub fn env_example_problems(
    example: &str,
    compose: &[&str],
    pages: &[&str],
    reference: &str,
) -> Vec<String> {
    let entries = env_entries(example);
    let listed: BTreeSet<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    let mut out = Vec::new();
    for (k, v) in &entries {
        if !v.is_empty() {
            out.push(format!("{k} has a value (KICKOFF rule 8: empty values)"));
        }
    }
    let mut wanted: BTreeSet<String> = FIRST_RUN_KEYS.iter().map(|k| (*k).to_string()).collect();
    for c in compose {
        wanted.extend(compose_variables(c));
    }
    for p in pages {
        wanted.extend(app_keys(p));
    }
    for k in &wanted {
        if !listed.contains(k.as_str()) {
            out.push(format!("{k} is not listed in config/.env.example (OPS-2)"));
        }
    }
    for k in FIRST_RUN_KEYS {
        if !reference.contains(&format!("`{k}`")) {
            out.push(format!(
                "{k} is not a setting of the configuration reference"
            ));
        }
    }
    out
}

/// The text of a page section: from the line `== <title>` up to the next `== ` heading
/// (deeper headings belong to the section). `None` when the page has no such section.
#[must_use]
pub fn section<'a>(page: &'a str, title: &str) -> Option<&'a str> {
    let head = format!("== {title}");
    let mut at = 0;
    let mut start = None;
    for line in page.split_inclusive('\n') {
        match start {
            None if line.trim_end() == head => start = Some(at),
            Some(s) if line.starts_with("== ") => return Some(&page[s..at]),
            _ => {}
        }
        at += line.len();
    }
    start.map(|s| &page[s..])
}

/// The lines of a commented-out service of a compose file (`  # name:` and the `  #   ...`
/// lines below it), uncommented; empty when the file has none.
#[must_use]
pub fn commented_service(text: &str, name: &str) -> Vec<String> {
    let head = format!("  # {name}:");
    text.lines()
        .skip_while(|l| l.trim_end() != head)
        .skip(1)
        .take_while(|l| l.starts_with("  #   "))
        .map(|l| l["  # ".len()..].to_string())
        .collect()
}
