//! A plain `HttpFetch` for user-configured targets (notification channels): one attempt, no
//! allowlist, cache or rate limit, no redirects, short timeouts. Unless private targets are
//! allowed (as a whole, or for this host on the operator's list, review SEC2-3), its
//! resolver refuses names that resolve to private addresses, at connect time, so a DNS
//! answer cannot change between a check and the request (G2-7). IP literals do not go
//! through a resolver; callers check those (the notify crate does). Proxy settings of the
//! environment are not used (review SEC2-9). Retries are the caller's job.

use std::time::Duration;

use app::ports::{HttpError, HttpFetch, HttpRequest, HttpResponse};
use async_trait::async_trait;
use domain::net::PrivateAllow;

pub struct PlainHttp {
    /// Resolves names to public addresses only.
    client: reqwest::Client,
    /// Resolves names normally: for hosts [`PlainHttp::allow_private`] allows.
    lan_client: reqwest::Client,
    allow_private: PrivateAllow,
}

impl PlainHttp {
    /// # Panics
    /// If the TLS backend cannot be initialised.
    #[must_use]
    pub fn new(user_agent: &str, allow_private: PrivateAllow) -> Self {
        let builder = || {
            reqwest::Client::builder()
                .user_agent(user_agent)
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
        };
        Self {
            client: builder()
                .dns_resolver(std::sync::Arc::new(crate::resolve::PublicOnly))
                .build()
                .expect("http client"),
            lan_client: builder().build().expect("http client"),
            allow_private,
        }
    }

    fn client_for(&self, url: &str) -> &reqwest::Client {
        let allowed = url::Url::parse(url).is_ok_and(|u| {
            u.host_str()
                .is_some_and(|h| self.allow_private.host(h, u.port()))
        });
        if allowed {
            &self.lan_client
        } else {
            &self.client
        }
    }
}

#[async_trait]
impl HttpFetch for PlainHttp {
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let method =
            reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut rb = self.client_for(&req.url).request(method, &req.url);
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
    use domain::net::{PrivateAllow, PrivateHosts};

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
        let strict = PlainHttp::new("test", PrivateAllow::fixed(false));
        assert!(matches!(
            strict.fetch(get(url.clone())).await,
            Err(HttpError::Network(_))
        ));
        let lan = PlainHttp::new("test", PrivateAllow::fixed(true));
        assert_eq!(lan.fetch(get(url.clone())).await.unwrap().status, 204);
        // review SEC2-3: private targets refused, but this host and port on the operator's
        // list: the name may resolve to a private address
        let listed = PlainHttp::new(
            "test",
            PrivateAllow::new(
                std::sync::Arc::new(|| false),
                PrivateHosts::new([format!("localhost:{port}")]),
            ),
        );
        assert_eq!(listed.fetch(get(url)).await.unwrap().status, 204);
        // the same name on another port is not listed
        let s2 = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(wiremock::ResponseTemplate::new(204))
            .mount(&s2)
            .await;
        let other = format!("http://localhost:{}/x", s2.address().port());
        assert!(matches!(
            listed.fetch(get(other)).await,
            Err(HttpError::Network(_))
        ));
    }
}
