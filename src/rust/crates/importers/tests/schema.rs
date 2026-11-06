//! The JSON Schema of the generic import/export row is published in the docs; this test
//! keeps it current (`UPDATE_DOCS=1 cargo test -p importers` rewrites it).

#[test]
fn published_schema_is_current() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/modules/ROOT/attachments/library-export-row.schema.json");
    let want = serde_json::to_string_pretty(&importers::row::row_schema()).unwrap() + "\n";
    if std::env::var_os("UPDATE_DOCS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &want).unwrap();
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        have == want,
        "schema is stale: run `UPDATE_DOCS=1 cargo test -p importers`"
    );
}
