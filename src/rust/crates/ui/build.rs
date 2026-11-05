//! Generates `IconName` from `icons/icons.toml` and the vendored SVGs, so only allowlisted
//! icons are compiled in and no icon is ever fetched at runtime (DEC-043).

use std::fmt::Write as _;
use std::path::Path;

fn pascal(name: &str) -> String {
    name.split('-')
        .map(|p| {
            let mut c = p.chars();
            c.next().map_or_else(String::new, |f| {
                f.to_ascii_uppercase().to_string() + c.as_str()
            })
        })
        .collect()
}

/// Inner markup of an SVG file: everything between the opening `<svg ...>` and `</svg>`,
/// without comments and without invisible bounding-box paths.
fn inner(svg: &str) -> String {
    let start = svg.find("<svg").expect("svg element");
    let open_end = start + svg[start..].find('>').expect("svg open tag end") + 1;
    let close = svg.rfind("</svg>").expect("svg close tag");
    svg[open_end..close]
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("<!--")
                && !l.contains("stroke=\"none\" d=\"M0 0h24v24H0z\"")
        })
        .collect::<Vec<_>>()
        .join("")
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons");
    println!("cargo:rerun-if-changed={}", dir.display());
    let toml = std::fs::read_to_string(dir.join("icons.toml")).expect("icons.toml");
    let mut entries = Vec::new();
    for line in toml.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let (name, spec) = line.split_once('=').expect("name = \"source:file\"");
        let spec = spec.trim().trim_matches('"');
        let (source, file) = spec.split_once(':').expect("source:file");
        let vendor = dir.join("vendor").join(source);
        assert!(
            vendor.join("LICENSE").exists(),
            "missing LICENSE for icon source `{source}`"
        );
        let svg = std::fs::read_to_string(vendor.join(format!("{file}.svg")))
            .unwrap_or_else(|_| panic!("icon `{spec}` is not vendored"));
        entries.push((name.trim().to_string(), source.to_string(), inner(&svg)));
    }
    let mut out = String::from(
        "/// Icons available in this build (generated from `icons/icons.toml`).\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum IconName {\n",
    );
    for (n, _, _) in &entries {
        writeln!(out, "    {},", pascal(n)).unwrap();
    }
    out.push_str("}\n\nimpl IconName {\n    pub const ALL: &'static [IconName] = &[\n");
    for (n, _, _) in &entries {
        writeln!(out, "        IconName::{},", pascal(n)).unwrap();
    }
    out.push_str("    ];\n\n    /// Semantic name from the allowlist, e.g. `kind-movie`.\n    #[must_use]\n    pub const fn name(self) -> &'static str {\n        match self {\n");
    for (n, _, _) in &entries {
        writeln!(out, "            IconName::{} => {n:?},", pascal(n)).unwrap();
    }
    out.push_str("        }\n    }\n\n    /// Vendored icon set the glyph comes from.\n    #[must_use]\n    pub const fn source(self) -> &'static str {\n        match self {\n");
    for (n, s, _) in &entries {
        writeln!(out, "            IconName::{} => {s:?},", pascal(n)).unwrap();
    }
    out.push_str("        }\n    }\n\n    /// Inner SVG markup (24x24 viewBox, stroke icons).\n    #[must_use]\n    pub const fn body(self) -> &'static str {\n        match self {\n");
    for (n, _, b) in &entries {
        writeln!(out, "            IconName::{} => {b:?},", pascal(n)).unwrap();
    }
    out.push_str("        }\n    }\n\n    #[must_use]\n    pub fn from_name(name: &str) -> Option<Self> {\n        Self::ALL.iter().copied().find(|i| i.name() == name)\n    }\n}\n");
    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("icons_gen.rs");
    std::fs::write(dest, out).expect("write icons_gen.rs");
}
