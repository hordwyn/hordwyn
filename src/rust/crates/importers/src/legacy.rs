//! Legacy export of the owner's current library (D-16, KICKOFF Q4).
//!
//! The format may only be derived from the owner's export file at
//! `crates/testkit/fixtures/import/legacy/export.json` (rule 6). That file is not in the
//! repository yet, so this importer is a stub: every file is reported as one row error.
//! The ignored test in `tests/legacy.rs` fails until the importer is written (DEC-117).

use crate::row::{ImportRow, RowError};

pub const FORMAT: &str = "legacy-json";

/// Not implemented: reports the whole file as one error.
pub fn parse(_bytes: Vec<u8>) -> Box<dyn Iterator<Item = Result<ImportRow, RowError>>> {
    Box::new(std::iter::once(Err(RowError {
        line: 1,
        source_id: None,
        message: "the legacy importer is not available yet (owner export fixture missing)".into(),
    })))
}
