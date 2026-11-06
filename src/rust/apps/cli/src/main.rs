//! Admin CLI: migrate, storage report, reindex, user management, imports.
//!
//! `cli [--config <file>] <command>`; the configuration is the server's (file + `APP__*`).
#![deny(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

const USAGE: &str = "usage: cli [--config <file>] <command>

  migrate                          apply database migrations
  storage report [--dry-run] [--json]
                                   storage usage, limits, S3 counters and what eviction
                                   would remove now (--dry-run: nothing is changed)
  storage evict                    run eviction now (normally a background job)";

#[derive(Debug, PartialEq, Eq)]
enum Cmd {
    Migrate,
    StorageReport { json: bool },
    StorageEvict,
    Help,
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    config: Option<PathBuf>,
    cmd: Cmd,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut config = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => config = Some(PathBuf::from(it.next().context("--config needs a path")?)),
            _ => rest.push(a.as_str()),
        }
    }
    let flags: Vec<&str> = rest
        .iter()
        .copied()
        .filter(|a| a.starts_with("--"))
        .collect();
    let words: Vec<&str> = rest
        .iter()
        .copied()
        .filter(|a| !a.starts_with("--"))
        .collect();
    let known = |allowed: &[&str]| -> Result<()> {
        for f in &flags {
            if !allowed.contains(f) {
                bail!("unknown option `{f}`\n\n{USAGE}");
            }
        }
        Ok(())
    };
    let cmd = match words.as_slice() {
        ["migrate"] => {
            known(&[])?;
            Cmd::Migrate
        }
        ["storage", "report"] => {
            known(&["--dry-run", "--json"])?;
            Cmd::StorageReport {
                json: flags.contains(&"--json"),
            }
        }
        ["storage", "evict"] => {
            known(&[])?;
            Cmd::StorageEvict
        }
        [] | ["help"] => Cmd::Help,
        _ => bail!("unknown command `{}`\n\n{USAGE}", words.join(" ")),
    };
    Ok(Args { config, cmd })
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    // display only
    #[allow(clippy::cast_precision_loss)]
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", UNITS[u])
}

fn print_report(r: &assets::StorageReport, pg_bytes: i64) {
    let pct = |n: u64| {
        // display only
        #[allow(clippy::cast_precision_loss)]
        let p = n as f64 * 100.0 / r.max_total_bytes.max(1) as f64;
        format!("{p:.1} %")
    };
    println!("storage backend      {:?}", r.backend);
    println!(
        "used                 {} ({} of cap, {} objects)",
        human(r.usage.total),
        pct(r.usage.total),
        r.usage.objects
    );
    println!("  pinned             {}", human(r.usage.pinned));
    println!("  cache              {}", human(r.usage.cache));
    println!(
        "  http cache         {} (limit {})",
        human(r.usage.http_cache),
        human(r.http_cache_max_bytes)
    );
    println!("hard cap             {}", human(r.max_total_bytes));
    println!(
        "watermarks           high {} / low {}",
        human(r.high_watermark_bytes),
        human(r.low_watermark_bytes)
    );
    println!("effective cap        {}", human(r.effective_cap));
    if let (Some(a), Some(t)) = (r.disk_available, r.disk_total) {
        println!("volume               {} free of {}", human(a), human(t));
    }
    let o = r.s3_ops_today;
    println!(
        "s3 today             {} PUT, {} GET, {} DELETE, {} egress",
        o.puts,
        o.gets,
        o.deletes,
        human(o.egress_bytes)
    );
    println!(
        "at hard cap          {}",
        if r.at_hard_cap { "yes" } else { "no" }
    );
    println!(
        "eviction (dry run)   would remove {} objects, {}",
        r.eviction.objects,
        human(r.eviction.bytes)
    );
    println!(
        "postgres             {} (reported, not budgeted)",
        human(u64::try_from(pg_bytes).unwrap_or(0))
    );
}

/// Clock for the budget outside the server.
struct SystemClock;

impl app::ports::Clock for SystemClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    fn hlc(&self) -> domain::hlc::Hlc {
        let ms = u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0);
        domain::hlc::Hlc::new(ms, 0, 0)
    }
}

/// Events from the CLI are not delivered anywhere (no browsers are connected).
struct NoEvents;

#[async_trait::async_trait]
impl app::ports::EventBus for NoEvents {
    async fn publish(&self, _event: app::ports::AppEvent) {}
}

async fn run(args: Args) -> Result<()> {
    if args.cmd == Cmd::Help {
        println!("{USAGE}");
        return Ok(());
    }
    let cfg = config::AppConfig::load(args.config.as_deref())?;
    let pool = store_pg::connect(&cfg.database.url, 2)
        .await
        .context("connect to the database")?;
    match args.cmd {
        Cmd::Migrate => {
            store_pg::migrate(&pool).await?;
            println!("migrations applied");
        }
        Cmd::StorageReport { json } => {
            let budget = budget(&cfg, &pool)?;
            let report = budget.report().await?;
            let pg_bytes: i64 = sqlx::query_scalar("SELECT pg_database_size(current_database())")
                .fetch_one(&pool)
                .await?;
            if json {
                let mut v = serde_json::to_value(&report)?;
                v["postgres_bytes"] = pg_bytes.into();
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                print_report(&report, pg_bytes);
            }
        }
        Cmd::StorageEvict => {
            let r = budget(&cfg, &pool)?.evict().await?;
            println!(
                "evicted {} objects, {} ({} -> {})",
                r.objects,
                human(r.bytes),
                human(r.before),
                human(r.after)
            );
        }
        Cmd::Help => unreachable!(),
    }
    Ok(())
}

fn budget(cfg: &config::AppConfig, pool: &sqlx::PgPool) -> Result<assets::StorageBudget> {
    Ok(assets::StorageBudget::open(
        cfg.storage.clone(),
        Arc::new(assets::pg::PgObjectIndex::new(pool.clone())),
        Arc::new(SystemClock),
        Arc::new(NoEvents),
    )?)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(parse(&args)?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Args> {
        parse(&s.split_whitespace().map(String::from).collect::<Vec<_>>())
    }

    #[test]
    fn parses_commands() {
        assert_eq!(
            p("--config app.toml storage report --dry-run").unwrap(),
            Args {
                config: Some("app.toml".into()),
                cmd: Cmd::StorageReport { json: false }
            }
        );
        assert_eq!(
            p("storage report --json").unwrap().cmd,
            Cmd::StorageReport { json: true }
        );
        assert_eq!(p("migrate").unwrap().cmd, Cmd::Migrate);
        assert_eq!(p("").unwrap().cmd, Cmd::Help);
        assert!(p("storage report --force").is_err());
        assert!(p("frobnicate").is_err());
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(human(0), "0.0 B");
        assert_eq!(human(1536), "1.5 KiB");
        assert_eq!(human(20 << 30), "20.0 GiB");
    }
}
