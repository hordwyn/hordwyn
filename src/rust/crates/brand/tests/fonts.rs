//! Optional `[fonts]` section (design.md 2.8): self-hosted web fonts from the brand directory.

use std::path::{Path, PathBuf};

use brand::{Brand, FONT_ROUTE};

fn default_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../config/brand/default")
}

fn temp_brand(toml: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "brand-fonts-{}-{}",
        std::process::id(),
        toml.len() + files.len()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    std::fs::write(dir.join("brand.toml"), toml).unwrap();
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    dir
}

#[test]
fn fonts_section_parses() {
    let b = Brand::parse(
        "name = \"A\"\nshort_name = \"a\"\n[fonts]\nsans = \"Inter\"\n\
         [[fonts.files]]\nfile = \"fonts/Inter.woff2\"\nweight = \"100 900\"\n\
         [[fonts.files]]\nfile = \"fonts/Inter-Italic.woff2\"\nweight = \"100 900\"\nstyle = \"italic\"\n",
    )
    .unwrap();
    let f = b.fonts.as_ref().unwrap();
    assert_eq!(f.sans, "Inter");
    assert_eq!(f.files.len(), 2);
    assert_eq!(f.files[0].style, "normal");
    assert_eq!(f.files[1].style, "italic");
    // without the section nothing changes
    let plain = Brand::parse("name = \"A\"\nshort_name = \"a\"\n").unwrap();
    assert_eq!(plain.fonts, None);
    assert_eq!(plain.font_css(), "");
}

#[test]
fn invalid_font_entries_are_rejected() {
    let base = "name = \"A\"\nshort_name = \"a\"\n[fonts]\n";
    for bad in [
        "sans = \"Evil\\\";}</style>\"\n",
        "sans = \"\"\n",
        "sans = \"A\"\n[[fonts.files]]\nfile = \"../secret.woff2\"\n",
        "sans = \"A\"\n[[fonts.files]]\nfile = \"/etc/x.woff2\"\n",
        "sans = \"A\"\n[[fonts.files]]\nfile = \"fonts/a.ttf\"\n",
        "sans = \"A\"\n[[fonts.files]]\nfile = \"fonts/a.woff2\"\nstyle = \"oblique;x\"\n",
        "sans = \"A\"\n[[fonts.files]]\nfile = \"fonts/a.woff2\"\nweight = \"bold\"\n",
        "sans = \"A\"\n[[fonts.files]]\nfile = \"fonts/a.woff2\"\nweight = \"0 2000\"\n",
    ] {
        assert!(Brand::parse(&format!("{base}{bad}")).is_err(), "{bad}");
    }
}

#[test]
fn default_brand_ships_atkinson_hyperlegible_next() {
    let b = Brand::load(&default_dir()).unwrap();
    assert_eq!(b.fonts.as_ref().unwrap().sans, "Atkinson Hyperlegible Next");
    let assets = b.font_assets();
    assert_eq!(assets.len(), 2, "regular and italic variable fonts");
    for a in assets {
        assert!(a.url.starts_with(FONT_ROUTE), "{}", a.url);
        assert_eq!(&a.bytes[..4], b"wOF2");
        assert_eq!(
            b.font_asset(&a.url).map(|x| x.url.as_str()),
            Some(a.url.as_str())
        );
    }
    assert!(b.font_asset("/brand/fonts/nope/x.woff2").is_none());
    assert!(
        default_dir().join("fonts/OFL.txt").is_file(),
        "licence next to the fonts"
    );
}

#[test]
fn font_css_declares_same_origin_faces_and_the_stack() {
    let b = Brand::load(&default_dir()).unwrap();
    let css = b.font_css();
    assert_eq!(css.matches("@font-face{").count(), 2, "{css}");
    assert_eq!(css.matches("font-display:swap").count(), 2, "{css}");
    assert!(
        css.contains("font-family:\"Atkinson Hyperlegible Next\""),
        "{css}"
    );
    assert!(css.contains("font-style:italic"), "{css}");
    assert!(css.contains("font-weight:200 800"), "{css}");
    for a in b.font_assets() {
        assert!(
            css.contains(&format!("src:url(\"{}\") format(\"woff2\")", a.url)),
            "{css}"
        );
    }
    assert!(!css.contains("http"), "no CDN, no other origin: {css}");
    assert!(
        css.contains(
            ":root{--font-sans:\"Atkinson Hyperlegible Next\",ui-sans-serif,system-ui,sans-serif,"
        ),
        "brand font first, then the system stack: {css}"
    );
    // the head gets fonts and theme together
    assert!(b.head_css().starts_with(&css));
}

#[test]
fn missing_or_wrong_font_files_fail_at_load() {
    let toml = "name = \"A\"\nshort_name = \"a\"\n[fonts]\nsans = \"A\"\n\
                [[fonts.files]]\nfile = \"fonts/a.woff2\"\n";
    let dir = temp_brand(toml, &[]);
    assert!(Brand::load(&dir).is_err(), "missing file");
    let dir = temp_brand(toml, &[("fonts/a.woff2", b"<html>not a font")]);
    assert!(Brand::load(&dir).is_err(), "not a woff2 file");
    let dir = temp_brand(toml, &[("fonts/a.woff2", b"wOF2rest-of-font")]);
    let b = Brand::load(&dir).unwrap();
    assert_eq!(b.font_assets().len(), 1);
    // the URL changes with the content, so it can be cached as immutable
    let url = b.font_assets()[0].url.clone();
    let dir = temp_brand(toml, &[("fonts/a.woff2", b"wOF2other-font")]);
    assert_ne!(Brand::load(&dir).unwrap().font_assets()[0].url, url);
    let _ = std::fs::remove_dir_all(dir);
}
