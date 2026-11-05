//! Enforces the dependency rule from DESIGN.md D-3 on the real workspace.

#[test]
fn workspace_follows_dependency_rule() {
    let meta = cargo_metadata::MetadataCommand::new()
        .manifest_path(xtask::workspace_root().join("Cargo.toml"))
        .exec()
        .expect("cargo metadata");
    let graph = xtask::arch::Graph::from_metadata(&meta);
    let violations = xtask::arch::check(&graph);
    assert!(
        violations.is_empty(),
        "dependency rule violated:\n{}",
        violations.join("\n")
    );
}
