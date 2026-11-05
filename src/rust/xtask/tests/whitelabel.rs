//! Fails if the codename appears in code outside `config/brand/` and `docs/` (KICKOFF rule 9).

#[test]
fn codename_not_hard_coded() {
    let found = xtask::whitelabel::offenders(&xtask::repo_root());
    assert!(
        found.is_empty(),
        "codename found outside config/brand/ and docs/: {found:?}"
    );
}
