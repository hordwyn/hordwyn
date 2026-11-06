//! A plain `HttpFetch` for user-configured targets (notification channels): one attempt, no
//! allowlist, cache or rate limit, no redirects, short timeouts. Callers check targets
//! themselves (the notify crate refuses private addresses). Retries are the caller's job.

use std::time::Duration;

use app::ports::{HttpError, HttpFetch, HttpRequest, HttpResponse};
use async_trait::async_trait;

pub struct PlainHttp {
    client: reqwest::Client,
}

impl PlainHttp {
    /// # Panics
    /// If the TLS backend cannot be initialised.
    #[must_use]
    pub fn new(user_agent: &str) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("http client");
        Self { client }
    }
}

#[async_trait]
impl HttpFetch for PlainHttp {
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let method =
            reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut rb = self.client.request(method, &req.url);
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        if let Some(b) = req.body {
            rb = rb.body(b);
        }
        let resp = rb
            .send()
            .await
            .map_err(|e| HttpError::Network(e.without_url().to_string()))?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
            .collect();
        let body = resp
            .bytes()
            .await
            .map_err(|e| HttpError::Network(e.without_url().to_string()))?
            .to_vec();
        tracing::debug!(connector = %req.connector, op = %req.op, status, "plain http");
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}
