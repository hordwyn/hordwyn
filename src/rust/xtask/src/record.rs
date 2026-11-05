//! `cargo xtask record <connector>`: re-record a connector's HTTP fixtures from the live API
//! when its key is in the environment (KICKOFF section 5). Uses `curl` so xtask needs no
//! HTTP client.

use std::process::Command;

use anyhow::{Context, Result, bail};

/// `(connector, env var, header template)`; `{}` is replaced by the key.
const AUTH: &[(&str, &str, &str)] = &[("tmdb", "KK_TMDB_API_KEY", "Authorization: Bearer {}")];

/// Re-record every fixture of `connector`.
///
/// # Errors
/// Unknown connector, missing key, or a failed request.
pub fn record(connector: &str) -> Result<()> {
    let Some((_, env, template)) = AUTH.iter().find(|(c, _, _)| *c == connector) else {
        bail!("no recording support for `{connector}`");
    };
    let key = std::env::var(env).with_context(|| format!("{env} is not set"))?;
    let dir = crate::workspace_root()
        .join("crates/connectors/fixtures")
        .join(connector);
    let mut paths: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    let date = Command::new("date").arg("+%F").output()?;
    let today = String::from_utf8_lossy(&date.stdout).trim().to_string();
    for path in paths {
        let mut f: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let url = f["request"]["url"]
            .as_str()
            .context("request.url")?
            .to_string();
        let out = Command::new("curl")
            .args([
                "-sS",
                "-w",
                "\n%{http_code}",
                "-H",
                "Accept: application/json",
                "-H",
            ])
            .arg(template.replace("{}", &key))
            .arg(&url)
            .output()?;
        let text = String::from_utf8(out.stdout)?;
        let (body, status) = text.rsplit_once('\n').context("curl output")?;
        f["status"] = status.trim().parse::<u16>()?.into();
        f["body"] = serde_json::from_str(body)?;
        f["fixture"]["synthetic"] = false.into();
        f["fixture"]["source"] = format!("recorded from {url}").into();
        f["fixture"]["checked"] = today.clone().into();
        std::fs::write(&path, serde_json::to_string_pretty(&f)? + "\n")?;
        println!("recorded {}", path.display());
    }
    Ok(())
}
