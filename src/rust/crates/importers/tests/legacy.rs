//! Fails until the legacy importer exists (KICKOFF Q4): it needs the owner's export at
//! `crates/testkit/fixtures/import/legacy/export.json`.

#[test]
#[ignore = "legacy export fixture not supplied yet (DEC-117)"]
fn legacy_export_imports() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../testkit/fixtures/import/legacy/export.json");
    assert!(path.exists(), "owner export missing at {}", path.display());
    let rows: Vec<_> = importers::legacy::parse(std::fs::read(path).unwrap()).collect();
    assert!(
        rows.iter().all(Result::is_ok),
        "legacy importer not implemented"
    );
}
