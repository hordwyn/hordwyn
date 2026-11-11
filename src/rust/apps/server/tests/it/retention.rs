//! DEC-453 (TMDB terms 1.C): the hourly `catalog.retention` job on the background lane purges
//! TMDB data older than `jobs.catalog_max_age_days` when no refresh is possible (here: no TMDB
//! connector), through the real worker and Postgres.

use std::sync::Arc;
use std::time::Duration;

use app::NewCatalogItem;
use app::ports::{JobQueue, Lane, NewJob};
use domain::catalog::{EntityState, ExternalId};
use domain::kind::KindId;
use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

async fn start() -> Option<Server> {
    let db_url = store_pg::testing::fresh_db_url().await?;
    let toml = format!(
        "[database]\nurl = \"{db_url}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n\
         [jobs]\ncatalog_max_age_days = 30\n[brand]\ndir = \"{}/../../../../config/brand/default\"\n",
        env!("CARGO_MANIFEST_DIR"),
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    Some(
        Server::build(
            cfg,
            config::ServerRole::All,
            Overrides {
                registry: Some(catalog_core::Registry::default()),
                http: Some(Arc::new(testkit::fakes::ReplayHttp::default())),
                search: None,
            },
        )
        .await
        .unwrap(),
    )
}

async fn movie(s: &Server, title: &str) -> domain::ids::ItemId {
    s.app
        .upsert_catalog_item(NewCatalogItem {
            kind: KindId::new("movie").unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(1995),
            description: Some("From TMDB.".into()),
            external_ids: vec![ExternalId::new("tmdb", title)],
            attributes: serde_json::json!({}),
            cover_url: Some("https://image.tmdb.org/t/p/w780/a.jpg".into()),
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn the_retention_job_purges_old_tmdb_data_without_a_source() {
    let Some(s) = start().await else {
        return;
    };
    s.schedule_admin_jobs().await;
    let (cron, lane): (String, i16) = sqlx::query_as(
        "SELECT schedule, lane FROM cron_schedules WHERE name = 'catalog.retention'",
    )
    .fetch_one(&s.pool)
    .await
    .unwrap();
    assert_eq!(cron, "41 * * * *");
    assert_eq!(lane, 2, "the background_refresh lane");

    let old = movie(&s, "Heat").await;
    let young = movie(&s, "Ronin").await;
    // UX4-9: an item in a library is purged, one nobody refers to is deleted
    let unowned = movie(&s, "Thief").await;
    let u = s
        .app
        .register_user("alice", "Alice", None, domain::user::Role::Member)
        .await
        .unwrap();
    let me = domain::user::Actor {
        user_id: u.id,
        role: u.role,
    };
    s.app.add_to_library(me, old, None).await.unwrap();
    sqlx::query(
        "UPDATE catalog_items SET provider_data_at = now() - interval '31 days'
         WHERE id = ANY($1)",
    )
    .bind(vec![old.0, unowned.0])
    .execute(&s.pool)
    .await
    .unwrap();
    s.queue
        .enqueue(NewJob {
            lane: Lane::BackgroundRefresh,
            kind: catalog_core::retention::RETENTION_JOB.into(),
            payload: serde_json::json!({}),
            dedupe_key: None,
            priority: 0,
            run_at: None,
        })
        .await
        .unwrap();
    let stop = CancellationToken::new();
    let worker = tokio::spawn(s.worker().run(stop.clone()));
    let mut item = s.app.catalog_item(old).await.unwrap();
    for _ in 0..100 {
        item = s.app.catalog_item(old).await.unwrap();
        if item.state == EntityState::Stub && s.app.catalog_item(unowned).await.is_err() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stop.cancel();
    let _ = worker.await;
    assert_eq!(item.state, EntityState::Stub);
    assert!(item.title_unavailable());
    assert_eq!(item.year, None);
    assert!(s.app.catalog_item(unowned).await.is_err(), "deleted");
    assert_eq!(item.description, None);
    assert_eq!(item.cover_url, None);
    let kept = s.app.catalog_item(young).await.unwrap();
    assert_eq!(kept.description.as_deref(), Some("From TMDB."));
}
