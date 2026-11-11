//! WCAG contrast of the brand accent (design.md 2.7 and 7.4): ratios of the example values.

use brand::Brand;
use brand::contrast::{Rgb, contrast_ratio, parse_color};

fn ratio(a: &str, b: &str) -> f64 {
    let a = parse_color(a).unwrap_or_else(|| panic!("parse {a}"));
    let b = parse_color(b).unwrap_or_else(|| panic!("parse {b}"));
    contrast_ratio(a, b)
}

#[test]
fn ratios_of_the_design_examples() {
    // light: violet-700 on white
    // design.md prints 7.29; the exact value is 7.298 (two decimals, cut off)
    let v = ratio("#7008e7", "#ffffff");
    assert!((v - 7.29).abs() < 0.01, "{v}");
    // dark: zinc-950 on violet-400
    assert!((ratio("#a684ff", "#09090b") - 6.98).abs() < 0.01);
    // default tokens: zinc-50 on zinc-900
    assert!((ratio("#18181b", "#fafafa") - 16.98).abs() < 0.01);
    // zinc-500 field border on white (WCAG 1.4.11)
    assert!((ratio("#71717b", "#ffffff") - 4.83).abs() < 0.01);
    // the ratio is symmetric
    assert!((ratio("#ffffff", "#7008e7") - 7.29).abs() < 0.01);
}

#[test]
fn parses_the_usual_css_colour_forms() {
    assert_eq!(parse_color("#fff"), Some(Rgb(255, 255, 255)));
    assert_eq!(parse_color(" #09090B "), Some(Rgb(9, 9, 11)));
    assert_eq!(parse_color("white"), Some(Rgb(255, 255, 255)));
    assert_eq!(parse_color("black"), Some(Rgb(0, 0, 0)));
    assert_eq!(parse_color("rgb(112, 8, 231)"), Some(Rgb(112, 8, 231)));
    assert_eq!(parse_color("rgb(112 8 231)"), Some(Rgb(112, 8, 231)));
    // Tailwind v4 violet-700 is oklch(49.1% 0.27 292.581) = #7008e7
    let v = parse_color("oklch(49.1% 0.27 292.581)").unwrap();
    assert!(
        (contrast_ratio(v, Rgb(255, 255, 255)) - 7.29).abs() < 0.05,
        "{v:?}"
    );
    assert_eq!(parse_color("oklch(1 0 0)"), Some(Rgb(255, 255, 255)));
    assert_eq!(parse_color("var(--color-violet-700)"), None);
    assert_eq!(parse_color("#12345"), None);
}

#[test]
fn default_brand_passes() {
    let b = Brand::parse("name = \"A\"\nshort_name = \"a\"\n").unwrap();
    assert_eq!(b.contrast_findings().len(), 0);
}

#[test]
fn low_contrast_primary_is_reported_per_theme() {
    // amber-400 with the default light foreground (zinc-50): far below 4.5
    let b = Brand::parse(
        "name = \"A\"\nshort_name = \"a\"\n[theme]\nprimary = \"#ffb900\"\n\
         [theme_dark]\nprimary = \"#a684ff\"\nprimary-foreground = \"#09090b\"\n",
    )
    .unwrap();
    let f = b.contrast_findings();
    // the text on the accent (plain and as the count of a pressed chip), and the accent as
    // text or bar on the four light surfaces
    assert_eq!(f.len(), 6, "{f:?}");
    assert!(f.iter().all(|c| c.theme == "light"), "{f:?}");
    assert_eq!(f[0].foreground_token, "primary-foreground");
    assert_eq!(f[0].background_token, "primary");
    assert!(
        f.iter()
            .all(|c| c.foreground_token == "primary" || c.background_token == "primary"),
        "{f:?}"
    );
    assert!(f[0].ratio.is_some_and(|r| r < 4.5), "{f:?}");
}

#[test]
fn example_accent_passes_in_both_themes() {
    let b = Brand::parse(
        "name = \"A\"\nshort_name = \"a\"\n[theme]\nprimary = \"#7008e7\"\n\
         primary-foreground = \"white\"\n[theme_dark]\n\"--primary\" = \"#a684ff\"\n\
         primary-foreground = \"#09090b\"\n",
    )
    .unwrap();
    assert_eq!(b.contrast_findings().len(), 0);
}

#[test]
fn unparsable_values_are_reported_without_a_ratio() {
    let b = Brand::parse(
        "name = \"A\"\nshort_name = \"a\"\n[theme]\nprimary = \"var(--color-violet-700)\"\n",
    )
    .unwrap();
    let f = b.contrast_findings();
    // every light pair with the accent in it
    assert_eq!(f.len(), 6, "{f:?}");
    assert!(
        f.iter().all(|c| c.theme == "light" && c.ratio.is_none()),
        "{f:?}"
    );
}

#[test]
fn accent_pairs_cover_both_themes_with_their_ratio() {
    // the admin Branding page shows both swatches, also when the brand keeps the defaults
    let b = Brand::parse("name = \"A\"\nshort_name = \"a\"\n").unwrap();
    let pairs = b.accent_pairs();
    let themes: Vec<_> = pairs.iter().map(|p| p.theme).collect();
    assert_eq!(themes, ["light", "dark"]);
    assert_eq!(pairs[0].primary, "#18181b");
    assert!(
        pairs[0].ratio.is_some_and(|r| (r - 16.98).abs() < 0.01),
        "{pairs:?}"
    );
    let b =
        Brand::parse("name = \"A\"\nshort_name = \"a\"\n[theme]\nprimary = \"#ffb900\"\n").unwrap();
    let pairs = b.accent_pairs();
    assert_eq!(pairs[0].primary, "#ffb900");
    assert!(pairs[0].ratio.is_some_and(|r| r < 4.5));
    assert_eq!(pairs[1].primary, "#fafafa", "dark keeps the default");
}

// --- OT-13 (DEC-540): the accent, its soft surface and the four group tints -----------------

fn default_brand() -> Brand {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../config/brand/default");
    Brand::load(&dir).unwrap()
}

const GROUPS: [&str; 4] = ["watch", "read", "listen", "play"];

/// The ratios of `docs/fable/round2/ux-owner-test.md` 3.1 (computed outside this crate):
/// theme, foreground token, background token, ratio.
// 6.28 is the ratio of blue-700 on blue-50, not an approximation of TAU
#[allow(clippy::approx_constant)]
const UX_PASS: [(&str, &str, &str, f64); 47] = [
    ("light", "primary-foreground", "primary", 7.30),
    ("light", "primary", "background", 7.30),
    ("light", "primary", "muted", 6.64),
    ("light", "primary", "primary-soft", 6.65),
    ("light", "foreground", "primary-soft", 18.14),
    ("light", "group-watch", "background", 6.27),
    ("light", "group-watch", "muted", 5.71),
    ("light", "group-watch", "group-watch-soft", 5.84),
    ("light", "foreground", "group-watch-soft", 18.53),
    ("light", "group-read", "background", 5.22),
    ("light", "group-read", "muted", 4.75),
    ("light", "group-read", "group-read-soft", 4.92),
    ("light", "foreground", "group-read-soft", 18.74),
    ("light", "group-listen", "background", 5.36),
    ("light", "group-listen", "muted", 4.88),
    ("light", "group-listen", "group-listen-soft", 5.14),
    ("light", "foreground", "group-listen-soft", 19.08),
    ("light", "group-play", "background", 6.83),
    ("light", "group-play", "muted", 6.22),
    ("light", "group-play", "group-play-soft", 6.28),
    ("light", "foreground", "group-play-soft", 18.28),
    ("dark", "primary-foreground", "primary", 6.98),
    ("dark", "primary", "background", 6.98),
    ("dark", "primary", "card", 6.21),
    ("dark", "primary", "muted", 5.22),
    ("dark", "primary", "primary-soft", 5.34),
    ("dark", "foreground", "primary-soft", 14.60),
    ("dark", "group-watch", "background", 7.68),
    ("dark", "group-watch", "card", 6.84),
    ("dark", "group-watch", "muted", 5.75),
    ("dark", "group-watch", "group-watch-soft", 5.74),
    ("dark", "foreground", "group-watch-soft", 14.25),
    ("dark", "group-read", "background", 8.37),
    ("dark", "group-read", "card", 7.45),
    ("dark", "group-read", "muted", 6.26),
    ("dark", "group-read", "group-read-soft", 6.58),
    ("dark", "foreground", "group-read-soft", 15.00),
    ("dark", "group-listen", "background", 10.67),
    ("dark", "group-listen", "card", 9.50),
    ("dark", "group-listen", "muted", 7.99),
    ("dark", "group-listen", "group-listen-soft", 7.77),
    ("dark", "foreground", "group-listen-soft", 13.89),
    ("dark", "group-play", "background", 7.54),
    ("dark", "group-play", "card", 6.72),
    ("dark", "group-play", "muted", 5.65),
    ("dark", "group-play", "group-play-soft", 5.60),
    ("dark", "foreground", "group-play-soft", 14.16),
];

#[test]
fn the_default_brand_sets_the_accent_and_the_group_tints_in_both_themes() {
    let b = default_brand();
    for (theme, vars) in [("light", &b.theme), ("dark", &b.theme_dark)] {
        let mut keys = vec![
            "primary".to_string(),
            "primary-foreground".into(),
            "primary-soft".into(),
        ];
        for g in GROUPS {
            keys.push(format!("group-{g}"));
            keys.push(format!("group-{g}-soft"));
        }
        for k in keys {
            let v = vars
                .get(&k)
                .unwrap_or_else(|| panic!("{theme}: {k} missing"));
            assert!(parse_color(v).is_some(), "{theme}: {k} = {v} is checkable");
        }
    }
    assert_eq!(b.theme.get("primary").map(String::as_str), Some("#7008e7"));
    assert_eq!(
        b.theme_dark.get("primary").map(String::as_str),
        Some("#a684ff")
    );
}

#[test]
fn every_pair_of_the_default_brand_passes_in_light_and_dark() {
    // WCAG 1.4.3 (4.5:1, text) and 1.4.11 (3:1, bars and markers on the muted track)
    let b = default_brand();
    let checks = b.contrast_checks();
    // per theme: 8 accent pairs and 7 pairs per group
    assert_eq!(checks.len(), 2 * (8 + 4 * 7));
    for c in &checks {
        println!(
            "{:5} | {:18} {} on {:18} {} | {:5.2} | needs {}",
            c.theme,
            c.foreground_token,
            c.foreground,
            c.background_token,
            c.background,
            c.ratio.unwrap_or(0.0),
            c.required
        );
        assert!(c.ratio.is_some_and(|r| r >= c.required), "{c:?}");
    }
    assert_eq!(b.contrast_findings().len(), 0);
}

#[test]
fn the_brand_crate_confirms_the_ratios_of_the_ux_pass() {
    let checks = default_brand().contrast_checks();
    for (theme, fg, bg, expected) in UX_PASS {
        let c = checks
            .iter()
            .find(|c| c.theme == theme && c.foreground_token == fg && c.background_token == bg)
            .unwrap_or_else(|| panic!("{theme}: {fg} on {bg} is not checked"));
        let got = c.ratio.unwrap();
        assert!(
            (got - expected).abs() < 0.015,
            "{theme}: {fg} on {bg}: {got:.3}, the UX pass says {expected}"
        );
    }
}

#[test]
fn a_brand_without_theme_values_keeps_neutral_tints_that_pass() {
    // the stylesheet falls back to grey: group-* = muted-foreground, group-*-soft = muted,
    // primary-soft = secondary
    let b = Brand::parse("name = \"A\"\nshort_name = \"a\"\n").unwrap();
    let checks = b.contrast_checks();
    assert_eq!(checks.len(), 72);
    let watch = checks
        .iter()
        .find(|c| {
            c.theme == "light"
                && c.foreground_token == "group-watch"
                && c.background_token == "group-watch-soft"
        })
        .unwrap();
    assert_eq!(
        (watch.foreground.as_str(), watch.background.as_str()),
        ("#52525c", "#f4f4f5")
    );
    assert_eq!(b.contrast_findings().len(), 0);
}

#[test]
fn a_fallback_follows_the_token_the_brand_sets() {
    // a brand that changes `muted` changes the soft surfaces that fall back to it
    let b =
        Brand::parse("name = \"A\"\nshort_name = \"a\"\n[theme]\nmuted = \"#52525c\"\n").unwrap();
    let f = b.contrast_findings();
    assert!(
        f.iter().any(|c| c.theme == "light"
            && c.foreground_token == "group-read"
            && c.background_token == "group-read-soft"
            && c.ratio.is_some_and(|r| r < 1.01)),
        "{f:?}"
    );
    assert!(f.iter().all(|c| c.theme == "light"), "{f:?}");
}

#[test]
fn a_group_tint_below_the_threshold_is_reported_with_its_tokens() {
    // orange-400 as text on white is about 2.2:1
    let b = Brand::parse("name = \"A\"\nshort_name = \"a\"\n[theme]\ngroup-read = \"#ff8904\"\n")
        .unwrap();
    let f = b.contrast_findings();
    let tokens: Vec<_> = f
        .iter()
        .map(|c| {
            (
                c.theme,
                c.foreground_token.as_str(),
                c.background_token.as_str(),
            )
        })
        .collect();
    assert_eq!(
        tokens,
        [
            ("light", "group-read", "background"),
            ("light", "group-read", "card"),
            ("light", "group-read", "muted"),
            ("light", "group-read", "info-soft"),
            ("light", "group-read", "group-read-soft"),
        ]
    );
    assert!((f[0].required - 4.5).abs() < f64::EPSILON);
    assert!((f[2].required - 3.0).abs() < f64::EPSILON);
}

/// `fg` at `alpha` over `bg` (the hover state of a filled button is `bg-primary/90`).
fn over(fg: Rgb, bg: Rgb, alpha: f64) -> Rgb {
    // 0..=255 by construction
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mix = |a: u8, b: u8| (f64::from(a) * alpha + f64::from(b) * (1.0 - alpha)).round() as u8;
    Rgb(mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2))
}

#[test]
fn the_hovered_filled_button_keeps_its_text_contrast() {
    let b = default_brand();
    for (vars, page) in [(&b.theme, "#ffffff"), (&b.theme_dark, "#09090b")] {
        let primary = parse_color(&vars["primary"]).unwrap();
        let text = parse_color(&vars["primary-foreground"]).unwrap();
        let hovered = over(primary, parse_color(page).unwrap(), 0.9);
        let r = contrast_ratio(text, hovered);
        println!("hover on {page}: {r:.2}");
        assert!(r >= 4.5, "{r}");
    }
}

// --- review of ot2d (OT2D-6, OT2D-7): pairs beyond the first set, and brands that warn -------

fn repo(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .join(path)
}

fn tokens(found: &[brand::contrast::PairCheck]) -> Vec<(&str, &str, &str)> {
    found
        .iter()
        .map(|c| {
            (
                c.theme,
                c.foreground_token.as_str(),
                c.background_token.as_str(),
            )
        })
        .collect()
}

#[test]
fn the_count_of_a_pressed_chip_is_checked_at_its_opacity() {
    // `primary-foreground` at 75 % over `primary` (ui/src/chip.rs): 4.52 in light, 4.84 in
    // dark with the default brand, so a slightly lighter accent already fails
    let checks = default_brand().contrast_checks();
    for (theme, blended, expected) in [("light", "#dbc1f9", 4.52), ("dark", "#302848", 4.84)] {
        let c = checks
            .iter()
            .find(|c| c.theme == theme && c.foreground_token == "primary-foreground/75")
            .unwrap();
        assert_eq!(c.background_token, "primary");
        assert_eq!(c.foreground, blended, "{theme}");
        let got = c.ratio.unwrap();
        assert!((got - expected).abs() < 0.015, "{theme}: {got:.3}");
        assert!((c.required - 4.5).abs() < f64::EPSILON);
    }
    // an accent pair of 5:1 passes as a button label and fails as the count
    let b = Brand::parse(
        "name = \"A\"\nshort_name = \"a\"\n[theme]\nprimary = \"#008236\"\n\
         primary-foreground = \"#ffffff\"\nprimary-soft = \"#f0fdf4\"\n",
    )
    .unwrap();
    assert_eq!(
        tokens(&b.contrast_findings()),
        [("light", "primary-foreground/75", "primary")]
    );
    // a value the check cannot read is reported for the count too, without a ratio
    let b = Brand::parse(
        "name = \"A\"\nshort_name = \"a\"\n[theme]\nprimary-foreground = \"var(--x)\"\n",
    )
    .unwrap();
    let f = b.contrast_findings();
    assert_eq!(
        tokens(&f),
        [
            ("light", "primary-foreground", "primary"),
            ("light", "primary-foreground/75", "primary")
        ]
    );
    assert_eq!(f[1].foreground, "var(--x) at 75 %");
    assert_eq!(f[1].ratio, None);
}

#[test]
fn the_group_tints_keep_their_icon_contrast_on_the_info_surface() {
    // the kind icon of a shared History row (web/src/history.rs): 3:1, WCAG 1.4.11
    let checks = default_brand().contrast_checks();
    let on_info: Vec<_> = checks
        .iter()
        .filter(|c| c.background_token == "info-soft")
        .collect();
    assert_eq!(on_info.len(), 8);
    for c in on_info {
        assert!(c.foreground_token.starts_with("group-"), "{c:?}");
        assert!((c.required - 3.0).abs() < f64::EPSILON);
        assert!(c.ratio.is_some_and(|r| r >= 4.5), "{c:?}");
    }
}

#[test]
fn the_sample_brand_gets_the_warnings_its_light_green_accent_earns() {
    // src/web/tests/brands/sample (the brand-swap test): `primary = oklch(0.55 0.2 145)` in
    // light only, everything else the stylesheet's. A mid green is fine as a bar and too
    // light for text: the server logs exactly these at start-up, dark stays neutral.
    let b = Brand::load(&repo("src/web/tests/brands/sample")).unwrap();
    let f = b.contrast_findings();
    assert_eq!(
        tokens(&f),
        [
            ("light", "primary-foreground", "primary"),
            ("light", "primary-foreground/75", "primary"),
            ("light", "primary", "background"),
            ("light", "primary", "card"),
            ("light", "primary", "primary-soft"),
        ]
    );
    for c in &f {
        println!(
            "{} on {}: {:.2}",
            c.foreground_token,
            c.background_token,
            c.ratio.unwrap()
        );
        assert!(c.ratio.is_some_and(|r| (2.5..4.5).contains(&r)), "{c:?}");
        assert!((c.required - 4.5).abs() < f64::EPSILON);
    }
    // as a bar or marker on the muted track it passes (3:1)
    assert!(b.contrast_checks().iter().any(|c| c.theme == "light"
        && c.foreground_token == "primary"
        && c.background_token == "muted"
        && !c.fails()));
}

#[test]
fn the_example_of_the_white_labelling_guide_passes() {
    // the `[theme]` and `[theme_dark]` tables of the guide's brand.toml, as printed there
    let page =
        std::fs::read_to_string(repo("docs/modules/ROOT/pages/white-labelling.adoc")).unwrap();
    let block = page
        .split("----")
        .nth(1)
        .expect("the first source block of the guide");
    let start = block.find("[theme]").expect("[theme] in the example");
    let end = block.find("[logos]").expect("[logos] after the themes");
    let toml = format!("name = \"A\"\nshort_name = \"a\"\n{}", &block[start..end]);
    let b = Brand::parse(&toml).unwrap();
    assert!(b.theme.contains_key("primary") && b.theme_dark.contains_key("primary"));
    let f = b.contrast_findings();
    assert_eq!(f.len(), 0, "{f:#?}");
}
