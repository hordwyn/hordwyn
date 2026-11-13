//! Fails if the product name or the codename it replaced appears in code outside
//! `config/brand/` and `docs/` (KICKOFF rule 9).

#[test]
fn codename_not_hard_coded() {
    let found = xtask::whitelabel::offenders(&xtask::repo_root());
    assert!(
        found.is_empty(),
        "product name found outside config/brand/ and docs/: {found:?}"
    );
}

/// Colours are brand values (DEC-540): component code (`crates/ui`, `crates/web`,
/// `apps/server`) names theme tokens and writes down no colour of any kind (palette class,
/// arbitrary value, hex, colour function, SVG or inline style literal; allowed are
/// `currentColor`, `none`, `transparent` and `var(--token)`), and the stylesheet names no
/// palette family the brand colours come from (review OT2D-3).
#[test]
fn brand_colours_not_hard_coded() {
    let found = xtask::whitelabel::colour_offenders(&xtask::repo_root());
    assert_eq!(
        found.len(),
        0,
        "colour outside config/brand/ (path:line: literal):\n{}",
        found.join("\n")
    );
}
