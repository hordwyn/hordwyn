//! How long a request body may take to arrive (DEC-653, SEC-5 scenario "many connections
//! that send almost a whole body and then stall").
//!
//! Two limits, all settings (`server.body_idle_timeout_secs`,
//! `server.body_read_timeout_secs`, `server.import_read_timeout_secs`): the longest pause
//! between two pieces of a body, for every request; and the time for the whole body, with a
//! longer time for the import upload, which may be 64 MiB over a slow line (review SEC2-7:
//! without one, a trickle held 64 MiB for ever). A body that breaks a limit ends with an
//! error for whoever reads it, and the request is answered `408` in the shape of every
//! other error. The time for the head of a request is a setting of the listener
//! ([`crate::serve`]).
//!
//! The limits run only while something reads the body: a handler that never reads it (a
//! page, a file) holds nothing of it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::body::{Body, HttpBody};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use futures::StreamExt;

/// The limits of one server.
#[derive(Debug, Clone, Copy)]
pub struct ReadLimits {
    /// Longest pause between two pieces of a body.
    pub idle: Duration,
    /// Longest time for a whole body (not for the [`import_upload`]).
    pub total: Duration,
    /// Longest time for the body of the [`import_upload`].
    pub import_total: Duration,
}

impl ReadLimits {
    #[must_use]
    pub fn from_config(s: &config::ServerConfig) -> Self {
        Self {
            idle: Duration::from_secs(s.body_idle_timeout_secs),
            total: Duration::from_secs(s.body_read_timeout_secs),
            import_total: Duration::from_secs(s.import_read_timeout_secs),
        }
    }

    /// The time for the whole body of a request to `path`.
    fn total_for(&self, path: &str) -> Duration {
        if import_upload(path) {
            self.import_total
        } else {
            self.total
        }
    }
}

/// The import upload: large on purpose, so it has a longer total time.
fn import_upload(path: &str) -> bool {
    path == "/api/v1/imports"
}

fn timed_out() -> Response {
    api::ApiError::new(
        StatusCode::REQUEST_TIMEOUT,
        "request_timeout",
        "the request body did not arrive in time",
    )
    .into_response()
}

/// `body`, ending in an error when the next piece does not come within `idle` or the whole
/// body is not there `total` after the first read; `late` is set then.
fn limited(body: Body, idle: Duration, total: Duration, late: Arc<AtomicBool>) -> Body {
    struct State {
        pieces: axum::body::BodyDataStream,
        /// Set at the first read.
        deadline: Option<tokio::time::Instant>,
        done: bool,
    }
    let state = State {
        pieces: body.into_data_stream(),
        deadline: None,
        done: false,
    };
    Body::from_stream(futures::stream::unfold(state, move |mut st| {
        let late = late.clone();
        async move {
            if st.done {
                return None;
            }
            let now = tokio::time::Instant::now();
            let deadline = *st.deadline.get_or_insert(now + total);
            let wait = idle.min(deadline.saturating_duration_since(now));
            match tokio::time::timeout(wait, st.pieces.next()).await {
                Ok(Some(Ok(piece))) => Some((Ok(piece), st)),
                Ok(Some(Err(e))) => {
                    st.done = true;
                    Some((Err(std::io::Error::other(e)), st))
                }
                Ok(None) => None,
                Err(_) => {
                    late.store(true, Ordering::SeqCst);
                    st.done = true;
                    let e = std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "the request body did not arrive in time",
                    );
                    Some((Err(e), st))
                }
            }
        }
    }))
}

/// Middleware: put the limits on the body of `req`; when the handler gave up because the
/// body was late, answer `408` instead of whatever it made of the broken body.
pub async fn layer(State(limits): State<ReadLimits>, req: Request, next: Next) -> Response {
    // nothing to wait for: no body, or one that is already complete
    if req.body().is_end_stream() {
        return next.run(req).await;
    }
    let late = Arc::new(AtomicBool::new(false));
    let total = limits.total_for(req.uri().path());
    let req = req.map(|body| limited(body, limits.idle, total, late.clone()));
    let resp = next.run(req).await;
    if late.load(Ordering::SeqCst) {
        return timed_out();
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_import_upload_has_the_longer_total() {
        let l = ReadLimits {
            idle: Duration::from_secs(1),
            total: Duration::from_secs(2),
            import_total: Duration::from_secs(3),
        };
        assert_eq!(l.total_for("/api/v1/imports"), Duration::from_secs(3));
        for p in [
            "/api/v1/imports/123/apply",
            "/api/v1/auth/sign-in",
            "/_fn/sign_in",
        ] {
            assert_eq!(l.total_for(p), Duration::from_secs(2), "{p}");
        }
    }
}
