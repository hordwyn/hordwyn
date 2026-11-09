//! UX3-25 panic guard (DEC-384, WP4-8): `cargo xtask e2e` fails when the server log of the run
//! has a `panicked at` line, and names it with the lines that follow (the message).

use xtask::e2e_log::panics;

#[test]
fn panics_are_found_with_their_message() {
    let log = "2026-11-08T10:00:00Z  INFO server: listening\n\
               thread 'tokio-runtime-worker' panicked at /reg/leptos/src/transition.rs:58:9:\n\
               Tried to access a reactive value that has already been disposed.\n\
               note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n\
               2026-11-08T10:00:01Z  INFO server: GET /library 200\n";
    assert_eq!(
        panics(log),
        vec![
            "thread 'tokio-runtime-worker' panicked at /reg/leptos/src/transition.rs:58:9:\n\
             Tried to access a reactive value that has already been disposed."
                .to_string()
        ]
    );
}

#[test]
fn a_clean_log_has_none() {
    let log = "INFO server: listening\nWARN server: the panic word alone is fine\n";
    assert_eq!(panics(log).len(), 0);
    assert_eq!(panics("").len(), 0);
}

#[test]
fn every_panic_is_listed() {
    let log = "thread 'a' panicked at x.rs:1:1:\nboom\nthread 'b' panicked at y.rs:2:2:\nbang";
    assert_eq!(panics(log).len(), 2);
}

/// A file in the system temp dir (never the repo) holding `text`.
fn temp_log(name: &str, text: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("e2e-log-test-{}-{name}.log", std::process::id()));
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn the_log_is_kept_when_it_has_a_panic_or_the_tests_failed() {
    // UX4-4: a reproduced panic must leave the log behind, its path printed
    let panicked = temp_log("panic", "thread 'a' panicked at x.rs:1:1:\nboom\n");
    let found = xtask::e2e_log::settle(&panicked, true).unwrap();
    assert_eq!(found.panics.len(), 1);
    assert!(found.kept && panicked.exists());
    let failed = temp_log("failed", "INFO listening\n");
    let found = xtask::e2e_log::settle(&failed, false).unwrap();
    assert_eq!(found.panics.len(), 0);
    assert!(found.kept && failed.exists());
    let clean = temp_log("clean", "INFO listening\n");
    let found = xtask::e2e_log::settle(&clean, true).unwrap();
    assert!(!found.kept && !clean.exists());
    for p in [panicked, failed] {
        std::fs::remove_file(p).unwrap();
    }
}

#[test]
fn the_ci_e2e_job_runs_the_guard() {
    // UX4-4: the GitLab job captures the server log and scans it with the same guard, and
    // keeps the log as a failure artifact
    let ci = std::fs::read_to_string(xtask::repo_root().join(".gitlab-ci.yml")).unwrap();
    let job = ci.split("\ne2e:\n").nth(1).unwrap();
    let job = job.split("\n\n").next().unwrap();
    assert!(job.contains("E2E_SERVER_LOG"), "{job}");
    assert!(job.contains("cargo xtask e2e-scan"), "{job}");
    assert!(job.contains("e2e-server.log"), "{job}");
}

#[test]
fn a_missing_or_empty_log_is_an_error_not_a_clean_run() {
    // a scan of a log the server never wrote finds no panic only because it read nothing:
    // the guard would pass vacuously, so both cases fail with the reason
    let missing =
        std::env::temp_dir().join(format!("e2e-log-test-{}-missing.log", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    let err = xtask::e2e_log::settle(&missing, true).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert!(err.to_string().contains("was not written"), "{err}");
    let empty = temp_log("empty", "");
    let err = xtask::e2e_log::settle(&empty, true).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("is empty"), "{err}");
    assert!(empty.exists(), "an empty log is kept, like any failed run");
    std::fs::remove_file(empty).unwrap();
}
