//! Tracing setup: human-readable or JSON logs, filter from config or `RUST_LOG`. Metrics are
//! `tracing` events with target `metrics` until an exporter is added (DEC-060).
#![deny(unsafe_code)]

use config::{LogFormat, TelemetryConfig};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

/// Filter directives: `RUST_LOG` wins over the configured filter.
#[must_use]
pub fn filter(cfg: &TelemetryConfig) -> EnvFilter {
    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&cfg.filter))
        .unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Install the global subscriber (once; later calls are ignored).
pub fn init(cfg: &TelemetryConfig) {
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter(cfg))
        .with_span_events(FmtSpan::CLOSE);
    let res = match cfg.log_format {
        LogFormat::Text => builder.try_init(),
        LogFormat::Json => builder.json().flatten_event(true).try_init(),
    };
    if res.is_err() {
        tracing::debug!("tracing subscriber already installed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_twice_is_harmless() {
        let cfg = TelemetryConfig::default();
        init(&cfg);
        init(&cfg);
        assert!(filter(&cfg).to_string().contains("info"));
    }
}
