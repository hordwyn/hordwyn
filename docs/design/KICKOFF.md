# KICKOFF: kithkeep (autonomous build, v1)

You are an autonomous coding agent. You work alone, without the owner present.
Read this whole file, then `docs/design/DESIGN.md`, before writing any code.

## 1. Mission

Build `kithkeep`, an MIT-licensed, self-hosted, multi-user, white-label tracker for media
(movie, show, anime, manga, book, audiobook, podcast, music, video game, visual novel,
comic, board game) with companions, sharing, imports, notifications and integrations.
Full Rust: axum + Postgres (sqlx) + Meilisearch on the server, Leptos 0.8.x (SSR + hydrate)
with a reusable shadcn-style `ui` crate on the client. End state: `docker compose up`
starts app + Postgres + Meilisearch; a user can sign up, search all kinds, track
progress, log board game plays with companions, import from the listed sources, receive
configurable notifications, and the whole thing passes CI on GitLab. Pages never wait on
third-party providers, and all stored bytes stay inside a configurable storage budget.

`docs/design/DESIGN.md` is the specification. Section numbers below (for example "D-8")
refer to its sections. If this file and DESIGN.md disagree, this file wins; log it.

## 2. Ground rules

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

## 3. Repo layout

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

## 4. Engineering standard

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

## 5. Context-specific rules

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

## 6. Milestones

Each milestone is a spike with an acceptance command. `||` marks milestones that may run
in parallel (separate subagents or worktrees) once their dependencies are done.

**M00 Bootstrap.** Repo layout from section 3, flake, toolchain, empty crates from D-3
with `lib.rs` and one passing test each, xtask, CI file, cargo-deny config, architecture
test, white-label grep test, Antora skeleton, DECISIONS.md, compose file, MIT LICENSE.
Also: commit-lint CI job (rule 10) and project settings that disable AI attribution.
Accept: `cargo xtask ci` green; `nix flake check` green (or a DECISIONS entry if no Nix).

**M01 Domain and app core** (after M00). Domain types: users, companions (self-companion,
link, merge), catalog items, `KindDescriptor`, `ProgressModel` (all five), statuses,
progress events (append-only), ratings, reviews, lists, sessions with participants,
external ids. `kinds` crate with the built-in kinds and typed attributes + generated JSON
Schema. `app` ports from D-4 and use-cases for: add to library, log progress, rate,
review, list management, log session, companion CRUD/link/merge/promote/demote.
`testkit` fakes for every port. UUIDv7, HLC for LWW fields (D-12).
Accept: `cargo test -p domain -p kinds -p app -p testkit`; adding a dummy kind in a test
requires no change outside `kinds` (test proves it).

**M02 Persistence and jobs** (after M01) `||` M03 `||` M06.
`store-pg`: migrations, repo adapters, `outbox` table written in the same transaction,
per-user change sequence for sync. `jobs`: lanes (interactive, user_import,
background_refresh, prefetch), priorities, dedupe keys, retry with backoff, cron,
graceful shutdown.
Accept: `cargo test -p store-pg -p jobs` against a real Postgres; tests prove dedupe,
lane priority under load, and outbox atomicity (rollback leaves no outbox row).

**M03 Catalog core + TMDB** (after M01) `||` M02 `||` M06.
Capability traits, registry with `ConnectorInfo` (incl. attribution), governed HTTP
client (governor, semaphore, circuit breaker, Retry-After, jitter), singleflight, HTTP
cache (content-addressed, ETag/Last-Modified, TTL), host allowlist, merge with field
provenance, stale-while-revalidate states. TMDB connector (movie, show) with fixtures.
All connector inputs/outputs plain serialisable data (WIT-compatible, D-5.2).
Accept: `cargo test -p catalog-core -p connectors --features tmdb`; tests prove 100
concurrent identical requests -> 1 upstream call; 429 + Retry-After honoured; open
circuit fails fast; no outgoing request bypasses the governed client (a test with a
non-allowlisted host fails).

**M04 Assets and storage budget** (after M02) `||` M05 `||` M08.
opendal FS + S3 (S3 tests against minio in compose, skipped with a clear message if not
running), blake3 addressing, variants AVIF/WebP, thumbhash, dominant colour, dedicated
rayon pool, `StorageBudget` with every setting in D-8, fetch priorities, pinned vs
cache tier, LRU eviction with watermarks, hard-cap fallback to redirect, S3 daily op
counters, `cli storage report --dry-run`.
Accept: `cargo test -p assets`; property tests: eviction never removes pinned assets and
always ends below the low watermark; at hard cap no bytes are written and requests still
succeed (redirect); image work never runs on a tokio worker (test with a current-thread
runtime + timing).

**M05 Auth, API, realtime, server** (after M02, M03).
`auth` (argon2, sessions, OIDC, TOTP, roles), REST `/api/v1` with utoipa OpenAPI,
SSE `realtime` (entity.updated, job.progress, import.progress), `apps/server` with
`--role=all|web|worker`, config crate (file + env, validated, generates the config
reference doc), telemetry.
Accept: `cargo test -p auth -p api -p server`; **latency test:** with a fake connector
that sleeps 10 s, `GET` item detail and search endpoints answer in < 200 ms and an SSE
`entity.updated` arrives after the fake finishes.

**M06 UI component library** (after M00) `||` M02 `||` M03.
First, icon research (rule 11, section 5): spawn a researcher subagent, compare the
candidate libraries, write `docs/research/icons.md` with a recommendation (one UI icon
set, one service-logo source), log the choice in DECISIONS, then integrate it as an
`Icon` component in `ui` (icons selected by name, tree-shaken, no runtime fetch).
`ui` crate: primitives listed in D-3, Tailwind v4 with CSS-variable tokens, light/dark,
variants helper, `class` override, accessible (keyboard, aria). A component gallery page
served by `cargo leptos watch` in a dev-only route.
Accept: `docs/research/icons.md` and its DECISIONS entry exist; `cargo test -p ui` + wasm tests (`wasm-bindgen-test`) for interactive pieces;
Playwright gallery smoke test with axe accessibility check, no violations of level A/AA.

**M07 Web app vertical slice** (after M04, M05, M06).
`web` crate (ssr/hydrate/csr features): sign-up/in, library grid with thumbhash
placeholders, item detail, progress logging, ratings, lists, Ctrl-K search palette with
streamed connector results, companions page. `brand` crate applied (D-10), i18n wired.
Accept: Playwright e2e: sign up -> search a movie (TMDB fixtures via a fixture-serving
mock) -> add -> mark watched with a companion -> appears in library; brand swap test
renders a second brand without code changes; `cargo leptos build --release` green.

**M08 Search** (after M02) `||` M04 `||` M05.
`search` adapters: Meilisearch and pg-fallback (FTS + pg_trgm) behind `SearchIndex`;
indexer worker consuming the outbox in batches; indexes library/catalog/people (D-13);
server-side tenant filters; scoped keys only; `cli reindex`.
Accept: `cargo test -p search` against real Meilisearch and Postgres; the same test suite
passes for both adapters; a test proves user A never sees user B's private docs.

**GATE G1: Fable architecture review** (after M01-M08, with one kind end-to-end).
Spawn a `claude-fable-5-1` reviewer that has not seen your work. Give it DESIGN.md, this
file and the repo. It answers all twelve questions of D-15, including the architect
judgement (hexagonal per crate, KISS/YAGNI/DRY, crate boundaries, biggest risk).
Output `docs/reviews/<date>-fable-architecture.md`. Apply or reject each finding with a
DECISIONS entry. Nothing after G1 starts until this is done.

**M09 Remaining connectors and kinds** (after G1). One subagent per connector `||`:
TVDB, IGDB, OpenLibrary, Hardcover, AniList, MusicBrainz, Podcast Index, VNDB, Metron,
BGG (bearer token, D-5.3). Each starts with `docs/connectors/<id>.md`.
Accept per connector: `cargo test -p connectors --features <id>`; search + details +
images covered by fixtures; attribution shown on the detail page (e2e assertion).

**M10 Board game research** (after G1) `||` M09. Spawn a `claude-fable-5-1` researcher
with the D-18 brief. Output `docs/research/board-games.md` with a concrete proposal.
Turn it into DECISIONS entries and an updated D-9a section in `docs/` (AsciiDoc).

**M11 Board games** (after M09 BGG, M10). Collection status, ownership details, loans,
plays/sessions with participants and everything accepted from M10, game nights,
campaigns (if accepted), stats (H-index, milestones, cost per play, win rate per
companion, most-played-with, unplayed owned).
Accept: `cargo test -p app -p store-pg` board-game suites; e2e: log a 4-player play with
two companions and a linked user, the linked user sees it in their shared view.

**M12 Importers** (after G1) `||` M09 `||` M13. One subagent per importer: Trakt,
Goodreads, BG Stats, BGG, generic CSV/JSON (own documented schema), legacy export
(from the owner's file only, rule 6, Q4). Streaming, idempotent, dry-run, resumable, per-row report, SSE progress,
`user_import` lane, catalog matching queued not inline (D-16).
Accept: `cargo test -p importers --features <id>`; running each import twice yields no
duplicates; killing the worker mid-import and restarting finishes the import.

**M13 Notifications** (after G1) `||` M12. Event catalogue, subscriptions with filters,
channels, immediate/digest, quiet hours, timezone, rate limits, templates with brand and
admin overrides, delivery log, dedupe (D-17). Channels: inbox, SMTP, ntfy, Gotify,
Discord, Matrix, Apprise, signed webhook, web push.
Accept: `cargo test -p notify -p app`; digest test with a fixed clock; dedupe test (same
episode released twice -> one notification).

**M14 Integrations** (after M05). Jellyfin webhook + optional poll; Radarr/Sonarr
read-only (availability, calendar). Respect rule 7.
Accept: `cargo test -p integrations` with recorded payloads; e2e: posting a Jellyfin
playback-stop payload creates a progress event and an SSE update.

**M15 Sharing and companion flows** (after M07, M11). Visibility (private, instance,
link), link request / accept, promote companion -> user (invite), demote user ->
companion (snapshot), merge companions, claim/copy shared session, per-user privacy.
Accept: e2e covering promote, accept, see history, demote, merge; history unchanged.

**M16 Sync protocol** (after M05). Command batches with client op ids (idempotent),
changes-since-cursor pull, HLC LWW for mutable fields, append-only events. Optional
spike: `apps/desktop` Tauri 2 shell building `web` in csr mode; only "it builds and
shows the login page", record findings in DECISIONS.
Accept: `cargo test -p api -p app` sync suite: two simulated offline clients edit the
same item, both converge to the same state after sync; replaying a batch is a no-op.

**M17 Hardening and release** (last). Full Playwright suite, load test of search and
library pages with 50k items, docker image < 150 MB if feasible (log otherwise),
compose quick start verified from a clean machine, Antora docs complete, `REPORT.adoc`.
**GATE G2:** a fresh `claude-fable-5-1` reviewer checks the finished system against
DESIGN.md and this file; findings applied or rejected in DECISIONS.
Accept: `cargo xtask ci` + `cargo xtask e2e` green; `docker compose -f
src/docker/compose.yml up` -> healthy within 60 s.

## 7. Open questions (use these defaults, log them, continue)

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

## 8. Final report

Write `docs/REPORT.adoc` (also on any early stop) with:
- What was built, per milestone, with tag and commit.
- Test results: commands run and their output summaries, coverage if measured,
  e2e results, latency test numbers, image size.
- Decisions taken (link DECISIONS.md, list the most important ten).
- Deviations from DESIGN.md and this file, with reasons.
- Review outcomes of G1 and G2 (accepted / rejected findings).
- Known gaps and bugs.
- Next steps, ordered by value.
