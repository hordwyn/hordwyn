# Handoff: state, rules, owner rulings, open items (updated 2026-11-10)

Read this first when resuming. It replaces the conversation context: the rules that govern
all work, where the build stands, every owner ruling (2026-11-06 and 2026-11-07), what is
still open, and how the next work is run. Detail lives in the linked files; this file says
what is true and what to do next.

Sources of truth, in this order: the owner's words in this file (section 4), the KICKOFF
rules (section 2, verbatim), `docs/DECISIONS.md`, `docs/design/DESIGN.md`, the UX design
(`docs/design/ux/design.md` plus `addendum-1.md`), `docs/REPORT.adoc`.

## 0. Resume here (2026-11-10, phase 4 done; handover before a restart)

**Update 2026-11-10 (after the restart), this wins over the threads below:**

- Owner tested `ux4` at home: TMDB and IGDB work. The podman and TMDB threads below are
  closed (company laptop behind Cisco SSE; no code change).
- 17 owner test findings OT-1..OT-17 and the new plan P0..P4:
  `docs/handoff/owner-test-2026-11-10.md`.
- Fable round 2 DONE 2026-11-10 (all four complete, no TODO): owner-review-page.md, ux-owner-test.md, bug-diagnosis.md, attribution-and-connectors.md in docs/fable/round2/. Next: build the owner review page from F1 + F2, walk through it with the owner, then fix pack ot1: briefs
  `docs/handoff/fable-briefs/` (common.md + F1 review page, F2 UX pass, F3 bug diagnosis,
  F4 attribution and connectors). All read-only, no builds; outputs to `docs/fable/round2/`.
  Model alias `fable` = `claude-fable-5-2` (DEC-495).

**Open threads at the handover (2026-11-10), do these first:**

- **Eight read-only Fable reviews, owner has not reviewed them yet:** on branch
  `wip/fable-reviews` (pushed), folder `docs/fable/`: `security-audit.md` (SEC-1..12, 5
  medium), `hub-design.md` (BGG replacement as a hub role; 15 owner questions with
  defaults), `architecture-review.md` (12 work packages incl. the `web` crate split),
  `open-data-sources.md` (open source per kind: TVmaze, Wikidata, MusicBrainz, Open
  Library/DNB, VNDB; 10 connector packages), `decisions-current-state.md` (93 supersessions,
  12 contradictions, 58-item backlog, log restructuring proposal), `perf-proposals.md`
  (9-step plan; `/people` has an unmeasured N+1), `ops-readiness.md` (OPS-1..12: no `cli`
  in the image, PUBLIC_URL/TRUSTED_PROXIES/SECURE_COOKIES missing from `.env.example`,
  improved Dockerfile proposal, backup runbook), `test-audit.md` (shared e2e instance state,
  CI retries hide flakes, never-red tests). Next: walk through them with the owner (hub
  questions and SEC findings first), then merge the folder into `main`.
- **Owner's podman build** (native rootless, Ubuntu 26.04, 16 GB): still failed after commit
  f1eeefa (frontend and server built one after the other, `CARGO_BUILD_JOBS` build arg,
  default 2), but got further (about 900 s). Waiting for the log tail; next try
  `CARGO_BUILD_JOBS=1`; see `ops-readiness.md` section 0/1 for the memory peaks and a
  cargo-chef style Dockerfile. Tell the owner to set `APP__SERVER__PUBLIC_URL` (and
  `APP__AUTH__SECURE_COOKIES=false` on plain http) for a non-localhost test.
- **Owner's TMDB key problem:** the owner's read access token works from this VM (`curl`
  and `cargo test -p connectors --features all --test tmdb -- --ignored`), so the code is
  fine; the owner's instance showed "retrying" (circuit open) and no results, which points
  at the container's network/DNS/TLS. Waiting for `podman logs` and a `curl` from inside
  the container. The token was posted in the chat; the owner will rotate it.
- **Plan after that (owner agreed in principle, to be finalised after testing):** W0 release
  build check of the image's feature set in CI, W1 attribution page, W2 performance
  follow-ups, W3 board games on Wikidata (now possibly the hub, see hub-design.md),
  W4 anime/manga, W5 more connectors (see open-data-sources.md), W6 web push, W7 "date
  unknown" on events; blocked on the owner: Goodreads/BG Stats/BGG samples, saved views
  (after the funnel review on 2026-12-06). Definition of Done per workstream: planned
  packages, test-first, DECs, independent review applied, `cargo xtask ci` + `cargo xtask
  e2e` (0 panics) green, release load run for performance-relevant work, no test
  workarounds for app races, squash onto `main`, tag (`ws1`, ...), push, HANDOFF + memory.
- Code work is paused until the owner has tested and finalised the plan.
- Last change on `main`: 99d1e0f (Ctrl+K puts the cursor in the search field).

**Standing instruction from the owner (2026-11-07, confirmed in chat 2026-11-07 with explicit
permission to commit, tag and push):** work through the open phases sequentially WITHOUT
waiting for a go between phases, under the KICKOFF rules (section 2): test-driven, decisions
logged, an independent review per phase whose findings are applied or rejected in DECISIONS,
`cargo xtask ci` and `cargo xtask e2e` green, commit, push, tag. Report a short status at each
boundary; ask the owner only when a decision really needs them.

**Where the code is:**

- `main` = tag `ux4` (UX phases 0-4 done; phase 4 is one squashed commit on top of `ux3`). The
  branch `wip/ux4` holds the per-package wip history and may be deleted.
- Plans: `docs/handoff/phase3-plan.md`, `docs/handoff/phase4-plan.md` (how a phase is cut into
  packages); agent briefs in `docs/handoff/agent-briefs/` (rules.md first).

**Next steps, in order:**

1. `nix develop`, then `cd src/rust && cargo xtask services up` (services do not survive a
   reboot). 16 GB RAM, 4 vCPU: `CARGO_BUILD_JOBS=3`. Check free disk first (section 8).
2. The design's UX phases are done. Continue with the parallel workstreams (section 7), each as
   a planned package with review: attribution page (credit lines incl. the Trakt line from
   DEC-360 and TMDB/TheTVDB notices); board games on Wikidata (DEC-192..198; brings the owned
   flag, then "Mark owned" and "unplayed owned games", DEC-420); anime/manga connectors
   (DEC-213..221); Open Library, MusicBrainz, VNDB, Metron, Radarr/Sonarr, Jellyfin;
   importers waiting for owner samples (Goodreads, BG Stats, BGG); web push (R-P5, DEC-141).
3. Performance workstream follow-ups (DEC-476 P-1..P-3, DEC-494): the first stats read after a
   zone change (~750 ms, target 500), the library query count on the first page (~210 ms p95
   under 8 clients), `/people` in the load set, the optional `sessions (started_at DESC, id
   DESC)` index, cover lookups batched in `dto::item`.
4. Other deferred items are listed by DEC: "Date unknown" on events (DEC-355, own package);
   saved views (decide after the funnel review on 2026-12-06, section 6); avatar upload, fuzzy
   duplicate suggestions, restoring a removed person (DEC-353); TheTVDB episode paging
   (DEC-447); per-episode runtime in stats and the "Up to date" badge (DEC-450); tests deferred
   in DEC-457, 473, 483.
5. Harness notes: `cargo xtask e2e` captures the server log (set `E2E_SERVER_LOG` outside the
   repo) and fails on any `panicked at` or a missing/empty log (DEC-456, 463, 487);
   `cargo xtask load [--release] [--clients N]` with `LOAD_*` variables (DEC-399) needs 15 GB
   free disk, delete `target/release` afterwards; catalog events go only to their audience
   (DEC-490, 492), so e2e background events must come from the same user (`changeMine`).

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
  importers not done), `ux0` (UX phase 0), `ux1` (UX phase 1), `ux2` (UX phase 2), `ux3` (UX phase 3), `ux4` (UX phase 4, numbers in the phase 4 bullet below). Last green runs at `ux3`: `cargo xtask ci` 679 passed / 0 failed / 4 ignored, `cargo xtask e2e` 94 passed, load test p95 search 106, Home 164, stats 90, library 64-89, activity 13 ms (Home and stats grew with the per-user time zone; budget 300 ms). At `ux2`: `cargo xtask ci` 538 passed / 0 failed / 3 ignored, `cargo xtask e2e` 62 passed, load test p95 under 106 ms everywhere (search 95, Home 105). At `ux1`:
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
- **UX phase 3 (done, `ux3`):** review `docs/reviews/2026-11-08-ux3-review.md` (UX3-1..26) applied, deferred or rejected in DEC-367..398. Packages:
  - WP3-1 instance-server done (DEC-298..307): audit rows in the change's transaction
    (roles, deletion and operations still after, DEC-298); client address and request id
    behind `server.trusted_proxies`; SMTP in the database with an encrypted password, swappable
    mailer and "send a test"; CLI escape hatches (`admin promote|reset-password|reset-totp|
    disable-sso-only`, `setup reopen`, `secrets status|rotate|prune`, actor `cli`,
    `POST /auth/reset-password`); `auth.sign_in_mode` defined (`auto` refused until WP3-3);
    SIGHUP key reload; source Disabled switch; wizard done flags; per-user time zone server half
    (`PUT /auth/me/time-zone`, stats months, `local_day`/`local_time`, `ClientConfig.today`);
    the DEC-281 server tests. Migration `0017_phase3_instance.sql`. Web pages for these: WP3-4.
  - WP3-2 oidc-providers done (DEC-308..316): several OIDC providers in `oidc_providers` (client
    secret encrypted, purpose `oidc_client_secret`, write-only), the environment's `auth.oidc` as a
    read-only row; discovery per provider (24 h cache, refreshed by Test and on a signature miss),
    PKCE S256, signed single-use pending cookie (HKDF `cookie_key`, modes
    interactive|silent|link|reauth; silent/reauth refused until WP3-3), RFC 9207 `iss` or ID-token
    `iss` mix-up check; account resolution steps 1-4 (no auto-link; "Is this your account?" via
    `GET/POST /auth/oidc/link`; link mode; unlink guard), allowed/admin groups with
    `role_source=provider` (never the last admin, `self_demotion`, role locked in Users); admin API
    `/admin/oidc*` (re-auth gated, audit in the transaction, Test), runtime OIDC host allowlist,
    Overview row from stored fields. Test issuer `testkit::oidc` (RS256). Migration `0018_oidc.sql`.
    Tests: `apps/server/tests/oidc.rs` (6 scenarios), `auth/tests/oidc.rs`, cookie unit tests,
    governed allowlist test. Web side: WP3-4 (redirect targets `/sign-in?error=<code>`,
    `/sign-in/link`, `/settings/account?error=<code>`).
  - WP3-3 sso-logout-modes done (DEC-317..326): local-first sign-out `POST /auth/logout` then
    RP-initiated logout (`id_token_hint` kept encrypted per session in `session_secrets`,
    purpose `session_secret`, dropped with the session); back-channel logout
    `POST /auth/oidc/backchannel-logout` (signature, iss/aud/iat/exp, logout event, no nonce,
    `jti` cache 10 min, by `sid` else `sub`, audit `oidc.backchannel_logout`); silent sign-in
    (`prompt=none`, hint cookie, 5-minute loop guard, interaction errors -> interactive, outage
    path with a 3-in-10-minutes retry cap, `retry=0`); `auth.sign_in_mode=auto` with its server
    preconditions (409 `auto_needs_tested_provider` / `auto_needs_admin_authenticator`) and the
    302 of signed-out page visits with a `role=status` body (`/signed-out` and the other
    exemptions never redirect); break-glass `POST /auth/sign-in/local` (admins only in auto,
    TOTP only if enrolled, throttle, audit `auth.local_sign_in`, notify `admin.local_sign_in`);
    re-auth round trip `mode=reauth` (`max_age=0`, `confirm_oidc_reauth`, pending action
    `PUT/GET /auth/reauth/pending`); Test probes `probe_silent` / `probe_max_age`. Migration
    `0019_sso_sessions.sql`. Tests: 5 new scenarios in `apps/server/tests/oidc.rs` plus the
    `#[ignore]` authentik `max_age=0` live check (`KK_OIDC_LIVE_*`, not run). The full route and
    error-code contract for WP3-4 is DEC-326.
  - WP3-4 signin-admin-web (DEC-327..336): `/sign-in` provider-first with the password form in a
    `details`, the `?error=` states (outage alert with Try again / `retry=0`, refusals, "Your
    session ended"), `/sign-in/local`, `/signed-out`, `/sign-in/link`, `/reset-password/:token`;
    Sign out as a form post to `POST /api/v1/auth/logout`. Admin area as the eight pages of
    addendum-1 3.4 (DEC-255 reversed): Overview with the brand card, Instance (hosting, sign-up,
    public address, Mail with "Save and send a test", Secrets at rest, Advanced), Sign-in and SSO
    (URIs, providers with write-only secret, Test and probes, Add a provider, mode radios), Users
    with "via group", Data sources with "Use this source"; `/admin/brand` and `/admin/security`
    are 404. Re-auth dialog "Confirm with <provider>" (pending action, settings replayed).
    Settings > Account: linked sign-ins (Link/Unlink) and the time zone; Home/History show local
    times. Browser REST calls in `web::rest` (DEC-327). E2E: `oidc-mock.mjs`, `signin.spec.ts` in
    its own Playwright project `sso` after `chromium`, `settings.spec.ts` (DEC-281 items),
    `admin.spec.ts` additions; SSR tests in `auth_pages.rs`/`admin_pages.rs`. Cuts in DEC-336.
  - WP3-5 notifications-integrations (DEC-338..345): Settings > Notifications with tabs Inbox
    (grouped by the local day, Mark all read via `POST /notifications/inbox/read-all`), Channels
    (cards with health from the latest delivery, inline "Add and send a test" per type, secrets as
    password fields), Rules (events x channels matrix, defaults labelled, locked rows, the
    defaults offer on a new channel's first switch, rules for one list or person), Log (error in
    a `details`); bell count capped at `9+`. Settings > Integrations: sender cards with health,
    last recorded item and the refusal explained inline (field, redacted body <= 4 KiB, migration
    `0020_integrations_payload.sql`), three-step connect panel on SSE `integration.call` with the
    60 s "check the token" state, `POST /integrations/{id}/test` (records nothing, marked test),
    revoke with confirm. Tests: `apps/server/tests/webhook.rs` (2 new), `notifications.rs` (new,
    2), unit tests in integrations/notify/web, `notifications.spec.ts` (rewritten),
    `integrations.spec.ts` (new, 2). Cuts in DEC-344 (item rules, channel editing -> phase 4).
  - WP3-6 people-lists (DEC-346..353): `App::companion_stats` (together, plays, wins over decided
    plays with ties as wins, last, most played; merged entries folded in, sessions a linked
    person shared counted), merge suggestions by `name_key`, merge undo from the merge record and
    "delete keeps history" (`removed_at`), migration `0021_people_lists.sql`;
    `set_list_visibility` (link kept unless `new_link`, private/instance revoke), "Shared with me"
    (instance lists of people linked either way) and "On this instance"; REST `/people`,
    `/people/{id}`, `/companions/{id}` PATCH/DELETE, `/unmerge`, `/lists/overview`,
    `/lists/{id}` GET/PATCH/DELETE, item remove and position, visibility, "Add all to my library"
    (also by share token); server functions for the four page loads (`api::people`). Web:
    `/people` (request cards, tabs, person cards, dialogs mounted outside the refetched data),
    `/people/:id`, `/lists` (tabs and cards), `/lists/:id` (edit in place, keyboard Move up/down
    with focus kept, Remove, Add through the scoped palette `PaletteTarget`), the share dialog
    (`list_share.rs`), the public page polish. Tests: `app/tests/people.rs` (new, 6) and
    `app/tests/lists.rs` (+3), `apps/server/tests/sharing.rs` (new, 3), `people.spec.ts` and
    `lists.spec.ts` (new), `sharing.spec.ts` (rewritten); `shell`, `slice` and `notifications`
    specs add people through the new sheet. Cuts in DEC-353 (Game groups, drag handles, "Log
    with", avatar upload -> phase 4).
  - WP3-7 imports-history (DEC-354..365): legacy importer from the owner's export only (fixture
    committed: kinds, ids, episodes and dates, statuses from the built-in collections, other
    collections as lists, undated views on the day the item entered Completed reported as
    estimates, the rest reported as not kept); Trakt file importer (`trakt-import`, default on,
    synthetic fixtures, credit on the Trakt card until an attribution page exists); runner with
    a preview per import (`ImportRecord.preview`), an unmatched list with manual match and
    replay, cancel per checkpoint (migration `0022_imports_page.sql`), `GET /imports`,
    `POST /imports/{id}/run|cancel|match`, CSV export; optional `lists` and `event_id` in the
    generic row (version 1 kept). Web: `/settings/imports` (format cards with drop zone,
    preview, "x of y" with Cancel, result with the unmatched list matched through the
    palette, Try again, export downloads), History H1 link to Export, episode list "Mark
    show" and "Spread over a date range". Tests: `importers/tests/{legacy,trakt}.rs`,
    `apps/server/tests/imports.rs`, `imports.spec.ts`, `item.spec.ts` (+1). Cuts in DEC-365
    ("On release dates" needs per-episode air dates, DEC-363; Goodreads, BG Stats, BGG wait
    for samples).
- **UX phase 4 (done, `ux4`):** plan `docs/handoff/phase4-plan.md`; review `docs/reviews/2026-11-09-ux4-review.md` (UX4-1..22) applied, deferred or rejected in DEC-460..494. Last green runs at `ux4`: `cargo xtask ci` 834 passed / 0 failed / 4 ignored, `cargo xtask e2e` 125 passed with 0 server panics; release load (8 clients, 2 writers, 40k events) in DEC-494.
  - WP4-1 measure-perf done (DEC-399..404): load harness `LOAD_CLIENTS`, `LOAD_SESSIONS`,
    `LOAD_SHARED`, `LOAD_WRITERS`, new endpoints (`/stats/series`, History person/year, `POST`
    progress, totals after a zone change), `cargo xtask load [--release] [--clients N]` (not in
    `ci`, refuses below 15 GB free disk); baselines in DEC-399. Statistics cache (migration
    `0023_stats_cache.sql`: per item and month, rolled up per kind and month, dirty marks by
    triggers, zone stamp, lazy recompute on read); activity and History read sessions in
    windows (`SessionRepo::*_window`, `CompanionRepo::get_many`); Home reads its parts
    concurrently; per-user fair share on the interactive lane in hosted mode (`Ctx.user`,
    `HttpRequest.user`, `Gate::acquire_for`); TheTVDB token cache keyed by blake3(PIN). p95
    before -> after, debug 40k events / 20k sessions / 2k shared: stats totals 386 -> 11, series
    382 -> 6, Home 652 -> 52, activity 220 -> 18, History person 255 -> 86, year 217 -> 19;
    debug 5k: totals 91 -> 10, Home 168 -> 49; release single client all <= 100; release 8
    clients + 2 writers: Home 1400 -> 143, totals 1000 -> 66, one of two runs failed only on the
    unchanged deep library page (306 ms, the other 297). Missed: first stats read after a zone
    change 773 ms (target 500, DEC-404). After a SQL change on `0023` in a dev database, undo
    it before `cargo xtask sqlx-prepare` (checksum).
  - WP4-2 admin-ops done (DEC-405..415; migration `0024_admin_ops.sql`): Overview reads the
    health rows from `health_probes`, written by cron `admin.probes` every minute (and once at
    start, and at once for the row an admin's own change affects); missing rows "Not checked
    yet", rows older than 3 min "Not checked recently", times in the admin's zone. Data-source
    counters flushed by every process every 60 s into `source_stats` / `source_stats_hours`
    and read back (status = database + unflushed deltas). Back-channel `jti` replay cache in
    `oidc_used_jti`. `locked_totp_count` by key version. Users row menu "More": Reset
    authenticator (`POST /admin/users/{id}/reset-totp`) and Unlink provider (`DELETE
    /admin/users/{id}/identities/{identity}`), gated, audited in the transaction, the person
    notified (`account.authenticator_reset`, `account.sign_in_unlinked`), last method refused.
    Providers table "Linked accounts". `ui::Table` `stacked` (+ `TableCell` `label`) on
    providers, Users and the delivery log; the rules matrix keeps the scroll wrapper (DEC-412).
    Emailed one-time link on `/sign-in/link` (`POST /auth/oidc/link/email`, `.../confirm`; 30
    min, single use, hashed, bound to the browser's link ticket, throttled, audited). OD-21
    email sync closed by the owner's ruling (DEC-414). E2E has a mail sink
    (`src/web/tests/mail-sink.mjs`, SMTP 3013 / HTTP 3014). Tests: `catalog-core/tests/
    stats_shared.rs`, `admin.rs` (probes), `oidc.rs` (+3), `auth/tests/pg.rs`, `admin.spec.ts`
    (row menu, stacked Users, local time), `signin.spec.ts` (emailed link).
  - WP4-3 bulk-lists done (DEC-416..421, no migration). Keyset paging for the library (added
    by the orchestrator): `GET /library` and `POST /library/query` take `after` and answer
    `next` instead of `offset` (opaque cursor, `app::ports::LibraryCursor`); `GET /library`
    reads its items in one batch; the web "Load more" follows the cursor past 500 rows; release
    8 clients + 2 writers p95: library last 100 310 -> 27/31 ms, first 223 -> 29/27, query last
    247 -> 146/146 (DEC-416). Remove from library (`App::remove_from_library`, `DELETE
    /library/{item_id}`): entry and tags go, events, plays, reviews and list items stay. Bulk
    `POST /library/bulk` (set_status, add_tag, remove_tag, add_to_list, remove; at most 500
    ids, 400 above; per-id results; one undo token = snapshot held by the client) and `POST
    /library/bulk/undo`. Web: list view checkboxes, "Select all on this page", the sticky bulk
    bar (Set status, Add tag, Add to list, Remove with a confirm, Clear; toasts with Undo),
    selection kept through refreshes and cleared on any URL state change; quick menu "Add to
    list..." and "Remove from library"; item overflow "Remove from library"; list page drag
    handles (pointer events, aria-hidden, Lucide `grip-vertical`) next to Move up/down. No
    "Mark owned" (DEC-420). Tests: `app/tests/bulk.rs`, keyset walks in `app/tests/browse.rs`
    and `store-pg/tests/phase1.rs` (+ remove on Postgres), `library_page.rs` (500 / 501 ids),
    `rest.rs` cursors, `library.spec.ts` (+2 bulk), `lists.spec.ts` (+1 drag).
  - WP4-4 keyboard-palette done (DEC-422..430, no server change, no migration). Keyboard map:
    `G then H/L/Y/T/P` (1.5 s, "G pressed: ..." hint in a polite live region, focus on the new
    page's `h1`), never while typing, in a dialog or menu, or with Ctrl/Cmd/Alt; `?` lists
    every shortcut by section (`web/src/keyboard.rs`, pure rules in `ui/src/keys.rs`: `Chord`,
    `roving_target`, `columns_from_tops`). Page keys via `keyboard::page_keys` (listener removed
    with the page; the old Library `/` listener leaked). Home `J`/`K` focus the Continue cards'
    next step; Library grid roving tabindex (one card's link, pill and menu in the tab order;
    arrows from the rendered column count, Home/End, Ctrl+Home/End, Enter, `W`, `T`, `L`); item
    page `W`/`R`/`T`/`L` (`L` = the WP4-3 list picker). "Keyboard shortcuts" switch in Settings
    > Account (WCAG 2.1.4; `localStorage` `app-shortcuts`, DEC-423). Palette: `ui::Command`
    `on_secondary` + `CommandHints`; Ctrl+Enter adds a result as Planned (toast, Undo = WP4-3
    remove) or runs an entry's one-click step, palette stays open (DEC-426); below two
    characters the last 8 opened items (`localStorage` `app-recent-<username>`, never sent) and
    suggestions Go to Lists, New list (`/lists?new=1`), Log a play of <last game>
    (`/items/{id}?log=1`), Import a file. `PaletteScope(Option<PaletteTarget>)` with `List`,
    `Match`, `RuleSubject` ("Add a rule for one item", DEC-344) and `LogWith` (People card "Log
    with...", opens the item's sheet with the person, `?log=1&with=`). Channel edit in place
    (secrets write-only, blank sends the stored placeholder back, DEC-429). Item page now
    re-renders when the kinds arrive after a client-side navigation (it had no log sheets
    then). Tests: `keyboard.spec.ts` (5), `shell.spec.ts`, `people.spec.ts`,
    `notifications.spec.ts` (+1 each), ui unit + wasm tests (keys, Ctrl+Enter), web unit tests
    (`keyboard`, `recent`, `palette`, `notify_channels`). Not built: `1-5` on a focused rating;
    "Log with" for page/position steps or items not in the library (DEC-425, 428).
  - WP4-5 offline-funnel done (DEC-431..438; migration `0025_usage_funnel.sql`). Offline queue
    (`web/src/offline.rs`): on a network failure the next step, episode marks, status changes
    and the item rating go into `localStorage` `app-offline-<username>` as sync commands (log op
    ids kept, client HLC `ClientClock` because `web` may not use `domain`, DEC-433); toast
    "Saved offline, will sync" with Undo; flushed on page load and on `online` through `POST
    /sync/commands` in one push in queue order with the queue-time stamps (DEC-455 replaced the
    two pushes with send-time stamps of DEC-434); refusals
    dropped with a toast naming the item; the banner says "N changes waiting". Sheets and the
    bulk bar are disabled offline; plays, sessions, notes, tags and the inline page/position
    fields stay online-only (DEC-432). Logging funnel (`web/src/funnel.rs`, `api/src/usage.rs`):
    flows palette, next_step, sheet, episode_mark, bulk from intent to logged/abandoned with tap
    and time buckets, plus D16 disclosure numbers from `ui::DisclosureWatch` (shown, opened,
    opened within 2 s, open at a save); one keepalive beacon `POST /usage/funnel` (signed in,
    30 a minute, at most 50 events, no ids) into `usage_funnel_daily` (no user or item column,
    UTC day); cron `usage.funnel_daily` emits one `metrics` event per flow and keeps 180 days;
    `GET /admin/usage` + CSV and "Usage measurement" on Admin > Storage and jobs; setting
    `usage.funnel` (default on, `APP__USAGE__FUNNEL`, editable); one line on Settings > Privacy
    (DEC-435..437). Review the numbers on 2026-12-06 (section 6). Tests: `apps/server/tests/
    usage.rs` (4), `offline.rs` (2), `offline.spec.ts` (3), `home.spec.ts` beacon (+1), ui wasm
    `disclosures_report_shown_opened_and_open_at_save`, web unit tests (`offline`, `funnel`),
    `api::usage` and config unit tests.
  - WP4-6 stats-goals-people done (DEC-439..445; migration `0026_goals_groups.sql`). Opt-in
    yearly goals (`goals`, one per year, group or all, measure; private, another user's goal
    is 404): Stats page only, cards "120 of 500 in 2026" with a `ProgressBar`, Edit dialog,
    Delete confirm, "Set a goal" behind a disclosure; progress = the year's months of the WP4-1
    cache; never on Home, no pace wording (R-04). "<year> so far" against the same days last
    year (Feb 29 -> Feb 28, the user's zone) on Stats per tab and in the History side panel:
    whole months from the cache plus the current month from `ProgressRepo::window_amounts`
    (SQL over the items touched in the window). "Most often with" (top 5 by sessions together;
    3 on History). Game groups (`game_groups`, `game_group_members`, `sessions.game_group_id`):
    People tab "Game groups" with cards (members through merges, plays logged as the group,
    last), "Log a play with" via `PaletteTarget::LogWithGroup` (`?log=1&group=`), new/edit/delete
    dialogs; play sheet "Groups" chips fill you plus the members; a removed group keeps its
    plays. REST `/stats/goals`, `/stats/compare`, `/game-groups`. `home::MeasureChart` and
    `widgets::QuickMenu` split under 100 lines. Not built: H-index and unplayed owned games on
    History (DEC-440). Tests: `app/tests/{goals,game_groups}.rs`, `store-pg/tests/goals_groups.rs`
    and the window/comparison test in `phase1.rs`, `stats.spec.ts` (new, 2), `people.spec.ts`
    (+2), web unit tests (`stats_extras`, `log_form`). Found, not fixed: concurrent imports of
    one TMDB id can create duplicate catalog items (DEC-445).
  - WP4-7 episode-air-dates done (DEC-446..451; migration `0027_catalog_episodes.sql`). TMDB
    `GET /3/tv/{id}/season/{n}` and TheTVDB `GET /series/{id}/episodes/official?page=0&season=n`
    verified against the official docs on 2026-11-08 (docs/connectors/{tmdb,tvdb}.md); only
    episode number, air date and runtime are kept (`catalog_episode_seasons`,
    `catalog_episodes`; `SeasonInfo.air_date`). TMDB terms raise no doubt for these fields, so
    the fetch is on (no off flag). New capability `catalog_core::Episodes`; the detail refresh
    queues `catalog.episodes` on `background_refresh` when a season needs asking (new, changed
    count or air date, or episodes still to air); one call per season through the http cache.
    `ItemPageDto.episodes`; `BulkDates` "On release dates" only when every open episode of the
    season shown has a release day up to today, else a hint with the count; a bulk mark with a
    missing or future episode is refused. `tmdb-mock.mjs` serves a synthetic show 990001 of its
    own. Not built: runtimes into stats minutes, "Up to date" badge, TheTVDB paging (the purge of
    TMDB data older than six months for unviewed items, DEC-450, was built in WP4-8, DEC-453).
    Tests: connectors
    `tmdb.rs`/`tvdb.rs` (+3), `app/tests/episodes.rs` (3) and an app unit test,
    `store-pg/tests/episodes.rs`, a web unit test in `episode_plan`, `item.spec.ts` (+1).
  - WP4-8 hardening done (DEC-452..459; migration `0028_catalog_retention.sql`). Catalog race
    (DEC-445) fixed in the database: `CatalogRepo::insert_new` claims all external ids in one
    transaction, a loser updates the winner (connector data), fills it (search) or leaves it
    (user data); old duplicates not migrated (DEC-452). TMDB terms 1.C: hourly cron
    `catalog.retention` (lane `background_refresh`) refreshes items whose TMDB data is older
    than `jobs.catalog_max_age_days` (default 150, 1..170) or purges it when no refresh is
    possible or past 175 days (title, year, ids stay; cover image deleted; stale HTTP cache
    entries go too); `catalog_items.provider_data_at` (DEC-453, docs/connectors/tmdb.md). D16
    "open at a save" through `reauth::gated` verified by `admin.spec.ts` (DEC-454). Offline
    queue: commands keep their queue-time stamps, one push in queue order, a log carries the
    stamp of the status it implies (`SyncCommandDto::LogProgress.hlc`), so another device's later
    change wins (DEC-455, revises DEC-434). UX3-25 panic guard: `cargo xtask e2e` logs the
    server to a temp file and fails on `panicked at`; it caught the panic once (leptos
    Transition memo read with a try-lock while another thread wrote the task set); fixed by
    rendering the Library Transition without `set_pending` on the server; then 3 x 117 passed,
    0 panics (DEC-456). Deferred tests: 13 groups added, 2 already covered, 13 cut and named
    (DEC-457); a Trakt rating outside 1..10 no longer creates an empty row. Splits: `cli.rs`,
    `instance.rs` without file-level allows, `ui::DropdownMenu` under 100 lines (DEC-458).
    Final load (DEC-459), p95 ms: debug 5k: search 90, library 10.7/10.7, Home 53, query 86/64,
    totals 9.8, series 3.4, activity 15, person 59, year 13, POST 8.4, zone change 142; debug
    40k: search 107, Home 52, query 89/64, totals 12, series 5.7, activity 18, person 85, year
    19, zone change 770; release 8 clients + 2 writers, 40k: search 290, library 37/51, query
    250/184, Home 207, person 233, year 67, activity 59, totals 76, series 59, POST 51, writers
    80, zone change 767 - all under the 300 ms budget, one run, higher than the WP4-1/WP4-3
    runs across the board (host variation, not proven). Open: first stats read after a zone
    change still ~770 ms (target 500), "library query, first 100" ~250 ms under 8 clients
    (full scan for the count; follow-up in DEC-459).
  - Review ux4 fixes, server half (DEC-460..476, review `docs/reviews/2026-11-09-ux4-review.md`;
    migrations `0029_stats_dirty_gen.sql`, `0030_catalog_expiry.sql`): stats marks with a
    generation (no lost mark, UX4-1); fair-share rounds and a cancel-safe gate (UX4-2/3); the
    panic guard also in the GitLab e2e job, log kept on failure (UX4-4); bulk undo is the
    per-op inverse in an HMAC-signed, user-bound, one-hour token, refused over 1 MiB (UX4-5/12);
    strict TMDB retention: title and year purged too, placeholder, refetch once TMDB is usable,
    unreferenced items deleted (UX4-9, owner may reverse, section 6); admin menu self and
    break-glass guards (UX4-10); emailed-link re-check (UX4-11); pairing window (UX4-16); one
    session read on People, 404 for foreign people (UX4-17); shared-hash `forget` (UX4-18).
    Load after the fixes (DEC-476), release, 8 clients + 2 writers, 40k, two runs, p95 ms:
    search 216/208, library 26/26 and 29/31, query 214/215 and 148/157, Home 144/140, person
    207/202, year 58/45, activity 53/45, totals 58/66, series 43/46, POST 33/30, writers
    40/42 (n 323/320; the count follows the client phase length), zone change 768/779 (now
    asserted under 1000). Follow-ups P-1 zone-change rebuild, P-2 query count, P-3 `/people` in
    the load set: performance workstream after the tag (DEC-476). Search before the tag
    (DEC-493): words first, typos (`word_similarity`) only when the page is not full, hits read
    in one batch; search p95 282.7 -> 85.7/84.0 (two passing runs; person 227/232, Home 184/184,
    query 215/210 and 144/143). `GET /items/{id}` watches the item when it queues a refresh
    (DEC-492). History person filter (DEC-494): the person's sessions drive the reads (regions
    of 121 s around them, one span read per source per round, window walk only after a dense
    round) instead of filtering doubling windows; same rows (new equivalence tests in
    `app/tests/activity_person.rs` and `store-pg/tests/windows.rs`). Release, 8 clients + 2
    writers, 40k, three runs (the first right after the build), p95 ms: History person
    45.5/45.7/46.5 (was 335.8/347.2 right after a build, ~230 later), activity 49/55/47, year
    68/60/61, Home 192/158/185, query 212/207/212 and 144/155/147, library 39/29/33 and
    38/30/39, search 108/100/102, totals 58/56/53, series 42/45/44, POST 34/35/32, writers
    48/44/43, zone change 745/755/753; all asserts pass in all three.
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
- **Review the logging funnel after four weeks, on 2026-12-06** (design.md 9, R-54, DEC-431):
  read Admin > Storage and jobs > Usage measurement (or `/api/v1/admin/usage.csv`), cut what is
  not used, promote a disclosure opened on more than half of its visits (D16), and decide on
  saved views (phase4-plan.md deferrals).
- **TMDB retention, strict reading (orchestrator decision after review UX4-9, DEC-466):** past
  the six-month limit without a possible refresh every TMDB field goes, title and year included
  (a neutral placeholder "Title unavailable - source data expired" is shown, the item is fetched
  again once TMDB is usable), and items nobody refers to are deleted. To reverse: keep title and
  year as the person's record (DEC-453's reading) - see DEC-466 "how to reverse".
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
- **Phase 3 (done, `ux3`), sign-in and the stale pages.** OIDC multi-provider and true SSO (addendum 4,
  OD-15..21 with OD-17 as changed; re-auth OIDC round trip for DEC-191 via
  `Auth::confirm_oidc_reauth`; encrypt OIDC client secrets and the SMTP password; the
  Sign-in-and-SSO and Instance admin pages DEC-255 left out), Notifications, Integrations,
  People, Lists, Imports (with the legacy Ryot importer from the fixture, Trakt), per-user
  time zone on the settings page (DEC-239), "Mark show" and backfill date modes and the
  History export link (DEC-237).
- **Phase 4 (done, `ux4`), fluency and measurement.** Keyboard map, bulk edits, offline queue (DEC-237),
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
  -executable -delete`. Old rlib/rmeta builds pile up too (58 copies of `libserver` were
  found once; deleting them freed 10 GB): in `src/rust/target/debug/deps` run
  `rtk proxy ls -t | grep -E '^lib.+-[0-9a-f]{16}\.(rlib|rmeta|so)$|^.+-[0-9a-f]{16}\.d$' |
  perl -ne 'chomp; /^(.+)-[0-9a-f]{16}\.(\w+)$/ and do { $k="$1.$2"; unlink $_ if $seen{$k}++ }'`
  (keeps the newest per crate; cargo rebuilds anything still needed; plain `ls` is rewritten
  by rtk and then matches nothing). If still short, `rm -rf src/rust/target/debug` (rebuild
  ~1 h). Drop
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
  families); on a client-side navigation a page is built before `use_query_map` has the new
  URL's values and before every resource of its `<Transition>` is there, so read query
  parameters in effects and key a `<For>` on what can arrive late (DEC-428); `window_event_listener`
  is not removed with the component, so pair it with `on_cleanup(move || h.remove())`. WP4-5: inside
  `view!` an attribute closure like `when=move || n() > 0` ends the tag at `>` (define the
  closure before `view!`); `<Show>` children must be `Fn`, so a closure there must not move an
  `Arc` (call `use_i18n()` inside it instead); `web_sys::RequestInit` 0.3.107 has no
  `set_keepalive` (set it through `js_sys::Reflect`).
- GitLab: `glab` works on this host (`glab ci list -R hordwyn/hordwyn`).
- Commits: Conventional Commits, no attribution lines of any kind, never force-push `main`,
  tag milestones.

## 9. Links

- Round 2 review page (2026-11-10, F1 + F2 decisions, 64 radios + 53 "object" items): https://claude.ai/artifact/7cUpvVeCKLEx7qCtE1Rars (store `answers`, doc id = decision id, `{id, choice, note, updated}`; no doc = unanswered or, for defaults, no objection). Source `docs/fable/round2/review-page.html`.
- Review page (answered): https://claude.ai/artifact/DXNJa8yNPXcUH3nd227wKZ (store `reviews`)
- Open decisions page (partly answered, keeps the unreviewed DECISIONS entries):
  https://claude.ai/artifact/EMtPcfBRksJoFv4LDVHfYv (store `answers`; read it back with the
  ArtifactData tool before each phase to pick up new owner answers; last read 2026-11-06,
  nothing new)
- Design: `docs/design/ux/design.md`, `addendum-1.md`, `research.md`, `mockups/index.html`
- Agent briefs: `docs/handoff/agent-briefs/`
- Research: `docs/research/`; connector docs: `docs/connectors/`; reviews: `docs/reviews/`
