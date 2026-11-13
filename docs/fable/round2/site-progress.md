# Lane site: progress (2026-11-13)

Brief: project website, built with the `ui` crate, published with GitLab Pages. Worktree
`kithkeep-lane-site`, branch `lane/site`. Nothing is committed, pushed or deployed.
Decisions: site-decisions.md (L-S-1 .. L-S-18). Owner guide and DNS: docs/website/README.md.

| Item | State | Evidence |
|---|---|---|
| Reference studied (structure, content model, deploy) | done | L-S-2; clone only in the scratch directory |
| App `src/rust/apps/website` (generator, `ui` + `brand`) | done | `cargo test -p website`: 7 unit + 14 integration tests pass |
| Content en + de: landing (with the name story), features, self-hosting, hub, changelog, security, credits, imprint, privacy, 404 | done | `content/`; parity test |
| `/.well-known/security.txt`, `robots.txt`, `sitemap.xml`, licence texts, fonts | done | generator; browser test |
| Stylesheet `src/web/style/site.css`, theme script | done | 65 kB CSS (12 kB gzip), 0.6 kB JS |
| `cargo xtask site [--out] [--strict]` | done | 30 files in `build/public` |
| Colour guard covers the website | done | seen red with a seeded `text-red-500` |
| CI jobs `website`, `website-browser`, `pages` (opt-in) | written, not run in CI | `.gitlab-ci.yml` |
| Playwright: axe WCAG 2.2 AA on 20 pages x 2 themes x 2 widths, 320-2560 px, theme, menu, skip link, language, 404 | done | 66 passed (ports 3500) |
| Screenshots 1280 and 390, light and dark | done | `docs/fable/round2/site-shots/` (80 files, git-ignored) |
| Antora under `/docs/` | job written; built once locally in the scratch directory (paths `/tracker/install.html` confirmed) | not part of the local site build |

Tests seen red: the axe run found `scrollable-region-focusable` on the code block of the
self-hosting page at 390 px (fixed: `<pre tabindex="0">`, unit test). By seeded defects:
product name in a content file, a missing German label, a broken internal link, a colour
literal in the website's code (each failed its test, then restored).

Checks run: `cargo fmt --all --check`; `cargo clippy -p website -p xtask --all-targets --
-D warnings -W clippy::pedantic`; `cargo test -p website -p xtask` (architecture, white-label,
colour, locales guards included); `cargo deny --all-features check`; the Playwright site
config. Not run: the workspace-wide clippy and test runs, `cargo xtask ci`/`e2e`
(orchestrator), the CI pipeline, a real Pages deployment, a docker build with the new
workspace member.

Open for the owner: the PLACEHOLDER slots (`cargo run -p website -- check` lists 32 lines),
`source_url`, screenshots, a legal check of the privacy draft, DNS, `PAGES_DEPLOY`.

Noticed, not built:
- The v1 claims (books, board games, manual entry, security headers) describe packages that
  other lanes are still building; re-read features.toml against the `v1` tag.
- `docs/antora.yml` still names the component `tracker`; if the rename lane changes it,
  `docs_install_url` in `content/site.toml` must follow.
- The Antora default UI is fetched from gitlab.com at build time (existing playbook).
- No favicon or social preview image: the default brand has no drawn mark.
- The website CSS carries the application's utilities too (L-S-6).
- German pages link to English documentation (marked "(englisch)").

# Review round (docs/reviews/2026-11-13-site-review.md), 2026-11-13

The table and notes above are the state before the review; where they differ, this section
holds (the empty screenshot frames, "32 slots", the test that pinned six kinds are gone).

| Finding | State | What | Evidence |
|---|---|---|---|
| SITE-1 release gate | applied | `status` + id per kind and feature, `content/release.toml`, books / board games / manual entry `coming`, fact bar counts shipped kinds, `<!-- shipped:ID -->` blocks | L-S-12; tests `shipped_claims_block_until_the_release_file_confirms_them`, `before_the_first_release_unbuilt_things_are_marked_coming`, `text_of_a_confirmed_release_appears`; seen red with books flipped to shipped |
| SITE-2 wrong claims | applied | Trakt JSON files, no product named for the legacy importer, no year in review, no podcast episode lists, security headers behind the gate, export sentence | L-S-17 |
| SITE-3 pages | applied | `pages` needs `website`, `website-browser`, `antora`; publishes the tested build; `src/sh/pages-assemble.sh` | L-S-14; script run locally: blocked report, foreign `src`, missing docs link each exit 1, pass exits 0. Not run in CI |
| SITE-4 strict gate | applied | `website::gate::publish_blockers`, `check --strict --report`, notice derived from open slots, `#` rule for TOML only | L-S-13; 5 unit tests in `gate.rs`, `the_gate_passes_a_finished_site_and_nothing_less` (9 single defects), `a_page_shows_its_notice_exactly_while_it_has_open_slots` |
| SITE-5 kroki | applied, not verified in CI | `kroki-fetch-diagram: true`; gate check on the playbook; foreign-`src` check in `pages` | L-S-15; test seen red with the value set back to false |
| SITE-6 Wordhord | applied | two confirmed sources | test asserts the third is gone |
| SITE-7 source_url | applied | marked slot, gate blocker, `{source}` token, `git clone` step | L-S-13 |
| SITE-8 docs title | applied (check only) | gate: title must carry the product name; `pages`: every `/docs/` link exists | rest deferred to rename lane / v1-release |
| SITE-9 security policy | applied | two owner slots instead of statements; hub page in the owner checklist | test `owner_slots_are_marked_and_found` |
| SITE-10 language marks | applied | `lang="ang"` on every Old English word, `hreflang="en"` on every documentation link of German pages, English source titles marked | 2 tests |
| SITE-11 axe incomplete | applied | browser test fails on unjudged contrast outside `[data-band]` and the top bar; skip-link test checks a drawn ring | brand contrast unit test deferred (L-S-18) |
| SITE-12 weak tests | applied | exact language link, name scan (case-insensitive for code, incl. site-tests and the config), unfilled-token test, strict `security.txt` in the browser test (`SITE_STRICT`), `generate::build` test | |
| SITE-13 DNS guide | applied | page address, subgroup default address and primary domain, single A/CNAME note, security.txt check after first deploy | docs/website/README.md |
| SITE-14 HomePage | applied | one component per band; every function in `pages.rs` is under 100 lines, no `too_many_lines` allow | clippy pedantic clean |
| SITE-15 robustness | applied / part | output directory emptied (refuses a foreign directory), `serve.mjs` exits with a message; name validated instead of filling after parsing | L-S-18 |
| SITE-16 404 | applied | each 404 page has a line in the other language; root-relative addresses stay | |
| SITE-17 pages details | part | artifact reused (SITE-3); `pages: true` and UI bundle pin deferred | L-S-18 |
| SITE-18 merge notes | nothing to build | plus: this lane edits one line of `docs/antora-playbook.yml` | L-S-18 |
| SITE-19 look | applied | 24 real captures (6 views x en/de x light/dark), hero picture, picture section, `og:image`, features page as one grid, no half-empty rows; no drawn logo or favicon | L-S-16 |
| SITE-20 small points | applied | Wikidata wording, "comes from an Old English word", languages wording | |

Checks run on the final tree (CARGO_BUILD_JOBS=1):
- `cargo fmt --all --check`: clean.
- `cargo clippy -p website -p xtask --all-targets -- -D warnings -W clippy::pedantic`: clean.
- `cargo test -p website -p xtask`: website 16 unit + 28 integration, xtask 17 + architecture 1,
  e2e_log 6, load 3, locales 5, ops 19, whitelabel 2; all pass.
- `cargo deny --all-features check`: advisories, bans, licenses, sources ok.
- Playwright site config (port 3500): 68 passed; 80 screenshots rewritten in
  `docs/fable/round2/site-shots/`.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`
  (for the captures): built; `capture.mjs` run twice (the second time under the product name).
- `cargo xtask site`: 54 files. `cargo xtask site --strict`: fails with 61 blockers, as
  intended: 35 marked slots (32 in imprint, privacy and security pages en + de, 3 in
  `site.toml`), 8 pages showing their notice, 2 `security.txt` values, `source_url`, 14 of
  the release gate (`version` empty + 13 shipped ids), 1 documentation title.

Not run: workspace-wide clippy and tests, the server page tests and the application's
Playwright project (no application code changed), `cargo xtask ci`/`e2e`, a CI pipeline, the
Antora build with fetched diagrams (L-S-15), a docker build.

Noticed in the application while capturing (not changed, visible in the pictures or relevant
to them):
- Home shows "0 books finished" although three books are completed (a paged entry gets no
  finished event); visible in the wide picture.
- A board game play is called "Session logged" in recent activity while the button says
  "Log a play".
- Unselected radio buttons of the play dialog are solid white discs in the dark theme
  (visible in `log-*-dark.png`).
- The bell shows "1" in every picture (the import-finished notification of the seeding).
- Imported shows have no episode titles ("Episode 1").
- On a phone the item page opens with cover and facts before the title.
