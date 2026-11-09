//! The Library page's state as it lives in the URL (design.md 3.5): every filter, the text,
//! the sort and the view are query parameters, so a view can be bookmarked and shared and the
//! back button undoes a change. Pure functions only; the page in `library.rs` renders them.
//!
//! Parameters, in the canonical order of [`LibraryState::query`]:
//! `group`, `kind` (repeated), `status`, `tag` (repeated), `facet` (repeated, `attr:value`),
//! `q`, `sort`, `dir` (`asc`/`desc`, only when it differs from the sort's default), `view`
//! (`list`; grid is the default).

use std::fmt::Write as _;

use contracts::{FacetFilterDto, KindsDto, LibraryCountsDto, LibraryQueryDto};

/// Sort keys the server answers (`LibraryQueryDto.sort`), in menu order. Progress, next
/// release and priority are not built yet (phase 1 API).
pub const SORTS: [&str; 6] = ["last_activity", "title", "added", "year", "rating", "plays"];

/// Rows per request; "Load more" asks for the next block (DEC-222).
pub const PAGE_SIZE: u32 = 48;

/// Kinds whose art is square (album covers, podcast art, board game boxes).
pub const SQUARE_KINDS: [&str; 3] = ["music", "podcast", "board_game"];

/// The kind group ids (`domain::kind::KindGroup`); a URL naming another group is ignored.
pub const GROUP_IDS: [&str; 5] = ["watch", "read", "listen", "play", "other"];

#[must_use]
pub fn square_art(kind: &str) -> bool {
    SQUARE_KINDS.contains(&kind)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Grid,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryState {
    /// `None`: all groups.
    pub group: Option<String>,
    pub kinds: Vec<String>,
    /// `None`: every status.
    pub status: Option<String>,
    pub tags: Vec<String>,
    /// `(attribute, value)`.
    pub facets: Vec<(String, String)>,
    pub q: String,
    /// `None`: last activity.
    pub sort: Option<String>,
    /// `None`: the sort's default direction.
    pub descending: Option<bool>,
    pub view: View,
}

/// One active filter, for the removable badges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    Tag(String),
    Facet(String, String),
    Text(String),
}

fn clean(v: &str) -> Option<String> {
    let v = v.trim();
    (!v.is_empty()).then(|| v.to_string())
}

fn push_unique(list: &mut Vec<String>, v: String) {
    if !list.contains(&v) {
        list.push(v);
    }
}

/// Percent-encode a query component (RFC 3986 unreserved characters stay).
#[must_use]
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

impl LibraryState {
    /// From the query parameters; `get_all(name)` answers every value of one parameter.
    /// Unknown sorts, views and directions and empty values are ignored.
    pub fn from_lookup(get_all: impl Fn(&str) -> Vec<String>) -> Self {
        let one = |k: &str| get_all(k).into_iter().find_map(|v| clean(&v));
        let many = |k: &str| {
            let mut out = Vec::new();
            for v in get_all(k).iter().filter_map(|v| clean(v)) {
                push_unique(&mut out, v);
            }
            out
        };
        let mut tags: Vec<String> = Vec::new();
        for t in many("tag") {
            if !tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                tags.push(t);
            }
        }
        let mut facets = Vec::new();
        for f in many("facet") {
            if let Some((a, v)) = f.split_once(':')
                && let (Some(a), Some(v)) = (clean(a), clean(v))
                && !facets.contains(&(a.clone(), v.clone()))
            {
                facets.push((a, v));
            }
        }
        Self {
            group: one("group").filter(|g| GROUP_IDS.contains(&g.as_str())),
            kinds: many("kind"),
            status: one("status").filter(|s| s != "all"),
            tags,
            facets,
            q: one("q").unwrap_or_default(),
            sort: one("sort").filter(|s| SORTS.contains(&s.as_str()) && s != "last_activity"),
            descending: match one("dir").as_deref() {
                Some("asc") => Some(false),
                Some("desc") => Some(true),
                _ => None,
            },
            view: match one("view").as_deref() {
                Some("list") => View::List,
                _ => View::Grid,
            },
        }
        .normalised()
    }

    /// From `(name, value)` pairs (tests, links).
    #[must_use]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self::from_lookup(|k| {
            pairs
                .iter()
                .filter(|(n, _)| *n == k)
                .map(|(_, v)| (*v).to_string())
                .collect()
        })
    }

    /// The sort in effect.
    #[must_use]
    pub fn sort_key(&self) -> &str {
        self.sort.as_deref().unwrap_or("last_activity")
    }

    /// The server's default direction for a sort: titles A to Z, the rest newest or highest
    /// first.
    #[must_use]
    pub fn default_descending(sort: &str) -> bool {
        sort != "title"
    }

    #[must_use]
    pub fn is_descending(&self) -> bool {
        self.descending
            .unwrap_or_else(|| Self::default_descending(self.sort_key()))
    }

    /// Drop a direction that equals the default, so equal states have equal URLs.
    fn normalised(mut self) -> Self {
        if self.descending == Some(Self::default_descending(self.sort_key())) {
            self.descending = None;
        }
        self
    }

    /// The query string without `?` (empty for the default state).
    #[must_use]
    pub fn query(&self) -> String {
        let mut p: Vec<(&str, String)> = Vec::new();
        if let Some(g) = &self.group {
            p.push(("group", g.clone()));
        }
        p.extend(self.kinds.iter().map(|k| ("kind", k.clone())));
        if let Some(s) = &self.status {
            p.push(("status", s.clone()));
        }
        p.extend(self.tags.iter().map(|t| ("tag", t.clone())));
        p.extend(
            self.facets
                .iter()
                .map(|(a, v)| ("facet", format!("{a}:{v}"))),
        );
        if !self.q.trim().is_empty() {
            p.push(("q", self.q.trim().to_string()));
        }
        if let Some(s) = &self.sort {
            p.push(("sort", s.clone()));
        }
        if let Some(d) = self.descending {
            p.push(("dir", if d { "desc" } else { "asc" }.to_string()));
        }
        if self.view == View::List {
            p.push(("view", "list".into()));
        }
        p.iter()
            .map(|(k, v)| format!("{k}={}", encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// The page URL of this state.
    #[must_use]
    pub fn href(&self) -> String {
        let q = self.query();
        if q.is_empty() {
            "/library".into()
        } else {
            format!("/library?{q}")
        }
    }

    /// The server query for one block of rows.
    #[must_use]
    pub fn query_dto(&self, limit: u32) -> LibraryQueryDto {
        LibraryQueryDto {
            group: self.group.clone(),
            kinds: self.kinds.clone(),
            statuses: self.status.iter().cloned().collect(),
            tags: self.tags.clone(),
            facets: self
                .facets
                .iter()
                .map(|(a, v)| FacetFilterDto {
                    attribute: a.clone(),
                    value: v.clone(),
                })
                .collect(),
            q: clean(&self.q),
            sort: Some(self.sort_key().to_string()),
            descending: Some(self.is_descending()),
            after: None,
            limit,
        }
    }

    /// Another group: kinds and facets belong to a group, so they are cleared.
    #[must_use]
    pub fn with_group(&self, group: Option<String>) -> Self {
        let group = group.filter(|g| g != "all");
        if group == self.group {
            return self.clone();
        }
        Self {
            group,
            kinds: Vec::new(),
            facets: Vec::new(),
            ..self.clone()
        }
    }

    /// Toggle one kind chip; facets only apply to a single kind, so they are cleared.
    #[must_use]
    pub fn toggled_kind(&self, kind: &str) -> Self {
        let mut s = self.clone();
        if let Some(i) = s.kinds.iter().position(|k| k == kind) {
            s.kinds.remove(i);
        } else {
            s.kinds.push(kind.to_string());
        }
        s.facets.clear();
        s
    }

    #[must_use]
    pub fn without_kinds(&self) -> Self {
        Self {
            kinds: Vec::new(),
            facets: Vec::new(),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn with_status(&self, status: Option<String>) -> Self {
        Self {
            status: status.filter(|s| s != "all"),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn toggled_tag(&self, tag: &str) -> Self {
        let mut s = self.clone();
        if let Some(i) = s.tags.iter().position(|t| t.eq_ignore_ascii_case(tag)) {
            s.tags.remove(i);
        } else {
            s.tags.push(tag.to_string());
        }
        s
    }

    /// Set (or with an empty value, clear) the filter on one facet attribute.
    #[must_use]
    pub fn with_facet(&self, attribute: &str, value: &str) -> Self {
        let mut s = self.clone();
        s.facets.retain(|(a, _)| a != attribute);
        if let Some(v) = clean(value) {
            s.facets.push((attribute.to_string(), v));
        }
        s
    }

    #[must_use]
    pub fn with_text(&self, q: &str) -> Self {
        Self {
            q: q.trim().to_string(),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn with_sort(&self, sort: &str) -> Self {
        let sort = SORTS.contains(&sort).then(|| sort.to_string());
        Self {
            sort: sort.filter(|s| s != "last_activity"),
            descending: None,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn reversed(&self) -> Self {
        Self {
            descending: Some(!self.is_descending()),
            ..self.clone()
        }
        .normalised()
    }

    #[must_use]
    pub fn with_view(&self, view: View) -> Self {
        Self {
            view,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn without(&self, f: &Filter) -> Self {
        match f {
            Filter::Tag(t) => self.toggled_tag(t),
            Filter::Facet(a, _) => self.with_facet(a, ""),
            Filter::Text(_) => self.with_text(""),
        }
    }

    /// Tags, facets and text (the filters behind the Filters button and the text box).
    #[must_use]
    pub fn filters(&self) -> Vec<Filter> {
        let mut out: Vec<Filter> = self.tags.iter().cloned().map(Filter::Tag).collect();
        out.extend(
            self.facets
                .iter()
                .map(|(a, v)| Filter::Facet(a.clone(), v.clone())),
        );
        if !self.q.is_empty() {
            out.push(Filter::Text(self.q.clone()));
        }
        out
    }

    /// Every filter cleared; group, kinds, sort and view stay (they choose what to look at,
    /// not what to leave out).
    #[must_use]
    pub fn cleared(&self) -> Self {
        Self {
            status: None,
            tags: Vec::new(),
            facets: Vec::new(),
            q: String::new(),
            ..self.clone()
        }
    }

    /// Number of filters behind the Filters button (tags and facets).
    #[must_use]
    pub fn panel_filters(&self) -> usize {
        self.tags.len() + self.facets.len()
    }

    /// The single kind whose facets the Filters panel offers.
    #[must_use]
    pub fn facet_kind(&self) -> Option<&str> {
        match self.kinds.as_slice() {
            [k] => Some(k.as_str()),
            _ => None,
        }
    }
}

/// Is `kind` in `group` (`None`: every group)?
fn in_group(kinds: &KindsDto, group: Option<&str>, kind: &str) -> bool {
    group.is_none_or(|g| kinds.kinds.iter().any(|k| k.id == kind && k.group == g))
}

/// Kind chips under the group: kinds with entries, in the kinds' order, plus selected ones
/// (so they can be switched off). Counts ignore the status, so chips stay put while the
/// status tabs change.
#[must_use]
pub fn kind_chips(
    kinds: &KindsDto,
    counts: &LibraryCountsDto,
    s: &LibraryState,
) -> Vec<(String, u64)> {
    kinds
        .kinds
        .iter()
        .filter(|k| in_group(kinds, s.group.as_deref(), &k.id))
        .filter_map(|k| {
            let n = counts
                .kinds
                .iter()
                .find(|c| c.key == k.id)
                .map_or(0, |c| c.count);
            (n > 0 || s.kinds.contains(&k.id)).then(|| (k.id.clone(), n))
        })
        .collect()
}

/// Entries under the group and kind selection, per status (statuses with entries, core
/// first, plus the selected one); and their sum for the "All" tab.
#[must_use]
pub fn status_tabs(
    kinds: &KindsDto,
    counts: &LibraryCountsDto,
    s: &LibraryState,
) -> (u64, Vec<(String, u64)>) {
    let cells = counts.cells.iter().filter(|c| {
        s.group.as_deref().is_none_or(|g| c.group == g)
            && (s.kinds.is_empty() || s.kinds.contains(&c.kind))
            && in_group(kinds, s.group.as_deref(), &c.kind)
    });
    let mut per: Vec<(String, u64)> = counts
        .statuses
        .iter()
        .map(|st| (st.key.clone(), 0))
        .collect();
    let mut total = 0;
    for c in cells {
        total += c.count;
        match per.iter_mut().find(|(k, _)| *k == c.status) {
            Some((_, n)) => *n += c.count,
            None => per.push((c.status.clone(), c.count)),
        }
    }
    if let Some(sel) = &s.status
        && !per.iter().any(|(k, _)| k == sel)
    {
        per.push((sel.clone(), 0));
    }
    per.retain(|(k, n)| *n > 0 || s.status.as_ref() == Some(k));
    (total, per)
}

/// The nearest status with entries when the selected one is empty (the empty state offers
/// it): the first other tab with a count.
#[must_use]
pub fn nearest_status(tabs: &[(String, u64)], s: &LibraryState) -> Option<String> {
    tabs.iter()
        .find(|(k, n)| *n > 0 && s.status.as_ref() != Some(k))
        .map(|(k, _)| k.clone())
}

/// Fluent key of a sort's label (literal keys, so the locale check sees them).
#[must_use]
pub fn sort_label_key(sort: &str) -> &'static str {
    match sort {
        "title" => "library-sort-title",
        "added" => "library-sort-added",
        "year" => "library-sort-year",
        "rating" => "library-sort-rating",
        "plays" => "library-sort-plays",
        _ => "library-sort-last-activity",
    }
}

#[cfg(test)]
mod tests {
    use contracts::{CountCellDto, CountDto, KindDto, KindGroupDto};

    use super::*;

    fn kinds() -> KindsDto {
        let k = |id: &str, group: &str| KindDto {
            id: id.into(),
            group: group.into(),
            label_key: format!("kind-{id}"),
            progress_model: "single".into(),
            statuses: vec![],
            facets: vec!["genres".into()],
        };
        KindsDto {
            groups: vec![
                KindGroupDto {
                    id: "watch".into(),
                    label_key: "kind-group-watch".into(),
                    kinds: vec!["movie".into(), "show".into()],
                },
                KindGroupDto {
                    id: "read".into(),
                    label_key: "kind-group-read".into(),
                    kinds: vec!["book".into()],
                },
            ],
            kinds: vec![k("movie", "watch"), k("show", "watch"), k("book", "read")],
        }
    }

    fn counts() -> LibraryCountsDto {
        let cell = |kind: &str, group: &str, status: &str, count| CountCellDto {
            kind: kind.into(),
            group: group.into(),
            status: status.into(),
            count,
        };
        let c = |key: &str, count| CountDto {
            key: key.into(),
            count,
        };
        LibraryCountsDto {
            total: 9,
            groups: vec![c("watch", 6), c("read", 3)],
            kinds: vec![c("movie", 4), c("show", 2), c("book", 3)],
            statuses: vec![c("planned", 4), c("in_progress", 3), c("completed", 2)],
            cells: vec![
                cell("movie", "watch", "planned", 3),
                cell("movie", "watch", "completed", 1),
                cell("show", "watch", "in_progress", 2),
                cell("book", "read", "planned", 1),
                cell("book", "read", "in_progress", 1),
                cell("book", "read", "completed", 1),
            ],
        }
    }

    #[test]
    fn the_url_round_trips() {
        let s = LibraryState::from_pairs(&[
            ("group", "watch"),
            ("kind", "show"),
            ("kind", "movie"),
            ("status", "in_progress"),
            ("tag", "cosy"),
            ("tag", "rainy day"),
            ("facet", "genres:Drama"),
            ("q", "north"),
            ("sort", "title"),
            ("dir", "desc"),
            ("view", "list"),
        ]);
        assert_eq!(s.group.as_deref(), Some("watch"));
        assert_eq!(s.kinds, ["show", "movie"]);
        assert_eq!(s.tags, ["cosy", "rainy day"]);
        assert_eq!(s.facets, [("genres".to_string(), "Drama".to_string())]);
        assert_eq!(s.view, View::List);
        assert!(s.is_descending());
        let q = s.query();
        assert_eq!(
            q,
            "group=watch&kind=show&kind=movie&status=in_progress&tag=cosy&tag=rainy%20day\
             &facet=genres%3ADrama&q=north&sort=title&dir=desc&view=list"
        );
        // parsing what we wrote gives the same state (the router decodes the values)
        let pairs: Vec<(String, String)> = q
            .split('&')
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap();
                (k.to_string(), v.replace("%20", " ").replace("%3A", ":"))
            })
            .collect();
        let refs: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(LibraryState::from_pairs(&refs), s);
    }

    #[test]
    fn defaults_and_junk_leave_no_trace_in_the_url() {
        let s = LibraryState::from_pairs(&[
            ("group", "all"),
            ("status", "all"),
            ("sort", "last_activity"),
            ("dir", "desc"),
            ("view", "grid"),
            ("sort", "nonsense"),
            ("tag", " "),
            ("tag", "Cosy"),
            ("tag", "cosy"),
            ("facet", "no-colon"),
            ("kind", "show"),
            ("kind", "show"),
        ]);
        assert_eq!(s.query(), "kind=show&tag=Cosy");
        assert_eq!(LibraryState::default().href(), "/library");
        // ascending titles are the default: no `dir`
        let t = LibraryState::from_pairs(&[("sort", "title"), ("dir", "asc")]);
        assert_eq!(t.href(), "/library?sort=title");
        assert_eq!(t.reversed().href(), "/library?sort=title&dir=desc");
        assert_eq!(t.reversed().reversed(), t);
    }

    #[test]
    fn changes_keep_what_still_applies() {
        let s = LibraryState::from_pairs(&[
            ("group", "watch"),
            ("kind", "show"),
            ("facet", "genres:Drama"),
            ("tag", "cosy"),
            ("status", "planned"),
        ]);
        let read = s.with_group(Some("read".into()));
        assert_eq!(read.query(), "group=read&status=planned&tag=cosy");
        assert_eq!(s.with_group(Some("watch".into())), s);
        assert_eq!(s.toggled_kind("movie").kinds, ["show", "movie"]);
        assert_eq!(s.toggled_kind("show").kinds.len(), 0);
        assert_eq!(s.toggled_kind("show").facets.len(), 0);
        assert_eq!(s.toggled_tag("COSY").tags.len(), 0);
        assert_eq!(s.with_facet("genres", "Comedy").facets[0].1, "Comedy");
        assert_eq!(s.with_facet("genres", "").facets.len(), 0);
        assert_eq!(s.facet_kind(), Some("show"));
        assert_eq!(s.panel_filters(), 2);
        let c = s.with_text("north").cleared();
        assert_eq!(c.query(), "group=watch&kind=show");
        assert_eq!(
            s.with_text(" north ").filters().last(),
            Some(&Filter::Text("north".into()))
        );
        assert_eq!(s.without(&Filter::Tag("cosy".into())).tags.len(), 0);
        assert_eq!(s.with_sort("year").with_sort("bogus").sort, None);
    }

    #[test]
    fn the_server_query_carries_the_state() {
        let s = LibraryState::from_pairs(&[("kind", "book"), ("status", "planned"), ("q", " x ")]);
        let q = s.query_dto(PAGE_SIZE);
        assert_eq!(q.kinds, ["book"]);
        assert_eq!(q.statuses, ["planned"]);
        assert_eq!(q.q.as_deref(), Some("x"));
        assert_eq!(q.sort.as_deref(), Some("last_activity"));
        assert_eq!(q.descending, Some(true));
        assert_eq!((q.after, q.limit), (None, 48));
        let t = LibraryState::from_pairs(&[("sort", "title")]).query_dto(1);
        assert_eq!(t.descending, Some(false));
    }

    #[test]
    fn chips_and_tabs_count_under_the_selection() {
        let (k, c) = (kinds(), counts());
        let all = LibraryState::default();
        assert_eq!(
            kind_chips(&k, &c, &all),
            [("movie".into(), 4), ("show".into(), 2), ("book".into(), 3)]
        );
        let watch = all.with_group(Some("watch".into()));
        assert_eq!(kind_chips(&k, &c, &watch).len(), 2);
        let (total, tabs) = status_tabs(&k, &c, &watch);
        assert_eq!(total, 6);
        assert_eq!(
            tabs,
            [
                ("planned".into(), 3),
                ("in_progress".into(), 2),
                ("completed".into(), 1)
            ]
        );
        let shows = watch
            .toggled_kind("show")
            .with_status(Some("planned".into()));
        let (total, tabs) = status_tabs(&k, &c, &shows);
        assert_eq!(total, 2);
        // the selected tab stays even when empty; the empty state offers the nearest one
        assert_eq!(tabs, [("planned".into(), 0), ("in_progress".into(), 2)]);
        assert_eq!(
            nearest_status(&tabs, &shows).as_deref(),
            Some("in_progress")
        );
    }

    #[test]
    fn sort_labels_and_square_art() {
        assert_eq!(sort_label_key("plays"), "library-sort-plays");
        assert_eq!(
            sort_label_key("last_activity"),
            "library-sort-last-activity"
        );
        assert!(square_art("board_game") && square_art("music") && square_art("podcast"));
        assert!(!square_art("movie"));
        assert_eq!(encode("a b:c/\u{e4}"), "a%20b%3Ac%2F%C3%A4");
    }

    #[test]
    fn unknown_groups_in_a_shared_url_are_ignored() {
        // UX1-7
        assert_eq!(LibraryState::from_pairs(&[("group", "foo")]).group, None);
        assert_eq!(
            LibraryState::from_pairs(&[("group", "listen")])
                .group
                .as_deref(),
            Some("listen")
        );
    }
}
