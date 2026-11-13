#![cfg(feature = "http")]
//! HTTP channels against a local mock server: request shape per protocol
//! (`docs/research/notification-channels.md`) and the mapping of answers to retry / permanent /
//! gone.

use std::sync::Arc;

use app::ports::{HttpError, HttpFetch, HttpRequest, HttpResponse};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use notify::channels::http::{HttpChannels, sign};
use notify::model::ChannelConfig;
use notify::{Message, SendError};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn msg() -> Message {
    Message {
        id: uuid::Uuid::from_u128(42),
        event: "release.episode".into(),
        title: "New episode".into(),
        body: "S1E2 is out.".into(),
        link: Some("https://shelf.example.org/items/1".into()),
        at: "2026-01-10T10:00:00Z".parse::<DateTime<Utc>>().unwrap(),
    }
}

/// Real requests (the server uses `catalog_core::PlainHttp`, which is the same thing).
struct Reqwest(reqwest::Client);

#[async_trait]
impl HttpFetch for Reqwest {
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let mut rb = self.0.request(req.method.parse().unwrap(), &req.url);
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        let resp = rb
            .body(req.body.unwrap_or_default())
            .send()
            .await
            .map_err(|e| HttpError::Network(e.to_string()))?;
        Ok(HttpResponse {
            status: resp.status().as_u16(),
            headers: vec![],
            body: resp.bytes().await.unwrap().to_vec(),
        })
    }
}

fn channels(allow_private: bool) -> HttpChannels {
    with_rule(domain::net::PrivateAllow::fixed(allow_private))
}

fn with_rule(rule: domain::net::PrivateAllow) -> HttpChannels {
    HttpChannels::new(Arc::new(Reqwest(reqwest::Client::new())), rule)
}

fn lan() -> HttpChannels {
    // the mock server listens on localhost
    channels(true)
}

#[tokio::test]
async fn ntfy_publishes_json_with_bearer_token() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .and(header("authorization", "Bearer tk_1"))
        .and(body_json(serde_json::json!({
            "topic": "alerts", "title": "New episode", "message": "S1E2 is out.",
            "click": "https://shelf.example.org/items/1",
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&s)
        .await;
    let c = ChannelConfig::Ntfy {
        server: s.uri(),
        topic: "alerts".into(),
        token: Some("tk_1".into()),
    };
    lan().send(&c, &msg()).await.unwrap();
}

#[tokio::test]
async fn gotify_uses_the_app_token_header_and_click_extra() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/gotify/message"))
        .and(header("x-gotify-key", "app-token"))
        .and(body_json(serde_json::json!({
            "title": "New episode", "message": "S1E2 is out.", "priority": 5,
            "extras": { "client::notification": { "click": { "url": "https://shelf.example.org/items/1" } } },
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&s)
        .await;
    let c = ChannelConfig::Gotify {
        server: format!("{}/gotify/", s.uri()),
        token: "app-token".into(),
    };
    lan().send(&c, &msg()).await.unwrap();
}

#[tokio::test]
async fn discord_rate_limits_are_retried_and_content_is_capped() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/webhooks/1/tok"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(serde_json::json!({ "retry_after": 1.5, "global": false })),
        )
        .mount(&s)
        .await;
    let c = ChannelConfig::Discord {
        webhook_url: format!("{}/api/webhooks/1/tok", s.uri()),
    };
    let mut m = msg();
    m.body = "x".repeat(5000);
    assert!(matches!(lan().send(&c, &m).await, Err(SendError::Retry(_))));
    let req = &s.received_requests().await.unwrap()[0];
    let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(body["content"].as_str().unwrap().chars().count(), 2000);
}

#[tokio::test]
async fn matrix_puts_an_m_text_event_with_the_message_id_as_txn() {
    let s = MockServer::start().await;
    let txn = uuid::Uuid::from_u128(42).to_string();
    Mock::given(method("PUT"))
        .and(path(format!(
            "/_matrix/client/v3/rooms/!room:example.org/send/m.room.message/{txn}"
        )))
        .and(header("authorization", "Bearer mx"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"event_id": "$1"})),
        )
        .expect(1)
        .mount(&s)
        .await;
    let c = ChannelConfig::Matrix {
        homeserver: s.uri(),
        room_id: "!room:example.org".into(),
        access_token: "mx".into(),
    };
    lan().send(&c, &msg()).await.unwrap();
    let req = &s.received_requests().await.unwrap()[0];
    let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(body["msgtype"], "m.text");
    assert!(
        body["body"]
            .as_str()
            .unwrap()
            .starts_with("New episode\nS1E2")
    );
}

#[tokio::test]
async fn apprise_uses_the_stateless_notify_endpoint() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/notify/"))
        .and(body_json(serde_json::json!({
            "urls": "tgram://bot/chat", "title": "New episode",
            "body": "S1E2 is out.\nhttps://shelf.example.org/items/1", "type": "info",
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&s)
        .await;
    let c = ChannelConfig::Apprise {
        server: s.uri(),
        urls: "tgram://bot/chat".into(),
    };
    lan().send(&c, &msg()).await.unwrap();
}

#[tokio::test]
async fn webhooks_are_signed_and_410_means_gone() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(204))
        .up_to_n_times(1)
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(410))
        .mount(&s)
        .await;
    let c = ChannelConfig::Webhook {
        url: format!("{}/hook", s.uri()),
        secret: "whsec_c2VjcmV0LWtleS1ieXRlcy0wMTIzNDU2Nzg5".into(),
    };
    lan().send(&c, &msg()).await.unwrap();
    let req: &Request = &s.received_requests().await.unwrap()[0];
    let h = |n: &str| req.headers.get(n).unwrap().to_str().unwrap().to_string();
    assert_eq!(h("webhook-id"), uuid::Uuid::from_u128(42).to_string());
    assert_eq!(h("webhook-timestamp"), "1768039200");
    let expected = sign(
        "whsec_c2VjcmV0LWtleS1ieXRlcy0wMTIzNDU2Nzg5",
        &h("webhook-id"),
        1_768_039_200,
        &req.body,
    );
    assert_eq!(h("webhook-signature"), expected);
    let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(body["type"], "release.episode");
    assert_eq!(body["data"]["title"], "New episode");
    assert!(matches!(
        lan().send(&c, &msg()).await,
        Err(SendError::Gone(_))
    ));
}

#[tokio::test]
async fn private_targets_are_refused_unless_allowed() {
    let s = MockServer::start().await;
    let c = ChannelConfig::Ntfy {
        server: s.uri(),
        topic: "t".into(),
        token: None,
    };
    let err = channels(false).send(&c, &msg()).await.unwrap_err();
    assert!(
        matches!(err, SendError::Permanent(ref m) if m.contains("private")),
        "{err:?}"
    );
    assert_eq!(s.received_requests().await.unwrap().len(), 0);
    let bad = ChannelConfig::Ntfy {
        server: "file:///etc/passwd".into(),
        topic: "t".into(),
        token: None,
    };
    assert!(matches!(
        channels(true).send(&bad, &msg()).await,
        Err(SendError::Permanent(_))
    ));
}

/// Review SEC2-3: one LAN target on `outbound.allow_private_hosts` is called while private
/// targets stay refused for every other host and port.
#[tokio::test]
async fn a_private_target_on_the_operator_list_is_called() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&s)
        .await;
    let port = s.address().port();
    let listed = with_rule(domain::net::PrivateAllow::new(
        Arc::new(|| false),
        domain::net::PrivateHosts::new([format!("127.0.0.1:{port}")]),
    ));
    let c = |host: &str| ChannelConfig::Ntfy {
        server: format!("http://{host}:{port}"),
        topic: "t".into(),
        token: None,
    };
    listed.send(&c("127.0.0.1"), &msg()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap().len(), 1);
    // `localhost` is another host name: not listed
    let err = listed.send(&c("localhost"), &msg()).await.unwrap_err();
    assert!(
        matches!(err, SendError::Permanent(ref m) if m.contains("private")),
        "{err:?}"
    );
    assert_eq!(s.received_requests().await.unwrap().len(), 1);
}
