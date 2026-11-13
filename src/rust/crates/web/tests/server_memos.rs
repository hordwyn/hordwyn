//! Race package F1 (race diagnosis 2.2): on the server no `Memo` reads a `Resource`. The
//! server renders on several threads; a memo that caches a resource read there can miss the
//! resource's answer and keep `None`, and the page is sent without a part the browser then
//! renders: a hydration panic (earlier cases: DEC-398, DEC-456). A value derived from a
//! resource comes from `pages::derived` (a memo only in the browser) or from a memo behind
//! `#[cfg(feature = "hydrate")]`.
//!
//! This test reads the crate's sources (comments and string literals blanked) and fails on a
//! `Memo::new`, `Memo::new_owning`, `Memo::new_with_compare` (any `Memo::new*`, also of
//! `ArcMemo`) outside such a `cfg` whose closure reads a resource:
//!
//! - a name bound to `Resource::new*(` in the enclosing function, or taken from the context
//!   (`expect_context::<T>()` / `use_context::<T>()` with a resource type `T`);
//! - a name declared with a resource type: `Resource<..>` and every alias of it in the crate
//!   (`type Res<T> = Resource<..>`, `type Data = Resource<..>`, `UserResource`, an alias of an
//!   alias), as a parameter, a typed `let` or a struct field (`lib.kinds.get()`,
//!   `self.data.get()`);
//! - a function or method of the same file whose body reads one of these (found, not listed:
//!   `kinds_now`, `header`), also through another such function;
//! - a local bound in the same function to a value that reads one of these: a
//!   `Signal::derive(..)`, a `derived(..)`, a closure (`let known = Signal::derive(move ||
//!   tags.get() ..)`, then `Memo::new(move |_| known.get())`).
//!
//! The enclosing function is found by its braces, so the methods of an `impl` block and
//! nested functions are scopes of their own.
//!
//! What it cannot see (RACE-3): a resource read behind a value that crosses a function
//! boundary without a resource type (a `Signal<..>` or `Callback` prop or parameter that the
//! caller fills from a resource, such as `widgets.rs` `of`; a `Signal` field of a struct; a
//! closure returned by a function of another file); a reader function of another file; a read
//! inside a macro other than `view!` that hides the receiver; a resource bound by a pattern
//! other than a plain name or a tuple. Such a memo still breaks the rule; the server detector
//! `library_page::every_page_of_many_at_once_has_its_header_rows` and the browser suite's
//! console guard are the nets behind this test.

use std::collections::BTreeSet;
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

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Replace `out[from..to]` by spaces, newlines kept.
fn blank(out: &mut [u8], from: usize, to: usize) {
    for b in &mut out[from..to] {
        if *b != b'\n' {
            *b = b' ';
        }
    }
}

/// From the `'` at byte `at`: the byte of the closing `'` of a character literal (`'x'`,
/// `'\n'`, `'\''`), or `None` for a lifetime (`'a`, `'static`).
fn char_literal_end(src: &str, at: usize) -> Option<usize> {
    let next = src[at + 1..].chars().next()?;
    if next == '\\' {
        return src.get(at + 3..)?.find('\'').map(|e| at + 3 + e);
    }
    let end = at + 1 + next.len_utf8();
    (src.as_bytes().get(end) == Some(&b'\'')).then_some(end)
}

/// From the `r` at byte `at`: the body and the end of a raw string literal, if one starts.
fn raw_string(src: &str, at: usize) -> Option<(usize, usize, usize)> {
    let rest = &src.as_bytes()[at..];
    let hashes = rest[1..].iter().take_while(|b| **b == b'#').count();
    if rest.get(1 + hashes) != Some(&b'"') {
        return None;
    }
    let close = format!("\"{}", "#".repeat(hashes));
    let body = at + 2 + hashes;
    let end = src[body..]
        .find(&close)
        .map_or(src.len(), |e| body + e + close.len());
    Some((body, end.saturating_sub(close.len()).max(body), end))
}

/// `src` with comments and the contents of string and character literals replaced by spaces
/// (newlines kept): byte offsets and line numbers stay, braces and names inside them go.
fn blanked(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        let prev_ident = src[..i].chars().next_back().is_some_and(is_ident);
        if rest.starts_with(b"//") {
            let end = src[i..].find('\n').map_or(bytes.len(), |e| i + e);
            blank(&mut out, i, end);
            i = end;
        } else if rest.starts_with(b"/*") {
            let end = src[i + 2..]
                .find("*/")
                .map_or(bytes.len(), |e| i + 2 + e + 2);
            blank(&mut out, i, end);
            i = end;
        } else if bytes[i] == b'r'
            && !prev_ident
            && let Some((body, body_end, end)) = raw_string(src, i)
        {
            blank(&mut out, body, body_end);
            i = end;
        } else if bytes[i] == b'"' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            blank(&mut out, i + 1, j.min(bytes.len()));
            i = j + 1;
        } else if bytes[i] == b'\''
            && let Some(end) = char_literal_end(src, i)
        {
            blank(&mut out, i + 1, end);
            i = end + 1;
        } else {
            i += 1;
        }
    }
    String::from_utf8(out).expect("blanking keeps the text valid")
}

/// The identifier that ends right before byte `end` of `src`.
fn ident_before(src: &str, end: usize) -> &str {
    let head = src[..end].trim_end();
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_ident(*c))
        .last()
        .map_or(head.len(), |(i, _)| i);
    &head[start..]
}

/// The identifier that starts at byte `at` of `src`.
fn ident_at(src: &str, at: usize) -> &str {
    let len = src[at..]
        .char_indices()
        .find(|(_, c)| !is_ident(*c))
        .map_or(src.len() - at, |(i, _)| i);
    &src[at..at + len]
}

/// Byte `open` holds an opening bracket: the offset of its partner (or the end).
fn partner(src: &str, open: usize) -> usize {
    let mut depth = 0usize;
    for (i, c) in src[open..].char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return open + i;
                }
            }
            _ => {}
        }
    }
    src.len()
}

/// From just after a `<`: the offset of its `>`.
fn partner_angle(src: &str, from: usize) -> usize {
    let mut depth = 1usize;
    for (i, c) in src[from..].char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return from + i;
                }
            }
            _ => {}
        }
    }
    src.len()
}

/// The occurrences of the word `name` in `src` (not part of a longer identifier).
fn words<'a>(src: &'a str, name: &'a str) -> impl Iterator<Item = usize> + 'a {
    src.match_indices(name).map(|(i, _)| i).filter(move |&i| {
        let before = src[..i].chars().next_back();
        let after = src[i + name.len()..].chars().next();
        before.is_none_or(|c| !is_ident(c)) && after.is_none_or(|c| !is_ident(c))
    })
}

/// The resource types of the crate: `Resource` and its aliases (`type X = Resource<..>`,
/// `type X<T> = Res<..>`), collected until no alias of an alias is left.
fn resource_types(sources: &[String]) -> BTreeSet<String> {
    let mut types = BTreeSet::from(["Resource".to_string()]);
    loop {
        let before = types.len();
        for src in sources {
            for at in words(src, "type") {
                let name = ident_at(src, at + "type ".len().min(src.len() - at)).to_string();
                let Some(eq) = src[at..].find(['=', ';']).map(|e| at + e) else {
                    continue;
                };
                if &src[eq..=eq] != "=" {
                    continue;
                }
                let rhs = src[eq + 1..].trim_start();
                let head = rhs.split('<').next().unwrap_or("");
                let last = head.rsplit("::").next().unwrap_or("").trim();
                if !name.is_empty() && types.contains(last) {
                    types.insert(name);
                }
            }
        }
        if types.len() == before {
            return types;
        }
    }
}

/// The names declared with a resource type in `src`: `x: Res<..>`, `x: Data`,
/// `x: crate::app::UserResource` (parameters, fields, typed `let`s).
fn typed_names(src: &str, types: &BTreeSet<String>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for ty in types {
        for at in words(src, ty) {
            // back over a path (`crate::app::`) to the `:` of the declaration
            let mut start = at;
            while start >= 2 && &src[start - 2..start] == "::" {
                start -= 2;
                start -= ident_before(src, start).len();
            }
            let head = src[..start].trim_end();
            if let Some(head) = head.strip_suffix(':')
                && !head.ends_with(':')
            {
                let name = ident_before(head, head.len());
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

/// The names bound in `scope` to a resource: `let x = Resource::new(..)` (any
/// `Resource::new*`) and `let x = expect_context::<T>()` / `use_context::<T>()` with a
/// resource type `T`.
fn bound_names(scope: &str, types: &BTreeSet<String>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut bind = |at: usize| {
        let head = scope[..at].trim_end();
        if let Some(head) = head.strip_suffix('=')
            && !head.ends_with(['=', '!', '<', '>'])
        {
            let name = ident_before(head, head.len());
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    };
    for (at, _) in scope.match_indices("Resource::new") {
        bind(at);
    }
    for marker in ["expect_context::<", "use_context::<"] {
        for (at, _) in scope.match_indices(marker) {
            let ty_start = at + marker.len();
            let ty = &scope[ty_start..partner_angle(scope, ty_start)];
            let last = ty.rsplit("::").next().unwrap_or("").trim();
            if types.contains(last) {
                bind(at);
            }
        }
    }
    names
}

/// A function of the file: its name and its text from `fn` to its closing brace.
struct Func {
    name: String,
    start: usize,
    end: usize,
}

impl Func {
    fn text<'a>(&self, code: &'a str) -> &'a str {
        &code[self.start..self.end.min(code.len())]
    }
}

/// Every function and method of `code` (a blanked file), by its braces.
fn functions(code: &str) -> Vec<Func> {
    let mut found = Vec::new();
    for at in words(code, "fn") {
        let after_fn = at + 2;
        let name = ident_at(
            code,
            after_fn + (code[after_fn..].len() - code[after_fn..].trim_start().len()),
        );
        if name.is_empty() {
            continue;
        }
        // the body: the first `{` after the parameter list (a `;` first: a declaration)
        let Some(params) = code[at..].find('(').map(|p| at + p) else {
            continue;
        };
        let after = partner(code, params);
        let Some(open) = code[after..].find(['{', ';']).map(|o| after + o) else {
            continue;
        };
        if &code[open..=open] == ";" {
            continue;
        }
        found.push(Func {
            name: name.to_string(),
            start: at,
            end: partner(code, open),
        });
    }
    found
}

/// The text of the outermost function around byte `at` (the whole file outside any).
fn scope_of<'a>(code: &'a str, funcs: &[Func], at: usize) -> &'a str {
    funcs
        .iter()
        .filter(|f| f.start <= at && at <= f.end)
        .min_by_key(|f| f.start)
        .map_or(code, |f| f.text(code))
}

const READS: [&str; 7] = [
    ".get()",
    ".get_untracked()",
    ".read()",
    ".read_untracked()",
    ".with(",
    ".with_untracked(",
    ".await",
];

/// Does `body` read `name` (`name.get()`, `name.with(` and so on)? As a field
/// (`lib.name.get()`) when `field`, else as a name of its own; a derived value (`call`) is
/// also read by calling it (`known()`).
fn reads(body: &str, name: &str, field: bool, call: bool) -> bool {
    body.match_indices(name).any(|(i, _)| {
        let before = body[..i].chars().next_back();
        let placed = if field {
            before == Some('.')
        } else {
            before.is_none_or(|c| !is_ident(c) && c != '.')
        };
        let rest = &body[i + name.len()..];
        placed && (READS.iter().any(|r| rest.starts_with(r)) || (call && rest.starts_with('(')))
    })
}

/// Does `body` call the function `name` (`name(` or `.name(`; not `Type::name(`, which is
/// another type's function of the same name)?
fn calls(body: &str, name: &str) -> bool {
    words(body, name).any(|i| body[i + name.len()..].starts_with('(') && !body[..i].ends_with("::"))
}

/// Names of reader functions that are not followed: they are the names of the reads
/// themselves (`x.get()` of any signal would count).
const NOT_READERS: [&str; 6] = [
    "get",
    "get_untracked",
    "read",
    "read_untracked",
    "with",
    "with_untracked",
];

/// What one file offers to every scope in it.
struct FileFacts {
    /// Fields and other names declared with a resource type anywhere in the file.
    fields: BTreeSet<String>,
    /// Functions of the file that read a resource.
    readers: BTreeSet<String>,
}

/// What `body` reads of a resource, given the names of its scope.
fn what_it_reads(
    body: &str,
    locals: &BTreeSet<String>,
    derived: &BTreeSet<String>,
    facts: &FileFacts,
) -> Option<String> {
    locals
        .iter()
        .find(|n| reads(body, n, false, false))
        .or_else(|| derived.iter().find(|n| reads(body, n, false, true)))
        .or_else(|| facts.fields.iter().find(|n| reads(body, n, true, false)))
        .cloned()
        .or_else(|| {
            facts
                .readers
                .iter()
                .find(|r| calls(body, r))
                .map(|r| format!("{r}("))
        })
}

/// From byte `from`: the `;` that ends the statement (outside brackets), or the end.
fn statement_end(src: &str, from: usize) -> usize {
    let mut depth = 0i32;
    for (i, c) in src[from..].char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth < 0 {
                    return from + i;
                }
            }
            ';' if depth == 0 => return from + i,
            _ => {}
        }
    }
    src.len()
}

/// The `let` bindings of `scope`: the names of the pattern and the value.
fn lets(scope: &str) -> Vec<(Vec<String>, &str)> {
    words(scope, "let")
        .filter_map(|at| {
            let eq = scope[at..].find('=').map(|e| at + e)?;
            if scope[eq + 1..].starts_with(['=', '>']) {
                return None;
            }
            let pattern = scope[at + 3..eq].split(':').next().unwrap_or("");
            let names = pattern
                .split(|c: char| !is_ident(c))
                .filter(|n| !n.is_empty() && *n != "mut" && *n != "ref")
                .map(str::to_string)
                .collect();
            Some((names, &scope[eq + 1..statement_end(scope, eq + 1)]))
        })
        .collect()
}

/// The `let` bindings of `scope` whose value reads a resource, directly or through another
/// such binding: a `Signal::derive`, a `derived`, a closure, a value.
fn derived_names(scope: &str, locals: &BTreeSet<String>, facts: &FileFacts) -> BTreeSet<String> {
    let bindings = lets(scope);
    let mut derived = BTreeSet::new();
    loop {
        let before = derived.len();
        for (names, value) in &bindings {
            if what_it_reads(value, locals, &derived, facts).is_some() {
                derived.extend(names.iter().cloned());
            }
        }
        if derived.len() == before {
            return derived;
        }
    }
}

/// The resource names of `scope`: bound there or declared with a resource type.
fn locals_of(scope: &str, types: &BTreeSet<String>) -> BTreeSet<String> {
    let mut locals = bound_names(scope, types);
    locals.extend(typed_names(scope, types));
    locals
}

/// The functions of the file that read a resource, also through each other.
fn readers(
    code: &str,
    funcs: &[Func],
    types: &BTreeSet<String>,
    fields: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut facts = FileFacts {
        fields: fields.clone(),
        readers: BTreeSet::new(),
    };
    loop {
        let before = facts.readers.len();
        for f in funcs
            .iter()
            .filter(|f| !NOT_READERS.contains(&f.name.as_str()))
        {
            let body = f.text(code);
            let locals = locals_of(body, types);
            let derived = derived_names(body, &locals, &facts);
            if what_it_reads(body, &locals, &derived, &facts).is_some() {
                facts.readers.insert(f.name.clone());
            }
        }
        if facts.readers.len() == before {
            return facts.readers;
        }
    }
}

/// The attribute of code that only the browser builds.
const BROWSER_ONLY: &str = "#[cfg(feature = \"hydrate\")]";

/// The blanked `code` of `src` as the server builds it: the statement or item after each
/// [`BROWSER_ONLY`] blanked too (a reader function whose read sits behind it reads nothing on
/// the server, `history.rs` `latest_page`).
fn server_view(src: &str, code: &str) -> String {
    let mut out = code.as_bytes().to_vec();
    for (at, _) in src.match_indices(BROWSER_ONLY) {
        let after = at + BROWSER_ONLY.len();
        let rest = code[after..].trim_start();
        let start = code.len() - rest.len();
        let block = [
            "fn ",
            "pub ",
            "pub(crate) ",
            "async ",
            "mod ",
            "impl ",
            "if ",
            "match ",
            "for ",
            "while ",
            "loop",
            "{",
        ]
        .iter()
        .any(|p| rest.starts_with(p));
        let end = match code[start..].find(['{', ';']).map(|o| start + o) {
            Some(o) if block && &code[o..=o] == "{" => partner(code, o) + 1,
            _ => statement_end(code, start) + 1,
        };
        blank(&mut out, at, end.min(code.len()));
    }
    String::from_utf8(out).expect("blanking keeps the text valid")
}

/// Is the statement that holds byte `at` behind [`BROWSER_ONLY`] (the attribute on the line
/// before it)? Read in the original text: the blanked one has no strings.
fn browser_only(src: &str, at: usize) -> bool {
    let line_start = src[..at].rfind('\n').map_or(0, |i| i + 1);
    let before = src[..line_start].trim_end();
    let prev = before.rsplit('\n').next().unwrap_or("").trim();
    prev == BROWSER_ONLY
}

/// The memos of `src` that read a resource on the server path, as `line: what`.
fn offenders(src: &str, types: &BTreeSet<String>) -> Vec<String> {
    let code = server_view(src, &blanked(src));
    let funcs = functions(&code);
    let fields = typed_names(&code, types);
    let facts = FileFacts {
        readers: readers(&code, &funcs, types, &fields),
        fields,
    };
    let mut found = Vec::new();
    for (at, _) in code.match_indices("Memo::new") {
        let open = at + "Memo::".len() + ident_at(&code, at + "Memo::".len()).len();
        if !code[open..].starts_with('(') || browser_only(src, at) {
            continue;
        }
        let body = &code[open..partner(&code, open)];
        let scope = scope_of(&code, &funcs, at);
        let locals = locals_of(scope, types);
        let derived = derived_names(scope, &locals, &facts);
        if let Some(what) = what_it_reads(body, &locals, &derived, &facts) {
            let line = src[..at].matches('\n').count() + 1;
            found.push(format!("{line}: reads `{what}`"));
        }
    }
    found
}

/// The aliases of the crate as the examples use them.
fn example_types() -> BTreeSet<String> {
    resource_types(&["type Res<T> = Resource<Result<T, ServerFnError>>;\n\
         type Data = Resource<Result<Vec<ImportDto>, ServerFnError>>;\n\
         pub type UserResource = Resource<Result<Option<UserDto>, ServerFnError>>;\n"
        .to_string()])
}

#[test]
fn the_rule_finds_a_memo_over_a_resource_and_spares_the_browser_only_one() {
    let types = example_types();
    let found = |src: &str| offenders(src, &types);
    let bad = "fn f() {\n    let data = Resource::new(|| (), |()| api::x());\n\
               let m = Memo::new(move |_| data.get().and_then(Result::ok));\n}\n";
    assert_eq!(found(bad), vec!["3: reads `data`".to_string()]);
    let field = "struct Lib { kinds: Res<KindsDto> }\n\
                 fn f(lib: Lib) { let m = Memo::new(move |_| lib.kinds.get()); }\n";
    assert_eq!(found(field), vec!["2: reads `kinds`".to_string()]);
    let browser = "fn f() {\n    let data = Resource::new(|| (), |()| api::x());\n\
                   #[cfg(feature = \"hydrate\")]\n\
                   let m = Memo::new(move |_| data.get());\n}\n";
    assert_eq!(found(browser).len(), 0);
    let signal = "fn f() {\n    let data = Resource::new(|| (), |()| api::x());\n\
                  let m = Memo::new(move |_| state.get());\n\
                  let d = derived(move || data.get());\n}\n";
    assert_eq!(found(signal).len(), 0);
    let commented = "fn f(data: Data) {\n    // Memo::new(move |_| data.get()) was here\n\
                     let s = \"Memo::new(move |_| data.get())\";\n    let c = '{';\n}\n";
    assert_eq!(found(commented).len(), 0);
}

#[test]
fn the_rule_sees_aliases_the_context_and_the_other_memo_constructors() {
    let types = example_types();
    let found = |src: &str| offenders(src, &types);
    // RACE-3 (1): an alias without generics, as a parameter
    let alias = "fn Imports(data: Data) {\n    let m = Memo::new(move |_| data.get());\n}\n";
    assert_eq!(found(alias), vec!["2: reads `data`".to_string()]);
    // an alias of an alias, found in the sources
    let chained =
        resource_types(&["type Res<T> = Resource<T>;\ntype Rows = Res<Vec<Row>>;\n".to_string()]);
    assert!(chained.contains("Rows"), "{chained:?}");
    // RACE-3 (2): a resource from the context, with its path
    let context = "fn Account() {\n    let user = expect_context::<crate::app::UserResource>();\n\
                   let name = Memo::new(move |_| user.get().and_then(Result::ok));\n}\n";
    assert_eq!(found(context), vec!["3: reads `user`".to_string()]);
    let typed_context = "fn Account() {\n    let user: UserResource = expect_context();\n\
                         let name = Memo::new(move |_| user.get());\n}\n";
    assert_eq!(found(typed_context), vec!["3: reads `user`".to_string()]);
    // RACE-3 (4): the other constructors
    let owning =
        "fn f(data: Data) {\n    let m = Memo::new_owning(move |_| (data.get(), true));\n}\n";
    assert_eq!(found(owning), vec!["2: reads `data`".to_string()]);
    let compare = "fn f(data: Data) {\n    let m = Memo::new_with_compare(move |_| data.get(), |a, b| a == b);\n}\n";
    assert_eq!(found(compare), vec!["2: reads `data`".to_string()]);
    let arc = "fn f(data: Data) {\n    let m = ArcMemo::new(move |_| data.get());\n}\n";
    assert_eq!(found(arc), vec!["2: reads `data`".to_string()]);
}

#[test]
fn the_rule_sees_reads_through_derived_values_and_reader_functions() {
    let types = example_types();
    let found = |src: &str| offenders(src, &types);
    // RACE-3 (3): a `Signal::derive`, a `derived` and a closure over a resource
    let derive = "fn Library() {\n    let tags = Resource::new(|| (), |()| api::tags());\n\
                  let known = Signal::derive(move || tags.get().and_then(Result::ok));\n\
                  let count = Memo::new(move |_| known.get().map(|v| v.len()));\n}\n";
    assert_eq!(found(derive), vec!["4: reads `known`".to_string()]);
    let helper = "fn f(data: Data) {\n    let now = derived(move || data.get());\n\
                  let m = Memo::new(move |_| now.with(|v| v.is_some()));\n}\n";
    assert_eq!(found(helper), vec!["3: reads `now`".to_string()]);
    let closure = "fn f(data: Data) {\n    let now = move || data.get().and_then(Result::ok);\n\
                   let later = move || now().map(|v| v.len());\n\
                   let m = Memo::new(move |_| later());\n}\n";
    assert_eq!(found(closure), vec!["4: reads `later`".to_string()]);
    // a reader function is found by its body, also through another one
    let reader = "struct Lib { kinds: Res<KindsDto> }\n\
                  impl Lib {\n    fn kinds_now(&self) -> Option<KindsDto> { self.kinds.get()?.ok() }\n\
                  fn first(&self) -> Option<Kind> { self.kinds_now()?.kinds.first().cloned() }\n}\n\
                  fn f(lib: Lib) {\n    let m = Memo::new(move |_| lib.first());\n}\n";
    assert_eq!(found(reader), vec!["7: reads `first(`".to_string()]);
    // a function whose read is behind the browser's `cfg` reads nothing on the server
    let browser_reader = "fn latest(rows: Data) -> Memo<u8> {\n    #[cfg(feature = \"hydrate\")]\n\
                          let page = Memo::new(move |_| rows.get());\n    #[cfg(not(feature = \"hydrate\"))]\n\
                          let page = Memo::new(move |_| 1);\n    page\n}\n\
                          fn f(rows: Data) {\n    let page = latest(rows);\n\
                          let m = Memo::new(move |_| page.get());\n}\n";
    assert_eq!(found(browser_reader).len(), 0);
}

#[test]
fn the_rule_reads_a_method_in_its_own_scope() {
    let types = example_types();
    // RACE-3 (5): the parameter of a method, not a name of the function above the `impl`
    let method = "fn before() {\n    let data = 1;\n}\n\
                  impl Page {\n    fn view(&self, data: Data) {\n        let m = Memo::new(move |_| data.get());\n    }\n}\n";
    assert_eq!(
        offenders(method, &types),
        vec!["6: reads `data`".to_string()]
    );
    // and a resource of another function does not leak into it
    let other = "fn first(data: Data) {\n    let _ = data;\n}\n\
                 impl Page {\n    fn view(&self, data: RwSignal<u8>) {\n        let m = Memo::new(move |_| data.get());\n    }\n}\n";
    assert_eq!(offenders(other, &types).len(), 0);
}

#[test]
fn no_memo_reads_a_resource_on_the_server() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    files.sort();
    let sources: Vec<String> = files
        .iter()
        .map(|f| blanked(&fs::read_to_string(f).expect("read a source file")))
        .collect();
    let types = resource_types(&sources);
    assert!(
        types.contains("UserResource") && types.contains("Res") && types.contains("Data"),
        "the aliases of the crate are found: {types:?}"
    );
    let mut found = Vec::new();
    for file in &files {
        let src = fs::read_to_string(file).expect("read a source file");
        let name = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .display()
            .to_string();
        found.extend(
            offenders(&src, &types)
                .into_iter()
                .map(|o| format!("{name}:{o}")),
        );
    }
    assert_eq!(
        found,
        Vec::<String>::new(),
        "a memo that reads a resource is evaluated on the server: use `pages::derived` \
         (race package, the rule next to DEC-398 and DEC-456)"
    );
}
