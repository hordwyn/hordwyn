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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let cfg = AppConfig::load(args.config.as_deref())?;
    telemetry::init(&cfg.telemetry);
    let role = args.role.unwrap_or(cfg.server.role);
    let server = Server::build(cfg, role, Overrides::default()).await?;
    let stop = CancellationToken::new();
    tokio::spawn(shutdown_signal(stop.clone()));

    let worker = role.runs_worker().then(|| {
        let stop = stop.clone();
        let server = server.clone();
        tokio::spawn(async move { server.run_worker(stop).await })
    });

    if role.runs_web() {
        let conf = get_configuration(None).context("leptos configuration")?;
        let mut options = conf.leptos_options;
        options.site_addr = server.config.server.bind.parse().context("server.bind")?;
        let app = server.app_router(&options);
        let listener = tokio::net::TcpListener::bind(options.site_addr)
            .await
            .with_context(|| format!("bind {}", options.site_addr))?;
        tracing::info!(addr = %options.site_addr, ?role, "listening");
        let s = stop.clone();
        let bus = server.bus.clone();
        axum::serve(listener, app)
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
    Ok(())
}
