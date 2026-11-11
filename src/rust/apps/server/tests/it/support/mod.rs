//! Shared by the modules of the server test binary: one request through the router, with a
//! time limit.

use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::{Request, Response};
use tower::ServiceExt;

/// How long one request may take, answer and whole body. A server-side render that never
/// completes (seen once under full CI load, DEC-384) fails the test naming the request
/// instead of blocking the whole run.
pub const REQUEST_LIMIT: Duration = Duration::from_secs(90);

/// Send `req` through `app` and read the whole body.
///
/// # Panics
/// When the answer or its body does not finish within [`REQUEST_LIMIT`] (the message names
/// the method and path), or when the router or the body fails.
pub async fn send(app: &axum::Router, req: Request<Body>) -> Response<Bytes> {
    let what = format!("{} {}", req.method(), req.uri());
    let run = async move {
        let resp = app.clone().oneshot(req).await.unwrap();
        let (parts, body) = resp.into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        Response::from_parts(parts, bytes)
    };
    let Ok(resp) = tokio::time::timeout(REQUEST_LIMIT, run).await else {
        panic!(
            "{what} did not finish within {} s: a server-side render that never completes?",
            REQUEST_LIMIT.as_secs()
        );
    };
    resp
}
