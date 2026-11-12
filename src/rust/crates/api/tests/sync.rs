//! M16 acceptance: two offline clients edit the same entry, both converge after syncing;
//! replaying a batch is a no-op. Runs against Postgres (the change feed lives there).

// One scenario on purpose: the steps share the server state of two simulated devices.
#![allow(clippy::too_many_lines)]

use std::sync::Arc;

use api::{ApiState, Naming};
use catalog_core::Registry;
use catalog_core::search::SearchFanout;
use contracts::{SyncBatchDto, SyncCommandDto, SyncCommandEnvelope};
use domain::hlc::Hlc;
use domain::user::{Actor, Role};

/// Wall time of the test clock (2026-01-01T00:00:00Z) in ms.
const T0: u64 = 1_767_225_600_000;
const MIN: u64 = 60_000;

fn hlc(ms: u64, node: u32) -> String {
    Hlc::new(ms, 0, node).to_string()
}

fn cmd(op: &str, c: SyncCommandDto) -> SyncCommandEnvelope {
    SyncCommandEnvelope {
        op_id: op.into(),
        command: c,
    }
}

#[tokio::test]
async fn offline_clients_converge_and_replays_are_no_ops() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let store = Arc::new(store_pg::PgStore::new(pool.clone()));
    t.app.repos = app::Repos {
        users: store.clone(),
        companions: store.clone(),
        link_requests: store.clone(),
        invites: store.clone(),
        catalog: store.clone(),
        library: store.clone(),
        progress: store.clone(),
        reviews: store.clone(),
        lists: store.clone(),
        sessions: store.clone(),
    };
    let user = t
        .app
        .register_user("anna", "Anna", None, Role::Member)
        .await
        .unwrap();
    let a = Actor {
        user_id: user.id,
        role: user.role,
    };
    let item = t
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: vec![],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let s = ApiState {
        app: t.app.clone(),
        auth: auth::Auth::new(
            t.app.clone(),
            Arc::new(auth::MemAuthStore::default()),
            auth::AuthSettings::default(),
        ),
        oidc: None,
        bus: realtime::Bus::local(),
        search: Arc::new(testkit::fakes::MemSearch::default()),
        fanout: SearchFanout::new(
            t.app.clone(),
            Registry::default(),
            Arc::new(testkit::fakes::ReplayHttp::default()),
        ),
        registry: Registry::default(),
        assets: None,
        importer: None,
        notify: None,
        integrations: None,
        public_url: "http://localhost".into(),
        changes: Some(store),
        naming: Naming::from_prefix("app", "App"),
        secure_cookies: true,
        default_kinds: vec![],
        admin: None,
    };
    let id = item.id.0.to_string();
    // both devices start from the same pulled state
    let base = api::sync::pull(&s, a, 0, 500).await.unwrap().cursor;
    let add = |op: &str| {
        cmd(
            op,
            SyncCommandDto::Add {
                item_id: id.clone(),
            },
        )
    };

    // phone (node 1) offline: rates 60 and notes "phone" at T0+1h
    let phone = SyncBatchDto {
        commands: vec![
            add("01900000-0000-7000-8000-000000000001"),
            cmd(
                "01900000-0000-7000-8000-000000000002",
                SyncCommandDto::Rate {
                    item_id: id.clone(),
                    rating: Some(60),
                    hlc: hlc(T0 + 60 * MIN, 1),
                },
            ),
            cmd(
                "01900000-0000-7000-8000-000000000003",
                SyncCommandDto::SetNotes {
                    item_id: id.clone(),
                    notes: Some("phone".into()),
                    hlc: hlc(T0 + 60 * MIN, 1),
                },
            ),
            cmd(
                "01900000-0000-7000-8000-000000000004",
                SyncCommandDto::LogProgress {
                    item_id: id.clone(),
                    progress: serde_json::json!({"type": "finished"}),
                    occurred_at: None,
                    hlc: None,
                },
            ),
        ],
    };
    // laptop (node 2) offline: rates 80 at T0+2h (later), status paused at T0+30min
    let laptop = SyncBatchDto {
        commands: vec![
            add("01900000-0000-7000-8000-000000000011"),
            cmd(
                "01900000-0000-7000-8000-000000000012",
                SyncCommandDto::Rate {
                    item_id: id.clone(),
                    rating: Some(80),
                    hlc: hlc(T0 + 120 * MIN, 2),
                },
            ),
            cmd(
                "01900000-0000-7000-8000-000000000013",
                SyncCommandDto::SetStatus {
                    item_id: id.clone(),
                    status: "paused".into(),
                    hlc: hlc(T0 + 30 * MIN, 2),
                },
            ),
        ],
    };
    // the devices come back online three hours later
    t.clock.advance(chrono::Duration::hours(3));
    // laptop pushes first, then the phone
    assert!(
        api::sync::push(&s, a, laptop.clone())
            .await
            .iter()
            .all(|r| r.ok)
    );
    assert!(
        api::sync::push(&s, a, phone.clone())
            .await
            .iter()
            .all(|r| r.ok)
    );

    // both pull and end up with the same, per-field newest state
    let pulled = api::sync::pull(&s, a, base, 500).await.unwrap();
    let entry = pulled
        .changes
        .iter()
        .rev()
        .find_map(|c| c.entry.clone())
        .expect("entry change");
    assert_eq!(entry.rating, Some(80), "laptop's later rating wins");
    assert_eq!(
        entry.notes.as_deref(),
        Some("phone"),
        "only the phone set notes"
    );
    // status: the phone synced last; its progress is stamped after everything the server
    // has seen, so `completed` beats the laptop's older `paused`
    assert_eq!(entry.status, "completed");
    let again = api::sync::pull(&s, a, base, 500).await.unwrap();
    assert_eq!(again, pulled, "every client sees the same changes");

    // replaying both batches changes nothing: no new change rows, same state
    let cursor = pulled.cursor;
    assert!(api::sync::push(&s, a, phone).await.iter().all(|r| r.ok));
    assert!(api::sync::push(&s, a, laptop).await.iter().all(|r| r.ok));
    let after = api::sync::pull(&s, a, cursor, 500).await.unwrap();
    assert!(
        after.changes.is_empty(),
        "replay produced changes: {:?}",
        after.changes
    );
    assert_eq!(t.app.progress(a, item.id).await.unwrap().len(), 1);

    // bad commands are reported per command and do not stop the batch
    let mixed = SyncBatchDto {
        commands: vec![
            cmd(
                "x",
                SyncCommandDto::Rate {
                    item_id: id.clone(),
                    rating: Some(250),
                    hlc: hlc(T0 + 180 * MIN, 1),
                },
            ),
            cmd(
                "y",
                SyncCommandDto::SetNotes {
                    item_id: id.clone(),
                    notes: None,
                    hlc: hlc(T0 + 180 * MIN, 1),
                },
            ),
        ],
    };
    let r = api::sync::push(&s, a, mixed).await;
    assert!(!r[0].ok && r[1].ok);
    assert_eq!(r[0].error.as_ref().unwrap().code, "bad_request");
}
