#![cfg(feature = "generic")]
//! v1-manual scope 5: the generic format keeps the picture address a person typed for an
//! item entered by hand (`cover_url`), in JSON Lines and CSV. It comes back with an item no
//! source answers for when the importing person may set one (the rule the importer is given:
//! everybody on a home instance, admins on a hosted one); the server never fetches it. An
//! address that is refused or not valid leaves the picture out with a row warning, never the
//! row (review MAN-3), and the dry run says the same.

use std::sync::Arc;

use app::ports::JobQueue;
use domain::user::{Actor, Role};
use importers::{ImportRow, ImportState, Importer, MemImportStore};
use testkit::TestEnv;
use testkit::fakes::MemBlobs;

const FILE: &str = r#"{"format":"library-export","version":1}
{"source_id":"g1","kind":"board_game","title":"Zine of the Marsh","year":2019,"status":"planned","cover_url":"https://img.example/box.jpg"}
{"source_id":"g2","kind":"board_game","title":"No picture","year":2018}
{"source_id":"m1","kind":"movie","title":"Heat","external_ids":{"tmdb":"949"},"cover_url":"https://img.example/heat.jpg"}
{"source_id":"g3","kind":"board_game","title":"Bad picture","cover_url":"http://img.example/plain.jpg"}
"#;

/// Where the import runs: a home instance (everybody may set a picture address) or a hosted
/// one (admins only).
#[derive(Clone, Copy)]
enum Mode {
    Home,
    Hosted,
}

async fn run(mode: Mode, role: Role, dry_run: bool) -> (TestEnv, Actor, importers::ImportRecord) {
    let t = TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let user = t
        .app
        .register_user("anna", "Anna", None, role)
        .await
        .unwrap();
    let actor = Actor {
        user_id: user.id,
        role: user.role,
    };
    let importer = Importer::new(
        t.app.clone(),
        Arc::new(MemImportStore::default()),
        Arc::new(MemBlobs::default()),
        t.jobs.clone() as Arc<dyn JobQueue>,
    )
    .with_cover_rule(Arc::new(move |a: Actor| match mode {
        Mode::Home => true,
        Mode::Hosted => a.is_admin(),
    }));
    let r = importer
        .start(actor, "generic-jsonl", FILE.as_bytes().to_vec(), dry_run)
        .await
        .unwrap();
    let r = importer.run(r.id).await.unwrap();
    (t, actor, r)
}

async fn item_titled(t: &TestEnv, actor: Actor, title: &str) -> domain::catalog::CatalogItem {
    for entry in t.app.library(actor).await.unwrap() {
        let item = t.app.catalog_item(entry.item_id).await.unwrap();
        if item.title == title {
            return item;
        }
    }
    panic!("`{title}` is not in the library");
}

/// The source ids and messages of the row warnings.
fn warnings(r: &importers::ImportRecord) -> Vec<(String, String)> {
    r.warnings
        .iter()
        .map(|w| (w.source_id.clone().unwrap_or_default(), w.message.clone()))
        .collect()
}

#[tokio::test]
async fn a_members_import_on_a_home_instance_brings_the_picture_address_back() {
    // as the dialog: on a home instance every member may set one
    let (t, actor, r) = run(Mode::Home, Role::Member, false).await;
    assert_eq!(r.state, ImportState::Done);
    assert_eq!(
        r.imported, 4,
        "every row, also the bad address: {:?}",
        r.errors
    );
    assert_eq!(r.errors.len(), 0, "{:?}", r.errors);
    let w = warnings(&r);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].0, "g3");
    assert!(w[0].1.starts_with("picture address not taken: "), "{w:?}");
    assert!(w[0].1.contains("https://"), "the reason: {w:?}");

    let marsh = item_titled(&t, actor, "Zine of the Marsh").await;
    assert_eq!(
        marsh.cover_url.as_deref(),
        Some("https://img.example/box.jpg")
    );
    assert_eq!(marsh.provenance["cover_url"], app::USER_PROVENANCE);
    assert_eq!(marsh.provenance["title"], app::USER_PROVENANCE);
    assert_eq!(marsh.year, Some(2019));
    assert_eq!(marsh.external_ids.len(), 0);
    // the row of the bad address is there, without a picture
    let bad = item_titled(&t, actor, "Bad picture").await;
    assert_eq!(bad.cover_url, None);
    // an item a source answers for never takes an address from a file (G2-2)
    let heat = item_titled(&t, actor, "Heat").await;
    assert_eq!(heat.cover_url, None);
    // no refresh is queued for the items without ids
    let refreshes: Vec<_> = t
        .jobs
        .jobs()
        .into_iter()
        .filter(|j| j.kind == app::REFRESH_JOB)
        .collect();
    assert_eq!(refreshes.len(), 1, "only for the item with a source id");
}

#[tokio::test]
async fn a_members_import_on_a_hosted_instance_leaves_the_address_out_and_says_so() {
    let (t, actor, r) = run(Mode::Hosted, Role::Member, false).await;
    assert_eq!(r.imported, 4, "{:?}", r.errors);
    assert_eq!(r.errors.len(), 0, "{:?}", r.errors);
    let w = warnings(&r);
    assert_eq!(
        w.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        ["g1", "g3"],
        "{w:?}"
    );
    assert!(w[0].1.contains("only an admin"), "{w:?}");
    let marsh = item_titled(&t, actor, "Zine of the Marsh").await;
    assert_eq!(marsh.cover_url, None);
    assert_eq!(marsh.year, Some(2019));
}

#[tokio::test]
async fn an_admins_import_on_a_hosted_instance_keeps_the_address_and_the_row_of_a_bad_one() {
    let (t, actor, r) = run(Mode::Hosted, Role::Admin, false).await;
    assert_eq!(r.imported, 4, "{:?}", r.errors);
    assert_eq!(r.errors.len(), 0, "{:?}", r.errors);
    assert_eq!(warnings(&r).len(), 1, "g3 only: {:?}", r.warnings);
    let marsh = item_titled(&t, actor, "Zine of the Marsh").await;
    assert_eq!(
        marsh.cover_url.as_deref(),
        Some("https://img.example/box.jpg")
    );
}

#[tokio::test]
async fn the_dry_run_reports_what_the_real_run_will_leave_out() {
    let (t, actor, r) = run(Mode::Hosted, Role::Member, true).await;
    assert_eq!(r.imported, 4, "{:?}", r.errors);
    assert_eq!(
        warnings(&r)
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ["g1", "g3"]
    );
    assert!(t.app.library(actor).await.unwrap().is_empty(), "a dry run");
}

#[tokio::test]
async fn an_item_without_ids_comes_back_as_the_persons_own() {
    // an export and an import keep the provenance of an item entered by hand, also without a
    // picture: a later source match does not overwrite its title and year (review MAN-3)
    let (t, actor, _) = run(Mode::Home, Role::Member, false).await;
    let plain = item_titled(&t, actor, "No picture").await;
    assert_eq!(plain.cover_url, None);
    assert_eq!(plain.provenance["title"], app::USER_PROVENANCE);
    assert_eq!(plain.provenance["year"], app::USER_PROVENANCE);
    assert_eq!(plain.state, domain::catalog::EntityState::Full);
    // an item of a source keeps the source's way
    let heat = item_titled(&t, actor, "Heat").await;
    assert!(heat.provenance.is_empty(), "{:?}", heat.provenance);
}

fn row(cover: Option<&str>) -> ImportRow {
    ImportRow {
        source_id: "s1".into(),
        kind: "board_game".into(),
        title: "Zine of the Marsh".into(),
        year: Some(2019),
        external_ids: std::collections::BTreeMap::new(),
        status: Some("planned".into()),
        rating: None,
        notes: None,
        history: Vec::new(),
        lists: Vec::new(),
        cover_url: cover.map(Into::into),
    }
}

#[test]
fn a_generic_row_with_cover_url_round_trips() {
    let with = row(Some("https://img.example/box.jpg"));
    // JSON Lines
    let line = serde_json::to_string(&with).unwrap();
    assert!(line.contains(r#""cover_url":"https://img.example/box.jpg""#));
    let text = format!("{}\n{line}\n", importers::generic::header_line());
    let back: Vec<_> = importers::generic::jsonl(std::io::Cursor::new(text)).collect();
    assert_eq!(back[0].as_ref().unwrap(), &with);
    // a row without one writes no field, and a file without the field still reads
    let without = serde_json::to_string(&row(None)).unwrap();
    assert!(!without.contains("cover_url"), "{without}");
    // CSV: a column at the end, optional when read
    let csv = importers::generic::csv_text(std::slice::from_ref(&with));
    assert!(
        csv.starts_with(
            "source_id,kind,title,year,status,rating,notes,external_ids,finished_dates,lists,cover_url\n"
        ),
        "{csv}"
    );
    let back: Vec<_> = importers::generic::csv(std::io::Cursor::new(csv)).collect();
    assert_eq!(back[0].as_ref().unwrap(), &with);
    let old = "source_id,kind,title,year,status,rating,notes,external_ids,finished_dates,lists\n\
               s1,board_game,Zine of the Marsh,2019,planned,,,,,\n";
    let back: Vec<_> = importers::generic::csv(std::io::Cursor::new(old.to_string())).collect();
    assert_eq!(back[0].as_ref().unwrap(), &row(None));
}
