//! The data of the upgrade seed (`crates/store-pg/tests/fixtures/upgrade-seed.sql`, REL-3):
//! a small household instance written through the server's API and use-cases, so that every
//! table an instance holds data in has rows (the store-pg test `the_seed_has_rows_in_every_
//! table` names a table without). The next release applies the seed to this release's schema
//! and migrates it (`cargo xtask upgrade-test`).
//!
//! Ignored: run it with `KEEP_TEST_DB=1` and `--nocapture`, then `cargo xtask upgrade-seed
//! <the printed database URL>` writes the seed file from it (see the seed's header).
//!
//! The values the upgrade test reads back stay the same in every release's seed: alice's
//! rating 80 of "Heat", the note "signed" of a list item, the play of "Azul" won with 61
//! points, the year 2001 of "Heat".

// the three steps read as scripts, one call after the other; splitting them buys nothing
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::sync::Arc;

use domain::list::Visibility;
use domain::progress::Progress;
use domain::session::Participant;
use domain::user::{Actor, Role};
use serde_json::{Value, json};
use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

struct T {
    base: String,
    http: reqwest::Client,
    srv: Server,
    cfg: config::AppConfig,
}

impl T {
    async fn call(&self, method: &str, path: &str, token: &str, body: Option<Value>) -> Value {
        let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        let mut req = self
            .http
            .request(method.clone(), format!("{}/api/v1{path}", self.base))
            .bearer_auth(token);
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await.unwrap();
        let status = r.status().as_u16();
        let text = r.text().await.unwrap_or_default();
        assert!(status < 300, "{method} {path}: {status} {text}");
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }

    async fn sign_up(&self, name: &str) -> (String, Actor) {
        let v: Value = self
            .http
            .post(format!("{}/api/v1/auth/sign-up", self.base))
            .json(&json!({"username": name, "display_name": name,
                "password": "correct horse battery"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let token = v["token"]
            .as_str()
            .unwrap_or_else(|| panic!("sign-up {name}: {v}"))
            .to_string();
        let me = self.call("GET", "/auth/me", &token, None).await;
        let id: uuid::Uuid = me["id"]
            .as_str()
            .unwrap_or_else(|| panic!("/auth/me: {me}"))
            .parse()
            .unwrap();
        let role = if me["role"] == "admin" {
            Role::Admin
        } else {
            Role::Member
        };
        (
            token,
            Actor {
                user_id: domain::ids::UserId(id),
                role,
            },
        )
    }
}

async fn start(db: &str, dir: &std::path::Path) -> T {
    // a brand of its own: the notification texts in the seed name the instance, and the
    // white-label guard keeps the product name out of the tree
    let brand = dir.join("brand");
    std::fs::create_dir_all(&brand).unwrap();
    std::fs::write(
        brand.join("brand.toml"),
        "name = \"Shelf\"\nshort_name = \"Shelf\"\ndefault_locale = \"en\"\n",
    )
    .unwrap();
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[auth]\nsignup = \"open\"\nsecure_cookies = false\n\
         [secrets]\nkey_file = \"{}\"\n[storage]\nbackend = \"fs\"\nfs_root = \"{}\"\n\
         [search]\nbackend = \"postgres\"\n[usage]\nfunnel = true\n\
         [server]\npublic_url = \"https://shelf.example.org\"\n[brand]\ndir = \"{}\"\n",
        dir.join("secrets.key").display(),
        dir.join("blobs").display(),
        brand.display(),
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(
        cfg.clone(),
        config::ServerRole::All,
        Overrides {
            registry: None,
            http: Some(Arc::new(testkit::fakes::ReplayHttp::default())),
            search: None,
        },
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = srv.base_router();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    T {
        base,
        http: reqwest::Client::new(),
        srv,
        cfg,
    }
}

fn totp_code(secret_base32: &str) -> String {
    let secret = totp_rs::Secret::try_from_base32(secret_base32).unwrap();
    totp_rs::Builder::new()
        .with_secret(secret)
        .with_account_name("root")
        .build()
        .unwrap()
        .generate_current()
        .to_string()
}

async fn item(
    app: &app::App,
    kind: &str,
    title: &str,
    attributes: Value,
) -> domain::catalog::CatalogItem {
    app.upsert_catalog_item(app::NewCatalogItem {
        kind: domain::kind::KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2001),
        description: Some(format!("{title}, a {kind}")),
        external_ids: vec![domain::catalog::ExternalId::new("test", title)],
        attributes,
        cover_url: None,
        popularity: Some(10.0),
    })
    .await
    .unwrap()
}

/// Accounts and what an admin keeps on the instance: sign-in with a password and an
/// authenticator, an invite, a disabled account, a password reset link, an SSO provider
/// with a linked identity, a data source key, a switch and a licence, settings, a template.
async fn instance(t: &T) -> (String, String, Actor, Actor) {
    let (root, _) = t.sign_up("root").await;
    // the admin hears about sign-ups on a webhook: the sign-ups below leave deliveries
    let hook = t
        .call(
            "POST",
            "/notifications/channels",
            &root,
            Some(json!({"name": "Admin hook",
                "config": {"kind": "webhook", "url": "http://127.0.0.1:9/admin", "secret": ""}})),
        )
        .await;
    t.call(
        "PUT",
        "/notifications/subscriptions",
        &root,
        Some(json!({"event": "admin.signup",
            "targets": {"type": "only", "ids": [hook["id"]]}})),
    )
    .await;
    let (alice, a) = t.sign_up("alice").await;
    let (bob, b) = t.sign_up("bob").await;
    let (_, dave) = t.sign_up("dave").await;

    let enrol = t.call("POST", "/auth/totp", &root, None).await;
    let code = totp_code(enrol["secret_base32"].as_str().unwrap());
    t.call(
        "POST",
        "/auth/totp/confirm",
        &root,
        Some(json!({"code": code})),
    )
    .await;

    t.call(
        "PUT",
        "/admin/sources/tmdb/credential",
        &root,
        Some(json!({"fields": {"api_key": "k-1"}})),
    )
    .await;
    t.call(
        "PUT",
        "/admin/sources/igdb/enabled",
        &root,
        Some(json!({"enabled": false})),
    )
    .await;
    t.call(
        "PUT",
        "/admin/sources/tmdb/licence",
        &root,
        Some(json!({"agreement_on_file": true})),
    )
    .await;
    t.call(
        "PUT",
        "/admin/settings",
        &root,
        Some(
            json!({"values": {"mail.host": "mail.example.org", "mail.port": 587,
            "instance.hosting": "home"}}),
        ),
    )
    .await;
    t.call(
        "POST",
        "/admin/invites",
        &root,
        Some(json!({"email": "new@example.org"})),
    )
    .await;
    t.call(
        "PUT",
        &format!("/admin/users/{}/disabled", dave.user_id),
        &root,
        Some(json!({"disabled": true})),
    )
    .await;
    let provider = t
        .call(
            "POST",
            "/admin/oidc/providers",
            &root,
            Some(json!({"slug": "pocket", "display_name": "Pocket ID",
                "issuer": "https://id.example.org", "client_id": "shelf",
                "client_secret": "s3cret"})),
        )
        .await;
    t.call(
        "PUT",
        "/notifications/templates",
        &root,
        Some(json!({"locale": "de", "key": "admin-signup-title",
            "text": "Hallo { $username }"})),
    )
    .await;
    t.call("POST", "/admin/secrets/backup-confirmed", &root, None)
        .await;
    t.call(
        "PUT",
        "/me/credentials/tvdb",
        &bob,
        Some(json!({"fields": {"pin": "5678"}})),
    )
    .await;

    // an identity of the SSO provider, and the ID token of a sign-in through it
    let admin = t.srv.api.admin.as_ref().unwrap();
    let store = auth::pg::PgAuthStore::new(t.srv.pool.clone(), admin.cipher.clone());
    let provider_id: uuid::Uuid = provider["id"]
        .as_str()
        .or_else(|| provider["provider"]["id"].as_str())
        .unwrap_or_else(|| panic!("provider: {provider}"))
        .parse()
        .unwrap();
    auth::AuthStore::link_identity(
        &store,
        &auth::OidcIdentity {
            id: uuid::Uuid::now_v7(),
            issuer: "https://id.example.org".into(),
            subject: "alice-at-pocket".into(),
            user_id: a.user_id,
            provider_id: Some(provider_id),
            sid: Some("sid-1".into()),
            linked_at: chrono::Utc::now(),
            last_claims_hash: Some("claims".into()),
            last_groups: Some(vec!["family".into()]),
        },
    )
    .await
    .unwrap();
    let token_hash: String =
        sqlx::query_scalar("SELECT token_hash FROM auth_sessions WHERE user_id = $1 LIMIT 1")
            .bind(a.user_id.0)
            .fetch_one(&t.srv.pool)
            .await
            .unwrap();
    auth::AuthStore::set_session_secret(
        &store,
        &token_hash,
        auth::SessionSecret::IdToken,
        Some("eyJhbGciOiJub25lIn0.e30."),
    )
    .await
    .unwrap();
    // a one-time password reset link (the shell escape hatch, audited as `cli`)
    server::escape::Escape::open(&t.cfg)
        .await
        .unwrap()
        .reset_password("bob")
        .await
        .unwrap();
    (alice, bob, a, b)
}

/// The library of the seed of v1.0.0 (the upgrade test reads these values back), and what
/// came after: tags, episodes, a watched show, a goal, a game group, a link request, an
/// invite of a person, a title match.
async fn library(t: &T, alice: Actor, bob: Actor) {
    let app = &t.srv.app;
    let movie = item(app, "movie", "Heat", json!({"genres": ["crime"]})).await;
    let show = item(
        app,
        "show",
        "The Wire",
        json!({"genres": ["drama"], "seasons": [{"number": 1, "episode_count": 2}]}),
    )
    .await;
    let book = item(
        app,
        "book",
        "The Dispossessed",
        json!({"authors": ["Ursula K. Le Guin"], "page_count": 387}),
    )
    .await;
    let game = item(
        app,
        "board_game",
        "Azul",
        json!({"players_min": 2, "players_max": 4, "mechanics": ["tile placement"]}),
    )
    .await;
    for i in [movie.id, show.id, book.id, game.id] {
        app.add_to_library(alice, i, None).await.unwrap();
    }
    app.add_to_library(bob, movie.id, None).await.unwrap();
    app.log_progress(alice, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();
    app.log_progress(bob, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();
    app.rate(
        alice,
        movie.id,
        Some(domain::library::Rating::new(80).unwrap()),
        None,
    )
    .await
    .unwrap();
    app.write_review(alice, movie.id, "Tense.", false, None)
        .await
        .unwrap();
    app.set_notes(alice, movie.id, Some("with dad".into()), None)
        .await
        .unwrap();
    app.add_tag(alice, book.id, "to buy").await.unwrap();
    let list = app
        .create_list(alice, "Owned", None, Visibility::Private)
        .await
        .unwrap();
    app.add_to_list(alice, list.id, game.id, None)
        .await
        .unwrap();
    app.add_to_list(alice, list.id, book.id, Some("signed".into()))
        .await
        .unwrap();
    let me: domain::user::User = app::ports::UserRepo::get(&*app.repos.users, alice.user_id)
        .await
        .unwrap()
        .unwrap();
    let anna = app.create_companion(alice, "Anna").await.unwrap();
    let mut winner = Participant::new(me.self_companion);
    winner.winner = true;
    winner.score = Some(61.0);
    app.log_session(
        alice,
        app::NewSession::new(game.id, vec![winner, Participant::new(anna.id)]),
    )
    .await
    .unwrap();
    let jonas = app.create_companion(alice, "Jonas").await.unwrap();
    app.create_game_group(
        alice,
        app::GameGroupInput {
            name: "Fridays".into(),
            members: vec![anna.id, jonas.id],
        },
    )
    .await
    .unwrap();
    app.set_goal(
        alice,
        app::NewGoal {
            year: 2026,
            group: Some(domain::kind::KindGroup::Watch),
            metric: domain::goal::GoalMetric::Completions,
            target: 12,
        },
    )
    .await
    .unwrap();
    app.store_episodes(
        show.id,
        &[app::SeasonFetch {
            key: app::SeasonKey {
                number: 1,
                episode_count: 2,
                air_date: None,
            },
            connector: "tmdb".into(),
            episodes: (1..=2)
                .map(|n| domain::catalog::CatalogEpisode {
                    season: 1,
                    number: n,
                    air_date: None,
                    runtime_minutes: Some(58),
                })
                .collect(),
        }],
    )
    .await
    .unwrap();
    app.watch_item(alice, show.id).await.unwrap();
    let ben = app.create_companion(alice, "Bob at work").await.unwrap();
    app.request_link(alice, ben.id, bob.user_id).await.unwrap();
    let carla = app.create_companion(alice, "Carla").await.unwrap();
    app.promote_companion(alice, carla.id, Some("carla@example.org".into()))
        .await
        .unwrap();
    store_pg::PgStore::new(t.srv.pool.clone())
        .log_catalog_match(&book, "openlibrary", "OL1W")
        .await
        .unwrap();
}

/// What a member keeps besides the library: a notification channel and a subscription, an
/// integration, an import, the usage beacon; and what the instance counts and caches on
/// its own (search documents, statistics, job schedules, source counters, probes).
async fn the_rest(t: &T, alice_token: &str, bob_token: &str) {
    let hook = t
        .call(
            "POST",
            "/notifications/channels",
            alice_token,
            Some(json!({"name": "Automation",
                "config": {"kind": "webhook", "url": "http://127.0.0.1:9/in", "secret": ""}})),
        )
        .await;
    let hook_id = hook["id"].as_str().unwrap().to_string();
    let events = t
        .call("GET", "/notifications/events", alice_token, None)
        .await;
    let event = events[0]["id"].as_str().unwrap().to_string();
    t.call(
        "PUT",
        "/notifications/subscriptions",
        alice_token,
        Some(json!({"event": event, "targets": {"type": "only", "ids": [hook_id]}})),
    )
    .await;
    t.call(
        "POST",
        "/integrations",
        alice_token,
        Some(json!({"name": "Home script"})),
    )
    .await;
    let bytes = std::fs::read(repo(
        "src/rust/crates/testkit/fixtures/import/legacy/export.json",
    ))
    .unwrap();
    // alice tries an export as a dry run, bob imports his for real (the rows of an import)
    for (token, dry_run) in [(alice_token, true), (bob_token, false)] {
        let r = t
            .http
            .post(format!(
                "{}/api/v1/imports?format=legacy-json&dry_run={dry_run}",
                t.base
            ))
            .bearer_auth(token)
            .body(bytes.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), 202);
        let import: Value = r.json().await.unwrap();
        let importer = t.srv.api.importer.as_ref().unwrap();
        importer
            .run(import["id"].as_str().unwrap().parse().unwrap())
            .await
            .unwrap();
    }
    t.call(
        "POST",
        "/usage/funnel",
        alice_token,
        Some(
            json!({"events": [{"flow": "next_step", "outcome": "logged", "taps": "1",
            "time": "lt2s"}]}),
        ),
    )
    .await;
    t.call("GET", "/stats/totals", alice_token, None).await;
    t.call("GET", "/stats/compare", alice_token, None).await;

    // what the worker and the source client keep on their own
    t.srv.schedule_admin_jobs().await;
    server::escape::index_now(&t.srv.pool, t.srv.search.clone())
        .await
        .unwrap();
    let mut hours = BTreeMap::new();
    hours.insert(chrono::Utc::now().timestamp() / 3600, (3_u64, 1_u64));
    let mut deltas = std::collections::HashMap::new();
    deltas.insert(
        "tmdb".to_string(),
        catalog_core::ConnectorStats {
            hours,
            marks: catalog_core::StatsMarks {
                last_ok_at: Some(chrono::Utc::now()),
                ..Default::default()
            },
        },
    );
    catalog_core::StatsStore::add(
        &catalog_core::pg::PgSourceStats::new(t.srv.pool.clone()),
        &deltas,
    )
    .await
    .unwrap();
    catalog_core::pg::PgPace::new(t.srv.pool.clone())
        .reserve("openlibrary", std::time::Duration::from_secs(1))
        .await
        .unwrap();
    let admin = t.srv.api.admin.as_ref().unwrap();
    auth::admin::AdminStore::put_probes(
        &*admin.store,
        &[auth::admin::ProbeRow {
            id: "tmdb".into(),
            status: "ok".into(),
            detail: "200".into(),
            args: BTreeMap::new(),
            checked_at: chrono::Utc::now(),
        }],
    )
    .await
    .unwrap();
    assets::index::ObjectIndex::bump_ops(
        &assets::pg::PgObjectIndex::new(t.srv.pool.clone()),
        chrono::Utc::now().date_naive(),
        assets::index::OpCounts {
            puts: 1,
            gets: 2,
            deletes: 0,
            egress_bytes: 1024,
        },
    )
    .await
    .unwrap();
    // a cover picture of a data source, registered for download
    t.srv
        .assets
        .register(
            assets::AssetKind::Cover,
            "https://image.tmdb.org/t/p/w500/heat.jpg",
            "tmdb",
            assets::FetchPriority::Library,
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "writes the data of the upgrade seed; see the module"]
async fn write_the_upgrade_seed() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("seed-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let t = start(&db, &dir).await;
    let (alice_token, bob_token, alice, bob) = instance(&t).await;
    library(&t, alice, bob).await;
    the_rest(&t, &alice_token, &bob_token).await;
    let empty: Vec<String> = store_pg::schema::contents(&t.srv.pool)
        .await
        .unwrap()
        .lines()
        .filter(|l| l.split(' ').nth(1) == Some("0"))
        .map(|l| l.split(' ').next().unwrap_or_default().to_string())
        .collect();
    println!("tables without rows: {empty:?}");
    println!("seed database: {db} (kept only with KEEP_TEST_DB=1)");
    std::fs::remove_dir_all(&dir).unwrap();
}
