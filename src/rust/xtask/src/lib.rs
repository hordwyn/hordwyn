//! Repository automation and repository-level checks (architecture rule, white-label).
#![deny(unsafe_code)]

pub mod arch;
pub mod e2e_log;
pub mod load;
pub mod locales;
pub mod ops;
pub mod record;
pub mod services;
pub mod upgrade;
pub mod whitelabel;

use std::path::{Path, PathBuf};

/// Root of the cargo workspace (`src/rust`).
///
/// # Panics
/// Never in practice: xtask is always built from inside the workspace.
#[must_use]
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the workspace")
        .to_path_buf()
}

/// Root of the git repository (two levels above the workspace).
///
/// # Panics
/// If the workspace is not located at `<repo>/src/rust`.
#[must_use]
pub fn repo_root() -> PathBuf {
    workspace_root()
        .parent()
        .and_then(Path::parent)
        .expect("workspace lives at <repo>/src/rust")
        .to_path_buf()
}
