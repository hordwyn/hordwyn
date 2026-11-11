//! OT-12 (DEC-497 leftover): a link or a button that looks like a button takes its classes
//! from `ui::Button` / `ui::ButtonLink`, so two of them in one row cannot drift apart again.
//! This test reads the crate's sources and fails on a hand-written button look.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir)
        .expect("read the source directory")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Where the string literal that opens at `start` (the index after its quote) ends: the index
/// of the closing quote. A raw string (`hashes` given) ends at a quote followed by that many
/// `#`; an ordinary one at the first quote that is not escaped.
fn literal_end(src: &[char], start: usize, hashes: Option<usize>) -> usize {
    let mut i = start;
    while i < src.len() {
        match (src[i], hashes) {
            ('\\', None) => i += 1,
            ('"', None) => return i,
            ('"', Some(n)) if src[i + 1..].iter().take(n).filter(|c| **c == '#').count() == n => {
                return i;
            }
            _ => {}
        }
        i += 1;
    }
    src.len()
}

/// The string literals of a Rust source. A quote that does not open one is skipped: the char
/// literal `'"'`, an escaped quote inside a string, the quotes inside a raw string, and
/// whatever a comment says (review OT2A-13: one `'"'` hid every class list after it).
fn literals(src: &str) -> Vec<String> {
    let src: Vec<char> = src.chars().collect();
    let at = |i: usize| src.get(i).copied();
    let word = |i: usize| i > 0 && (src[i - 1].is_alphanumeric() || src[i - 1] == '_');
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        if c == '/' && at(i + 1) == Some('/') {
            // a line comment
            while i < src.len() && src[i] != '\n' {
                i += 1;
            }
        } else if c == '\'' && at(i + 1) == Some('\\') {
            // an escaped char literal: '\'', '\\', '\u{..}'
            i += 3;
            while i < src.len() && src[i] != '\'' {
                i += 1;
            }
        } else if c == '\'' && at(i + 2) == Some('\'') {
            // a plain char literal, also '"'
            i += 2;
        } else if c == '"' || (c == 'r' && !word(i) && matches!(at(i + 1), Some('"' | '#'))) {
            let mut hashes = None;
            if c == 'r' {
                let n = src[i + 1..].iter().take_while(|c| **c == '#').count();
                if at(i + 1 + n) != Some('"') {
                    // `r#ident`
                    i += 1;
                    continue;
                }
                hashes = Some(n);
                i += 1 + n;
            }
            let end = literal_end(&src, i + 1, hashes);
            out.push(src[i + 1..end].iter().collect());
            i = end + hashes.unwrap_or(0);
        }
        i += 1;
    }
    out
}

/// The class lists (string literals) in `src` that spell out a button: a rounded box with
/// button text weight, horizontal padding and a filled or an outlined look.
fn button_looks(src: &str) -> Vec<String> {
    let has = |classes: &str, name: &str| classes.split_whitespace().any(|c| c == name);
    let padded = |classes: &str| {
        classes.split_whitespace().any(|c| {
            c.strip_prefix("px-")
                .is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
        })
    };
    literals(src)
        .into_iter()
        // a class list inside a raw string (`class="..."` in markup kept as text) counts too
        .flat_map(|literal| {
            literal
                .split('"')
                .map(|part| part.replace("\\\n", " "))
                .collect::<Vec<_>>()
        })
        .filter(|c| {
            has(c, "rounded-md")
                && has(c, "font-medium")
                && padded(c)
                && [
                    "bg-primary",
                    "bg-secondary",
                    "border-input",
                    "border-border",
                ]
                .iter()
                .any(|look| has(c, look))
        })
        .collect()
}

#[test]
fn the_detector_knows_a_hand_written_button() {
    let filled = r#"<a href="/x" class="inline-flex h-9 items-center rounded-md bg-primary px-4 text-sm font-medium">"#;
    assert_eq!(button_looks(filled).len(), 1);
    let outline = r#"<button class="rounded-md border border-input px-3 text-sm font-medium">"#;
    assert_eq!(button_looks(outline).len(), 1);
    // a text field, a card and a component call are none
    let field = r#"<input class="flex h-9 w-full rounded-md border border-input bg-background px-3 text-sm"/>"#;
    assert_eq!(button_looks(field).len(), 0);
    let call = r#"<ButtonLink href="/x" class="w-fit">"#;
    assert_eq!(button_looks(call).len(), 0);
}

/// OT2A-13: a quote that is not a string delimiter (a char literal, an escaped quote, a raw
/// string) does not hide the class lists after it, and the secondary looks count too.
#[test]
fn the_detector_is_not_thrown_off_by_other_quotes() {
    let after_char =
        r#"s.replace('"', "&quot;"); <a class="rounded-md bg-primary px-4 font-medium">"#;
    assert_eq!(button_looks(after_char).len(), 1);
    let after_escape =
        r#"let a = "say "hi" to"; let b = "rounded-md border border-input px-3 font-medium";"#;
    assert_eq!(button_looks(after_escape).len(), 1);
    let raw = "let a = r#\"<a class=\"x\">\"#; let b = \"rounded-md bg-primary px-4 font-medium\";";
    assert_eq!(button_looks(raw).len(), 1);
    let in_raw = "let a = r#\"<a class=\"rounded-md bg-primary px-4 font-medium\">\"#;";
    assert_eq!(button_looks(in_raw).len(), 1);
    let lifetime =
        r#"fn f<'a>(x: &'a str) -> &'a str { "rounded-md bg-primary px-4 font-medium" }"#;
    assert_eq!(button_looks(lifetime).len(), 1);
    let secondary =
        r#"<a class="inline-flex h-9 rounded-md bg-secondary px-4 text-sm font-medium">"#;
    assert_eq!(button_looks(secondary).len(), 1);
    let bordered =
        r#"<a class="inline-flex h-9 rounded-md border border-border px-4 text-sm font-medium">"#;
    assert_eq!(button_looks(bordered).len(), 1);
}

#[test]
fn no_hand_written_button_classes_in_the_web_crate() {
    // the file picker's label: a `<label>` that shows the focus of its hidden input (`peer-`
    // classes), which neither component can be
    let allowed = ["peer-focus-visible:ring-2"];
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 20, "sources not found");
    let mut found = Vec::new();
    for file in files {
        let src = fs::read_to_string(&file).expect("read a source file");
        for classes in button_looks(&src) {
            if !allowed.iter().any(|a| classes.contains(a)) {
                found.push(format!("{}: {classes}", file.display()));
            }
        }
    }
    assert_eq!(
        found.len(),
        0,
        "use ui::Button or ui::ButtonLink instead of:\n{}",
        found.join("\n")
    );
}
