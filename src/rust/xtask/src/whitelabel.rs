//! White-label guard: the codename must not appear in code (KICKOFF rule 9).

use std::fs;
use std::path::{Path, PathBuf};

/// The codename, assembled at runtime so this file does not match itself.
#[must_use]
pub fn codename() -> String {
    ["kith", "keep"].concat()
}

/// Paths (relative to the repo root, `/`-separated) that may contain the codename.
pub const ALLOWED_PREFIXES: &[&str] = &[
    "config/brand/",
    "docs/",
    ".git/",
    "README.md",
    "LICENSE",
    "flake.lock",
];

/// Directory names that are never scanned (build output, dependencies).
pub const SKIPPED_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".direnv",
    "test-results",
    "playwright-report",
    "pkg",
];

/// Return every file under `root` that contains the codename (case-insensitive) outside the
/// allowed locations.
#[must_use]
pub fn offenders(root: &Path) -> Vec<PathBuf> {
    let needle = codename();
    let mut out = Vec::new();
    walk(root, root, &needle, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, needle: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if ALLOWED_PREFIXES
            .iter()
            .any(|p| rel == p.trim_end_matches('/') || rel.starts_with(p))
        {
            continue;
        }
        let name = entry.file_name();
        if path.is_dir() {
            if SKIPPED_DIRS.iter().any(|d| name == *d) {
                continue;
            }
            walk(root, &path, needle, out);
        } else if let Ok(bytes) = fs::read(&path)
            && (String::from_utf8_lossy(&bytes)
                .to_lowercase()
                .contains(needle)
                || rel.to_lowercase().contains(needle))
        {
            out.push(PathBuf::from(rel));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_codename_outside_allowed_paths() {
        let dir = std::env::temp_dir().join(format!("wl-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("config/brand/default")).unwrap();
        fs::write(
            dir.join("src/a.rs"),
            format!("const N: &str = \"{}\";", codename().to_uppercase()),
        )
        .unwrap();
        fs::write(dir.join("config/brand/default/brand.toml"), codename()).unwrap();
        fs::write(dir.join("src/ok.rs"), "fn main() {}").unwrap();
        let found = offenders(&dir);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(found, vec![PathBuf::from("src/a.rs")]);
    }
}
