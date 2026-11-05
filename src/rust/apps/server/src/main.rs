//! Server binary: `--role=all|web|worker`. Wires adapters into the `app` core.
#![deny(unsafe_code)]

use anyhow::Context;
use axum::Router;
use axum::routing::get;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};

/// HTTP routes: health check plus the server-rendered web app.
fn router(options: LeptosOptions) -> Router {
    let routes = generate_route_list(web::App);
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || web::shell(options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(web::shell))
        .with_state(options)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let conf = get_configuration(None).context("leptos configuration")?;
    let addr = conf.leptos_options.site_addr;
    let app = router(conf.leptos_options);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    println!("listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_answers_ok() {
        let app = super::router(
            leptos::prelude::LeptosOptions::builder()
                .output_name("app")
                .build(),
        );
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
