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
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].theme, "light");
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
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].ratio, None);
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
