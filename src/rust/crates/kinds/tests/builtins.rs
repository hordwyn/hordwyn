use app::ports::KindCatalog;
use domain::kind::{KindId, ProgressModel};
use kinds::Registry;
use serde_json::json;

fn id(s: &str) -> KindId {
    KindId::new(s).unwrap()
}

#[test]
fn all_twelve_kinds_with_their_progress_models() {
    let r = Registry::with_builtins();
    let expected = [
        ("movie", ProgressModel::Single),
        ("show", ProgressModel::Episodic),
        ("anime", ProgressModel::Episodic),
        ("podcast", ProgressModel::Episodic),
        ("book", ProgressModel::Paged),
        ("manga", ProgressModel::Paged),
        ("comic", ProgressModel::Paged),
        ("audiobook", ProgressModel::Timed),
        ("music", ProgressModel::Timed),
        ("video_game", ProgressModel::Sessions),
        ("visual_novel", ProgressModel::Sessions),
        ("board_game", ProgressModel::Sessions),
    ];
    assert_eq!(r.all().len(), expected.len());
    for (k, model) in expected {
        assert_eq!(r.get(&id(k)).unwrap().progress_model, model, "{k}");
    }
}

#[test]
fn validation_is_typed_and_strict() {
    let r = Registry::with_builtins();
    let ok = r
        .validate_attributes(
            &id("board_game"),
            &json!({"players_min": 2, "players_max": 4, "mechanics": ["Tile Placement"], "weight": 1.8}),
        )
        .unwrap();
    assert_eq!(ok["players_max"], 4);
    assert!(ok.get("bgg_id").is_none(), "empty fields are omitted");

    assert!(
        r.validate_attributes(&id("board_game"), &json!({"players_min": "two"}))
            .is_err()
    );
    assert!(
        r.validate_attributes(&id("movie"), &json!({"colour": "red"}))
            .is_err()
    );
    assert!(
        r.validate_attributes(&id("movie"), &json!({"release_date": "1999-03-31"}))
            .is_ok()
    );
    assert!(
        r.validate_attributes(&id("movie"), &json!({"release_date": "yesterday"}))
            .is_err()
    );
    assert!(r.validate_attributes(&id("nope"), &json!({})).is_err());
}

#[test]
fn descriptor_schema_and_facets() {
    let r = Registry::with_builtins();
    let d = r.get(&id("board_game")).unwrap();
    assert_eq!(d.attributes_schema["type"], "object");
    assert!(d.attributes_schema["properties"]["players_best"].is_object());
    assert!(d.facets.contains(&"mechanics".to_string()));
    assert_eq!(d.default_connectors, vec!["bgg".to_string()]);
    assert!(
        r.get(&id("video_game"))
            .unwrap()
            .extra_statuses
            .contains(&"endless".to_string())
    );
}

/// The generated JSON Schemas are committed under `schemas/` for the UI and the docs.
/// Run with `UPDATE_SCHEMAS=1` to regenerate.
#[test]
fn committed_schemas_are_up_to_date() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let update = std::env::var_os("UPDATE_SCHEMAS").is_some();
    for d in Registry::with_builtins().all() {
        let path = dir.join(format!("{}.json", d.id));
        let generated = serde_json::to_string_pretty(&d.attributes_schema).unwrap() + "\n";
        if update {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &generated).unwrap();
        }
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{} missing; run with UPDATE_SCHEMAS=1", path.display()));
        assert_eq!(
            committed,
            generated,
            "{} is stale; run with UPDATE_SCHEMAS=1",
            path.display()
        );
    }
}

#[test]
fn every_builtin_kind_is_in_the_group_of_its_module() {
    use domain::kind::KindGroup;
    let r = Registry::with_builtins();
    let expected = [
        ("movie", KindGroup::Watch),
        ("show", KindGroup::Watch),
        ("anime", KindGroup::Watch),
        ("book", KindGroup::Read),
        ("manga", KindGroup::Read),
        ("comic", KindGroup::Read),
        ("audiobook", KindGroup::Listen),
        ("podcast", KindGroup::Listen),
        ("music", KindGroup::Listen),
        ("video_game", KindGroup::Play),
        ("visual_novel", KindGroup::Play),
        ("board_game", KindGroup::Play),
    ];
    for (k, group) in expected {
        assert_eq!(r.get(&id(k)).unwrap().group, group, "{k}");
    }
}
