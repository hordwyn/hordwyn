//! A plain `HttpFetch` for user-configured targets (notification channels): one attempt, no
//! allowlist, cache or rate limit, no redirects, short timeouts. Unless private targets are
//! allowed, its resolver refuses names that resolve to private addresses, at connect time,
//! so a DNS answer cannot change between a check and the request (G2-7). IP literals do not
//! go through a resolver; callers check those (the notify crate does). Retries are the
//! caller's job.

use std::time::Duration;

use app::ports::{HttpError, HttpFetch, HttpRequest, HttpResponse};
use async_trait::async_trait;

pub struct PlainHttp {
    client: reqwest::Client,
}

/// DNS that only answers with public addresses.
struct PublicOnly;

impl reqwest::dns::Resolve for PublicOnly {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let addrs: Vec<std::net::SocketAddr> =
                tokio::net::lookup_host((name.as_str(), 0)).await?.collect();
            if addrs.iter().any(|a| domain::net::is_private_ip(a.ip())) {
                return Err(format!("{} resolves to a private address", name.as_str()).into());
            }
            let addrs: reqwest::dns::Addrs = Box::new(addrs.into_iter());
            Ok(addrs)
        })
    }
}

impl PlainHttp {
    /// # Panics
    /// If the TLS backend cannot be initialised.
    #[must_use]
    pub fn new(user_agent: &str, allow_private: bool) -> Self {
        let mut b = reqwest::Client::builder();
        if !allow_private {
            b = b.dns_resolver(std::sync::Arc::new(PublicOnly));
        }
        let client = b
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

#[cfg(test)]
mod tests {
    use super::*;
    use app::ports::Lane;

    fn get(url: String) -> HttpRequest {
        HttpRequest {
            connector: "notify".into(),
            op: "test".into(),
            method: "GET".into(),
            url,
            headers: vec![],
            body: None,
            lane: Lane::Interactive,
            skip_auth_record: false,
            user: None,
        }
    }

    #[tokio::test]
    async fn names_of_private_addresses_are_refused_at_connect_time() {
        let s = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(wiremock::ResponseTemplate::new(204))
            .mount(&s)
            .await;
        let port = s.address().port();
        // `localhost` resolves to a loopback address
        let url = format!("http://localhost:{port}/x");
        let strict = PlainHttp::new("test", false);
        assert!(matches!(
            strict.fetch(get(url.clone())).await,
            Err(HttpError::Network(_))
        ));
        let lan = PlainHttp::new("test", true);
        assert_eq!(lan.fetch(get(url)).await.unwrap().status, 204);
    }
}
