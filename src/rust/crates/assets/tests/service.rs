//! Asset service: fetch through the `HttpFetch` port, process, serve, redirect (D-7, D-8).

use std::io::Cursor;
use std::sync::Arc;

use app::ports::Lane;
use assets::{
    AssetKind, AssetService, Backend, ByteSize, CpuPool, FetchPriority, MemAssetRepo,
    MemObjectIndex, NewObject, ObjectClass, ObjectIndex, Served, StorageBudget, StorageConfig,
    Tier, WriteOutcome,
};
use testkit::fakes::{FixedClock, MemJobs, RecordingEvents, ReplayHttp};

const URL: &str = "https://image.tmdb.org/t/p/w780/poster.png";

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([
            u8::try_from(x % 256).unwrap(),
            u8::try_from(y % 256).unwrap(),
            90,
            255,
        ])
    });
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

struct Env {
    svc: AssetService,
    index: Arc<MemObjectIndex>,
    jobs: Arc<MemJobs>,
    http: Arc<ReplayHttp>,
}

fn env(max: u64) -> Env {
    let cfg = StorageConfig {
        backend: Backend::Memory,
        max_total_bytes: ByteSize(max),
        http_cache_max_bytes: ByteSize(max / 4),
        ..StorageConfig::default()
    };
    let index = Arc::new(MemObjectIndex::default());
    let jobs = Arc::new(MemJobs::default());
    let http = Arc::new(ReplayHttp::default());
    http.record("GET", URL, 200, png(900, 1350));
    let budget = StorageBudget::open(
        cfg,
        index.clone(),
        Arc::new(FixedClock::default()),
        Arc::new(RecordingEvents::default()),
    )
    .unwrap();
    let svc = AssetService::new(
        Arc::new(budget),
        Arc::new(MemAssetRepo::default()),
        http.clone(),
        jobs.clone(),
        CpuPool::new(1).unwrap(),
    );
    Env {
        svc,
        index,
        jobs,
        http,
    }
}

#[tokio::test]
async fn register_queues_a_fetch_in_the_lane_of_its_priority() {
    let e = env(50 << 20);
    let a = e
        .svc
        .register(AssetKind::Cover, URL, "tmdb", FetchPriority::Library)
        .await
        .unwrap();
    let jobs = e.jobs.jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].kind, assets::FETCH_JOB);
    assert_eq!(jobs[0].lane, Lane::BackgroundRefresh);
    // idempotent per URL
    let again = e
        .svc
        .register(AssetKind::Cover, URL, "tmdb", FetchPriority::Library)
        .await
        .unwrap();
    assert_eq!(a.id, again.id);
    assert!(e.http.requests().is_empty(), "register never downloads");
}

#[tokio::test]
async fn library_policy_does_not_prefetch_catalog_images() {
    let e = env(50 << 20);
    e.svc
        .register(AssetKind::Cover, URL, "tmdb", FetchPriority::Catalog)
        .await
        .unwrap();
    assert!(
        e.jobs.jobs().is_empty(),
        "catalog images are not prefetched"
    );
}

#[tokio::test]
async fn fetched_assets_are_served_as_variants() {
    let e = env(50 << 20);
    let a = e
        .svc
        .register(AssetKind::Cover, URL, "tmdb", FetchPriority::Visible)
        .await
        .unwrap();
    let a = e.svc.fetch(a.id).await.unwrap();
    let req = &e.http.requests()[0];
    assert_eq!(
        req.connector, "tmdb",
        "download goes through the connector's policy"
    );
    assert_eq!(req.lane, Lane::Interactive);
    let hash = a.hash.clone().unwrap();
    assert!(a.thumbhash.is_some() && a.dominant_color.is_some());
    assert_eq!(
        a.path(assets::Variant::Card, assets::Format::Webp).unwrap(),
        format!("/assets/{hash}/card.webp")
    );
    match e.svc.serve(&hash, "card", "webp").await.unwrap() {
        Served::Bytes { mime, bytes } => {
            assert_eq!(mime, "image/webp");
            assert!(!bytes.is_empty(), "variant bytes");
        }
        other => panic!("expected bytes, got {other:?}"),
    }
    // not configured by default
    assert_eq!(
        e.svc.serve(&hash, "full", "webp").await.unwrap(),
        Served::NotFound
    );
    assert_eq!(
        e.svc.serve("nope", "card", "webp").await.unwrap(),
        Served::NotFound
    );
}

#[tokio::test]
async fn at_hard_cap_nothing_is_written_and_requests_redirect() {
    let e = env(10_000);
    // fill the budget with pinned data
    let filled = e
        .svc
        .budget()
        .write(
            NewObject {
                key: "up/x".into(),
                class: ObjectClass::Upload,
                tier: Tier::Pinned,
                owner: None,
                asset: None,
                priority: FetchPriority::Visible,
            },
            vec![0; 9_990],
        )
        .await
        .unwrap();
    assert_eq!(filled, WriteOutcome::Stored);
    let before = e.index.usage().await.unwrap();

    let a = e
        .svc
        .register(AssetKind::Cover, URL, "tmdb", FetchPriority::Visible)
        .await
        .unwrap();
    let a = e.svc.fetch(a.id).await.unwrap();
    assert_eq!(e.index.usage().await.unwrap(), before, "no bytes written");
    let hash = a.hash.unwrap();
    e.jobs.drain();
    assert_eq!(
        e.svc.serve(&hash, "card", "webp").await.unwrap(),
        Served::Redirect(URL.into())
    );
    assert!(e.svc.budget().at_hard_cap());
    assert!(
        e.jobs.jobs().is_empty(),
        "no regeneration storm while the budget is full"
    );
}

#[tokio::test]
async fn missing_variants_redirect_and_queue_regeneration() {
    let e = env(50 << 20);
    let a = e
        .svc
        .register(AssetKind::Cover, URL, "tmdb", FetchPriority::Visible)
        .await
        .unwrap();
    let a = e.svc.fetch(a.id).await.unwrap();
    let hash = a.hash.unwrap();
    let keys: Vec<String> = e.index.all().into_iter().map(|o| o.key).collect();
    e.svc.budget().remove(&keys).await.unwrap();
    e.jobs.drain();
    assert_eq!(
        e.svc.serve(&hash, "thumb", "avif").await.unwrap(),
        Served::Redirect(URL.into())
    );
    let jobs = e.jobs.jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].lane, Lane::Interactive);
}

#[tokio::test]
async fn pinning_moves_variants_out_of_the_cache_tier() {
    let e = env(50 << 20);
    let a = e
        .svc
        .register(AssetKind::Cover, URL, "tmdb", FetchPriority::Visible)
        .await
        .unwrap();
    e.svc.fetch(a.id).await.unwrap();
    assert!(e.index.all().iter().all(|o| o.tier == Tier::Cache));
    e.svc.pin(a.id).await.unwrap();
    assert!(e.index.all().iter().all(|o| o.tier == Tier::Pinned));
    e.svc.unpin(a.id).await.unwrap();
    assert!(e.index.all().iter().all(|o| o.tier == Tier::Cache));
}

struct BlockingHttp;

#[async_trait::async_trait]
impl app::ports::HttpFetch for BlockingHttp {
    async fn fetch(
        &self,
        _req: app::ports::HttpRequest,
    ) -> Result<app::ports::HttpResponse, app::ports::HttpError> {
        Err(app::ports::HttpError::Blocked("host not allowed".into()))
    }
}

#[tokio::test]
async fn blocked_image_hosts_fail_permanently_and_keep_redirecting() {
    let budget = StorageBudget::open(
        StorageConfig {
            backend: Backend::Memory,
            ..StorageConfig::default()
        },
        Arc::new(MemObjectIndex::default()),
        Arc::new(FixedClock::default()),
        Arc::new(RecordingEvents::default()),
    )
    .unwrap();
    let svc = AssetService::new(
        Arc::new(budget),
        Arc::new(MemAssetRepo::default()),
        Arc::new(BlockingHttp),
        Arc::new(MemJobs::default()),
        CpuPool::new(1).unwrap(),
    );
    let a = svc
        .register(
            AssetKind::Cover,
            URL,
            "podcastindex",
            FetchPriority::Visible,
        )
        .await
        .unwrap();
    let a = svc
        .fetch(a.id)
        .await
        .expect("not an error: the job must not retry");
    assert!(a.failed);
    assert!(a.hash.is_none());
}
