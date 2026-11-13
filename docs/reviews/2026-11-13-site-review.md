# Review: project website (lane/site), 2026-11-13

Scope: worktree `kithkeep-lane-site`, branch `lane/site` (no commits; all work is uncommitted
or untracked on top of main 8eac54a). Read-only review: no cargo, npm or Playwright run by the
reviewer. Sources read: the whole `src/rust/apps/website` crate, its content (en fully, de
sampled), `src/web/style/site.css`, `src/web/site/theme.js`, `src/web/site-tests/`,
`playwright.site.config.ts`, the diffs of `.gitlab-ci.yml`, `xtask`, `Cargo.toml`,
`Cargo.lock`, `.gitignore`, `docs/website/README.md`, site-decisions.md, site-progress.md,
four screenshots (en home 1280 light, de home 390 dark, en features 1280 dark, de imprint
1280 light). Claims were checked against main, the other lanes and v1-plan.md / v1-status.md.
GitLab's Pages documentation, CLASP, Scaife ATLAS and Old English Wordhord were fetched on
2026-11-13.

Counts: high 1, medium 7, low 12. Must fix before merge: SITE-1 .. SITE-4.

## What holds (checked, no finding)

- Design basis: the pages use `ui` (`ButtonLink`, `Card`, `CardContent`, `Badge`, `Alert`,
  `Icon`, `Tint`, `cn`) and theme tokens only; `site.css` imports the application's
  `input.css` and adds typography for Markdown with `var(--color-...)` values. No colour
  literal in the crate or the stylesheet. `COMPONENT_DIRS` gained
  `src/rust/apps/website/src`; `site.css` is under `STYLE_DIRS`. No parallel design.
- White-label: the capitalised name occurs in `content/site.toml` only (grep over the crate,
  stylesheet, script, browser tests, CI file, xtask). Lower-case forms in content are the
  Old English word and the CLASP address, which is correct.
- Personal data: grep of content, code, script and tests for names, street/postcode
  patterns, phone numbers and e-mail addresses finds only `security@example.org` in a test
  and the font copyright line. Imprint and privacy carry placeholders only; section 5 DDG
  and section 18 (2) MStV are the right references. The reference project was not on this
  machine any more, so "nothing copied" was checked by this grep and by reading the text,
  not by a diff against the reference.
- security.txt: with both values set the file has `Contact`, `Expires`,
  `Preferred-Languages`, `Canonical`, `Policy` in valid RFC 9116 form; without them it is
  comments only (see SITE-4 for the gap in the gate).
- Third-party requests of the website's own pages: none (fonts, CSS, script are local; the
  browser test fails on any non-local request). The documentation under `/docs/` is a
  different matter: SITE-5.
- Accessibility, by code and screenshots: one `h1`, `header`/`main`/`footer`, two labelled
  `nav`, skip link to `#main`, `lang` on `html` (en/de), `lang` + `hreflang` on the language
  link, `aria-current`, icons `aria-hidden`, no images (so no alt text needed), focus rings
  on every interactive element, `scroll-padding-top` and the reduced-motion rule inherited
  from `input.css`, targets 36 px or more. Contrast computed by hand for the cases axe
  cannot judge (text on the gradient and on `primary-soft`): light primary #7008e7 on
  #f5f3ff 6.6:1, muted text on #f5f3ff about 7:1; dark primary #a684ff on #2f0d68 5.3:1,
  muted text on #2f0d68 about 5.8:1. All pass AA.
- DNS table: the A record 35.185.44.232, the AAAA record 2600:1901:0:7b8a::, the CNAME to
  `<namespace>.gitlab.io` and the TXT name and value format match GitLab's current page
  (https://docs.gitlab.com/user/project/pages/custom_domains_ssl_tls_certification/ - the
  address without `_and_`; the one in the brief answers 404). Still to be verified by the
  owner against the dialog, as the README says. Details: SITE-13.
- Name story: `hord-wynn` "joy of the hoard" (Scaife ATLAS, Brunetti short definitions for
  Beowulf) and one occurrence at Beowulf 2270b (CLASP) are confirmed on the live pages; the
  dragon, the open hoard and the three hundred winters match the poem (2270-2278). The tone
  is fine. One citation does not hold: SITE-6.
- CI mechanics: `ci-tools.sh tailwind` installs to `$CARGO_HOME/bin` and the jobs put that
  on `PATH`; `website-browser` gets `build/public` through `needs: [website]` and
  `serve.mjs` reads exactly that path; `pages` gets `build/site` from `antora`; `/public/`
  is git-ignored; the Dockerfile copies `src/rust` whole, so the new workspace member does
  not break the image build context. `pages` publishes only the generated site and the
  Antora output.
- Merge surface today: no other lane touches `.gitlab-ci.yml`, `Cargo.lock`,
  `xtask/src/main.rs` or `.gitignore`. See SITE-18 for the two places that can conflict.

## Must fix before merge

### SITE-1 (high) The site states as shipped what main does not have, and nothing but the imprint stands between it and publication

- Where: `content/en/home.toml:12-13` ("6 media kinds in v1"), `:44-48`, `:62-66`, `:69`;
  `content/en/features.toml:5` ("This page lists what the first release really does"),
  `:42-53` (books), `:87-98` (board games), `:157`; the same lines in `content/de/`;
  `tests/site.rs:237-264`; `src/pages.rs:46-48` (badge "In v1" in the success colour).
- Problem: on main there are four kinds with a source (TMDB, TheTVDB, IGDB, Podcast Index).
  Open Library exists only as uncommitted work in the books lane. The Wikidata connector for
  board games (v1-plan package 4b) and manual entry (package v1-manual: "enter a book by
  hand", "enter a game by hand", "anime, manga ... can be entered by hand") are not started
  in any lane; on main a board game cannot be added except through an import file
  (v1-status.md:63). The page says "really does" and shows a green "In v1" badge on all six.
  L-S-11 notes that the text must be re-read against the `v1` tag, but no mechanism enforces
  it: `--strict` looks only for the word PLACEHOLDER, and the test
  `v1_claims_only_the_six_kinds_with_a_source` pins the six-kind claim instead of checking it.
- Failure scenario: the owner fills the imprint and the security contact (which they will do
  for legal reasons, independent of the state of v1), sets `PAGES_DEPLOY`, and hordwyn.com
  tells visitors that v1 tracks books and board games and takes manual entries while the
  code cannot. If package 4b or v1-manual slips out of v1, the site stays wrong silently.
- Fix: add a release gate that `--strict` checks, for example `released = false` in
  `site.toml` (strict refuses while false), or a slot line such as
  `v1_claims_checked = "" # PLACEHOLDER: re-read features against the v1 tag`. Until v1 is
  tagged, word the page as a plan ("v1 will cover"; the changelog already says "planned to
  cover") or mark books, board games and manual entry with the existing `coming` state.
  Rename the test to what it does, or drive the `ready` state from a list that is kept with
  the connectors.

### SITE-2 (medium) Three claims are wrong for any v1, and one depends on uncommitted work

- Where: `content/en/features.toml:20` and `:123`, `content/en/home.toml:95`,
  `content/en/features.toml:153` + `:159`, `content/en/security.md:38`; the same in `de/`.
- Problem and scenarios:
  1. "Import from Trakt or Ryot" / "Bring your history from Trakt or Ryot": the application
     never names Ryot. The importer is "Previous tracker export"
     (`crates/importers/src/legacy.rs:1-5`), built from one export file of the owner. A Ryot
     user arrives with a current Ryot export, it does not import, and the website promised
     it by product name.
  2. "so the year in review tells a story": there is no year-in-review feature in the code
     or in DECISIONS; there are statistics by year and a history filter. A visitor looks for
     the recap and does not find it.
  3. "Episode lists for podcasts" under "These are the next things on the list": this is a
     known gap (v1-status.md:55), but there is no package and no decision for it. The other
     seven items are on the roadmap.
  4. "the security headers the application sends" are described in the documentation only in
     the uncommitted sec1 work of the main checkout; committed `reverse-proxy.adoc:35-37`
     says the opposite. True once sec1 is merged, false before.
- Fix: (1) "Import from Trakt, or from a CSV / JSON Lines file in the documented format";
  drop the name Ryot or state exactly what was tested. (2) "so a year of evenings can be
  read back" or simply "statistics by year". (3) remove the item or move it out of "next on
  the list". (4) merge after sec1, or soften to "the security headers".
- Also worth a sentence, not wrong: the export does not carry plays (players, scores,
  winner, duration; v1-status.md G2) while `self-hosting.md:35` says "The export gives you
  your library"; Podman Compose was run only by the owner, never on the build host.

### SITE-3 (medium) `pages` deploys without waiting for the website's own checks

- Where: `.gitlab-ci.yml:216-229` (`needs: [antora]`).
- Problem: `needs` takes the job out of stage ordering. `pages` starts as soon as `antora`
  has succeeded. `website-browser` is in the same stage as `antora` and is not needed by
  `pages`, so its result is ignored.
- Failure scenario: a change breaks the layout at 320 px or adds a foreign request or an axe
  violation; `website-browser` fails after four minutes; `pages` finished after two and the
  broken site is live. The pipeline is red, the site is published anyway.
- Fix: `needs: [antora, website-browser]` (and `website`, if its artifact is reused).
  Better still: let `pages` publish the artifact that was tested - have `website` run
  `cargo run -p website -- check --strict` as a separate gate (see SITE-4) and `pages` copy
  `build/public` instead of compiling a second, untested build.

### SITE-4 (medium) `--strict` can be passed with an unfinished site; the gate has no test

- Where: `src/main.rs:59-74`, `src/content.rs:453-488` (`:474`), `src/generate.rs:145-153`,
  `content/site.toml:24-25`.
- Problem: strict mode is "no line contains PLACEHOLDER". That leaves these holes:
  1. `site.toml`: the owner deletes the two marker comments but leaves one value empty (or
     fills the contact and forgets the date). Strict passes; `security_txt` then publishes a
     comment-only file that begins "# PLACEHOLDER", which is not a valid RFC 9116 file
     (`Contact` and `Expires` are required), and the security page has no contact.
  2. `Expires` is not parsed: a date in the past, a date without time
     ("2027-11-01") or more than a year ahead is published as is. `Contact` is not checked
     to be a `mailto:` or `https:` URI ("security@..." without scheme is invalid).
  3. `content.rs:474` skips every line that starts with `#`. That is right for TOML
     comments and wrong for Markdown: a heading `## [PLACEHOLDER: ...]` is not counted.
  4. The `notice` in the page head ("This page is not finished") is not a slot. The owner
     fills the imprint, forgets the notice line, and the public imprint carries a warning
     box; or deletes the notice and keeps a slot hidden behind (3).
  5. The decision lives in `main.rs` and has no test; the task's central promise ("strict
     blocks publishing while placeholders are open") is unverified by the suite.
- Fix: move the gate into the library (`fn publish_blockers(site, slots) -> Vec<String>`)
  and test it. Blockers: any slot; empty `security_contact` or `security_expires`; contact
  without `mailto:`/`https:` scheme; expiry not RFC 3339, in the past or more than a year
  ahead; a page with `notice` set. Apply the `#` rule to `.toml` files only. Derive the
  notice from "this page has open slots" so the two cannot drift.

## May defer past the merge, but close before `PAGES_DEPLOY` is set

### SITE-5 (medium) The published documentation loads diagrams from kroki.io; footer and privacy page say nothing is loaded from other servers

- Where: `docs/antora-playbook.yml:10-13` (`kroki-fetch-diagram: false`, no
  `kroki-server-url`); `docs/modules/ROOT/pages/architecture.adoc:6`,
  `provider-pipeline.adoc:9`, `storage-budget.adoc:61` (`[mermaid]`);
  `content/en/strings.toml:22`, `content/en/privacy.md:18` and the de files;
  `.gitlab-ci.yml:226-227`.
- Problem: with `kroki-fetch-diagram: false` the generated pages reference each diagram as
  an image at `https://kroki.io/...`. The `pages` job publishes these pages under
  `hordwyn.com/docs/`. The footer of every website page says "This website ... loads nothing
  from other servers" and the privacy notice says "No page loads content from a third
  party". The browser test checks only the website's own ten pages, not `/docs/`.
- Failure scenario: a visitor opens `/docs/tracker/architecture.html`; the browser sends
  their IP address and the diagram source to kroki.io. The privacy notice of the same
  domain denies this - in Germany that is the kind of statement that draws a warning letter.
- Fix: set `kroki-fetch-diagram: true` (the diagrams are fetched at build time and
  published as files; the `antora` job has network access), and add a check on the built
  docs (`grep -r 'src="http' build/site` must be empty) to the `pages` job. Until then,
  narrow the two sentences to the website's own pages. Also confirm that the Antora default
  UI bundle (fetched unpinned from gitlab.com at build time) references no external font or
  script.

### SITE-6 (medium) One of the three sources of the name does not show what is quoted

- Where: `content/en/home.toml:120`, `content/de/home.toml` (same line of `[name]`).
- Problem: the text cites "Old English Wordhord" with the quoted gloss "hoard-joy,
  delightful treasure" and links to the home page of oldenglishwordhord.com. A search for
  the word on that site returns nothing; names.md lists this source as a "search result
  summary" of a category page, never opened. The two other sources are live and say what
  the site says.
- Failure scenario: a reader who knows Old English follows the third link, finds no entry,
  and the one passage of the landing page that claims to be sourced looks careless.
- Fix: drop the third source, or replace it with the Bosworth-Toller entry for
  `hord-wynn` after reading it (names.md 10.5 notes it was not seen). Name the ATLAS source
  as what it is: "Brunetti, short definitions for Beowulf (Scaife ATLAS)". Optional: cite
  the line as 2270b and add "three hundred winters" = line 2278.

### SITE-7 (medium) The site says "open source" and "host it yourself" but gives no way to get the source

- Where: `content/site.toml:20` (`source_url = ""`, not a slot),
  `content/en/self-hosting.md:15-19`, `content/en/home.toml:8`, `:20-21`, `:161`.
- Problem: the hero button is "Host it yourself"; the guide starts with
  `cp config/.env.example config/.env`, which needs a checkout; the fact bar says "MIT, open
  source". There is no repository link anywhere while `source_url` is empty, the GitLab
  project is private, and `--strict` does not ask for it.
- Failure scenario: published as is, the first visitor who wants to try it cannot find the
  code.
- Fix: make `source_url` a blocker of the gate (SITE-4) or a PLACEHOLDER slot, and add the
  clone step to the short version of the guide (`git clone {source}`), which needs a
  `{source}` token.

### SITE-8 (medium) The documentation that gets published under the product's domain still carries the working title and the old prefix, and no test knows the install link

- Where: `docs/antora-playbook.yml:2` ("Tracker documentation"), `docs/antora.yml:1-2`;
  `docs/modules/ROOT/pages/configuration.adoc:56`, `connectors.adoc:15` (`KK_...`);
  `content/site.toml:14`; `tests/site.rs:129-135`.
- Problem: `pages` copies the Antora site to `/docs/` as it is. Its title is "Tracker
  documentation" until the rename lane is merged; the `KK_` variables stay until v1-release.
  The link test skips everything below `docs_url`, so `/docs/tracker/install.html` (linked
  from the self-hosting page twice) is never checked against the Antora output. The rename
  lane keeps `name: tracker` today, so the link is right today.
- Failure scenario: a later change of the component name or of the page name turns the two
  most important links of the self-hosting page into 404s with all tests green.
- Fix: in the `pages` job, after the copy, assert that
  `public$(docs_install_url)` exists (one `test -f`). Merge the rename lane before the
  first deployment. The documentation also has a different look (Antora default UI) and no
  link back to the website; acceptable for a first version, worth a line in the README.

### SITE-9 (low) The security page makes commitments nobody has decided

- Where: `content/en/security.md:25-34`, de the same.
- Problem: "We confirm that we received your report", "Only the latest release receives
  security fixes", "There is no bug bounty", the scope paragraph: these are policy
  statements written without an owner decision, on a page whose only marked slot is the
  contact. Nothing is invented about a person, but the policy is.
- Failure scenario: the owner fills the contact, the rest goes live unread and binds them
  to a disclosure policy they never chose.
- Fix: add the page to the owner checklist in the README ("read and confirm the policy
  text") or mark the two sections with a slot line. The same applies in a weaker form to
  the hub page (CC0, invitation-only start, moderation from day one): confirm against the
  hub requirements before publishing.

## May defer

### SITE-10 (low) Old English words in running text carry no language mark

- Where: `content/*/home.toml` `[name] body` and `gloss`; `src/pages.rs:183` (only the two
  large parts have `lang="ang"`).
- Problem: *Hord*, *wynn*, *hord-wynn* in the paragraph and in the gloss line are rendered
  as `<em>` without `lang` (WCAG 3.1.2, AA). On the German pages the English source titles
  and the links to the English documentation ("(englisch)") have no `lang`/`hreflang="en"`.
- Scenario: a screen reader reads "wynn" with German or English phonemes; minor.
- Fix: raw HTML is allowed in content: `<i lang="ang">hord-wynn</i>`; add `hreflang="en"`
  to the documentation links on the German pages.

### SITE-11 (low) axe results marked "incomplete" are ignored, and they are exactly the hero

- Where: `src/web/site-tests/site.spec.ts:50-54`.
- Problem: axe cannot compute contrast over a background gradient or behind
  `backdrop-blur`; it reports those nodes as `incomplete`, and the test looks only at
  `violations`. The eyebrow, `h1` and lead of every page head sit on a gradient. They pass
  today (computed by hand above), but a change of `primary-soft` in the brand would not be
  caught here. The "focus is visible" test (`:113-121`) checks that the skip link is in the
  viewport, not that a ring is drawn.
- Fix: assert that `incomplete` has no `color-contrast` entry outside an allowlist, or add
  a unit test that runs the brand contrast check (`crates/brand/src/contrast.rs`) on the
  pairs primary/primary-soft and muted-foreground/primary-soft for both themes.

### SITE-12 (low) Tests that are weaker than their names

- `tests/site.rs:444-447`: the second alternative (`lang="en" hreflang="en"`) is true for
  any English language link, whatever its `href`; the assertion cannot fail on a wrong
  target. Assert the full `href="/features/" lang="en"` only.
- `tests/site.rs:316-340`: the name scan is case-sensitive and does not include
  `src/web/site-tests/` and `playwright.site.config.ts`. Clean today; add the two paths.
- No test fails on an unknown or mistyped token: `{nmae}` or `{source}` would be published
  literally. Add: no rendered page contains `{` followed by a lower-case word and `}`.
- `site.spec.ts:142`: `/^(# PLACEHOLDER|Contact: )/` accepts both states; in the `pages`
  build only `Contact:` is acceptable (covered if SITE-4 is done).
- `generate::build` (file writing, the returned count, the fingerprinted name) has no test.

### SITE-13 (low) DNS guide: correct, three additions

- Where: `docs/website/README.md:76-107`.
- The records match GitLab's page of 2026-11-13. Add the address of that page, and keep the
  sentence that the dialog is authoritative.
- `README.md:78-81`: the project lives in a subgroup, so the default address is
  `<namespace>.gitlab.io/products/tracker/<project>` and contains the old codename; with
  "unique domain" on, the generated host name starts with the project slug as well. After
  the custom domain works, set it as the primary domain (Deploy > Pages) so the gitlab.io
  address redirects, or the codename address stays a public duplicate of the site.
- GitLab's Let's Encrypt troubleshooting asks for a single A or CNAME record per name and
  mentions removing an AAAA record if issuance fails; note this next to the CAA line.
- Whether GitLab Pages serves `/.well-known/security.txt` from the artifact was not
  confirmed by the documentation; check it once after the first deployment.

### SITE-14 (low) One function over 100 lines

- `src/pages.rs:142-271` `HomePage`, 130 lines, with `#[allow(clippy::too_many_lines)]`.
  Fix: one small component per section (`Hero`, `NameSection`, `Steps`); the closures at
  the top move with them. Everything else is below 100 (`build` 86, `render_all` 76).

### SITE-15 (low) Generator and test server robustness

- `src/generate.rs:208-293`: the output directory is never emptied. Locally every change of
  the stylesheet leaves another `assets/site.<hash>.css`, and a removed page stays in
  `build/public`, so the browser test can pass on a page that no longer exists. Fix: remove
  the directory first when it contains the generator's own marker (or always, for the
  default path).
- `src/web/site-tests/serve.mjs:30-34`: without a built site `readFile(404.html)` rejects
  inside the request handler and the process dies; Playwright then reports a web server
  timeout. Fix: exit at start with "run cargo xtask site first" when the root is missing.
- `src/content.rs:365-373`: tokens are filled into the TOML text before parsing; a product
  name with a double quote or a backslash breaks every content file. Fill after parsing or
  document the limit (white-label rule).

### SITE-16 (low) Not-found page and root-relative addresses

- GitLab Pages serves only `/404.html`; `/de/404.html` is generated but never used as an
  error page, so a wrong German address shows the English page. Acceptable; the English
  404 could carry one German line.
- All addresses are root-relative; the README says so. The site cannot be previewed under
  the `/<group>/<project>/` address.

### SITE-17 (low) `pages` job details

- `.gitlab-ci.yml:216`: GitLab's documentation now describes `pages: true` (or
  `pages: { publish: ... }`) as the property that makes a job a Pages deployment; the job
  name still works. Add `pages: true` to be independent of the name.
- The Antora UI bundle is fetched from the HEAD of another project at build time
  (`antora-playbook.yml:15-17`, existing): what is published under the product's domain is
  not pinned. Pin a bundle version before the first deployment.
- `pages` compiles the generator a second time with its own target cache; reuse the
  `website` artifact (SITE-3).

### SITE-18 (low) Merge notes

- `src/rust/xtask/src/whitelabel.rs`: the rename lane edits the same file (and its test);
  the site lane adds two lines to `COMPONENT_DIRS`. Expect a small textual conflict.
- `src/rust/Cargo.lock`: the only lane that changes it. Main's uncommitted sec1 work changes
  `Cargo.toml`; after merging both, regenerate the lock file with a build instead of
  resolving hunks by hand, and re-run `cargo deny`.
- `src/rust/Cargo.toml`: one added line (`pulldown-cmark`) next to lines sec1 may touch.
- `.gitlab-ci.yml`, `xtask/src/main.rs`, `.gitignore`: no other lane touches them today.
- Antora: the rename lane keeps `name: tracker` and changes only titles, so
  `docs_install_url` stays valid (see SITE-8 for the missing check).
- Not run by the lane and still open: workspace-wide clippy and tests with the new member,
  the docker build, the three CI jobs on a real runner.

### SITE-19 (low) Look: what keeps it from the bar

- No favicon, no mark, no social preview image (`src/layout.rs:407-418` has no
  `rel="icon"`): the browser tab and every shared link are blank.
- `content/*/home.toml:97-104`, `src/pages.rs:231-234`: the landing page shows a section
  "Screenshots" with three dashed empty frames and "Screenshot to be added". Honest, but on
  a public page it reads as unfinished and takes a full screen on a phone. Fix: render the
  section only when pictures exist, and make "no screenshots yet" a blocker of the gate or
  an explicit owner choice.
- The features page has two groups with a single card in a two-column grid (Read, Listen),
  which leaves half the width empty at 1280 px.
- The hero is text only. With real screenshots next to it this would carry; without, the
  first screen is a headline on a pale gradient.

### SITE-20 (low) Small content points

- `content/en/attributions.md:16`: the Wikidata notice is worded differently from the plan's
  notice ("Data from Wikidata (CC0)."); align when the connector exists.
- `content/en/home.toml:111`: "{name} is an Old English word" - the Old English word is
  the two-n form; the last paragraph says so. "comes from an Old English word" is exact.
- `content/en/home.toml:24` "EN + DE" and the features card say both languages
  "throughout"; the documentation is English only (the German pages say so at each link).
