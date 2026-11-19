//! v1-wikidata review WD-2: a search asks the sources in the member's own language; a member
//! who left the language automatic is asked in the language the page shows (the browser's,
//! OT-34 / #4; REST: the first of `Accept-Language`), else in the instance language (the
//! brand's `default_locale`).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, Ctx, RateLimit,
    Registry, Search, SearchRequest, Snapshot,
};
use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

struct LanguageSpy(ConnectorInfo, Arc<Mutex<Vec<Option<String>>>>);

#[async_trait]
impl Search for LanguageSpy {
    async fn search(&self, _: &Ctx, req: &SearchRequest) -> Result<Vec<Snapshot>, ConnectorError> {
        self.1.lock().unwrap().push(req.language.clone());
        Ok(vec![])
    }
}

impl Connector for LanguageSpy {
    fn info(&self) -> &ConnectorInfo {
        &self.0
    }
    fn as_search(&self) -> Option<&dyn Search> {
        Some(self)
    }
}

fn spy(seen: Arc<Mutex<Vec<Option<String>>>>) -> LanguageSpy {
    LanguageSpy(
        ConnectorInfo {
            id: "spy".into(),
            name: "Spy".into(),
            kinds: vec!["movie".into()],
            capabilities: vec![Capability::Search],
            rate_limit: RateLimit {
                per_second: 10,
                burst: 10,
                max_concurrency: 4,
            },
            auth: Auth::None,
            attribution: Attribution {
                text: "Spy".into(),
                url: "https://example.invalid".into(),
                licence: "test".into(),
            },
            hosts: vec![],
            cache_ttl_secs: 0,
            contact: catalog_core::ContactUse::None,
        },
        seen,
    )
}

/// Wait until the spy has seen `n` searches.
async fn seen_after(seen: &Arc<Mutex<Vec<Option<String>>>>, n: usize) -> Vec<Option<String>> {
    let start = Instant::now();
    while seen.lock().unwrap().len() < n && start.elapsed() < Duration::from_secs(20) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    seen.lock().unwrap().clone()
}

#[tokio::test]
async fn a_search_asks_in_the_members_language_else_the_instance_language() {
    let Some(db_url) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let toml = format!(
        "[database]\nurl = \"{db_url}\"\n[auth]\nsignup = \"open\"\nsecure_cookies = false\n\
         [secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n\
         [brand]\ndir = \"{}/../../../../config/brand/default\"\n",
        env!("CARGO_MANIFEST_DIR"),
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut registry = Registry::default();
    registry.register(Arc::new(spy(seen.clone())));
    let srv = Server::build(
        cfg,
        config::ServerRole::All,
        Overrides {
            registry: Some(registry),
            http: Some(Arc::new(testkit::fakes::ReplayHttp::default())),
            search: None,
        },
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = CancellationToken::new();
    let web = {
        let options = leptos::prelude::LeptosOptions::builder()
            .output_name("app")
            .build();
        let router = srv.app_router(&options);
        let stop = stop.clone();
        let bus = srv.bus.clone();
        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    stop.cancelled().await;
                    bus.shutdown();
                })
                .await
                .unwrap();
        })
    };
    let http = reqwest::Client::new();
    let signed = http
        .post(format!("{base}/api/v1/auth/sign-up"))
        .json(&serde_json::json!({
            "username": "lena", "display_name": "Lena", "password": "correct horse battery"
        }))
        .send()
        .await
        .unwrap();
    // the page's session cookie, for the server function
    let cookie = signed.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let signup: serde_json::Value = signed.json().await.unwrap();
    let token = signup["token"].as_str().unwrap().to_string();
    let search = |q: &'static str| {
        http.get(format!("{base}/api/v1/search?q={q}&kind=movie"))
            .bearer_auth(&token)
            .send()
    };

    // automatic: the instance language of the default brand
    assert_eq!(search("alpha").await.unwrap().status(), 200);
    assert_eq!(seen_after(&seen, 1).await, [Some("en".to_string())]);

    // automatic with a German browser (OT-34, #4): the sources are asked in German, through
    // REST (the first language of Accept-Language) and through the page's server function
    // (the locale the page negotiated)
    let r = http
        .get(format!("{base}/api/v1/search?q=gamma&kind=movie"))
        .bearer_auth(&token)
        .header("accept-language", "de-DE,de;q=0.9,en;q=0.8")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(seen_after(&seen, 2).await[1], Some("de".to_string()));
    let r = http
        .post(format!(
            "{base}{}",
            <web::api::Search as leptos::server_fn::ServerFn>::PATH
        ))
        .header("cookie", &cookie)
        .header("accept-language", "de-AT,de;q=0.9")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "application/json")
        .body("q=delta&kind=movie")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap_or_default());
    assert_eq!(seen_after(&seen, 3).await[2], Some("de".to_string()));
    // a browser language the UI does not have: the page shows the instance language
    let r = http
        .post(format!(
            "{base}{}",
            <web::api::Search as leptos::server_fn::ServerFn>::PATH
        ))
        .header("cookie", &cookie)
        .header("accept-language", "xx")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "application/json")
        .body("q=epsilon&kind=movie")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(seen_after(&seen, 4).await[3], Some("en".to_string()));

    // chosen: German
    let r = http
        .put(format!("{base}/api/v1/auth/me/locale"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"locale": "de"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(search("beta").await.unwrap().status(), 200);
    assert_eq!(seen_after(&seen, 5).await[4], Some("de".to_string()));

    stop.cancel();
    web.await.unwrap();
}
