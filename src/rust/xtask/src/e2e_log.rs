//! The server log of an e2e run (UX3-25 panic guard, DEC-384): Playwright starts the server
//! with its output in a file outside the repo; a `panicked at` line anywhere fails the run.

/// Each panic in `log`: its `panicked at` line and the message line after it.
#[must_use]
pub fn panics(log: &str) -> Vec<String> {
    let lines: Vec<&str> = log.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("panicked at"))
        .map(|(i, l)| match lines.get(i + 1) {
            Some(next) if !next.contains("panicked at") => format!("{l}\n{next}"),
            _ => (*l).to_string(),
        })
        .collect()
}

/// What [`settle`] found in a server log.
#[derive(Debug)]
pub struct Settled {
    pub lines: usize,
    pub panics: Vec<String>,
    /// The file was left in place (a panic was found or the tests failed).
    pub kept: bool,
}

/// Scan the server log at `path` for panics; delete it only when the run was clean
/// (`tests_passed` and no panic), so a failure keeps something to attach (UX4-4).
///
/// A missing or empty log is an error, never a clean scan: the server writes lines as soon as
/// it starts, so no lines means its output was not captured (`E2E_SERVER_LOG` not passed to
/// the server, a server reused from an earlier start, a wrong path), and "0 panics" would let
/// the guard pass without having looked at anything.
///
/// # Errors
/// `NotFound` when the file is missing, `InvalidData` when it is empty (the file is kept);
/// any error reading it; a clean file that cannot be removed.
pub fn settle(path: &std::path::Path, tests_passed: bool) -> std::io::Result<Settled> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "the server log was not written, so the server output was not captured \
                 (is E2E_SERVER_LOG passed to the server, and was no old server reused?)",
            ));
        }
        Err(e) => return Err(e),
    };
    if text.trim().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the server log is empty, so the server output was not captured and there is \
             nothing to scan for panics",
        ));
    }
    let found = panics(&text);
    let kept = !tests_passed || !found.is_empty();
    if !kept && path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(Settled {
        lines: text.lines().count(),
        panics: found,
        kept,
    })
}
