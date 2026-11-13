# Lane site: decisions (2026-11-13)

Numbered L-S-n; the orchestrator renumbers them into docs/DECISIONS.md when merging.

## L-S-1: the website is a workspace app `website` that renders Leptos views to static files

Context: the owner wants a website that looks like the application and is built "with our
crate structure"; clarified on 2026-11-13: it must reuse the `ui` crate (components, tokens,
font, icons), not carry a design of its own. The reference project (the owner's
`tenants/eventsanitaeter/website`) is a single Leptos crate with an Axum SSR server, Tailwind
3, Markdown content with front matter and Fluent strings; it has no CI and no Pages deploy.
Decision: `src/rust/apps/website` (package and binary `website`; not `site`, because
cargo-leptos already owns `target/site`). It depends on `ui`, `brand`, `leptos` (ssr),
`pulldown-cmark`, `toml`, `serde`, `anyhow`. Views are rendered once with `to_html()` and
written as `index.html` files: no server, no hydration, no WebAssembly. Site-only layout
(top bar, hero, sections, footer) lives in the website crate; nothing was added to `ui`,
because none of it fits the application.
Rejected: a separate workspace under `src/site/` (a second lock file and a second copy of
the lints, and it could not use `ui` by path without leaving the architecture test);
cargo-leptos with a second `[[workspace.metadata.leptos]]` project and a server (GitLab
Pages serves files only); hydration for the theme button (2 MB of WebAssembly for one
button).
Architecture test: unchanged and green; `website` is an app, apps are unrestricted, `ui`
and `brand` gained no dependency.

## L-S-2: taken from the reference: structure and patterns only

Taken: the page model (landing page with hero, fact bar, card grid, split section, step
strip, closing call to action; four-column footer; sticky top bar with a drawer on phones;
light page header on the inner pages), the layout primitives `Container`/`Section`, typed
content loaded from Markdown with front matter, one test file that renders pages and
asserts on the markup, and the stance "no CDN, no tracker, self-hosted fonts".
Not taken: its styling stack (Tailwind 3 config, Inter, its colours), any text, name,
photo, address, phone number or other personal or client content. The reference has no
licence file; it is the owner's own project and nothing was copied verbatim.

## L-S-3: content in TOML and Markdown, not Fluent

Context: the application keeps every string in Fluent files with a parity check.
Decision: the website's text is page content (paragraphs, lists of cards, legal pages), so
it lives in `content/<locale>/*.toml` and `*.md`. Parity of the locales is a test of the
website crate (same keys, kinds, sections, open slots). English at `/`, German at `/de/`
with the same slugs. English content is ASCII; German content has umlauts (the same
exception as the German Fluent file).
Rejected: Fluent for chrome strings only (two systems for 26 labels).

## L-S-4: the product name lives in `content/site.toml` only

The white-label guard looks for the former codename and stays untouched; it does not know
the product name. For the website the rule is: the name is written once, in
`src/rust/apps/website/content/site.toml`; content files use `{name}`, Rust code, the
stylesheet and the script never name it. Test:
`the_product_name_is_written_down_in_site_toml_only`. The website reads theme and font from
`config/brand/default` (not the name: the default brand still carries the codename until
package v1-release renames it), so it follows the application's colours without a copy.

## L-S-5: the colour guard covers the website

`xtask::whitelabel::COMPONENT_DIRS` gains `src/rust/apps/website/src`: the website writes
no colour literal either, only theme tokens. `src/web/style/site.css` is already under
`STYLE_DIRS`. Seen red with a seeded `text-red-500`.

## L-S-6: one stylesheet for the website: `input.css` plus its own sources

`src/web/style/site.css` imports the application's `input.css` (tokens, type scale, dark
theme) and adds `@source` for the website crate and the typography of Markdown text
(`.prose-site`). The result contains the application's utilities as well: 65 kB, 12 kB
gzipped. Accepted for now; a split of `input.css` into tokens and sources would make it
smaller and is a change to the application's stylesheet (not done in this lane).
The file is published under a content fingerprint (`/assets/site.<fnv1a>.css`).

## L-S-7: theme and menu without the `ui` ThemeToggle

`ui::ThemeToggle` needs hydration. The website has an inline boot script (the application's
pattern: stored choice, else the system setting) and `src/web/site/theme.js` (0.6 kB) for
the button; the phone menu is a `<details>` element. Without JavaScript the site is
complete, in the light theme, and the button is hidden.

## L-S-8: owner slots are marked PLACEHOLDER and block publishing (extended by L-S-13)

Imprint, privacy, the security contact and `security.txt` need facts only the owner has.
Nothing is invented: each is a line marked `PLACEHOLDER`, the page shows a notice,
`website check` lists the slots, and `website build --strict` (the `pages` job) fails while
one is open. Without a contact `/.well-known/security.txt` is published as comments only.

## L-S-9: publishing is opt-in (CI variable `PAGES_DEPLOY`)

A job named `pages` deploys as soon as it runs on the default branch. The owner asked for
no deployment from this lane, so the job has the rule
`$CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH && $PAGES_DEPLOY == "true"`. The jobs `website`
(build) and `website-browser` (axe and layout checks) run always. Antora is copied to
`/docs/` from the existing `antora` job. DNS records and steps: docs/website/README.md.

## L-S-10: new dependency pulldown-cmark 0.13.6

MIT, `default-features = false, features = ["html"]`; brings `pulldown-cmark-escape`,
`unicase` and uses `bitflags`, `memchr` already in the tree. Used only by `website`.

## L-S-11: v1 claims on the website (replaced by L-S-12)

The features page lists the six kinds of the owner's v1 ruling (v1-plan.md section 0, DEC-641)
with their source; books (Open Library), board games (Wikidata) and manual entry are still
being built in other lanes when this is written, so the text must be re-read against the
`v1` tag before publishing. The test `v1_claims_only_the_six_kinds_with_a_source` pins the
list. Hub address: `hub.hordwyn.com` as the lane brief says (docs/research has an earlier
`hub.hordwyn.org` note).

# Review 2026-11-13 (SITE-1 .. SITE-20): decisions L-S-12 ..

## L-S-12: release gate - `status` per kind and feature, confirmed by `release.toml` (SITE-1)

Every kind (`features.toml`, `home.toml`) and every feature of "What comes with it" carries
`status = "shipped" | "coming"` and an id. `content/release.toml` (`version`, `confirmed`)
is set by the owner or the orchestrator at the `v1` tag. Strict mode refuses while `version`
is empty, a shipped id is not confirmed, or a confirmed id names nothing. Until then books,
board games and manual entry are `coming`; movies, shows, podcasts and video games stay
`shipped` (they are on main) and block until confirmed. The fact bar counts the shipped kinds
(`{kinds_shipped}`) instead of saying six. Text that is only true with a release sits in
`<!-- shipped:ID -->` blocks of the text pages and appears with the confirmation: the
security-headers sentence (`security_headers`, true once sec1 is in a release) and the
credits of Open Library (`book`) and Wikidata (`board_game`). The test that pinned the six
kinds is replaced by tests of the gate.
Rejected: `released = false` alone (does not tie the single claims to the tag); deriving the
status from the connectors crate (the website would depend on the application's crates).

## L-S-13: the strict gate is `website::gate::publish_blockers`, tested (SITE-4, SITE-7, SITE-8)

One function returns every blocker; `website check [--strict] [--report <file>]` and
`website build --strict` use it. Blockers: open slots (`.toml` and `.md` only; the `#` rule
for comments applies to TOML only, so a Markdown heading counts); a page with open slots (its
notice is derived from them, not from the page head, so the two cannot drift); `security.txt`
values (contact `mailto:`/`https:`, expiry RFC 3339, in the future, at most 366 days ahead);
an empty or non-https `source_url` (now a marked slot; the guide starts with `git clone
{source}`); the release gate (L-S-12); the documentation (title must contain the product
name - so "Tracker documentation" blocks - and `kroki-fetch-diagram: true`). The security
page takes its contact from `site.toml` (`{security_contact}`), so page and `security.txt`
cannot differ. New dependency edge: `website` uses the workspace's `chrono` (already in the
lock file) for the date check.

## L-S-14: `pages` publishes the tested build and compiles nothing (SITE-3)

`website` builds once and writes the gate report (`build/website-blockers.txt`);
`website-browser` tests that build; `pages` needs `website`, `website-browser` and `antora`,
and runs `src/sh/pages-assemble.sh`: refuse on a missing or non-empty report, copy the build
and the Antora site, check every `/docs/` link of the website against the Antora output
(SITE-8) and that no published page loads from another server (SITE-5). The three failure
branches and the pass were run locally against the earlier local Antora output.
Not done: `pages: true` (SITE-17) - a key that cannot be validated without a pipeline and
would break every pipeline if wrong; the job name works.

## L-S-15: documentation diagrams are fetched at build time (SITE-5)

`docs/antora-playbook.yml`: `kroki-fetch-diagram: true`, so the `antora` job stores the three
mermaid diagrams as files and `/docs/` loads nothing from kroki.io. Not run locally: building
it here would send the diagram sources of a private repository to kroki.io from this
session; the CI job has done that implicitly through every reader so far. Backstops if the
option does not hold in CI: the gate checks the playbook value, and `pages-assemble.sh` fails
on any foreign `src` in the published pages (seen red against the old local output, which
has the three kroki.io addresses). The `antora` job still asks kroki.io at build time; a
local renderer (mermaid-cli needs a browser in the Antora image) was not set up.

## L-S-16: real captures of the application, made by a script (SITE-19)

`content/shots/<view>-<locale>-<theme>.png`: six views (`home-top` in the hero, `home` wide,
`item`, `episodes`, `library`, `log` as phones), English and German, light and dark, 24
files, 2.1 MB. Made by `src/web/site-tests/capture/capture.mjs` against the application built
in this worktree, a fresh database (`ls_shots`, dropped afterwards) and a mock catalogue
with invented titles that serves no image: every cover is the application's placeholder, no
provider image is committed (KICKOFF rule 8). The script runs the application under the
product name of the website's `site.toml` (a temporary copy of the default brand), because
the default brand still carries the working name. Each picture has a written text
alternative per language and its pixel size; light and dark are two `<img>` of which the
theme shows one, lazy below the hero. The hero picture is also the `og:image`. No logo, mark
or favicon was drawn (rule 11); the wordmark is the name in the brand font, as before.
The features page is one grid of six kind cards (each names its group), and "What comes with
it" is two columns: no row is left half empty.

## L-S-17: content corrections (SITE-2, SITE-6, SITE-9, SITE-10, SITE-16, SITE-20)

- Importers as they are: Trakt from its JSON files (history, ratings, watchlist; no network
  call), CSV / JSON Lines in the documented format. The "previous tracker export" importer is
  built from one export file of the owner and is not advertised; no product is named for it.
- No "year in review"; no promise of podcast episode lists; manual entry is a feature marked
  coming, not a line in the kind cards.
- Name sources: the unverifiable third source is dropped; the ATLAS source is named
  "Brunetti, short definitions for Beowulf (Scaife ATLAS)"; "comes from an Old English word".
- Security page: "What to expect" and "Scope" are owner slots, not statements.
- Export sentence says what the export carries and that plays are not in it yet.
- Wikidata notice in the plan's wording ("Data from Wikidata (CC0)").

## L-S-18: deferred from the review

- SITE-8 rest: the documentation's title and the `KK_` variables change in the rename lane
  and in v1-release; here only the gate check on the title and the link check in `pages`.
- SITE-11 second half: a unit test of the brand contrast pairs on `primary-soft` - the
  browser test now fails on contrast axe cannot judge outside the two known places instead.
- SITE-15: tokens are still filled before parsing; the name is validated instead (no quote,
  backslash or brace) and the limit is documented.
- SITE-16: root-relative addresses stay (no preview under the `/<group>/<project>/` path).
- SITE-17: `pages: true` and pinning the Antora UI bundle (needs a pipeline and a chosen
  bundle version); both are in the owner guide.
- SITE-18: merge notes, nothing to build. One more for the merge: this lane now also edits
  `docs/antora-playbook.yml` (one line), which the rename lane touches for the title.
- SITE-19: favicon and social mark - rule 11 (no drawn logo); the shared-link picture is a
  capture instead.
- L-S-6 (stylesheet size) unchanged.
