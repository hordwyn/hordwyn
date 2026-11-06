//! Locale checks (owner ruling on i18n, DEC-171): parity of the Fluent `.ftl` files of every
//! crate with messages (same keys, same variables, no empty values), keys referenced in code
//! exist and existing keys are used, and no hard-coded user-visible text in `view!` markup.
//!
//! The source scans work on a small Rust lexer ([`lex`]) that masks comments and string
//! literals, so braces and markup are matched on code only.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fluent_syntax::ast;

/// What a parity check needs to know about one message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    /// Variables (`$name`) used anywhere in the value, the attributes and selectors.
    pub vars: BTreeSet<String>,
    /// No value and no attributes, or only whitespace text.
    pub empty: bool,
}

fn inline_vars(e: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match e {
        ast::InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_string());
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            for p in &arguments.positional {
                inline_vars(p, out);
            }
            for n in &arguments.named {
                inline_vars(&n.value, out);
            }
        }
        ast::InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => {
            for p in &arguments.positional {
                inline_vars(p, out);
            }
        }
        ast::InlineExpression::Placeable { expression } => expr_vars(expression, out),
        _ => {}
    }
}

fn expr_vars(e: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
    match e {
        ast::Expression::Select { selector, variants } => {
            inline_vars(selector, out);
            for v in variants {
                pattern_vars(&v.value, out);
            }
        }
        ast::Expression::Inline(i) => inline_vars(i, out),
    }
}

fn pattern_vars(p: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
    for el in &p.elements {
        if let ast::PatternElement::Placeable { expression } = el {
            expr_vars(expression, out);
        }
    }
}

fn pattern_empty(p: &ast::Pattern<&str>) -> bool {
    p.elements.iter().all(|el| match el {
        ast::PatternElement::TextElement { value } => value.trim().is_empty(),
        ast::PatternElement::Placeable { .. } => false,
    })
}

/// The messages of one `.ftl` source.
///
/// # Errors
/// Parse errors and duplicate keys (Fluent would silently keep the last one).
pub fn messages(src: &str) -> Result<BTreeMap<String, Message>, Vec<String>> {
    let res = match fluent_syntax::parser::parse(src) {
        Ok(r) => r,
        Err((_, errors)) => return Err(errors.iter().map(|e| format!("{e:?}")).collect()),
    };
    let mut out = BTreeMap::new();
    let mut problems = Vec::new();
    for entry in &res.body {
        let ast::Entry::Message(m) = entry else {
            continue;
        };
        let mut vars = BTreeSet::new();
        if let Some(v) = &m.value {
            pattern_vars(v, &mut vars);
        }
        for a in &m.attributes {
            pattern_vars(&a.value, &mut vars);
        }
        let empty = m.value.as_ref().is_none_or(pattern_empty)
            && m.attributes.iter().all(|a| pattern_empty(&a.value));
        if out
            .insert(m.id.name.to_string(), Message { vars, empty })
            .is_some()
        {
            problems.push(format!("duplicate key {}", m.id.name));
        }
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

/// Parity problems between `reference` (the first locale) and the others: missing or extra
/// keys, different variables, empty values. `name` labels the catalogue in the messages.
#[must_use]
pub fn parity(name: &str, catalogues: &[(&str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut parsed = Vec::new();
    for (locale, src) in catalogues {
        match messages(src) {
            Ok(m) => parsed.push((*locale, m)),
            Err(e) => problems.extend(e.into_iter().map(|e| format!("{name}/{locale}: {e}"))),
        }
    }
    let Some((first, reference)) = parsed.first() else {
        return problems;
    };
    for (locale, msgs) in &parsed {
        for (key, m) in msgs {
            if m.empty {
                problems.push(format!("{name}/{locale}: {key} is empty"));
            }
            match reference.get(key) {
                None => problems.push(format!("{name}/{locale}: {key} is not in {first}")),
                Some(r) if r.vars != m.vars => problems.push(format!(
                    "{name}/{locale}: {key} uses {:?}, {first} uses {:?}",
                    m.vars, r.vars
                )),
                Some(_) => {}
            }
        }
        for key in reference.keys() {
            if !msgs.contains_key(key) {
                problems.push(format!("{name}/{locale}: {key} is missing"));
            }
        }
    }
    problems
}

// ---------------------------------------------------------------- Rust source scanning

/// A string literal in Rust source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    /// The literal's text as written (escapes not resolved).
    pub text: String,
    /// Byte offsets of the whole token (quotes included) in the source.
    pub start: usize,
    pub end: usize,
    pub line: usize,
}

/// The source with comments and string literal tokens replaced by spaces (same byte length,
/// newlines kept), and the string literals.
#[must_use]
pub fn lex(src: &str) -> (String, Vec<Literal>) {
    let b = src.as_bytes();
    let mut masked = b.to_vec();
    let mut lits = Vec::new();
    let mut i = 0;
    let blank = |m: &mut Vec<u8>, from: usize, to: usize| {
        for c in &mut m[from..to] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let end = src[i..].find('\n').map_or(b.len(), |n| i + n);
            blank(&mut masked, i, end);
            i = end;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let end = src[i + 2..].find("*/").map_or(b.len(), |n| i + 2 + n + 2);
            blank(&mut masked, i, end);
            i = end;
        } else if c == b'\'' {
            // char literal or lifetime
            if b.get(i + 1) == Some(&b'\\') {
                let end = src[i + 2..].find('\'').map_or(b.len(), |n| i + 2 + n + 1);
                blank(&mut masked, i, end);
                i = end;
            } else if let Some(ch) = src[i + 1..].chars().next()
                && b.get(i + 1 + ch.len_utf8()) == Some(&b'\'')
            {
                let end = i + 1 + ch.len_utf8() + 1;
                blank(&mut masked, i, end);
                i = end;
            } else {
                i += 1;
            }
        } else if (c == b'r' || c == b'b')
            && (i == 0 || !ident(b[i - 1]))
            && raw_start(&b[i..]).is_some()
        {
            let (prefix, hashes) = raw_start(&b[i..]).unwrap_or((0, 0));
            let close = format!("\"{}", "#".repeat(hashes));
            let body = i + prefix;
            let end = src[body..]
                .find(&close)
                .map_or(b.len(), |n| body + n + close.len());
            lits.push(Literal {
                text: src[body..end.saturating_sub(close.len()).max(body)].to_string(),
                start: i,
                end,
                line: src[..i].matches('\n').count() + 1,
            });
            blank(&mut masked, i, end);
            i = end;
        } else if c == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(b.len());
            lits.push(Literal {
                text: src[i + 1..j.min(b.len())].to_string(),
                start: i,
                end,
                line: src[..i].matches('\n').count() + 1,
            });
            blank(&mut masked, i, end);
            i = end;
        } else {
            i += 1;
        }
    }
    (String::from_utf8(masked).unwrap_or_default(), lits)
}

/// `r"`, `r#"`, `br"`, ...: length of the prefix up to and including the quote, and the
/// number of hashes.
fn raw_start(b: &[u8]) -> Option<(usize, usize)> {
    let mut i = usize::from(b.first() == Some(&b'b'));
    if b.get(i) != Some(&b'r') {
        return None;
    }
    i += 1;
    let hashes = b[i..].iter().take_while(|c| **c == b'#').count();
    i += hashes;
    (b.get(i) == Some(&b'"')).then_some((i + 1, hashes))
}

/// Byte ranges of the bodies of `view!` macros in masked source.
fn view_bodies(masked: &str) -> Vec<(usize, usize)> {
    let b = masked.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(n) = masked[from..].find("view!") {
        let at = from + n + "view!".len();
        let Some(open) = masked[at..].find('{').map(|o| at + o) else {
            break;
        };
        let mut depth = 0usize;
        let mut end = b.len();
        for (k, c) in b.iter().enumerate().skip(open) {
            match c {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = k;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push((open, end));
        from = open + 1;
    }
    out
}

/// Attributes and props whose value is shown or read out to people.
fn visible_attribute(name: &str) -> bool {
    let name = name.trim_start_matches("attr:");
    matches!(
        name,
        "aria-label" | "aria-description" | "title" | "placeholder" | "alt" | "label" | "text"
    ) || name.ends_with("_label")
        || name.ends_with("-label")
}

/// Does the text contain a word (two letters in a row, or any non-ASCII letter)?
fn has_word(text: &str) -> bool {
    let mut run = 0;
    for c in text.chars() {
        if c.is_alphabetic() && !c.is_ascii() {
            return true;
        }
        run = if c.is_ascii_alphabetic() { run + 1 } else { 0 };
        if run >= 2 {
            return true;
        }
    }
    false
}

/// The attribute or prop a literal is the value of (`aria-label` for `aria-label="..."`).
fn attribute_of(before: &str) -> Option<String> {
    let b = before.strip_suffix('=')?;
    let mut name: Vec<char> = b
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || "_-:".contains(*c))
        .collect();
    name.reverse();
    Some(name.into_iter().collect())
}

/// Where the unit tests start (`#[cfg(test)]`); their literals are not app code.
fn code_end(masked: &str) -> usize {
    masked.find("#[cfg(test)]").unwrap_or(masked.len())
}

/// Text literals inside `view!` markup that people would read: text nodes (`>"..."` or
/// `"..."<`) and the values of visible attributes (`aria-label="..."`, `title`, `label`,
/// `*_label`, ...). Returns `(line, text)`.
#[must_use]
pub fn hard_coded(src: &str) -> Vec<(usize, String)> {
    let (masked, lits) = lex(src);
    let bodies = view_bodies(&masked);
    let end = code_end(&masked);
    lits.into_iter()
        .filter(|l| l.end <= end && bodies.iter().any(|(s, e)| l.start > *s && l.end <= *e))
        .filter(|l| has_word(&l.text))
        .filter(|l| {
            let before = masked[..l.start].trim_end();
            let after = masked[l.end..].trim_start();
            // `=>` and `->` end match arms and return types, not tags
            let tag_end =
                before.ends_with('>') && !before.ends_with("=>") && !before.ends_with("->");
            let text_node = tag_end || after.starts_with('<');
            text_node || attribute_of(before).is_some_and(|a| visible_attribute(&a))
        })
        .map(|l| (l.line, l.text))
        .collect()
}

/// Is `s` shaped like a message key (`nav-home`, `auth-error-invalid_totp`)?
#[must_use]
pub fn key_shaped(s: &str) -> bool {
    let mut parts = s.split('-');
    let first = parts.next().unwrap_or_default();
    let rest: Vec<_> = parts.collect();
    first.starts_with(|c: char| c.is_ascii_lowercase())
        && first
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && !rest.is_empty()
        && rest.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

/// Attributes whose values are element ids, never message keys.
const ID_ATTRIBUTES: &[&str] = &[
    "id",
    "for",
    "r#for",
    "aria-labelledby",
    "aria-describedby",
    "aria-controls",
    "attr:aria-controls",
    "data-testid",
];

/// Keys used in code: key-shaped literals whose first segment is a prefix of some catalogue
/// key (element ids such as `id="error-summary"` excluded); and the dynamic families, the
/// fixed part of `format!("prefix-{..}")` literals. Unit tests are not scanned.
#[must_use]
pub fn references(src: &str, prefixes: &BTreeSet<String>) -> (BTreeSet<String>, BTreeSet<String>) {
    let (masked, lits) = lex(src);
    let end = code_end(&masked);
    let mut keys = BTreeSet::new();
    let mut families = BTreeSet::new();
    for l in lits.into_iter().filter(|l| l.end <= end) {
        let first = l.text.split('-').next().unwrap_or_default();
        if !prefixes.contains(first) {
            continue;
        }
        if key_shaped(&l.text) {
            let id = attribute_of(masked[..l.start].trim_end())
                .is_some_and(|a| ID_ATTRIBUTES.contains(&a.as_str()));
            if !id {
                keys.insert(l.text);
            }
        } else if let Some((fixed, _)) = l.text.split_once('{')
            && fixed.ends_with('-')
            && key_shaped(&format!("{fixed}x"))
        {
            families.insert(fixed.to_string());
        }
    }
    (keys, families)
}

/// The `.rs` files under `dir`, sorted.
#[must_use]
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_variables_and_finds_empty_and_duplicate_messages() {
        let m = messages(
            "a = Hi { $name }\nb = { $count ->\n    [one] One\n   *[other] { NUMBER($count) } and { $x }\n}\nc =\n    .title = T\n",
        )
        .unwrap();
        assert_eq!(
            m["a"].vars.iter().collect::<Vec<_>>(),
            [&"name".to_string()]
        );
        assert_eq!(m["b"].vars.len(), 2);
        assert!(!m["c"].empty);
        assert!(messages("a = x\na = y\n").is_err());
    }

    #[test]
    fn parity_reports_missing_extra_and_different_variables() {
        let p = parity(
            "t",
            &[
                ("en", "a = A { $n }\nb = B\n"),
                ("de", "a = A { $m }\nc = C\n"),
            ],
        );
        assert!(p.iter().any(|x| x.contains("de: a uses")), "{p:?}");
        assert!(p.iter().any(|x| x.contains("de: b is missing")), "{p:?}");
        assert!(p.iter().any(|x| x.contains("de: c is not in en")), "{p:?}");
        assert_eq!(
            parity("t", &[("en", "a = A\n"), ("de", "a = B\n")]).len(),
            0
        );
    }

    #[test]
    fn lexer_masks_comments_chars_and_raw_strings() {
        let src = "// \"no\"\nlet a = '\"'; let b = r#\"x \"y\" z\"#; fn f<'a>() { \"k\" }";
        let (masked, lits) = lex(src);
        let texts: Vec<_> = lits.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["x \"y\" z", "k"]);
        assert_eq!(masked.len(), src.len());
        assert!(!masked.contains("no"));
    }

    #[test]
    fn finds_text_nodes_and_visible_attributes_in_views_only() {
        let src = r#"
fn a() { let x = "Not in a view"; view! { <p class="mt-2 text-sm">"Hello there"</p>
    <b aria-label="Close it" id="abc-def">{t("k-x")}</b>
    <Button size=ButtonSize::Sm>"OK" " - "</Button>
    <x dismiss_label="Dismiss"/>
    <y aria-expanded=move || if o { "true" } else { "false" }/> } }
"#;
        let found: Vec<_> = hard_coded(src).into_iter().map(|(_, t)| t).collect();
        assert_eq!(found, ["Hello there", "Close it", "OK", "Dismiss"]);
    }

    #[test]
    fn finds_key_references_and_dynamic_families() {
        let prefixes: BTreeSet<String> = ["nav", "auth"].iter().map(|s| (*s).to_string()).collect();
        let (keys, fams) = references(
            r#"t("nav-home"); let k = format!("auth-error-{code}"); id="channel-f1"; "#,
            &prefixes,
        );
        assert_eq!(keys.into_iter().collect::<Vec<_>>(), ["nav-home"]);
        assert_eq!(fams.into_iter().collect::<Vec<_>>(), ["auth-error-"]);
        assert!(key_shaped("auth-error-invalid_totp"));
        assert!(!key_shaped("Auth-x"));
        assert!(!key_shaped("nav"));
    }
}
