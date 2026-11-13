//! The security headers of every answer (SEC-1, DEC-536): a content security policy,
//! `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy`,
//! `Strict-Transport-Security` when the public URL is https, and `Cache-Control: no-store`
//! on what depends on the session.
//!
//! One middleware around every route ([`layer`]). It makes the nonce of the request before
//! the handler runs and hands it on as [`CspNonce`]; the page context gives it to Leptos, so
//! the inline scripts of the shell, the hydration script and the data the page streams
//! after the shell all carry the nonce the policy of the same answer names. A header a
//! handler set itself is kept.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// The policy of every answer that is not an HTML document: such an answer is data, and a
/// browser that is made to show it as a document (an SVG picture, a JSON answer opened in a
/// tab) loads and runs nothing.
pub const CLOSED_POLICY: &str = "default-src 'none'; frame-ancestors 'none'";

/// Features no page of the app uses, switched off for the page and everything it embeds.
/// Only names browsers know (an unknown name is a console warning on every page).
pub const PERMISSIONS: &str = "accelerometer=(), camera=(), display-capture=(), \
    geolocation=(), gyroscope=(), magnetometer=(), microphone=(), midi=(), payment=(), usb=()";

/// Sent to other origins: nothing. The pages have addresses that are secrets (a shared
/// list, an invitation, a password reset), and the name of a home instance is nobody's
/// business; requests to this origin keep the full address.
pub const REFERRER: &str = "same-origin";

static REPORT_ONLY: HeaderName = HeaderName::from_static("content-security-policy-report-only");
static PERMISSIONS_POLICY: HeaderName = HeaderName::from_static("permissions-policy");

/// The nonce of one request (128 random bits, base64url), in the request's extensions.
#[derive(Debug, Clone)]
pub struct CspNonce(pub Arc<str>);

impl CspNonce {
    fn new() -> Self {
        Self(URL_SAFE_NO_PAD.encode(auth::random_bytes::<16>()).into())
    }
}

/// Where the sign-out form may be redirected to, as the policy needs it now.
pub trait FormTargets: Send + Sync {
    /// Origins (`scheme://host[:port]`) besides this one.
    fn origins(&self) -> Vec<String>;
}

impl FormTargets for auth::oidc::Oidc {
    fn origins(&self) -> Vec<String> {
        self.end_session_origins()
    }
}

/// What the headers are built from; made once per server.
pub struct Headers {
    /// Send the policy as `Content-Security-Policy-Report-Only`.
    pub report_only: bool,
    /// `Strict-Transport-Security`, when it is sent at all.
    pub hsts: Option<HeaderValue>,
    /// The instance itself is served over plain http (a test on the LAN): pictures may come
    /// over http as well. Over https a browser upgrades or blocks them before the policy.
    pub http_images: bool,
    /// `cargo leptos watch` only: the port of its live-reload socket.
    pub reload_port: Option<u16>,
    pub form_targets: Option<Arc<dyn FormTargets>>,
}

impl Headers {
    /// From the configuration. HSTS only for an https public URL and an age above zero.
    #[must_use]
    pub fn new(server: &config::ServerConfig, form_targets: Option<Arc<dyn FormTargets>>) -> Self {
        let https = server
            .public_url
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("https://");
        let hsts = (https && server.hsts_max_age_secs > 0)
            .then(|| HeaderValue::from_str(&format!("max-age={}", server.hsts_max_age_secs)).ok())
            .flatten();
        Self {
            report_only: server.csp_report_only,
            hsts,
            http_images: !https,
            // set by `cargo leptos watch` for the server it starts (the page then opens a
            // socket to the reload port)
            reload_port: std::env::var_os("LEPTOS_WATCH").map(|_| {
                std::env::var("LEPTOS_RELOAD_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(3001)
            }),
            form_targets,
        }
    }

    /// The policy of an HTML document.
    ///
    /// - scripts: this origin (the bundle the hydration script imports) and inline scripts
    ///   with the nonce; `'wasm-unsafe-eval'` lets the bundle's WebAssembly be compiled and
    ///   nothing else (no `eval`);
    /// - styles: the stylesheet of this origin and the brand theme (nonce); `style`
    ///   attributes are allowed, because the server renders bar widths, brand swatches and
    ///   cover placeholders as such and an attribute can carry neither nonce nor hash;
    /// - pictures: this origin, `data:` (cover placeholders) and any https host: covers are
    ///   loaded from the provider (DEC-640), Open Library redirects to archive.org hosts,
    ///   podcast artwork lives wherever the feed says;
    /// - connections (server functions, REST, the event stream): this origin;
    /// - forms: this origin and where the sign-out form is redirected to (the provider's
    ///   end-session endpoint). CSP Level 3 checks each redirect of a form navigation; the
    ///   Chromium of the browser tests does not, so this is a precaution (DEC-656).
    #[must_use]
    pub fn page_policy(&self, nonce: &str) -> String {
        let images = if self.http_images { " http:" } else { "" };
        let reload = self.reload_port.map_or_else(String::new, |p| {
            format!(" ws://localhost:{p} ws://127.0.0.1:{p}")
        });
        let targets = self
            .form_targets
            .as_ref()
            .map(|t| t.origins())
            .unwrap_or_default()
            .into_iter()
            .filter(|o| source_expression(o))
            .fold(String::new(), |all, o| all + " " + &o);
        format!(
            "default-src 'self'; \
             script-src 'self' 'nonce-{nonce}' 'wasm-unsafe-eval'; \
             style-src 'self' 'nonce-{nonce}'; \
             style-src-attr 'unsafe-inline'; \
             img-src 'self' data: https:{images}; \
             font-src 'self'; \
             connect-src 'self'{reload}; \
             object-src 'none'; \
             base-uri 'none'; \
             form-action 'self'{targets}; \
             frame-ancestors 'none'"
        )
    }

    /// Set every header the answer does not carry yet.
    pub fn apply(&self, nonce: &str, path: &str, headers: &mut HeaderMap) {
        let html = is_document(headers);
        let policy = if html {
            HeaderValue::from_str(&self.page_policy(nonce)).ok()
        } else {
            None
        }
        .unwrap_or(HeaderValue::from_static(CLOSED_POLICY));
        let name = if self.report_only {
            REPORT_ONLY.clone()
        } else {
            header::CONTENT_SECURITY_POLICY
        };
        if !headers.contains_key(header::CONTENT_SECURITY_POLICY) && !headers.contains_key(&name) {
            headers.insert(name, policy);
        }
        let fixed = [
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::X_FRAME_OPTIONS, "DENY"),
            (header::REFERRER_POLICY, REFERRER),
            (PERMISSIONS_POLICY.clone(), PERMISSIONS),
        ];
        for (name, value) in fixed {
            if !headers.contains_key(&name) {
                headers.insert(name, HeaderValue::from_static(value));
            }
        }
        if let Some(hsts) = &self.hsts
            && !headers.contains_key(header::STRICT_TRANSPORT_SECURITY)
        {
            headers.insert(header::STRICT_TRANSPORT_SECURITY, hsts.clone());
        }
        if (html || depends_on_the_session(path)) && !headers.contains_key(header::CACHE_CONTROL) {
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
    }
}

/// Is the answer with these headers a document (`text/html`): a page, the 404 page, the
/// redirect interstitial? Those get the page policy and are never stored (DEC-658).
#[must_use]
pub fn is_document(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim_start().to_ascii_lowercase().starts_with("text/html"))
}

/// The REST API and the server functions answer for one session; pages do too, but are
/// told by their content type (the page routes have no common prefix).
fn depends_on_the_session(path: &str) -> bool {
    path.starts_with("/api/") || path.starts_with("/_fn/")
}

/// Is `origin` safe to write into the policy as one source: a plain `scheme://host[:port]`
/// without anything that would end the source list or the header.
fn source_expression(origin: &str) -> bool {
    let Some((scheme, host)) = origin.split_once("://") else {
        return false;
    };
    matches!(scheme, "https" | "http")
        && !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
}

/// The middleware: a nonce for the request, the headers on its answer.
pub async fn layer(State(h): State<Arc<Headers>>, mut req: Request, next: Next) -> Response {
    let nonce = CspNonce::new();
    req.extensions_mut().insert(nonce.clone());
    let path = req.uri().path().to_string();
    let mut resp = next.run(req).await;
    h.apply(&nonce.0, &path, resp.headers_mut());
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Targets(Vec<&'static str>);

    impl FormTargets for Targets {
        fn origins(&self) -> Vec<String> {
            self.0.iter().map(ToString::to_string).collect()
        }
    }

    fn headers(public_url: &str) -> Headers {
        let server = config::ServerConfig {
            public_url: public_url.into(),
            ..config::ServerConfig::default()
        };
        Headers::new(&server, None)
    }

    fn html() -> HeaderMap {
        let mut m = HeaderMap::new();
        m.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        m
    }

    #[test]
    fn the_page_policy_names_the_nonce_twice_and_never_eval() {
        let p = headers("https://tracker.example.org").page_policy("abc");
        assert_eq!(p.matches("'nonce-abc'").count(), 2, "{p}");
        assert!(p.contains("script-src 'self' 'nonce-abc' 'wasm-unsafe-eval';"));
        assert!(!p.contains("'unsafe-eval'"), "{p}");
        assert!(p.contains("img-src 'self' data: https:;"), "{p}");
        assert!(p.contains("connect-src 'self';"), "{p}");
        assert!(
            p.ends_with("form-action 'self'; frame-ancestors 'none'"),
            "{p}"
        );
    }

    #[test]
    fn a_plain_http_instance_may_load_pictures_over_http() {
        let p = headers("http://192.168.1.20:3000").page_policy("abc");
        assert!(p.contains("img-src 'self' data: https: http:;"), "{p}");
    }

    #[test]
    fn the_sign_out_form_may_reach_the_providers_and_nothing_malformed() {
        let mut h = headers("https://tracker.example.org");
        h.form_targets = Some(Arc::new(Targets(vec![
            "https://id.example.org",
            "http://127.0.0.1:3012",
            "https://evil.example; script-src *",
            "id.example.org",
            "javascript://x",
        ])));
        let p = h.page_policy("abc");
        assert!(
            p.contains("form-action 'self' https://id.example.org http://127.0.0.1:3012;"),
            "{p}"
        );
        assert!(!p.contains("evil"), "{p}");
    }

    #[test]
    fn the_live_reload_socket_is_allowed_only_when_there_is_one() {
        let mut h = headers("http://localhost:3000");
        assert!(h.page_policy("abc").contains("connect-src 'self';"));
        h.reload_port = Some(3001);
        assert!(
            h.page_policy("abc")
                .contains("connect-src 'self' ws://localhost:3001 ws://127.0.0.1:3001;")
        );
    }

    #[test]
    fn a_document_gets_the_page_policy_and_data_the_closed_one() {
        let h = headers("https://tracker.example.org");
        let mut page = html();
        h.apply("abc", "/library", &mut page);
        assert!(
            page[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("'nonce-abc'")
        );
        assert_eq!(page[header::CACHE_CONTROL], "no-store");
        assert_eq!(page[header::STRICT_TRANSPORT_SECURITY], "max-age=31536000");
        let mut file = HeaderMap::new();
        h.apply("abc", "/pkg/app.wasm", &mut file);
        assert_eq!(file[header::CONTENT_SECURITY_POLICY], CLOSED_POLICY);
        assert!(!file.contains_key(header::CACHE_CONTROL));
        assert_eq!(file[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(file[header::X_FRAME_OPTIONS], "DENY");
        assert_eq!(file[header::REFERRER_POLICY], "same-origin");
        assert!(file.contains_key(&PERMISSIONS_POLICY));
        let mut json = HeaderMap::new();
        h.apply("abc", "/api/v1/auth/me", &mut json);
        assert_eq!(json[header::CACHE_CONTROL], "no-store");
    }

    #[test]
    fn what_a_handler_set_is_kept() {
        let h = headers("https://tracker.example.org");
        let mut m = html();
        m.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=60"),
        );
        m.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'"),
        );
        h.apply("abc", "/", &mut m);
        assert_eq!(m[header::CACHE_CONTROL], "public, max-age=60");
        assert_eq!(m[header::CONTENT_SECURITY_POLICY], "default-src 'none'");
        assert_eq!(m.get_all(header::CONTENT_SECURITY_POLICY).iter().count(), 1);
    }

    #[test]
    fn hsts_needs_https_and_an_age() {
        assert!(headers("http://tracker.example.org").hsts.is_none());
        assert!(headers("HTTPS://tracker.example.org").hsts.is_some());
        let off = config::ServerConfig {
            public_url: "https://tracker.example.org".into(),
            hsts_max_age_secs: 0,
            ..config::ServerConfig::default()
        };
        assert!(Headers::new(&off, None).hsts.is_none());
    }

    #[test]
    fn report_only_moves_the_policy_to_the_other_header() {
        let mut h = headers("https://tracker.example.org");
        h.report_only = true;
        let mut m = html();
        h.apply("abc", "/", &mut m);
        assert!(!m.contains_key(header::CONTENT_SECURITY_POLICY));
        assert!(m[&REPORT_ONLY].to_str().unwrap().contains("'nonce-abc'"));
        assert_eq!(m[header::X_FRAME_OPTIONS], "DENY");
    }

    #[test]
    fn a_nonce_is_128_bits_and_new_each_time() {
        let (a, b) = (CspNonce::new(), CspNonce::new());
        assert_eq!(a.0.len(), 22);
        assert_ne!(a.0, b.0);
    }
}
