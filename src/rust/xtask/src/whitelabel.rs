//! White-label guards (KICKOFF rule 9): the product name (and the codename it replaced) must
//! not appear in code, and no colour is written down outside the brand folder.

use std::fs;
use std::path::{Path, PathBuf};

/// The name of the default brand, assembled at runtime so this file does not match itself.
#[must_use]
pub fn codename() -> String {
    ["hord", "wyn"].concat()
}

/// The codename the product had before it was renamed. It stays forbidden in code so that
/// the old name does not linger.
#[must_use]
pub fn former_codename() -> String {
    ["kith", "keep"].concat()
}

/// Every name the guard looks for (lowercase): the current one and the former one.
#[must_use]
pub fn codenames() -> Vec<String> {
    vec![codename(), former_codename()]
}

/// Paths (relative to the repo root, `/`-separated) that may contain a codename.
pub const ALLOWED_PREFIXES: &[&str] = &[
    "config/brand/",
    "docs/",
    ".git/",
    "README.md",
    "LICENSE",
    "flake.lock",
];

/// Directory names that are never scanned (build output, dependencies).
pub const SKIPPED_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".direnv",
    "test-results",
    "playwright-report",
    "pkg",
    // local Postgres/S3/Meilisearch state (`cargo xtask services`); runtime files contain
    // the absolute checkout path
    ".dev",
];

/// Return every file under `root` that contains one of the [`codenames`] (case-insensitive,
/// in its text or in its path) outside the allowed locations.
#[must_use]
pub fn offenders(root: &Path) -> Vec<PathBuf> {
    let needles = codenames();
    let mut out = Vec::new();
    walk(root, root, &needles, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, needles: &[String], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if ALLOWED_PREFIXES
            .iter()
            .any(|p| rel == p.trim_end_matches('/') || rel.starts_with(p))
        {
            continue;
        }
        let name = entry.file_name();
        if path.is_dir() {
            if SKIPPED_DIRS.iter().any(|d| name == *d) {
                continue;
            }
            walk(root, &path, needles, out);
        } else if let Ok(bytes) = fs::read(&path) {
            let text = String::from_utf8_lossy(&bytes).to_lowercase();
            let rel = rel.to_lowercase();
            if needles
                .iter()
                .any(|n| text.contains(n.as_str()) || rel.contains(n.as_str()))
            {
                out.push(PathBuf::from(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_codename_outside_allowed_paths() {
        let dir = std::env::temp_dir().join(format!("wl-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("config/brand/default")).unwrap();
        fs::write(
            dir.join("src/a.rs"),
            format!("const N: &str = \"{}\";", codename().to_uppercase()),
        )
        .unwrap();
        fs::write(dir.join("config/brand/default/brand.toml"), codename()).unwrap();
        fs::write(dir.join("src/ok.rs"), "fn main() {}").unwrap();
        let found = offenders(&dir);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(found, vec![PathBuf::from("src/a.rs")]);
    }

    /// The rename (2026-11-12): the new name is guarded, and the old one stays forbidden.
    #[test]
    fn detects_the_current_and_the_former_name() {
        let current = ["hord", "wyn"].concat();
        let former = ["kith", "keep"].concat();
        let dir = std::env::temp_dir().join(format!("wl-names-{}", std::process::id()));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("docs")).unwrap();
        let mut title = current.clone();
        title[..1].make_ascii_uppercase();
        fs::write(dir.join("src/new.rs"), format!("// {title}")).unwrap();
        fs::write(dir.join("src/old.rs"), format!("// {former}")).unwrap();
        fs::write(dir.join(format!("src/{current}.txt")), "x").unwrap();
        fs::write(dir.join("src/ok.rs"), "fn hoard() {}").unwrap();
        fs::write(dir.join("docs/a.md"), format!("{current} {former}")).unwrap();
        let found = offenders(&dir);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            found,
            vec![
                PathBuf::from(format!("src/{current}.txt")),
                PathBuf::from("src/new.rs"),
                PathBuf::from("src/old.rs"),
            ]
        );
    }
}

/// Tailwind palette families the default brand takes its accent and group tints from
/// (DEC-540). The stylesheet names theme tokens for them (`primary`, `group-watch`); the
/// values live in the brand file. Its own neutral and semantic families (zinc, red, emerald,
/// amber, sky) are the fallbacks and status colours and stay allowed there.
pub const BRAND_PALETTES: &[&str] = &["violet", "fuchsia", "orange", "teal", "blue", "purple"];

/// Every family of the Tailwind palette. Component code names none of them: a colour there
/// is a theme token (`text-success`, `bg-primary-soft`), also for the neutral and the status
/// colours (review OT2D-3).
pub const TAILWIND_PALETTES: &[&str] = &[
    "slate", "gray", "zinc", "neutral", "stone", "red", "orange", "amber", "yellow", "lime",
    "green", "emerald", "teal", "cyan", "sky", "blue", "indigo", "violet", "purple", "fuchsia",
    "pink", "rose",
];

/// Component code: no colour literal of any kind ([`colour_literal`]).
pub const COMPONENT_DIRS: &[&str] = &[
    "src/rust/crates/ui/src",
    "src/rust/crates/web/src",
    "src/rust/apps/server/src",
];

/// The stylesheet: no palette family of the brand ([`BRAND_PALETTES`]). Its neutral
/// fallbacks, the dark status surfaces (hex) and the shadows (`rgb()`) are its job.
pub const STYLE_DIRS: &[&str] = &["src/web/style"];

/// The only colour values component code may write down besides a theme token
/// (`var(--...)`).
pub const COLOUR_KEYWORDS: &[&str] = &["currentColor", "none", "transparent"];

/// Utilities that take a colour (`bg-...`, `text-...`).
const COLOUR_UTILITIES: &[&str] = &[
    "bg",
    "text",
    "border",
    "fill",
    "stroke",
    "ring",
    "outline",
    "decoration",
    "shadow",
    "from",
    "via",
    "to",
    "divide",
    "accent",
    "caret",
    "placeholder",
];

/// CSS colour functions.
const COLOUR_FUNCTIONS: &[&str] = &[
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "color-mix",
];

/// Presentation attributes that take a colour.
const COLOUR_ATTRIBUTES: &[&str] = &[
    "fill",
    "stroke",
    "stop-color",
    "flood-color",
    "lighting-color",
];

const fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn before(text: &str, at: usize) -> Option<char> {
    text[..at].chars().next_back()
}

/// The first `<family>-<digit>` in `text` for one of `families` (`bg-violet-700`,
/// `var(--color-teal-50)`); `shared-1` does not name `red`.
#[must_use]
pub fn palette_name<'a>(text: &'a str, families: &[&str]) -> Option<&'a str> {
    families.iter().find_map(|family| {
        text.match_indices(family).find_map(|(at, _)| {
            let end = at + family.len();
            let mut next = text[end..].chars();
            let shade = next.next() == Some('-') && next.next().is_some_and(|c| c.is_ascii_digit());
            (shade && !before(text, at).is_some_and(is_word)).then(|| &text[at..end + 2])
        })
    })
}

/// `<family>-<digit>` of a brand palette anywhere in `text`.
#[must_use]
pub fn names_brand_palette(text: &str) -> bool {
    palette_name(text, BRAND_PALETTES).is_some()
}

/// `bg-black/60`, `text-white`: black and white as a utility's colour.
fn black_or_white(line: &str) -> Option<&str> {
    ["black", "white"].iter().find_map(|name| {
        line.match_indices(name).find_map(|(at, _)| {
            let head = line[..at].strip_suffix('-')?;
            let utility = head.rsplit(|c: char| !c.is_ascii_lowercase()).next()?;
            let end = at + name.len();
            let ends = !line[end..].starts_with(|c: char| is_word(c) || c == '-');
            (ends && COLOUR_UTILITIES.contains(&utility))
                .then(|| &line[at - utility.len() - 1..end])
        })
    })
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`. A fragment link (`href="#add"`) and a
/// character reference (`&#160;`) are not colours.
fn hex_colour(line: &str) -> Option<&str> {
    line.match_indices('#').find_map(|(at, _)| {
        let digits = line[at + 1..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .count();
        let end = at + 1 + digits;
        let ends = !line[end..].starts_with(|c: char| is_word(c) || c == '-');
        let starts = !before(line, at).is_some_and(|c| is_word(c) || c == '&')
            && !line[..at].ends_with("href=\"");
        (matches!(digits, 3 | 4 | 6 | 8) && ends && starts).then(|| &line[at..end])
    })
}

/// `rgb(`, `oklch(` and the other colour functions.
fn colour_function(line: &str) -> Option<&str> {
    COLOUR_FUNCTIONS.iter().find_map(|name| {
        line.match_indices(name).find_map(|(at, _)| {
            let end = at + name.len();
            let called = line[end..].starts_with('(');
            let whole = !before(line, at).is_some_and(|c| is_word(c) || c == '-');
            (called && whole).then(|| &line[at..=end])
        })
    })
}

/// A value component code may give a colour: a keyword of [`COLOUR_KEYWORDS`] or a token.
fn allowed_colour(value: &str) -> bool {
    COLOUR_KEYWORDS.contains(&value) || (value.starts_with("var(--") && value.ends_with(')'))
}

/// An arbitrary value on a colour utility that is not a length or a token:
/// `bg-[#7008e7]`, `text-[oklch(0.5_0.2_290)]`, `fill-[red]`.
fn arbitrary_colour(line: &str) -> Option<&str> {
    line.match_indices("-[").find_map(|(at, _)| {
        let utility = line[..at]
            .rsplit(|c: char| !c.is_ascii_lowercase())
            .next()?;
        let value = line[at + 2..].split(']').next()?;
        let measure = value.starts_with(|c: char| c.is_ascii_digit() || c == '.' || c == '-')
            || ["length:", "calc(", "url("]
                .iter()
                .any(|p| value.starts_with(p));
        let token = allowed_colour(value) || allowed_colour(value.trim_start_matches("color:"));
        (COLOUR_UTILITIES.contains(&utility) && !measure && !token)
            .then(|| &line[at - utility.len()..(at + 3 + value.len()).min(line.len())])
    })
}

/// `fill="#1447e6"`, `stroke="red"`: a presentation attribute with a literal that is not in
/// the allow-list. A value computed in code is not a literal and is not seen here.
fn colour_attribute(line: &str) -> Option<&str> {
    COLOUR_ATTRIBUTES.iter().find_map(|name| {
        line.match_indices(name).find_map(|(at, _)| {
            let value = line[at + name.len()..]
                .strip_prefix("=\"")?
                .split('"')
                .next()?;
            let whole = !before(line, at).is_some_and(|c| is_word(c) || c == '-' || c == ':');
            (whole && !allowed_colour(value)).then(|| &line[at..at + name.len() + 2 + value.len()])
        })
    })
}

/// Does the CSS property take a colour (alone or in a shorthand)?
fn colour_property(property: &str) -> bool {
    property.contains("color")
        || matches!(
            property,
            "fill"
                | "stroke"
                | "background"
                | "border"
                | "border-top"
                | "border-right"
                | "border-bottom"
                | "border-left"
                | "outline"
                | "box-shadow"
                | "text-shadow"
        )
}

/// A declaration value without a colour literal: every part is a number or length, a
/// line style, a keyword of the allow-list or a token.
fn colourless(value: &str) -> bool {
    const LINE: &[&str] = &[
        "solid", "dashed", "dotted", "double", "inset", "thin", "medium", "thick",
    ];
    value
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
        .all(|part| {
            allowed_colour(part)
                || LINE.contains(&part)
                || part.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        })
}

/// An inline style that sets a colour to anything but the allow-list: `style="color: red"`,
/// `style=format!("background: {c}")`, `style:background-color="#fff"`.
fn inline_style_colour(line: &str) -> Option<&str> {
    line.match_indices("style").find_map(|(at, _)| {
        if before(line, at).is_some_and(|c| is_word(c) || c == '-') {
            return None;
        }
        let rest = &line[at + 5..];
        if let Some(one) = rest.strip_prefix(':') {
            // Leptos `style:property="value"`
            let (property, value) = one.split_once('=')?;
            let value = value.strip_prefix('"')?.split('"').next()?;
            return (colour_property(property) && !colourless(value)).then_some(&line[at..]);
        }
        let value = rest.strip_prefix('=')?;
        let value = value.strip_prefix("move || ").unwrap_or(value);
        let value = value.strip_prefix("format!(").unwrap_or(value);
        let value = value.strip_prefix('"')?.split('"').next()?;
        value
            .split(';')
            .filter_map(|d| d.split_once(':'))
            .any(|(property, value)| colour_property(property.trim()) && !colourless(value.trim()))
            .then_some(&line[at..])
    })
}

/// The first colour literal in one line of component code, or `None`: a Tailwind palette
/// class (any family, and black or white), an arbitrary colour value, a hex colour, a CSS
/// colour function, an SVG `fill` / `stroke` literal or an inline style colour. Allowed are
/// theme tokens and the keywords of [`COLOUR_KEYWORDS`] (KICKOFF rule 9, review OT2D-3).
#[must_use]
pub fn colour_literal(line: &str) -> Option<&str> {
    palette_name(line, TAILWIND_PALETTES)
        .or_else(|| black_or_white(line))
        .or_else(|| arbitrary_colour(line))
        .or_else(|| hex_colour(line))
        .or_else(|| colour_function(line))
        .or_else(|| colour_attribute(line))
        .or_else(|| inline_style_colour(line))
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Every colour written down where it must not be (KICKOFF rule 9), as
/// `path:line: literal`: any colour literal in [`COMPONENT_DIRS`] (comment lines are not
/// read, they paint nothing) and a palette family of the brand in [`STYLE_DIRS`].
#[must_use]
pub fn colour_offenders(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for (dirs, component) in [(COMPONENT_DIRS, true), (STYLE_DIRS, false)] {
        for dir in dirs {
            let mut files = Vec::new();
            files_under(&root.join(dir), &mut files);
            for path in files {
                let Ok(bytes) = fs::read(&path) else {
                    continue;
                };
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                for (n, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
                    let found = if component {
                        (!line.trim_start().starts_with("//"))
                            .then(|| colour_literal(line))
                            .flatten()
                    } else {
                        palette_name(line, BRAND_PALETTES)
                    };
                    if let Some(literal) = found {
                        out.push(format!("{rel}:{}: {literal}", n + 1));
                    }
                }
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod colour_tests {
    use super::*;

    #[test]
    fn palette_classes_and_variables_are_found_and_tokens_are_not() {
        assert!(names_brand_palette("class=\"bg-violet-700 p-2\""));
        assert!(names_brand_palette("--x: var(--color-teal-50);"));
        assert!(!names_brand_palette("bg-primary text-group-watch-soft"));
        assert!(!names_brand_palette(
            "var(--color-zinc-900) blue-ish orange-"
        ));
    }

    /// One case per kind of literal (review OT2D-3).
    #[test]
    fn every_kind_of_colour_literal_is_found_in_component_code() {
        for (line, literal) in [
            ("class=\"bg-violet-700 p-2\"", "violet-7"),
            ("class=\"text-indigo-600\"", "indigo-6"),
            ("class=\"text-emerald-600\"", "emerald-6"),
            ("class=\"border-zinc-200\"", "zinc-2"),
            ("class=\"fixed inset-0 bg-black/60\"", "bg-black"),
            ("class=\"text-white\"", "text-white"),
            ("class=\"bg-[#7008e7]\"", "bg-[#7008e7]"),
            (
                "class=\"text-[oklch(0.5_0.2_290)]\"",
                "text-[oklch(0.5_0.2_290)]",
            ),
            ("class=\"fill-[rgb(112_8_231)]\"", "fill-[rgb(112_8_231)]"),
            ("class=\"bg-[red]\"", "bg-[red]"),
            ("<rect fill=\"#1447e6\"/>", "#1447e6"),
            ("<rect fill=\"red\"/>", "fill=\"red"),
            ("<path stroke=\"rgb(0 120 111)\"/>", "rgb("),
            ("<path stroke=\"teal\"/>", "stroke=\"teal"),
            ("<p style=\"color: rgb(1 2 3)\">", "rgb("),
            ("<p style=\"color:#a800b7\">", "#a800b7"),
            (
                "<p style=\"margin: 0; color: rebeccapurple\">",
                "style=\"margin: 0; color: rebeccapurple\">",
            ),
            (
                "<p style=format!(\"background: {c}\")>",
                "style=format!(\"background: {c}\")>",
            ),
            (
                "<p style:background-color=\"navy\">",
                "style:background-color=\"navy\">",
            ),
            ("let c = \"#fff\";", "#fff"),
            ("let c = \"#fafafa80\";", "#fafafa80"),
            ("let c = \"hsl(270 90% 50%)\";", "hsl("),
            (
                "let c = \"color-mix(in oklab, var(--primary) 90%, transparent)\";",
                "color-mix(",
            ),
        ] {
            assert_eq!(colour_literal(line), Some(literal), "{line}");
        }
    }

    #[test]
    fn tokens_keywords_lengths_and_look_alikes_are_not_colour_literals() {
        for line in [
            "class=\"bg-primary text-group-watch-soft border-border fill-current\"",
            "class=\"aria-[current=page]:text-primary max-w-[96rem] text-[0.8125rem]\"",
            "class=\"grid-cols-[auto_1fr] bottom-[calc(var(--tab-bar-height)+1rem)]\"",
            "class=\"bg-[var(--primary-soft)] text-[length:var(--size)] shadow-[0_1px_2px_var(--border)]\"",
            "<svg fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\">",
            "<rect fill=\"transparent\"/> <path stroke=\"var(--border)\"/>",
            "<div style=\"width: 40%; border: 1px solid var(--border); color: currentColor\">",
            "<div style=format!(\"width: {w}%\") style:height=\"2px\">",
            "<a href=\"#add\">  <a href=\"#adm-failed-h\">  &#160;  #[derive(Debug)]  r#\"x\"#",
            "id=\"shared-1\" data-x=\"unread-3\" class=\"whitespace-nowrap\"",
            "let label = brand::contrast::parse_color(value); collab(1); // white-label",
            "let fill = tint.fill(); let stroke = \"stroke-border\";",
        ] {
            assert_eq!(colour_literal(line), None, "{line}");
        }
    }

    #[test]
    fn detects_colours_in_component_code_and_brand_palettes_in_the_stylesheet() {
        let dir = std::env::temp_dir().join(format!("wl-colour-{}", std::process::id()));
        let ui = dir.join("src/rust/crates/ui/src");
        let server = dir.join("src/rust/apps/server/src");
        let style = dir.join("src/web/style");
        for d in [&ui, &server, &style] {
            fs::create_dir_all(d).unwrap();
        }
        fs::create_dir_all(dir.join("config/brand/default")).unwrap();
        fs::write(
            ui.join("a.rs"),
            "fn a() {}\nconst C: &str = \"text-fuchsia-700\";",
        )
        .unwrap();
        fs::write(
            ui.join("ok.rs"),
            "// was bg-[#fff]\nconst C: &str = \"text-group-watch\";",
        )
        .unwrap();
        fs::write(
            server.join("shell.rs"),
            "const S: &str = \"<body style=\\\"background:#fff\\\">\";",
        )
        .unwrap();
        fs::write(
            style.join("input.css"),
            "--muted: var(--color-zinc-100);\n--x: #052e22;\n--p: var(--color-violet-700);",
        )
        .unwrap();
        fs::write(
            dir.join("config/brand/default/brand.toml"),
            "primary = \"#7008e7\" # violet-700",
        )
        .unwrap();
        let found = colour_offenders(&dir);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            found,
            [
                "src/rust/apps/server/src/shell.rs:1: #fff",
                "src/rust/crates/ui/src/a.rs:2: fuchsia-7",
                "src/web/style/input.css:3: violet-7",
            ]
        );
    }
}
