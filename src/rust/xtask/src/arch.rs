//! Dependency rule from DESIGN.md D-3, checked against `cargo metadata`.
//!
//! The checker works on a small [`Graph`] so the rules themselves can be unit tested with
//! hand-built graphs; [`Graph::from_metadata`] builds the real one.

use std::collections::{BTreeMap, BTreeSet};

use cargo_metadata::{DependencyKind, Metadata, PackageId};

/// Third-party crates that `domain` and `app` must not depend on, directly or transitively.
pub const FORBIDDEN_IN_CORE: &[&str] = &[
    "sqlx",
    "axum",
    "leptos",
    "reqwest",
    "opendal",
    "meilisearch-sdk",
    "wasmtime",
];

/// Extra crates forbidden in `domain` only (no IO, no async runtime).
pub const FORBIDDEN_IN_DOMAIN: &[&str] = &["tokio", "async-trait"];

/// Allowed workspace dependencies per workspace crate. Crates not listed are unrestricted
/// (adapters and apps may depend on anything below them).
pub const ALLOWED_WORKSPACE_DEPS: &[(&str, &[&str])] = &[
    ("domain", &[]),
    ("app", &["domain"]),
    ("contracts", &[]),
    ("ui", &[]),
    // DEC-006: web may read the brand config in addition to ui + contracts (+ app under ssr).
    ("web", &["ui", "contracts", "brand", "app"]),
];

/// Workspace dependencies that are only allowed behind a feature: (crate, dep, feature).
pub const FEATURE_GATED_DEPS: &[(&str, &str, &str)] = &[("web", "app", "ssr")];

/// One normal (non-dev, non-build) dependency edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub to: String,
    pub optional: bool,
}

/// Simplified dependency graph of the workspace.
#[derive(Debug, Default, Clone)]
pub struct Graph {
    /// Names of workspace members.
    pub members: BTreeSet<String>,
    /// Direct normal dependencies per package name (all packages, not just members).
    pub deps: BTreeMap<String, Vec<Edge>>,
    /// For each member: feature name -> list of feature entries (as in Cargo.toml).
    pub features: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl Graph {
    /// Build the graph from `cargo metadata` output.
    #[must_use]
    pub fn from_metadata(meta: &Metadata) -> Self {
        let mut g = Graph::default();
        let by_id: BTreeMap<&PackageId, &cargo_metadata::Package> =
            meta.packages.iter().map(|p| (&p.id, p)).collect();
        for id in &meta.workspace_members {
            let p = by_id[id];
            g.members.insert(p.name.to_string());
            g.features.insert(
                p.name.to_string(),
                p.features
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            );
        }
        let Some(resolve) = &meta.resolve else {
            return g;
        };
        for node in &resolve.nodes {
            let pkg = by_id[&node.id];
            let mut edges = Vec::new();
            for d in &node.deps {
                let normal = d.dep_kinds.iter().any(|k| k.kind == DependencyKind::Normal);
                if !normal {
                    continue;
                }
                let target = by_id[&d.pkg].name.to_string();
                let optional = pkg
                    .dependencies
                    .iter()
                    .filter(|dep| dep.kind == DependencyKind::Normal)
                    .any(|dep| dep.name == target && dep.optional);
                edges.push(Edge {
                    to: target,
                    optional,
                });
            }
            g.deps.insert(pkg.name.to_string(), edges);
        }
        g
    }

    /// All packages reachable from `root` through normal dependencies (excluding root).
    #[must_use]
    pub fn transitive(&self, root: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![root.to_string()];
        while let Some(n) = stack.pop() {
            for e in self.deps.get(&n).into_iter().flatten() {
                if seen.insert(e.to.clone()) {
                    stack.push(e.to.clone());
                }
            }
        }
        seen
    }
}

/// Check every rule and return human-readable violations (empty = OK).
#[must_use]
pub fn check(g: &Graph) -> Vec<String> {
    let mut v = Vec::new();

    for core in ["domain", "app"] {
        if !g.members.contains(core) {
            v.push(format!("workspace member `{core}` is missing"));
            continue;
        }
        let reach = g.transitive(core);
        let forbidden = FORBIDDEN_IN_CORE.iter().chain(if core == "domain" {
            FORBIDDEN_IN_DOMAIN.iter()
        } else {
            [].iter()
        });
        for f in forbidden {
            if reach.contains(*f) {
                v.push(format!(
                    "`{core}` depends (transitively) on forbidden crate `{f}`"
                ));
            }
        }
    }

    for (krate, allowed) in ALLOWED_WORKSPACE_DEPS {
        for e in g.deps.get(*krate).into_iter().flatten() {
            if g.members.contains(&e.to) && !allowed.contains(&e.to.as_str()) {
                v.push(format!(
                    "`{krate}` must not depend on workspace crate `{}`",
                    e.to
                ));
            }
        }
    }

    for (krate, dep, feature) in FEATURE_GATED_DEPS {
        let Some(edge) = g
            .deps
            .get(*krate)
            .into_iter()
            .flatten()
            .find(|e| e.to == *dep)
        else {
            continue;
        };
        let gated_by_feature = g
            .features
            .get(*krate)
            .and_then(|f| f.get(*feature))
            .is_some_and(|entries| {
                entries
                    .iter()
                    .any(|e| e == &format!("dep:{dep}") || e == dep)
            });
        let enabled_elsewhere = g.features.get(*krate).is_some_and(|f| {
            f.iter()
                .filter(|(name, _)| name != feature)
                .any(|(_, entries)| {
                    entries
                        .iter()
                        .any(|e| e == &format!("dep:{dep}") || e == dep)
                })
        });
        if !edge.optional || !gated_by_feature || enabled_elsewhere {
            v.push(format!(
                "`{krate}` may depend on `{dep}` only as an optional dependency enabled by feature `{feature}`"
            ));
        }
    }

    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(edges: &[(&str, &str, bool)], members: &[&str]) -> Graph {
        let mut g = Graph {
            members: members.iter().map(ToString::to_string).collect(),
            ..Graph::default()
        };
        for (from, to, optional) in edges {
            g.deps.entry((*from).to_string()).or_default().push(Edge {
                to: (*to).to_string(),
                optional: *optional,
            });
        }
        g
    }

    const MEMBERS: &[&str] = &["domain", "app", "ui", "web", "contracts", "store-pg"];

    #[test]
    fn clean_graph_passes() {
        let g = graph(
            &[("app", "domain", false), ("store-pg", "sqlx", false)],
            MEMBERS,
        );
        assert!(check(&g).is_empty(), "{:?}", check(&g));
    }

    #[test]
    fn transitive_forbidden_crate_in_app_is_caught() {
        let g = graph(
            &[
                ("app", "domain", false),
                ("domain", "helper", false),
                ("helper", "reqwest", false),
            ],
            MEMBERS,
        );
        let v = check(&g);
        assert!(
            v.iter()
                .any(|m| m.contains("`domain`") && m.contains("reqwest"))
        );
        assert!(
            v.iter()
                .any(|m| m.contains("`app`") && m.contains("reqwest"))
        );
    }

    #[test]
    fn tokio_in_domain_is_caught() {
        let g = graph(&[("domain", "tokio", false)], MEMBERS);
        assert!(check(&g).iter().any(|m| m.contains("tokio")));
    }

    #[test]
    fn app_depending_on_adapter_is_caught() {
        let g = graph(&[("app", "store-pg", false)], MEMBERS);
        assert!(
            check(&g)
                .iter()
                .any(|m| m.contains("`app` must not depend on workspace crate `store-pg`"))
        );
    }

    #[test]
    fn ui_with_workspace_dep_is_caught() {
        let g = graph(&[("ui", "contracts", false)], MEMBERS);
        assert_ne!(check(&g).len(), 0);
    }

    #[test]
    fn web_app_dep_must_be_ssr_gated() {
        let mut g = graph(&[("web", "app", false)], MEMBERS);
        assert!(!check(&g).is_empty(), "non-optional app dep must fail");

        g = graph(&[("web", "app", true)], MEMBERS);
        g.features.insert(
            "web".into(),
            BTreeMap::from([("ssr".to_string(), vec!["dep:app".to_string()])]),
        );
        assert!(check(&g).is_empty(), "{:?}", check(&g));

        g.features
            .get_mut("web")
            .unwrap()
            .insert("hydrate".into(), vec!["dep:app".into()]);
        assert!(!check(&g).is_empty(), "app enabled by hydrate must fail");
    }
}
