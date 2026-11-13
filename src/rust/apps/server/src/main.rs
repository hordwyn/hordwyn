//! Server binary: `server [--config <file>] [--role=all|web|worker]`.
#![deny(unsafe_code)]

use anyhow::{Context, bail};
use config::{AppConfig, ServerRole};
use leptos::prelude::*;

use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

struct Args {
    config: Option<std::path::PathBuf>,
    role: Option<ServerRole>,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = Args {
        config: None,
        role: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if let Some(r) = a.strip_prefix("--role=") {
            args.role = Some(r.parse().map_err(anyhow::Error::msg)?);
        } else if a == "--role" {
            args.role = Some(
                it.next()
                    .context("--role needs a value")?
                    .parse()
                    .map_err(anyhow::Error::msg)?,
            );
        } else if a == "--config" {
            args.config = Some(it.next().context("--config needs a path")?.into());
        } else if let Some(p) = a.strip_prefix("--config=") {
            args.config = Some(p.into());
        } else {
            bail!(
                "unknown argument `{a}`; usage: server [--config <file>] [--role=all|web|worker]"
            );
        }
    }
    Ok(args)
}

async fn shutdown_signal(stop: CancellationToken) {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
    stop.cancel();
}

/// SIGHUP re-reads the secrets key set (DEC-283): a set that drops a version still in use is
/// refused and the loaded keys stay (logged).
#[cfg(unix)]
async fn reload_on_sighup(server: Server) {
    use tokio::signal::unix::{SignalKind, signal};
    let Ok(mut hup) = signal(SignalKind::hangup()) else {
        tracing::warn!("cannot listen for SIGHUP; a new secrets key needs a restart");
        return;
    };
    while hup.recv().await.is_some() {
        match api::admin::reload_keys(&server.api).await {
            Ok(v) => tracing::info!(versions = ?v, "reloaded the secrets key set"),
            Err(e) => {
                tracing::warn!(error = %e, "secrets key set not reloaded; keeping the loaded keys");
            }
        }
    }
}

#[cfg(not(unix))]
async fn reload_on_sighup(_server: Server) {}

/// Worker stack size: server-side rendering of the deepest pages needs just over the 2 MiB
/// default in debug builds (DEC-188).
const STACK_SIZE: usize = 8 * 1024 * 1024;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(STACK_SIZE)
        .build()?
        .block_on(run())
}

async fn run() -> anyhow::Result<()> {
    let args = parse_args()?;
    let cfg = AppConfig::load(args.config.as_deref())?;
    telemetry::init(&cfg.telemetry);
    let role = args.role.unwrap_or(cfg.server.role);
    let server = Server::build(cfg, role, Overrides::default()).await?;
    let stop = CancellationToken::new();
    tokio::spawn(shutdown_signal(stop.clone()));
    tokio::spawn(reload_on_sighup(server.clone()));
    let stats_sync = {
        let stop = stop.clone();
        let server = server.clone();
        tokio::spawn(async move { server.run_stats_sync(stop).await })
    };

    let worker = role.runs_worker().then(|| {
        let stop = stop.clone();
        let server = server.clone();
        tokio::spawn(async move { server.run_worker(stop).await })
    });

    if role.runs_web() {
        let secure = server.api.admin.as_ref().map_or(
            server.config.auth.secure_cookies,
            api::admin::AdminApi::secure_cookies,
        );
        for warning in server::infra::exposure_warnings(
            &server.config.server.public_url,
            &server.config.server.bind,
            secure,
        ) {
            tracing::warn!("{warning}");
        }
        let conf = get_configuration(None).context("leptos configuration")?;
        let mut options = conf.leptos_options;
        options.site_addr = server.config.server.bind.parse().context("server.bind")?;
        options.hash_files = server::infra::bundle_hashes(
            options.hash_files,
            &server::infra::hash_file_path(&options.hash_file),
            &std::path::Path::new(&*options.site_root).join(&*options.site_pkg_dir),
            &options.output_name,
        )?;
        let app = server.app_router(&options);
        let listener = tokio::net::TcpListener::bind(options.site_addr)
            .await
            .with_context(|| format!("bind {}", options.site_addr))?;
        tracing::info!(addr = %options.site_addr, ?role, "listening");
        let s = stop.clone();
        let bus = server.bus.clone();
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            s.cancelled().await;
            // open SSE streams would otherwise keep graceful shutdown waiting forever
            bus.shutdown();
        })
        .await?;
    } else {
        tracing::info!(?role, "worker running");
        stop.cancelled().await;
    }
    if let Some(w) = worker {
        let _ = w.await;
    }
    let _ = stats_sync.await;
    Ok(())
}
