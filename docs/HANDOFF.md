# Handoff: state, rules, owner rulings, open items (updated 2026-11-07)

Read this first when resuming. It replaces the conversation context: the rules that govern
all work, where the build stands, every owner ruling (2026-11-06 and 2026-11-07), what is
still open, and how the next work is run. Detail lives in the linked files; this file says
what is true and what to do next.

Sources of truth, in this order: the owner's words in this file (section 4), the KICKOFF
rules (section 2, verbatim), `docs/DECISIONS.md`, `docs/design/DESIGN.md`, the UX design
(`docs/design/ux/design.md` plus `addendum-1.md`), `docs/REPORT.adoc`.

## 0. Resume here (2026-11-07, phase 2 done)

**Standing instruction from the owner (2026-11-07):** work through the open phases
sequentially (phase 2, then 3, then 4, then the parallel workstreams) WITHOUT waiting for a
go between phases, under the KICKOFF rules (section 2): test-driven, decisions logged, an
independent review per phase whose findings are applied or rejected in DECISIONS, `cargo
xtask ci` and `cargo xtask e2e` green, commit, push, tag `ux<N>`. Report a short status at
each phase boundary; ask the owner only when a decision really needs them.

**Where the code is:**

- `main` = tag `ux2` (UX phase 2 done, one squashed commit on top of `ux1`). The branch
  `wip/ux2` is merged and may be deleted.
- Untracked on purpose: `src/rust/crates/testkit/fixtures/import/legacy/` (anonymised extract
  of the owner's Ryot export, owner-approved; commit it together with the legacy importer in
  phase 3). The full export is in the git-ignored `owner-inbox/legacy/export.json`.
- Agent brief files are in `docs/handoff/agent-briefs/` (rules.md, ui-api.md, p1-api.md,
  p2-api.md). Point every workstream agent at `rules.md` first.

**Next steps, in order:**

1. `nix develop`, then `cd src/rust && cargo xtask services up` (services do not survive a
   reboot). The VM has 16 GB RAM and 4 vCPU: builds use `CARGO_BUILD_JOBS=3`.
2. Phase 3 (section 7), then phase 4, then the parallel workstreams. Items deferred to phase 3
   by the ux2 review: DEC-282 (hosting mode change), DEC-283 (CLI escape hatches, SIGHUP key
   reload), DEC-284 (audit client address / request id), DEC-281 (remaining untested
   behaviours), DEC-289 (source Disabled switch), DEC-292 (wizard checklist done flags).
3. e2e harness traps: `cargo test -p server` relinks `target/debug/server` without the
   cargo-leptos settings, so run `cargo leptos build` (or `cargo xtask e2e`) before Playwright;
   `--repeat-each` also repeats the setup project.

## 1. How we continue: agent-based work

The owner asked to clear the context and continue in an agent-based style, under the KICKOFF
rules and the decisions below.

- **Orchestrator (main session).** Owns the plan in section 7, this file, `DECISIONS.md` and
  the CI gate. It spawns agents, reviews their output against the rules, runs the full
  `cargo xtask ci` and `cargo xtask e2e`, commits, tags and pushes. It never edits the tree
  while CI or e2e runs.
- **Workstream agents.** One per work package (section 7), with a brief that quotes the
  relevant rules and decisions, the files to touch, the acceptance test to write first
  (rule 4) and "do not commit". They run only targeted builds and tests.
- **Research agents (Fable model).** Licence check first, then docs; cite every source;
  write into `docs/research/` or `docs/connectors/`; never read GPL code (section 4, ruling
  R-P1 now allows reading public API docs and terms of GPL services, not their code).
- **Review agent (Fable model)** at the end of each phase against DESIGN.md, the UX design and
  the rules; findings are applied or rejected in `DECISIONS.md`, as with G1 and G2.
- **Build host limits (important for parallelism).** 4 vCPU, 16 GB RAM, 57 GB disk of
  which a full debug build plus the wasm build needs about 9 GB. Run one cargo build at a time
  (`CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0` on the 16 GB VM, see section 8); git worktrees each get their own `target/` and
  do not fit more than one at a time, so parallel agents share the main checkout and the
  orchestrator serialises builds. Free disk with
  `rtk proxy find src/rust/target/debug/deps -maxdepth 1 -type f -executable -delete`, drop
  `test_%`/`e2e_%` databases, or `rm -rf src/rust/target/debug` when below 3 GB.
- **Owner review pages.** Decisions for the owner are collected on review pages (claude.ai
  artifacts, private to the owner) and read back before building; see section 9.

## 2. KICKOFF rules (verbatim)

Copied from `docs/design/KICKOFF.md` sections 2 to 5 and 7. Section 4 of this file records
where the owner has since changed or clarified a rule; those rulings win.

### 2. Ground rules

1. **Do not hallucinate.** Research everything: read crate docs (docs.rs), source,
   changelogs, provider API docs. When a decision rests on a source, cite it (URL and
   version) in the commit message or in `docs/DECISIONS.md`. Never invent API
   endpoints, fields, crate features or config keys. If you cannot verify something,
   say so and choose the conservative option.
2. **Never block waiting for the owner.** On any open question pick a sane default,
   record it in `docs/DECISIONS.md` and continue. Entry format:
   `DEC-NNN | question | choice | alternatives | how to reverse | source`.
3. **ASCII only** in code, docs, commit messages and file names (characters on a
   QWERTY/QWERTZ keyboard). No em/en dashes, no smart quotes, no arrows; write `->`.
   Exception: i18n translation files for non-English locales and test fixtures that
   contain real-world data.
4. **Test-driven.** For every behaviour: write the failing test, run it and see it fail,
   then write the code. A milestone is done only when its acceptance command passes
   from a clean checkout and you have re-read the acceptance criteria against the code.
5. **Agents by fit.**
   - Mechanical work (boilerplate, renames, fixture conversion, doc formatting): spawn
     a cheap, fast subagent (`claude-haiku-5-5`).
   - Design and implementation: you.
   - Review: a separate, stronger model reviews finished work **without having seen
     it produced**. Give it the spec, the diff and the acceptance criteria, not your
     reasoning. Use `claude-fable-5-1` for the gates in section 6 and for D-15 and D-18.
   - Apply or explicitly reject (with a DECISIONS entry) every review finding.
6. **No GPL code (hard rule).** Do not look at, fetch, clone, copy or quote source code,
   schemas, migrations, docs, UI text or assets of any project licensed under GPL, AGPL
   or LGPL. Check a project's licence before reading its code; if you notice afterwards
   that something you read is GPL-family, stop, do not use anything from it, and log it
   in DECISIONS. Feature ideas come from DESIGN.md only. The legacy-export importer is
   built only from the owner's export file (Q4), never from any other description of
   that format.
7. **Legal boundary.** Never implement anything that searches for, indexes or downloads
   media releases, or talks to indexers, trackers, Prowlarr/Jackett or torrent/usenet
   clients. Radarr/Sonarr integration is read-only in v1 (D-11).
8. **Secrets.** API keys only via environment variables or a git-ignored `.env`.
   Commit `config/.env.example` with every key name and an empty value. Never log
   secrets. Never commit provider images.
9. **White-label.** No product name, colour, logo or domain hard-coded outside the
   default brand config (D-10). A test greps for the string `kithkeep` outside
   `config/brand/` and `docs/` and fails if it finds it in code.
10. **No AI attribution in the history.** Commit messages, tags, merge request texts,
    code comments, docs and file headers must not contain AI attribution of any kind:
    no `Co-Authored-By` trailers naming a model or AI tool, no "Generated with ..."
    lines, no model names or session links. Disable any tool default that adds them
    (for Claude Code set `includeCoAuthoredBy: false` in the project settings and do
    not add such trailers by hand). A CI job (`commit-lint`) rejects commits whose
    message matches `(?i)co-authored-by:.*(claude|gpt|copilot|ai\b)|generated with|claude\.ai|anthropic`.
11. **Icons and logos: free libraries only, never invented.** Do not draw, generate or
    hand-write SVG paths for icons or logos. Every icon comes from a free, openly
    licensed library; every third-party logo (Jellyfin, TMDB, BGG, ...) comes from a
    free logo collection or the owner's official press kit, used as their terms allow.
    Candidates to evaluate: Lucide, Material Design Icons (pictogrammers.com/library/mdi),
    selfh.st/icons (service logos), The Noun Project, and comparable sets you find.
    **Before integrating any icon set**, run the icon research in M06 and get the
    licence and attribution question settled (section 5).

### 3. Repo layout

Minimal top level. All executable code under `src/<technology>/`.

```
README.md
LICENSE                     # MIT
flake.nix, flake.lock       # must be at the root for `nix develop`
.gitlab-ci.yml              # must be at the root for GitLab
config/
  brand/default/            # default brand: names, logos, tokens, i18n overrides
  app.example.toml          # documented example config
  .env.example
docs/
  antora.yml
  modules/ROOT/...          # AsciiDoc pages, Mermaid via asciidoctor-kroki
  design/DESIGN.md          # input spec (copied from the kickoff bundle, keep it)
  DECISIONS.md
  REPORT.adoc               # final report (section 8)
  reviews/                  # Fable review outputs
  research/                 # Fable research outputs
  connectors/<id>.md        # API terms, licence, attribution, rate limits per connector
src/
  rust/                     # cargo workspace root (Cargo.toml, rust-toolchain.toml)
    apps/server, apps/cli
    crates/<name>           # exactly the crates in D-3
    xtask/                  # cargo xtask ci | fmt | e2e | dev
  docker/
    Dockerfile
    compose.yml             # app + postgres + meilisearch (+ optional minio for S3 tests)
  web/                      # tailwind input css, playwright e2e tests (only non-Rust bits)
```

Keep `docs/design/DESIGN.md` and this KICKOFF.md as Markdown (they are inputs).
Everything you write in `docs/` is AsciiDoc for Antora.

### 4. Engineering standard

- **Dev shell:** Nix flake with pinned Rust toolchain (matches `rust-toolchain.toml`),
  `cargo-leptos`, `cargo-deny`, `sqlx-cli`, `wasm-bindgen-cli` (version locked to the
  crate), tailwindcss v4 standalone binary, postgres, meilisearch, playwright.
  If Nix is not available in the runtime (for example a cloud sandbox), install the
  same versions with rustup/cargo/apt, record it in DECISIONS, and keep the flake correct.
- **Lints:** `clippy::pedantic` as errors (`-D warnings -W clippy::pedantic`), allow-list
  individual lints only with a comment. `rustfmt` check in CI.
- **Safety:** `#![deny(unsafe_code)]` in every crate. Exceptions need a DECISIONS entry.
- **Supply chain:** `cargo-deny` (licences: allow MIT, Apache-2.0, BSD-2/3, ISC, Zlib,
  Unicode, MPL-2.0 (file-level, log it), CC0; deny GPL/AGPL/LGPL/SSPL/BUSL), advisories,
  bans, sources.
- **Architecture test:** a test reads `cargo metadata` and enforces the dependency rule
  in D-3 (`domain`/`app` free of sqlx, axum, leptos, reqwest, opendal, meilisearch-sdk,
  wasmtime; `ui` has no workspace deps).
- **Errors:** `thiserror` in libraries, `anyhow` only in `apps/*` and `xtask`.
- **Observability:** `tracing` everywhere; every outgoing HTTP call has a span with
  connector id, op and cache status.
- **Docs:** AsciiDoc for Antora, Mermaid diagrams via kroki. Required pages: architecture
  overview (crate graph diagram), provider pipeline, storage budget, configuration
  reference (generated from the config structs), connector list with attribution,
  white-labelling guide, operations (backup, reindex, storage report).
- **CI (`.gitlab-ci.yml`):** stages lint, test, build, docs. Services: postgres,
  meilisearch. Jobs: fmt, clippy, deny, unit+integration tests, `cargo leptos build
  --release`, docker image build, antora build. Cache cargo and target dirs.
- **Single entry point:** `cd src/rust && cargo xtask ci` runs everything CI runs that
  does not need docker.

### 5. Context-specific rules

- **Licensed data and assets.** Provider data and images keep their own licences.
  Before implementing a connector, write `docs/connectors/<id>.md` with: API docs URL,
  terms URL, licence, attribution text, auth, rate limits, date checked. If terms are
  unclear, still implement it behind a feature flag that is off by default and add an
  open question. Never commit provider images; fixtures may contain small JSON
  responses only, with their source and licence noted in the same doc.
- **Icons and logos.** Allowed only after `docs/research/icons.md` exists and records,
  per library: licence, attribution requirement and how we meet it (NOTICE file,
  about page, docs), whether bundling into an MIT repo and a docker image is allowed,
  coverage of our needs (media kinds, board game terms, status icons, service logos),
  style consistency, Leptos integration path (inline SVG components, sprite, or an
  existing crate), and size impact. Prefer licences that need no per-use attribution
  (ISC, MIT, Apache-2.0, CC0) for UI icons. Per-icon or CC-BY sets (for example The Noun
  Project) only if attribution is workable and recorded; otherwise reject with a reason.
  Trademarked service logos are shown only to identify that service (integrations,
  data-source attribution), never as our branding. Record the source and licence of every
  vendored icon/logo file in `config/brand/ICONS.adoc` (or a NOTICE section) and keep
  third-party icon files out of `config/brand/default/` logo slots.
- **Provider keys.** Connector tests run against recorded fixtures. If a key is present
  in the environment (`KK_<CONNECTOR>_API_KEY` or similar), `cargo xtask record <id>`
  re-records fixtures and an `#[ignore]` live smoke test can run. Without keys, build
  fixtures by hand from the official API docs' examples and mark them `synthetic` in the
  fixture header. Never skip a connector because a key is missing.
- **Commits.** Conventional Commits, ASCII only. Unsigned by default (Q2). Small commits,
  each green. Work on branch `main` of the new repo; push to `origin` (GitLab) after each
  green milestone if a remote is configured, otherwise stay local. Never force-push.
  Tag each completed milestone `m<NN>`.


### 7. Open questions (use these defaults, log them, continue)

- **Q1 Runtime.** Claude Code cloud sandbox or the owner's VM. Default: detect; use Nix
  if available, otherwise rustup + apt with the same versions. If docker is missing,
  run Postgres and Meilisearch as local processes (xtask starts them).
- **Q2 Commit signing.** Default: unsigned commits. Push to `origin` only if configured.
- **Q3 GitLab remote and CI runner.** Default: write `.gitlab-ci.yml` for a docker
  executor using the official Rust image plus Nix-free install steps; do not depend on
  shared GitLab features beyond services and cache.
- **Q4 Legacy export fixture.** The owner supplies the JSON export of their current
  library at `src/rust/crates/testkit/fixtures/import/legacy/export.json`. Derive the
  format only from that file. Default if missing when M12 starts: build the other
  importers, leave the legacy importer as a stub with a failing `#[ignore]` test and a
  DECISIONS entry. Do not look up the format anywhere else.
- **Q5 BGG token.** Default: fixtures only; live tests ignored unless `KK_BGG_TOKEN` is set.
- **Q6 Versions.** Default: latest stable Rust; Leptos 0.8.x latest patch (not 0.9);
  Postgres 17 or newer; latest Meilisearch v1 release; Tauri 2 latest. Record exact
  versions in DECISIONS.
- **Q7 i18n library.** Default: `leptos_i18n` if it supports the pinned Leptos version,
  else Fluent; locales en and de.
- **Q8 Default brand.** Default: name "kithkeep", neutral palette (zinc/neutral tokens),
  logo = the word "kithkeep" set in an OFL-licensed font (no drawn or generated mark),
  favicon = an icon from the chosen free icon library. The owner replaces it later.
- **Q13 Icon library.** Default if the research is inconclusive: Lucide for UI icons
  (ISC) with Material Design Icons (Apache-2.0) as a fallback for missing glyphs, and
  selfh.st/icons for service logos if its licence allows bundling; otherwise link to
  logos only in docs and show the service name as text.
- **Q9 Job queue vs apalis.** Default: own Postgres queue (D-3). If G1 argues otherwise,
  follow the review.
- **Q10 Generic import schema.** Default: our own versioned JSON schema (also used for
  our own export), documented in docs and published as JSON Schema.
- **Q11 Connectors with unclear terms.** Default: implement behind a feature flag that is
  off by default, record the question.
- **Q12 Scope pressure.** If time or budget runs short, priority order: M00-M08, G1,
  M09 (TMDB/IGDB/OpenLibrary/BGG first), M11, M12, M15, M13, M14, M16, M17. Document what
  was cut in REPORT.adoc.


## 3. State of the build

- **Tags on `origin`:** `m00`-`m17` (see REPORT.adoc; M10/M11 board games and most of M12
  importers not done), `ux0` (UX phase 0), `ux1` (UX phase 1), `ux2` (UX phase 2). Last green runs at `ux2`: `cargo xtask ci` 538 passed / 0 failed / 3 ignored, `cargo xtask e2e` 62 passed, load test p95 under 106 ms everywhere (search 95, Home 105). At `ux1`:
  `cargo xtask ci` 449 passed / 0 failed / 3 ignored, `cargo xtask e2e` 51 passed; GitLab
  pipeline for `ux0` green incl. the S3 test against RustFS in CI.
- **UX phase 0 (done, `ux0`):** tokens per design 2.7, Atkinson Hyperlegible Next vendored
  (DEC-175), 14 new `ui` components plus Disclosure, AppShell with signed-in state (fixes
  "Create account when signed in"), routes `/history` `/people` without redirects, sign-up
  with one error code per rule and the username normalisation (the owner's "Robin" bug),
  every string in Fluent en/de with parity checks and per-user language (DEC-199..201),
  RustFS instead of versitygw (DEC-202), review `docs/reviews/2026-11-06-ux0-review.md`
  applied (DEC-203). `xtask dev` fixed to use the local services.
- **UX phase 1 (done, `ux1`):** server (kind groups, library counts/query, tags, next step,
  undo as compensating event, activity feed, stats, continue/up next, plays with scores),
  Library page, Home with all-time SVG charts, Stats page, History, item page with action bar,
  log sheets and episode list (DEC-204..212, 222..242). Review
  `docs/reviews/2026-11-07-ux1-review.md` applied. Load test (M17, `#[ignore]`, single
  client, 50 000 items, debug build on 2 vCPU, p95 budget 300 ms): search 100 ms, library
  pages 60/83 ms, Home 117 ms, library query 75/88 ms, stats 45 ms, activity 15 ms; with 40 000
  events stats 197 ms, Home 270 ms. Still linear: stats scan all events, activity reads all
  sessions (DEC-235) - optimise in phase 4.
- **UX phase 2 (done, `ux2`):** review `docs/reviews/2026-11-07-ux2-review.md` (UX2-1..12 plus addenda W1..W21 and S1..S12) applied or deferred in DEC-270..296. Packages:
  - WP2-server done (DEC-243..254): secrets at rest (AES-256-GCM envelope, key file
    generated on first start with mode 0600, `APP__SECRETS__KEYS` override, re-wrap job,
    rotation reminder), TOTP recovery codes, credential model (connectors always registered,
    credential state, key slots, Test), source status counters, Ask admin event, instance
    settings with env/file provenance, audit log with CSV, re-auth window (DEC-191) enforced
    server-side (`reauth_required`), lock-out guards, setup wizard state machine, admin API
    for eight pages. Gaps it recorded: audit rows not in the same transaction, counters per
    process and OIDC/SMTP secret encryption left for phase 3. The CLI escape hatches and the
    SIGHUP key reload were not recorded then (review UX2-4); DEC-283 defers them to phase 3.
  - WP2-admin done (DEC-255..265): `/admin/*` eight pages, re-auth dialog, settings area
    `/settings/{account,notifications,integrations,privacy,sources}`; old `/notifications`,
    `/integrations`, `/sources` are 404 (no redirects). Its last full Playwright run: 58 passed.
  - WP2-setup done (DEC-266..269): setup wizard `/setup` (account, hosting, sources, done;
    resumable; 404 after completion) and the 303 redirect of every page to `/setup` before the
    first account (API, `/_fn`, `/pkg`, files exempt); the Playwright setup project creates
    `e2eadmin` through the wizard in the browser (axe + reflow per step). Palette scope
    segments (All/Watch/Read/Listen/Play, `group` on search), source status footer and the
    no-results card with Ask admin (members) / Set up (admins). Home admin nudge (open setup,
    groups without source, failing sources; 7-day snooze in the browser). Hosted-mode banner
    on Admin > Data sources and Overview (built in WP2-admin, now covered by
    `server/tests/setup_page.rs`). TheTVDB uses the member's own PIN for their searches
    (DEC-269; detail/refresh jobs still use the instance key). Server page tests now create a
    first account before rendering pages (auth_pages, brand, shell). Verified: workspace
    clippy (native and wasm) and fmt clean; server, web, api, catalog-core, connectors and
    xtask tests green with services; Playwright `global.setup.ts` + `sources.spec.ts` 5/5.
    Not run by the package: the full `cargo xtask ci` / `cargo xtask e2e`, `sqlx prepare
    --check` (no SQL changed in this package).
- **Research done** (in `docs/research/`): icons, notification channels, RustFS, OD-12
  re-auth window, board game sources, anime/manga sources. Connector doc for BGG rewritten
  from the owner-supplied terms (`docs/connectors/bgg.md`, DEC-190).
- **Reviews:** `docs/reviews/` (prior run assessment, G1, G2, ux0, ux1).

## 4. Owner rulings, 2026-11-06

From the two review pages. Each must be recorded in `DECISIONS.md` (as DEC-169 onward) by the
first work package and applied to `docs/design/ux/design.md` and `addendum-1.md`.

### 4.1 Rule changes and clarifications

- **R-P1 (rule 6).** Reading the public API documentation and terms of GPL/AGPL-licensed
  services to write our own HTTP client is allowed. Their code stays off-limits, nothing is
  copied or linked. Unblocks: Open Library, MusicBrainz, VNDB, Metron, Radarr and Sonarr
  (read-only, rule 7 unchanged), Jellyfin polling, Zitadel docs. Supersedes the strict
  reading in DEC-106 and DEC-148/149.
- **R-F3 (rule 8).** Attribution logos that a provider requires (TMDB) may be committed,
  unmodified, from the provider's official source, with their terms noted. The owner asked
  "what would be bad?": only the rule wording; the remaining care points are to use the
  official, unmodified file, follow the provider's placement rules and record the licence.
  Also build a proper open-source attribution page (licences of crates, fonts, icons, data
  sources). Supersedes DEC-164 item "TMDB logo".
- **Accessibility.** WCAG 2.2 AA is a requirement everywhere, together with the density
  budget (OD-01).
- **i18n.** Done properly: every user-visible string in the locale files (pages, server
  errors, notifications, emails), locale-aware dates and numbers, plurals, per-user language
  with the brand default as fallback, `lang` attributes, a parity test. Format stays Fluent
  `.ftl` (owner chose it over JSON). Right-to-left out of scope.

### 4.2 Design rulings (review page)

| Item | Ruling |
|---|---|
| Status label | **Completed** (not Done) in en; German "Abgeschlossen"; keys unchanged |
| Field borders | darker border (zinc-500), WCAG 1.4.11 |
| Default hosting mode | home; the wizard asks |
| Routes | rename `/sessions` -> `/history`, `/companions` -> `/people`; **no redirects** (greenfield) |
| Mobile navigation | five items with a You hub |
| Font | ship **Atkinson Hyperlegible Next** (OFL 1.1, from github.com/googlefonts/atkinson-hyperlegible-next), self-hosted, no CDN; record its licence on the attribution page |
| TMDB logo | commit the required logo (R-F3) |
| Secrets | encrypted at rest (addendum section 2) |
| Brand, copy, tokens, type, concepts, navigation, next step, filters, history/people/sharing, settings/admin, hosting modes and keys, wizard, failing sources, forms, logging, notifications, integrations, accessibility, phases | approved as in design.md |
| Owner notes (declutter, a11y, admin area, OIDC, true SSO) | approved as in addendum-1.md |
| Home stats | all-time totals per group on Home as native SVG charts with text equivalents; everything over time on a Stats page with a tab per group |

### 4.3 Addendum choices (OD-xx)

Approved as defaulted: OD-01 density budget (5 inputs / 7 rows; 7 / 10 on desktop-only admin
pages), OD-02 remember disclosure state, OD-03 Home activity 5 rows + link, OD-04 one Filters
button, OD-05 AES-256-GCM, OD-06 generate `secrets.key` on first start, OD-07 key file first
then env, OD-08 TOTP recovery codes, OD-09 rotation reminder after 2 years, OD-13 audit
retention 365 days, OD-14 storage and jobs on one page, OD-15 never auto-link by email,
OD-16 provider roles only when admin groups are set, OD-18 no front-channel logout, OD-19
silent probe gates auto mode, OD-20 `/signed-out` never redirects, OD-21 no email sync.

Changed by the owner:

- **OD-10:** storage budget numbers are **editable in the admin UI** (not env-only). Keep the
  env override: a value set in env shows read-only with the variable name.
- **OD-11:** session length and cookie security are **editable in the admin UI**, same env
  override rule. Changing cookie security must not lock the admin out (apply on the next
  session; warn when the public URL is not https).
- **OD-17:** the break-glass `/sign-in/local` in auto mode needs **password only** (not
  mandatory TOTP). Keep the throttle, the audit row and the admin notification; TOTP is used
  if the account has it.

Decided later the same day: **OD-12** 15 minutes, sliding, 1 hour cap, provider round trip with
`max_age=0` for password-less admins (DEC-191, docs/research/od-12-reauth-window.md).

### 4.4 Project questions

- **R-P1** see 4.1.
- **R-P2 BGG:** the owner will provide the BGG XML API docs, terms and a token; board games
  (M10/M11) wait for them.
- **R-P3 anime/manga:** research other sources (licence and terms first); AniList stays out
  (terms, DEC-108).
- **R-P4 importers:** the owner will provide sample exports (legacy fixture, Goodreads,
  BG Stats, BGG). Trakt can be built from its public API docs meanwhile.
- **R-P5 web push:** build as planned in DEC-141 (RFC 8291/8292 on p256 0.14, tested against
  the RFC 8291 example), then the browser opt-in.

### 4.5 Reviewed entries of DECISIONS.md

- Owned by the owner: DEC-001 to DEC-007, DEC-009 to DEC-018, DEC-089, DEC-090.
- **DEC-008, reconsider:** owner comment "RustFS". Replace the S3-compatible dev/test service
  (currently versitygw, chosen in M04) with RustFS if its licence allows (check the licence
  first, then its docs), in compose, `cargo xtask services` and CI; record it as a new DEC.
- **All other entries (DEC-019 to DEC-088, DEC-091 to DEC-168) stay open for owner review.**
  They are in force until the owner reconsiders one; do not treat them as owner-approved.
  They remain on the open-decisions page (section 9), where the owner can keep reviewing.


### 4.6 Owner rulings and answers after the review pages (2026-11-06 evening and 2026-11-07)

- **OD-12:** re-auth window 15 minutes, sliding, 1 hour cap; password-less admins get an OIDC
  round trip with `max_age=0` and a checked `auth_time`, never skipped (DEC-191). Phase 3
  must test whether the owner's authentik honours `max_age=0` (undocumented); fallback in
  `docs/research/od-12-reauth-window.md` 4.4.
- **BGG:** the owner applied for a non-commercial application (approval can take a week+);
  terms received and recorded (DEC-190): per-instance token `KK_BGG_TOKEN`, data unmodified
  and read-only, credit plus "Powered by BGG" logo, optional and off by default, covers not
  copied. AI clause: owner ruled that implementing a client from the documented API is fine;
  the app sends no BGG data to any AI. Raw texts in the git-ignored `owner-inbox/bgg/`.
  Still missing from the owner: the token, the "Powered by BGG" logo files (from
  https://boardgamegeek.com/image/7779581 - the build host cannot reach boardgamegeek.com).
- **Board games direction (owner: "generally fine", keep BGG for those with a token):** own
  catalogue seeded from Wikidata (CC0), user entry and imports, Commons images, "Suggest to
  Wikidata" via the user's browser, BGG optional (DEC-192..198,
  `docs/research/board-game-sources.md`). Open: whether to also check open product databases
  (Open Products Facts) as a barcode source - offered, not answered.
- **Anime/manga (R-P3):** defaults recorded, all off by default (DEC-213..221,
  `docs/research/anime-manga-sources.md`): MAL API v2 primary (owner must read the developer
  agreement at client registration, then `KK_MAL_CLIENT_ID`), Kitsu fallback and id mapper
  (owner may save kitsu.app/terms into owner-inbox for a check), MangaDex not in v1 unless
  the owner allows metadata-only. AniList stays out.
- **Legacy (Ryot) export:** delivered; owner chose "anonymised subset" for the repo fixture
  (21 of 135 items; see the README next to it). Ryot is GPL: build the importer only from the
  file (rule 6 / Q4), never from Ryot docs or code.
- **Owner-inbox:** `owner-inbox/` at the repo root is git-ignored; owner hand-overs go there
  and are reviewed before anything is committed. Still expected: Goodreads, BG Stats and BGG
  collection exports.
- **Phases:** work through all open phases sequentially without waiting for a go (section 0).
- **Build host:** the owner raises RAM (4 GB -> 6 GB now, more possible after another restart).

## 5. Action list from the rulings

1. Rulings recorded as DEC-169 onward and applied to design.md and addendum-1.md: done.
2. Rule 6 and rule 8 handling amended in `docs/DECISIONS.md` (R-P1, R-F3): done.
3. DEC-008: RustFS licence check and switch: done (DEC-202).
4. OD-12: done (DEC-191).
5. Attribution page (crate licences from cargo metadata / cargo-deny, font, icons from
   `config/brand/ICONS.adoc`, data sources and their required notices and logos): OPEN,
   parallel workstream.

## 6. Still open for the owner

- The unreviewed DECISIONS entries (section 4.5) - they stay in force until reconsidered.
- Deliveries: BGG token and the "Powered by BGG" logo files; Goodreads, BG Stats and BGG
  collection exports.
- Questions with safe defaults in force: MAL developer agreement (register a client, then
  `KK_MAL_CLIENT_ID`), Kitsu terms (save the page into owner-inbox), MangaDex metadata yes/no,
  open product databases as a barcode source yes/no.
- Optional: more RAM for the VM (section 8).
- New owner questions go to a review page or a short chat question; never block work (rule 2).

## 7. Work plan

Phases from `docs/design/ux/design.md` section 9, with the addendum and the rulings folded
in. Each phase: workstream agents (one per package, sequential when they share the `web`
crate), independent review, fixes, `cargo xtask ci` and `cargo xtask e2e` green, the load
test by hand, commit, push, tag.

- **Phase 0 (done, `ux0`)** and **Phase 1 (done, `ux1`)**: see section 3.
- **Phase 2 (done, `ux2`): why is nothing happening, and the admin area.**
  Packages: WP2-server (done), WP2-admin (done), WP2-setup (to finish: wizard and the
  redirect to `/setup` before the first account, palette source status footer and no-results
  card with Ask admin / Set up, Home admin nudge (DEC-237), hosted-mode banner, TheTVDB member
  PIN in requests (DEC-246), keep the Playwright setup project creating the first admin
  through the wizard). Then the phase review and the tag `ux2`.
- **Phase 3, sign-in and the stale pages.** OIDC multi-provider and true SSO (addendum 4,
  OD-15..21 with OD-17 as changed; re-auth OIDC round trip for DEC-191 via
  `Auth::confirm_oidc_reauth`; encrypt OIDC client secrets and the SMTP password; the
  Sign-in-and-SSO and Instance admin pages DEC-255 left out), Notifications, Integrations,
  People, Lists, Imports (with the legacy Ryot importer from the fixture, Trakt), per-user
  time zone on the settings page (DEC-239), "Mark show" and backfill date modes and the
  History export link (DEC-237).
- **Phase 4, fluency and measurement.** Keyboard map, bulk edits, offline queue (DEC-237),
  stats polish and goals, in-app logging funnel measurement; performance: incremental stats
  totals and a paged activity query (DEC-235), a release-build load run and a
  concurrent-clients load variant.
- **Parallel workstreams (research done where noted):** attribution page; board games on
  Wikidata (research done, DEC-192..198) and the optional BGG connector once the token
  exists; anime/manga connectors (research done, DEC-213..221); connectors unblocked by R-P1
  (Open Library, MusicBrainz, VNDB, Metron), Radarr/Sonarr read-only, Jellyfin polling;
  Hardcover research (docs need JavaScript); importers (legacy, Trakt, Goodreads, BG Stats,
  BGG when the exports arrive); web push (R-P5, DEC-141).

## 8. Environment notes

- **Host:** NixOS VM on Proxmox (owner's), 4 vCPU and 16 GB RAM since the restart on
  2026-11-07 (was 2 vCPU / 4 GB; memory is not hot-pluggable), 2 GB swap, 57 GB disk.
  `nix develop` shell, Rust 1.99.1, no python3 (use perl, sed, node from the dev shell).
- **Memory (important):** one `rustc` on the `web`/`server` crates reaches about 2.9 GB. Two
  in parallel made the OOM killer kill the Claude session itself three times on 2026-11-07
  (kernel log: `oom-kill ... task_memcg=.../claude-rc-kithkeep.service`). With 16 GB use
  `CARGO_BUILD_JOBS=3` (at most 3 big rustc at about 2.9 GB each); below 8 GB use 1; never run two cargo commands at once; the dev profile
  already uses `debug = "line-tables-only"`.
- **Disk:** `src/rust/target/debug/deps` grows by GBs per phase. When free disk is below
  ~5 GB and no cargo runs: `rtk proxy find src/rust/target/debug/deps -maxdepth 1 -type f
  -executable -delete`; if still short, `rm -rf src/rust/target/debug` (rebuild ~1 h). Drop
  leftover `e2e_*` / `test_*` databases. The owner allowed deleting build output of
  `<projects dir>/{testingapi,pegaprox-rs,organizer}` (done once); never touch `<projects dir>/osd`.
- **Local services:** `cargo xtask services up` (Postgres 55432, RustFS S3 55433,
  Meilisearch 55434); they do not survive a restart. `cargo xtask dev` serves
  http://127.0.0.1:3000 with the dev database. Tests that need services run with
  `TEST_DATABASE_URL` / `REQUIRE_SERVICES=1` / `TEST_MEILI_*` / `TEST_S3_*`; `cargo xtask ci`
  sets them, plain `cargo test` silently skips them. The M17 load test is `#[ignore]` and must
  be run by hand.
- **Stopping processes:** use `pkill -x <name>` (e.g. `pkill -x cargo-leptos`, `pkill -x
  server`); `pkill -f <pattern>` matches the calling shell and kills it.
- `rtk` rewrites common commands; use `rtk proxy <cmd>` for raw output.
- Clippy pedantic is enforced: no `assert!(x.is_empty())` (use `assert_eq!(x.len(), 0)`),
  functions at most 100 lines (the crate-wide `too_many_lines` allow was removed in phase 1;
  per-function allows need a reason comment, DEC-234), no single-character binding clusters.
- After changing SQL queries run `cargo xtask sqlx-prepare`; after config schema changes
  `UPDATE_DOCS=1 cargo test -p config`.
- Leptos gotchas found so far (also in the agent briefs): `StoredValue::new_local` /
  `Action::new_local` abort SSR streams; read resources only inside `<Suspense>` /
  `<Transition>`; key refetched row lists with `<For>` or stale click handlers remain; test
  and DOM ids must not start with a Fluent key prefix (the locale checker treats them as key
  families).
- GitLab: `glab` works on this host (`glab ci list -R hordwyn/hordwyn`).
- Commits: Conventional Commits, no attribution lines of any kind, never force-push `main`,
  tag milestones.

## 9. Links

- Review page (answered): https://claude.ai/artifact/DXNJa8yNPXcUH3nd227wKZ (store `reviews`)
- Open decisions page (partly answered, keeps the unreviewed DECISIONS entries):
  https://claude.ai/artifact/EMtPcfBRksJoFv4LDVHfYv (store `answers`; read it back with the
  ArtifactData tool before each phase to pick up new owner answers; last read 2026-11-06,
  nothing new)
- Design: `docs/design/ux/design.md`, `addendum-1.md`, `research.md`, `mockups/index.html`
- Agent briefs: `docs/handoff/agent-briefs/`
- Research: `docs/research/`; connector docs: `docs/connectors/`; reviews: `docs/reviews/`
