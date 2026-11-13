//! Admin CLI: migrate, storage report and eviction, search reindex, and the escape hatches
//! that need shell access (addendum-1 3.3, DEC-283): promote an admin, a one-time password
//! reset link, remove an authenticator, back to the sign-in page, reopen the setup wizard,
//! secrets key status, rotation and pruning. Each escape hatch writes an audit row with actor
//! `cli`. (Everyday user management and imports are in the web app and the REST API.)
//!
//! `cli [--config <file>] <command>`; the configuration is the server's (file + `APP__*`).
#![deny(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

const USAGE: &str = "usage: cli [--config <file>] <command>

  migrate [--dry-run]              apply database migrations (--dry-run: list the pending
                                   ones, nothing is changed)
  storage report [--dry-run] [--json]
                                   storage usage, limits, S3 counters and what eviction
                                   would remove now (--dry-run: nothing is changed)
  storage evict                    run eviction now (normally a background job)
  reindex [--clear] [--now]        queue every catalog item, library entry and companion for
                                   the search indexer (--clear: empty the indexes first;
                                   --now: index them in this process and wait, for an
                                   instance without a worker, such as a staging copy)
  refresh-failed [--kind <kind>] [--apply]
                                   ask the sources again for every item that never got its
                                   data (a failed refresh, or an imported title that was
                                   never fetched); books with an Open Library id are filled
                                   with one request per 50, the rest is queued for the
                                   worker. Prints what it would do; --apply does it
  match-books [--apply | --applied]
                                   find the Open Library work of every book that has none,
                                   by title (books entered by hand are left out); prints the
                                   matches, --apply stores them (and logs what each book was
                                   before), --applied lists that log
  admin promote <username>         make an account admin
  admin reset-password <username>  print a one-time password reset link (1 hour)
  admin reset-totp <username>      remove the authenticator and its recovery codes
  admin disable-sso-only           use the sign-in page again (auth.sign_in_mode = page)
  setup reopen                     open the setup wizard again
  secrets status                   key versions and how many values each encrypts
  secrets rotate                   add a new active key to the key file (then SIGHUP)
  secrets prune                    drop key versions no stored value uses (after SIGHUP)";

#[derive(Debug, PartialEq, Eq)]
enum Cmd {
    Migrate { dry_run: bool },
    StorageReport { json: bool },
    StorageEvict,
    Reindex { clear: bool, now: bool },
    RefreshFailed { kind: Option<String>, apply: bool },
    MatchBooks { apply: bool },
    MatchesApplied,
    Promote(String),
    ResetPassword(String),
    ResetTotp(String),
    DisableSsoOnly,
    SetupReopen,
    SecretsStatus,
    SecretsRotate,
    SecretsPrune,
    Help,
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    config: Option<PathBuf>,
    cmd: Cmd,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut config = None;
    let mut kind = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => config = Some(PathBuf::from(it.next().context("--config needs a path")?)),
            "--kind" => kind = Some(it.next().context("--kind needs a kind id")?.clone()),
            _ => rest.push(a.as_str()),
        }
    }
    let (flags, words): (Vec<&str>, Vec<&str>) = rest.iter().partition(|a| a.starts_with("--"));
    let cmd = command(&words, &flags, &mut kind)?;
    if kind.is_some() {
        bail!("--kind belongs to refresh-failed\n\n{USAGE}");
    }
    Ok(Args { config, cmd })
}

/// The command of the words and options of a call; `kind` is the value of `--kind`, taken
/// by the command it belongs to.
fn command(words: &[&str], flags: &[&str], kind: &mut Option<String>) -> Result<Cmd> {
    let known = |allowed: &[&str]| -> Result<()> {
        for f in flags {
            if !allowed.contains(f) {
                bail!("unknown option `{f}`\n\n{USAGE}");
            }
        }
        Ok(())
    };
    Ok(match words {
        ["migrate"] => {
            known(&["--dry-run"])?;
            Cmd::Migrate {
                dry_run: flags.contains(&"--dry-run"),
            }
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
        ["reindex"] => {
            known(&["--clear", "--now"])?;
            Cmd::Reindex {
                clear: flags.contains(&"--clear"),
                now: flags.contains(&"--now"),
            }
        }
        ["refresh-failed"] => {
            known(&["--apply"])?;
            Cmd::RefreshFailed {
                kind: kind.take(),
                apply: flags.contains(&"--apply"),
            }
        }
        ["match-books"] => {
            known(&["--apply", "--applied"])?;
            match (flags.contains(&"--apply"), flags.contains(&"--applied")) {
                (true, true) => bail!("--apply or --applied, not both\n\n{USAGE}"),
                (_, true) => Cmd::MatchesApplied,
                (apply, false) => Cmd::MatchBooks { apply },
            }
        }
        ["admin", "promote", u] => {
            known(&[])?;
            Cmd::Promote((*u).to_string())
        }
        ["admin", "reset-password", u] => {
            known(&[])?;
            Cmd::ResetPassword((*u).to_string())
        }
        ["admin", "reset-totp", u] => {
            known(&[])?;
            Cmd::ResetTotp((*u).to_string())
        }
        ["admin", "disable-sso-only"] => {
            known(&[])?;
            Cmd::DisableSsoOnly
        }
        ["setup", "reopen"] => {
            known(&[])?;
            Cmd::SetupReopen
        }
        ["secrets", "status"] => {
            known(&[])?;
            Cmd::SecretsStatus
        }
        ["secrets", "rotate"] => {
            known(&[])?;
            Cmd::SecretsRotate
        }
        ["secrets", "prune"] => {
            known(&[])?;
            Cmd::SecretsPrune
        }
        [] | ["help"] => Cmd::Help,
        _ => bail!("unknown command `{}`\n\n{USAGE}", words.join(" ")),
    })
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
    if let Some(done) = escape(&args.cmd, &cfg).await? {
        println!("{done}");
        return Ok(());
    }
    if let Some(done) = catalog(&args.cmd, &cfg).await? {
        println!("{done}");
        return Ok(());
    }
    let pool = store_pg::connect(&cfg.database.url, 2)
        .await
        .context("connect to the database")?;
    match args.cmd {
        Cmd::Migrate { dry_run: true } => {
            let differ = store_pg::mismatched_migrations(&pool).await?;
            if !differ.is_empty() {
                let lines: Vec<String> = differ.iter().map(|m| format!("  {m}")).collect();
                bail!(
                    "the database's migrations differ from this build's, so the start would \
                     refuse it; nothing was changed:\n{}\nA database made by a build from \
                     before 1.0.0 cannot be upgraded; a changed released migration is a bug of \
                     the release (see the release notes).",
                    lines.join("\n")
                );
            }
            let pending = store_pg::pending_migrations(&pool).await?;
            if pending.is_empty() {
                println!("no pending migrations");
            } else {
                println!(
                    "pending migrations ({}), nothing was changed:",
                    pending.len()
                );
                for m in pending {
                    println!("  {:04} {}", m.version, m.description);
                }
            }
        }
        Cmd::Migrate { dry_run: false } => {
            store_pg::migrate(&pool).await?;
            println!("migrations applied");
        }
        Cmd::StorageReport { json } => {
            let budget = budget(&cfg, &pool)?;
            let report = budget.report().await?;
            let pg_bytes: i64 = sqlx::query_scalar("SELECT pg_database_size(current_database())")
                .fetch_one(&pool)
                .await?;
            let meili = meili_stats(&cfg).await;
            if json {
                let mut v = serde_json::to_value(&report)?;
                v["postgres_bytes"] = pg_bytes.into();
                if let Some((docs, bytes)) = meili {
                    v["meilisearch"] = serde_json::json!({ "documents": docs, "bytes": bytes });
                }
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                print_report(&report, pg_bytes);
                if let Some((docs, bytes)) = meili {
                    println!(
                        "meilisearch          {} in {docs} documents (reported, not budgeted)",
                        human(bytes)
                    );
                }
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
        Cmd::Reindex { clear, now } => {
            let index = open_index(&cfg, &pool, clear).await?;
            let n = search::indexer::Indexer::enqueue_all(&pool).await?;
            if now {
                let done = server::escape::index_now(&pool, index).await?;
                println!("queued {n} documents and indexed them ({done} outbox rows)");
            } else {
                println!("queued {n} documents; the worker's indexer picks them up");
            }
        }
        _ => unreachable!("handled above"),
    }
    Ok(())
}

/// `refresh-failed` and `match-books` (`server::book_stubs`); `None` for the other commands.
async fn catalog(cmd: &Cmd, cfg: &config::AppConfig) -> Result<Option<String>> {
    use server::book_stubs::{Catalog, match_text, refresh_text};
    Ok(Some(match cmd {
        Cmd::RefreshFailed { kind, apply } => {
            let c = Catalog::open(cfg).await?;
            refresh_text(&c.refresh_failed(kind.as_deref(), *apply).await?)
        }
        Cmd::MatchesApplied => Catalog::open(cfg).await?.applied_matches().await?,
        Cmd::MatchBooks { apply } => {
            let c = Catalog::open(cfg).await?;
            match_text(&c.match_books(*apply).await?, *apply)
        }
        _ => return Ok(None),
    }))
}

/// The escape hatches (`server::escape`); `None` for the other commands.
async fn escape(cmd: &Cmd, cfg: &config::AppConfig) -> Result<Option<String>> {
    let is_escape = matches!(
        cmd,
        Cmd::Promote(_)
            | Cmd::ResetPassword(_)
            | Cmd::ResetTotp(_)
            | Cmd::DisableSsoOnly
            | Cmd::SetupReopen
            | Cmd::SecretsStatus
            | Cmd::SecretsRotate
            | Cmd::SecretsPrune
    );
    if !is_escape {
        return Ok(None);
    }
    let e = server::escape::Escape::open(cfg).await?;
    Ok(Some(match cmd {
        Cmd::Promote(u) => {
            e.promote(u).await?;
            format!("{u} is an admin now")
        }
        Cmd::ResetPassword(u) => format!(
            "one-time link for {u} (valid {} minutes, once):\n{}",
            auth::PASSWORD_RESET_MINUTES,
            e.reset_password(u).await?
        ),
        Cmd::ResetTotp(u) => {
            if e.reset_totp(u).await? {
                format!("removed the authenticator of {u}; they sign in with the password")
            } else {
                format!("{u} had no authenticator")
            }
        }
        Cmd::DisableSsoOnly => {
            if e.disable_sso_only().await? {
                "sign-in mode is `page` again: the sign-in page with the password form".into()
            } else {
                "sign-in mode was `page` already".into()
            }
        }
        Cmd::SetupReopen => {
            e.setup_reopen().await?;
            "the setup wizard is open again at /setup".into()
        }
        Cmd::SecretsStatus => status_text(&e.secrets_status().await?),
        Cmd::SecretsRotate => format!(
            "added key v{} as the active version; send SIGHUP to every server and worker process \
             (or restart them) to load it; the nightly job re-encrypts stored values",
            e.secrets_rotate().await?
        ),
        Cmd::SecretsPrune => {
            let dropped = e.secrets_prune().await?;
            if dropped.is_empty() {
                "nothing to prune: every older version still encrypts stored values".into()
            } else {
                format!("dropped {dropped:?} from the key file; send SIGHUP to the server")
            }
        }
        _ => unreachable!("not an escape hatch"),
    }))
}

fn status_text(r: &server::escape::SecretsReport) -> String {
    use std::fmt::Write;
    let mut out = format!(
        "key source           {} ({})\nactive version       v{}\n",
        r.key_source, r.key_file, r.active_version
    );
    for v in &r.versions {
        let _ = writeln!(
            out,
            "  v{:<4} {:>6} values  {}{}",
            v.version,
            v.rows,
            if v.loaded { "loaded" } else { "NOT LOADED" },
            if v.active { ", active" } else { "" }
        );
    }
    let _ = write!(
        out,
        "to re-encrypt        {}\nlocked               {}",
        r.rows_to_rewrap, r.locked_rows
    );
    out
}

/// Meilisearch documents and database size, if that backend is configured and reachable.
async fn meili_stats(cfg: &config::AppConfig) -> Option<(u64, u64)> {
    if cfg.search.backend != config::SearchBackend::Meilisearch {
        return None;
    }
    let key = cfg.search.meili_master_key.as_ref()?;
    let m = search::meili::MeiliIndex::connect(
        &cfg.search.meili_url,
        key.expose(),
        &cfg.search.index_prefix,
    )
    .await
    .ok()?;
    m.stats().await.ok()
}

/// The configured search index; `clear` empties its indexes first.
async fn open_index(
    cfg: &config::AppConfig,
    pool: &sqlx::PgPool,
    clear: bool,
) -> Result<Arc<dyn app::ports::SearchIndex>> {
    let index: Arc<dyn app::ports::SearchIndex> = match cfg.search.backend {
        config::SearchBackend::Postgres => {
            let pg = search::pg::PgSearch::new(pool.clone());
            if clear {
                for ix in search::INDEXES {
                    pg.clear(ix).await?;
                }
            }
            Arc::new(pg)
        }
        config::SearchBackend::Meilisearch => {
            let key = cfg
                .search
                .meili_master_key
                .as_ref()
                .context("search.meili_master_key is required")?;
            let m = search::meili::MeiliIndex::connect(
                &cfg.search.meili_url,
                key.expose(),
                &cfg.search.index_prefix,
            )
            .await?;
            if clear {
                for ix in search::INDEXES {
                    m.clear(ix).await?;
                }
            }
            Arc::new(m)
        }
    };
    if clear {
        println!("search indexes cleared");
    }
    Ok(index)
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
        assert_eq!(p("migrate").unwrap().cmd, Cmd::Migrate { dry_run: false });
        assert_eq!(
            p("migrate --dry-run").unwrap().cmd,
            Cmd::Migrate { dry_run: true }
        );
        assert!(p("migrate --clear").is_err());
        assert_eq!(
            p("reindex --clear").unwrap().cmd,
            Cmd::Reindex {
                clear: true,
                now: false
            }
        );
        assert_eq!(
            p("reindex --clear --now").unwrap().cmd,
            Cmd::Reindex {
                clear: true,
                now: true
            },
            "--now: index in this process, for an instance without a worker (staging)"
        );
        assert_eq!(p("").unwrap().cmd, Cmd::Help);
        assert!(p("storage report --force").is_err());
        assert_eq!(
            p("admin promote mia").unwrap().cmd,
            Cmd::Promote("mia".into())
        );
        assert_eq!(
            p("admin reset-password mia").unwrap().cmd,
            Cmd::ResetPassword("mia".into())
        );
        assert_eq!(
            p("admin reset-totp mia").unwrap().cmd,
            Cmd::ResetTotp("mia".into())
        );
        assert_eq!(
            p("admin disable-sso-only").unwrap().cmd,
            Cmd::DisableSsoOnly
        );
        assert_eq!(p("setup reopen").unwrap().cmd, Cmd::SetupReopen);
        assert_eq!(p("secrets status").unwrap().cmd, Cmd::SecretsStatus);
        assert_eq!(p("secrets rotate").unwrap().cmd, Cmd::SecretsRotate);
        assert_eq!(p("secrets prune").unwrap().cmd, Cmd::SecretsPrune);
        assert!(p("admin promote").is_err());
        assert!(p("frobnicate").is_err());
        assert_eq!(
            p("refresh-failed").unwrap().cmd,
            Cmd::RefreshFailed {
                kind: None,
                apply: false
            },
            "a dry run unless asked"
        );
        assert_eq!(
            p("refresh-failed --kind book --apply").unwrap().cmd,
            Cmd::RefreshFailed {
                kind: Some("book".into()),
                apply: true
            }
        );
        assert!(p("refresh-failed --kind").is_err());
        assert!(p("refresh-failed --applied").is_err());
        assert_eq!(p("match-books --applied").unwrap().cmd, Cmd::MatchesApplied);
        assert!(p("match-books --apply --applied").is_err());
        assert!(
            p("reindex --kind book").is_err(),
            "--kind is refresh-failed's"
        );
        assert_eq!(
            p("match-books").unwrap().cmd,
            Cmd::MatchBooks { apply: false },
            "a dry run unless asked"
        );
        assert_eq!(
            p("match-books --apply").unwrap().cmd,
            Cmd::MatchBooks { apply: true }
        );
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(human(0), "0.0 B");
        assert_eq!(human(1536), "1.5 KiB");
        assert_eq!(human(20 << 30), "20.0 GiB");
    }
}
