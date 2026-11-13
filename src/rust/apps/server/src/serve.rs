//! The listener: accepts connections and serves the router on them, with a limit on how
//! long a client may take to send the head of a request (DEC-653, the half of it that is a
//! setting of the listener).
//!
//! `axum::serve` sets no timer on its HTTP/1 connections, so hyper's header read timeout
//! never runs there (hyper 1.12 `server::conn::http1::Builder::header_read_timeout`:
//! "Requires a `Timer` set by `Builder::timer` to take effect"); a client could open
//! connections, send half a request line and keep them for ever. This loop is the same as
//! axum's (accept, one task per connection, graceful shutdown) with the timer set. hyper
//! starts the timeout whenever it waits for a request head, so it is also how long an idle
//! keep-alive connection is held, and it runs from the moment a connection is accepted, so
//! a connection that sends nothing is closed too. Only HTTP/1 is served.
//!
//! The time a request *body* may take is a layer of the router (`read_timeout`), because it
//! differs by route.

use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use hyper::server::conn::http1::Builder;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::graceful::GracefulShutdown;
use hyper_util::service::TowerToHyperService;
use tower::ServiceExt;

/// Serve `app` on `listener` until `shutdown` resolves, then let the open connections
/// finish. Every request carries the peer address as `ConnectInfo<SocketAddr>`.
pub async fn serve<F>(
    listener: tokio::net::TcpListener,
    app: Router,
    header_read_timeout: Duration,
    shutdown: F,
) where
    F: Future<Output = ()> + Send,
{
    // HTTP/1 only (review SEC2-2): hyper-util's listener for both protocols first reads up
    // to 24 bytes to tell them apart, without a timer, and HTTP/2 has no head limit; so a
    // client that sent nothing or the start of the HTTP/2 preface held its connection for
    // ever. Pages, the REST API and server-sent events need nothing beyond HTTP/1.1 (no
    // protocol upgrades are used), and the documented reverse proxies speak HTTP/1.1 to the
    // application.
    let mut builder = Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(header_read_timeout);
    let graceful = GracefulShutdown::new();
    let mut shutdown = std::pin::pin!(shutdown);
    loop {
        let (stream, peer) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    // out of file descriptors and the like: wait instead of spinning
                    if !matches!(
                        e.kind(),
                        std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::ConnectionReset
                    ) {
                        tracing::warn!(error = %e, "accepting a connection failed");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    continue;
                }
            },
            () = &mut shutdown => break,
        };
        let service = app
            .clone()
            .map_request(move |req: hyper::Request<hyper::body::Incoming>| {
                let mut req = req.map(Body::new);
                req.extensions_mut().insert(ConnectInfo::<SocketAddr>(peer));
                req
            });
        let conn =
            builder.serve_connection(TokioIo::new(stream), TowerToHyperService::new(service));
        let conn = graceful.watch(conn);
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::trace!(error = %e, %peer, "connection ended with an error");
            }
        });
    }
    drop(listener);
    graceful.shutdown().await;
}
