//! Cross-site request protection for cookie-authenticated requests (G2-1): a state-changing
//! request (not GET/HEAD/OPTIONS) that a browser marks as cross-site, or whose `Origin` is
//! neither the public URL nor the requested host, is refused with 403. Requests without
//! these headers (API clients with bearer tokens, webhooks, curl) are not affected: they do
//! not carry the session cookie of a victim's browser.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// `scheme://host[:port]` of a URL, lowercased.
fn origin_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    Some(u.origin().ascii_serialization().to_ascii_lowercase())
}

/// Is this request allowed to change state?
#[must_use]
pub fn allowed(method: &Method, headers: &HeaderMap, public_origin: Option<&str>) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return true;
    }
    let get = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    if get("sec-fetch-site") == Some("cross-site") {
        return false;
    }
    let Some(origin) = get(header::ORIGIN.as_str()) else {
        return true;
    };
    let origin = origin.to_ascii_lowercase();
    if public_origin.is_some_and(|p| p == origin) {
        return true;
    }
    // same host as requested (other ports or names of the same server, e.g. in development)
    let host = get(header::HOST.as_str()).map(str::to_ascii_lowercase);
    origin_of(&origin)
        .and_then(|o| o.split_once("://").map(|(_, h)| h.to_string()))
        .is_some_and(|h| host.as_deref() == Some(h.as_str()))
}

/// Middleware; the state is the public URL's origin.
pub async fn guard(State(public): State<Option<String>>, req: Request, next: Next) -> Response {
    if allowed(req.method(), req.headers(), public.as_deref()) {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, "cross-site request refused").into_response()
    }
}

/// The public URL's origin for [`guard`].
#[must_use]
pub fn public_origin(public_url: &str) -> Option<String> {
    origin_of(public_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        m
    }

    #[test]
    fn cross_site_posts_are_refused() {
        let public = public_origin("https://shelf.example.org/");
        let p = public.as_deref();
        assert_eq!(p, Some("https://shelf.example.org"));
        let post = Method::POST;
        // a login form on another site posting to us (login CSRF)
        assert!(!allowed(
            &post,
            &h(&[("origin", "https://evil.example")]),
            p
        ));
        assert!(!allowed(&post, &h(&[("sec-fetch-site", "cross-site")]), p));
        // our own pages
        assert!(allowed(
            &post,
            &h(&[("origin", "https://shelf.example.org")]),
            p
        ));
        assert!(allowed(
            &post,
            &h(&[
                ("origin", "http://127.0.0.1:3010"),
                ("host", "127.0.0.1:3010")
            ]),
            p
        ));
        assert!(!allowed(
            &post,
            &h(&[
                ("origin", "http://127.0.0.1:9999"),
                ("host", "127.0.0.1:3010")
            ]),
            p
        ));
        // API clients and webhooks without browser headers, and reads
        assert!(allowed(&post, &h(&[]), p));
        assert!(allowed(
            &Method::GET,
            &h(&[("origin", "https://evil.example")]),
            p
        ));
        assert!(!allowed(&Method::DELETE, &h(&[("origin", "null")]), p));
    }
}
