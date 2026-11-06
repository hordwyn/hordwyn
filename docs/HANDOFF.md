# Handoff: state, rules, owner rulings, open items (2026-11-06)

Read this first when resuming. It replaces the conversation context: the rules that govern
all work, where the build stands, every ruling the owner made on 2026-11-06, what is still
open, and how the next phase is run. Detail lives in the linked files; this file says what
is true and what to do next.

Sources of truth, in this order: the owner's words in this file (section 4), the KICKOFF
rules (section 2, verbatim), `docs/DECISIONS.md`, `docs/design/DESIGN.md`, the UX design
(`docs/design/ux/design.md` plus `addendum-1.md`), `docs/REPORT.adoc`.

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
- **Build host limits (important for parallelism).** 2 vCPU, about 4 GB RAM, 57 GB disk of
  which a full debug build plus the wasm build needs about 9 GB. Run one cargo build at a time
  (`CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0`); git worktrees each get their own `target/` and
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

- `origin/main` at the commit of this file; tags `m00`-`m09` (part), `m13`, `m14`, `m15`, `m16`,
  `m17`. Not done: M10/M11 board games (BGG, ruling R-P2), rest of M12 importers (R-P4).
- Last green run: `cargo xtask ci` 259 tests passed, 0 failed, 3 ignored; `cargo xtask e2e`
  12 Playwright tests. GitLab: test, leptos-build, docker-image green; antora fixed by the
  kroki pin (DEC-147, verify once).
- Compose quick start verified by the owner on rootless podman (Ubuntu): needs
  `config/certs/*.crt` behind the Cisco proxy (DEC-166), `systemctl --user enable --now
  podman.socket`, and the override `src/docker/compose.podman.yml` because container DNS
  (aardvark-dns) fails there (DEC-168); sign-up then worked.
- Branch `wip/signup-username` (commit e1b0350): username normalisation (trim, lowercase),
  `invalid_username` / `invalid_display_name` error codes and messages, the rules hint on the
  sign-up form, a test. Not built or tested; fold into phase 0 (it is the owner's sign-up
  bug: "Robin" was refused with only "check the fields").
- Known bugs from the owner's test: "Create account" still shows when signed in; search is
  silent without data sources; Notifications, Integrations and People pages look unfinished.
  All are covered by the UX plan (section 7).
- Reports and reviews: `docs/REPORT.adoc`, `docs/reviews/2026-11-06-prior-run-assessment.md`
  (prior run: B+), `docs/reviews/2026-11-06-architecture-g1.md`,
  `docs/reviews/2026-11-06-g2-review.md`.

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

## 5. Action list from the rulings

1. Record the rulings as DEC-169 onward and update design.md and addendum-1.md (status label,
   no redirects, font, logo, OD-10/11/17 changes, Home stats, i18n scope).
2. Amend rule 6 and rule 8 handling in `docs/DECISIONS.md` (R-P1, R-F3); do not edit
   KICKOFF.md itself.
3. DEC-008: RustFS licence check and switch: done (DEC-202).
4. OD-12: done (DEC-191).
5. Attribution page: crate licences (from cargo metadata / cargo-deny), font, icons
   (`config/brand/ICONS.adoc`), data sources and their required notices and logos.

## 6. Still open for the owner

- The unreviewed DECISIONS entries (section 4.5).
- Deliveries promised by the owner: BGG docs, terms and token; sample exports.
- New owner questions that come up go to a review page, never block work (rule 2).

## 7. Work plan

Phases from `docs/design/ux/design.md` section 9, with the addendum and the rulings folded
in. Each phase ends with `cargo xtask ci` and `cargo xtask e2e` green, a review, a commit and
a tag (`ux0` to `ux4`).

- **Phase 0, foundations.** Tokens (2.7, darker inputs), Atkinson Hyperlegible Next vendored
  with licence, new `ui` components (Alert, EmptyState, StatusDot, Chip, Segmented,
  ErrorSummary, Steps, TaskList, SideNav, TabBar, CopyField, KeyHint, ProgressBar, Rating,
  Disclosure per addendum 1.2), `ClientConfig` with signed-in state, the new shell (top bar,
  mobile tab bar) - fixes "Create account when signed in"; forms with inline rules and the
  `wip/signup-username` work; routes renamed without redirects; Fluent extraction of every
  string with the parity test.
- **Phase 1, where am I and what is next.** Kind groups, library (groups, chips, status tabs,
  tags, one Filters button, views), next step, Home (continue, up next, 5 activity rows, all-
  time stat charts), Stats page, log sheets with undo, item page, History.
- **Phase 2, why is nothing happening, and the admin area.** Credential model with encrypted
  storage (addendum 2), source status in the palette, setup wizard, admin area with eight pages
  (addendum 3; OD-10/11 editable), audit log, lock-out guards.
- **Phase 3, sign-in and the stale pages.** OIDC multi-provider and true SSO (addendum 4, with
  OD-15..21 and OD-17 as changed), Notifications, Integrations, People, Lists, Imports.
- **Phase 4, fluency and measurement.** Keyboard map, bulk edits, stats polish, in-app logging
  funnel measurement.
- **Parallel workstreams (research first):** RustFS (DEC-008); connectors unblocked by R-P1
  (Open Library, MusicBrainz, VNDB, Metron), Radarr/Sonarr read-only and Jellyfin polling;
  Hardcover research (docs need JavaScript); anime/manga sources (R-P3); Trakt importer;
  web push (R-P5); attribution page.

## 8. Environment notes

- Host: NixOS VM, `nix develop` shell, Rust 1.99.1, no python3 (use perl, sed, node from the
  dev shell). Local services: `cargo xtask services up` (Postgres 55432, RustFS S3 55433,
  Meilisearch 55434). Tests that need services run with `TEST_DATABASE_URL` /
  `REQUIRE_SERVICES=1`; `cargo xtask ci` sets them, plain `cargo test` silently skips them.
- `rtk` rewrites common commands; use `rtk proxy <cmd>` for raw output.
- Clippy pedantic is enforced: no `assert!(x.is_empty())` (use `assert_eq!(x.len(), 0)`),
  functions at most 100 lines, no single-character binding clusters.
- After changing SQL queries run `cargo xtask sqlx-prepare`; after config schema changes
  `UPDATE_DOCS=1 cargo test -p config`.
- Commits: Conventional Commits, no attribution lines of any kind, never force-push, tag
  milestones.

## 9. Links

- Review page (answered): https://claude.ai/artifact/DXNJa8yNPXcUH3nd227wKZ (store `reviews`)
- Open decisions page (partly answered, keeps the unreviewed DECISIONS entries):
  https://claude.ai/artifact/EMtPcfBRksJoFv4LDVHfYv (store `answers`; read it back with the
  ArtifactData tool before each phase to pick up new owner answers)
- Design: `docs/design/ux/design.md`, `addendum-1.md`, `research.md`, `mockups/index.html`
