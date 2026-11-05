# Design: self-hosted life tracker (codename: `kithkeep`)

Status: draft v0.3 - input for the kickoff prompt.
License: MIT.
Clean-room note: this document is the **only** reference for implementation. No GPL-family
(GPL, AGPL, LGPL) code, schemas, docs, UI text or assets may be looked at or used.
Feature ideas are described here in our own words.

---

## 1. Goals and non-goals

**Goals (v1)**
- Track media of many kinds: movie, show, anime, manga, book, audiobook, podcast, music,
  video game, visual novel, comic, **board game**.
- Multi-user instance with roles, sharing, and **companions** (people you do things with)
  that can be promoted to users and linked back.
- Fast: no page and no user action ever waits on a third-party provider.
- Bounded resource use: a configurable **storage budget** for disk and S3.
- Offline-capable client later (Tauri preferred), so the sync model is designed in v1.
- **White-label ready**: no product name, colours or copy hard-coded anywhere.

**Non-goals (v1)**
- Fitness tracking (postponed; the user uses Hevy).
- Anything that searches for, downloads or indexes media files (see D-11).
- SQLite on the server. Postgres is the only server database. SQLite may appear later as the
  local store of the offline client.

**Deployment target:** docker compose with `app` + `postgres` + `meilisearch`.

---

## 2. Stack

| Concern | Choice |
|---|---|
| Language | Rust everywhere (server, web UI, desktop/mobile) |
| Web UI | Leptos 0.8.x (pin it; 0.9 is in progress and breaking), SSR + hydrate, cargo-leptos |
| Styling | Tailwind v4, theme values as CSS variables (shadcn-style) |
| HTTP | axum |
| DB | Postgres via sqlx (compile-checked queries), sqlx migrations |
| Search | Meilisearch (primary), Postgres FTS + pg_trgm (fallback) behind one port |
| Blob storage | opendal (local FS default, S3-compatible optional) |
| Images | `image` + `fast_image_resize`, AVIF/WebP encoders, thumbhash |
| Jobs | own Postgres queue (`FOR UPDATE SKIP LOCKED`) |
| Auth | argon2 passwords, sessions, OIDC (`openidconnect`), TOTP |
| Rate limiting | `governor` |
| API docs | utoipa (OpenAPI) for the REST API |
| i18n | `leptos_i18n` or Fluent; every user-visible string goes through it |
| Desktop/mobile | Tauri 2 (later milestone), Leptos in CSR mode |
| IDs | UUIDv7 everywhere (sortable, client-generatable for offline) |

---

## 3. Workspace layout

```
apps/
  server/        # one binary: --role=all|web|worker. Wires adapters into app.
  cli/           # admin: migrate, reindex, storage report, user mgmt, imports
  desktop/       # later: Tauri 2 shell, reuses crates/web in csr mode + crates/ui
crates/
  domain/        # pure types + rules. No IO, no async, no framework deps.
  app/           # use-cases + ports (traits). Depends on domain only (+ async-trait, thiserror).
  contracts/     # DTOs shared server <-> client (serde, wasm-safe). No domain internals.
  kinds/         # built-in media-kind modules (descriptor + typed attributes) - see D-5
  store-pg/      # Postgres adapter for repo ports + migrations + outbox
  search/        # SearchIndex adapters: meilisearch, pg-fallback
  jobs/          # Postgres job queue: lanes, priorities, dedupe keys, retry, cron
  realtime/      # event bus -> SSE (entity.updated, job.progress, import.progress)
  catalog-core/  # connector traits, registry, governor, singleflight, http cache, merge
  connectors/    # one crate, one module + cargo feature per provider
  assets/        # blob store, image pipeline, storage budget + eviction (D-7, D-8)
  importers/     # one crate, module + feature per import format, streaming parsers
  integrations/  # inbound (webhooks), pollers, outbound - Jellyfin, *arr read-only
  notify/        # notifier adapters (email, ntfy, Discord, Apprise, webhook)
  auth/          # passwords, sessions, tokens, OIDC, TOTP
  brand/         # white-label config + build-time asset generation (D-10)
  config/        # typed config: file + env, validated at startup
  telemetry/     # tracing, metrics
  testkit/       # in-memory fakes for every port, fixtures, recorded HTTP
  ui/            # generic shadcn-style Leptos components. Depends on nothing in-workspace.
  web/           # Leptos app: routes, pages, domain widgets. features: ssr | hydrate | csr
```

Dependency rule (enforced by a test that reads `cargo metadata`):
`domain` <- `app` <- adapters (`store-pg`, `search`, `jobs`, `connectors`, `assets`, ...) <- `apps/*`.
`ui` has no workspace deps. `web` depends on `ui` + `contracts`, and on `app` only under `ssr`.
`domain` and `app` must not depend on sqlx, axum, leptos, reqwest, opendal or meilisearch.

**UI split:** `ui` = primitives (Button, Card, Dialog, Sheet, Command, Combobox, Tabs, Table,
Toast, Tooltip, Skeleton, Badge, Avatar, DropdownMenu, form fields, DataGrid). Compound
components, `class` override, variants via a small `cva`-like helper. `web` holds domain widgets
(PosterCard with thumbhash placeholder, RatingInput, ProgressBar, PlayScoresheet, CompanionPicker).

**APIs:** Leptos server functions for the UI; versioned REST `/api/v1` + OpenAPI for
integrations, browser extension, scripts and the future Tauri sync. Both call the same `app`
use-cases.

---

## 4. Where hexagonal architecture applies (and where not)

**Applies - the `app` core.** Ports (traits) owned by `app`, adapters in their own crates:

| Port | Adapters |
|---|---|
| `Repos` (per aggregate) | store-pg; in-memory (testkit); later sqlite (offline client) |
| `SearchIndex` | meilisearch; pg-fallback; in-memory |
| `BlobStore` | opendal FS / S3; in-memory |
| `CatalogConnector` (+ capability traits) | compiled-in connectors; later WASM connectors |
| `JobQueue` | jobs (pg); in-memory |
| `EventBus` | realtime; in-memory |
| `Notifier` | notify/*; recording fake |
| `Clock`, `IdGen` | system; fixed (tests) |
| `HttpFetch` | governed reqwest; recorded replay (tests) |

Benefits: every use-case is testable without Postgres/network; the same `app` core can later run
inside the Tauri client against a SQLite adapter for offline mode.

**Applies - media kinds and connectors (the "dynamic media" question).** See D-5.

**Does not apply:** `domain` (pure), `ui` (components), `contracts` (data). Don't add ports for
things that will only ever have one implementation and no test value (e.g. config loading).

---

## 5. Media kinds and connectors

### 5.1 Kind model: core + extensions
A media kind is described by a `KindDescriptor`, not by a Rust enum scattered through the code:

```
KindDescriptor {
  id: KindId ("movie", "board_game", ...),       // stable string, namespaced for plugins: "ext:foo/bar"
  progress_model: ProgressModel,               // closed set, see below
  statuses: [planned, in_progress, completed, paused, dropped, + kind-specific],
  attributes_schema: JSON Schema,              // validated on write; drives facets + forms
  facets: [attribute paths exposed to search filters],
  relations: allowed relation types (sequel, adaptation, expansion_of, part_of_series, ...),
  default_connectors: [ConnectorId],
}
```

Storage: one generic `catalog_item` table (kind, title, original_title, year, description,
cover asset, external ids, common columns) + `attributes jsonb` validated against the kind's
schema. Built-in kinds live in `crates/kinds` as modules that implement a `KindModule` trait
with a **typed** attributes struct (serde + schemars generates the JSON Schema), so Rust code gets
types while storage and the UI stay generic.

`ProgressModel` is a small closed set (new kinds pick one; adding a model is a code change):
- `Single` (movie): done / not done, rewatch count.
- `Episodic` (show, anime, podcast): season/episode tree.
- `Paged` (book, manga, comic): page / chapter / volume.
- `Timed` (audiobook, music album): position in seconds.
- `Sessions` (video game, board game): list of sessions with duration, participants, outcome.

### 5.2 Connectors (providers)
Capability traits instead of one fat trait with "unsupported" defaults:
`Search`, `Details`, `People`, `Groups`, `Trending`, `Translations`, `Images`.
A connector registers `ConnectorInfo { id, kinds, capabilities, rate_limit, auth }`.

All inputs/outputs are plain serialisable data (no Rust-only types, no borrowed state) so the
same contract can later be expressed as a **WIT world** for WASM connectors:

- **Tier 1 (v1):** compiled-in Rust connectors, one cargo feature each.
- **Tier 2 (later):** WASM components via wasmtime + WIT (`wasm32-wasip2`). The host gives
  plugins `http-fetch` **through our governor, HTTP cache and host allowlist**, so a plugin can't
  bypass rate limits or the storage budget. A plugin may also ship a `KindDescriptor`
  -> this is how media kinds get added dynamically without recompiling.
  Choose wasmtime + WIT over Extism: typed interfaces, standards-based, Rust-native.
- Out-of-process HTTP connectors: not planned; WASM covers the use case with better isolation.

### 5.3 v1 connectors
TMDB (movie, show), TVDB (show), IGDB (video game), OpenLibrary + Hardcover (book, audiobook),
AniList (anime, manga), MusicBrainz (music), BoardGameGeek (board game; **needs a registered
application + bearer token**, approval can take a week - apply early),
**Podcast Index** (podcast), **VNDB** (visual novel), **Metron** (comic).

Selection rule: prefer the most open source (open data licence, free API, community-run). Data
licences (e.g. ODbL, CC-BY-SA) do not affect our MIT code licence, but they can require
attribution: each `ConnectorInfo` carries `attribution { text, url, licence }`, shown on detail
pages and in an "Data sources" page. The agent must verify each API's terms and record them in
`docs/connectors/<id>.md` before implementing it.

---

## 6. Provider pipeline (performance)

**Rule: requests never block on providers.**

1. **Stale-while-revalidate.** Each catalog entity has `state` (stub | partial | full | failed),
   `fetched_at`, `fresh_until`. Render what exists; if stale/partial enqueue a refresh and return.
   On completion publish `entity.updated` over SSE; Leptos resources refetch. No sleep-polling.
2. **Singleflight.** Concurrent requests for the same `(connector, op, key)` share one future.
3. **Per-connector governor.** Token bucket + concurrency semaphore + circuit breaker; honour
   `Retry-After` / 429; jittered backoff.
4. **Priority lanes** in the job queue and the governor: `interactive` > `user_import` >
   `background_refresh` > `prefetch`. Bulk imports cannot starve interactive work.
5. **HTTP cache.** Raw responses stored content-addressed with ETag/Last-Modified revalidation
   and TTL; counts toward the storage budget (D-8). Parsers can be re-run without refetching;
   recorded responses double as test fixtures.
6. **Search fan-out.** Query local Meilisearch first (instant); fan out to connectors in parallel;
   stream each connector's results to the UI as they arrive; dedupe via external IDs.
7. **Merge.** Snapshots from several connectors merge by per-kind field priority; each field keeps
   its provenance. `external_id(kind, namespace, value)` links records across providers.
8. **CPU isolation.** Image decode/resize/encode and big import parsing run on a dedicated,
   bounded rayon pool, never on tokio worker threads.

---

## 7. Assets

- Content-addressed (blake3) objects in a `BlobStore` (opendal). Dedupe is automatic.
- `asset` table: hash, kind (cover, backdrop, person, avatar, upload), source URL, bytes per
  variant, ref-count, pinned flag, last_accessed_at, tier.
- Variants: `thumb` (~200w), `card` (~400w), `detail` (~800w), `full`; AVIF + WebP;
  thumbhash + dominant colour stored in the DB and inlined into HTML for instant placeholders.
- Served from `/assets/{hash}/{variant}.{ext}` with `Cache-Control: public, max-age=31536000, immutable`.
  Optional CDN base URL in config.
- Missing variant -> 302 to the provider's original URL **and** enqueue generation (priority
  depends on why it was requested - see D-8).
- Originals are **not** kept by default (config flag); only configured variants are stored, with
  a max source dimension cap.

---

## 8. Storage budget (disk and S3 safety net)

Everything that writes bytes goes through a `StorageBudget` service in `assets`.

**Config (all optional, sensible defaults):**
```
storage:
  backend: fs | s3
  max_total_bytes: 20GiB          # hard cap for cached/derived data
  high_watermark: 0.90            # start evicting
  low_watermark: 0.75             # evict down to this
  min_free_disk: 5GiB | 10%       # fs only: never go below free space on the volume
  keep_originals: false
  variants: [thumb, card, detail] # which variants to materialise
  max_source_pixels: 4096x4096
  fetch_policy: lazy | library | eager   # default: library
  per_user_upload_quota: 1GiB     # user uploads (photos of plays, avatars)
  http_cache_max_bytes: 2GiB
  s3:
    max_put_per_day: 50000
    max_get_per_day: 500000
    max_egress_bytes_per_day: 20GiB   # estimate; when exceeded, stop proxying, redirect
```

**Fetch priority (what gets bytes first):**
1. `visible` - an image the client is rendering right now (requested via `/assets/...`).
2. `library` - covers of items in any user's library / lists / plays.
3. `related` - people, seasons, expansions of library items.
4. `catalog` - search results and trending that nobody has added. Only under `eager`.

**Tiers and eviction:**
- **Pinned** (never evicted, count toward quotas): user uploads, assets referenced by a library item.
- **Cache** (evictable): everything else. LRU by `last_accessed_at`, weighted by tier.
- Above `high_watermark` -> evict cache tier to `low_watermark` (background job).
- At hard cap -> stop materialising new variants, serve redirects to the provider URL, emit an
  admin notification and a metric. Never fail a user request because of the budget.
- On S3: no HEAD-before-PUT (hash is known in the DB), batch deletes, daily operation counters
  in Postgres, counters exposed in an admin "storage report" (also `cli storage report --dry-run`).
- Meilisearch index and Postgres size are reported (not budgeted) in the same report.

---

## 9. Users, companions, sharing

- **User**: account on this instance; roles `admin` | `member`; optional OIDC link; TOTP.
- **Companion**: a person owned by one user's address book ("Anna"). Has name, avatar, notes.
  Every user automatically has a **self-companion** representing themselves.
- **Participants always reference a companion**, never a user directly
  (play participants, "watched with", co-op sessions). This keeps one foreign key and makes
  promotion/demotion cheap.
- **Link**: `companion.linked_user_id` (nullable). Creating a link needs the target user's
  acceptance (invite/claim flow). One user can be linked from many owners' companions.
- **Promote companion -> user**: admin or owner creates/invites an account from the companion;
  on acceptance the link is set. Nothing is rewritten; the new user immediately sees shared
  sessions they participated in (read-only, with "copy to my log" / "claim").
- **Demote user -> companion**: on account deletion or unlink, companions keep a snapshot of
  name/avatar; history stays intact.
- **Merge companions** (duplicates) as a first-class operation.
- **Sharing**: lists and profiles with visibility `private` | `instance` | `link`; shared sessions
  are visible to linked participants by default (per-user privacy setting).

## 9a. Board games
- Catalog attributes: players min/max/best, playtime, min age, weight, mechanics, categories,
  designers/artists/publishers (as people), expansions (`expansion_of`), BGG id.
- Ownership (per user): owned, previously owned, wishlist(priority), want to play, for trade,
  preordered; purchase price/currency/date, condition, location/shelf, loaned to (companion) + dates.
- **Play (session)**: date, duration, location, expansions used, scenario, campaign,
  participants [companion, score, winner, rank, team, faction/colour, starting player, first play],
  co-op outcome, incomplete flag, notes, photos (upload quota applies).
- Stats: plays per game, H-index, 5/10/25 play milestones, cost per play, win rate per companion,
  most-played-with, unplayed owned games.
- Imports: BG Stats JSON, BGG plays + collection.
- Sessions are generic (`ProgressModel::Sessions`), so "watched with" and video game sessions reuse
  the same participant model.

---

## 10. White-labeling

- No product name, logo, colour, domain or support URL in code. All from `brand` config:
  name, short name, logos (light/dark), favicon, PWA manifest, theme tokens, email templates,
  legal links, default locale.
- Theme = CSS variables generated from brand tokens; `ui` only references variables.
- All copy through i18n; brand can override any string key.
- Cookie names, header prefixes, OpenAPI title, user-agent for outgoing requests: derived from
  brand config, not hard-coded.
- Tauri bundle id, icons and app name generated from the brand config at build time (`brand`
  crate has a build helper).
- Per-deployment feature toggles: enabled kinds, connectors, integrations, sign-up mode.
- Icons and logos only from free, openly licensed libraries (UI icons) and free logo
  collections or official press kits (service logos); never invented or generated. The
  choice and its attribution handling are researched before integration. Every vendored
  file is listed with source and licence.

---

## 11. Integrations (v1)

- **Jellyfin**: inbound webhook (playback start/stop/progress -> progress events) + optional
  library poll. Matching via provider IDs from Jellyfin metadata.
- **Radarr/Sonarr - read-only in v1**: read "in library / monitored / upcoming" to show
  availability and fill the calendar. Pushing "add/monitor" is a later, opt-in, off-by-default
  feature. Radarr/Sonarr are legal tools, and this project only exchanges metadata with them.
- **Hard boundary (all versions):** the project never integrates with indexers, trackers,
  Prowlarr/Jackett, torrent or usenet download clients, never searches for releases, and never
  handles media files. Documented in the README. (Not legal advice; the boundary keeps the
  project a metadata tracker.)

---

## 12. Offline and sync (design in v1, client later)

- UUIDv7 IDs generated client-side; all mutations are idempotent commands with a client op id.
- Progress/play events are **append-only** (no conflicts). Mutable fields (ratings, notes,
  statuses) use last-writer-wins with a hybrid logical clock per field.
- Server keeps a per-user change sequence; clients pull `changes since cursor`, push command batches.
- v1: implement the sync endpoints and use them from the web app (online). Later: Tauri client
  with a local SQLite adapter for the same `app` ports.
- Tauri vs PWA: Tauri preferred (native SQLite, filesystem, notifications). Tauri mobile is
  usable but less mature than desktop; re-evaluate when that milestone starts.

---

## 13. Search (Meilisearch)

- Postgres is the source of truth. Writes add rows to an `outbox` table in the same transaction;
  the indexer worker batches them into Meilisearch. Full reindex = a job (`cli reindex`).
- Indexes: `library` (per-user docs, filterable by user/kind/status/genre/year/rating/players...,
  includes notes/reviews), `catalog` (every connector result seen - instant repeat searches),
  `people` (people + companions, filtered by owner).
- Multi-tenant safety: every query is filtered by user/visibility server-side; never expose
  the Meilisearch master key or a broad key to clients.
- Later: hybrid/vector search for "more like this" and recommendations.

---

## 14. Testing approach

- TDD at the `app` level with `testkit` fakes for every port.
- `store-pg` tests against a real Postgres (testcontainers or CI service).
- Connector tests replay recorded HTTP fixtures; one opt-in live smoke test per connector.
- Storage budget: property tests for eviction (never evicts pinned; always converges below
  low watermark).
- Web: component tests for `ui`; Playwright end-to-end smoke tests for main flows.
- Architecture test: dependency rule from D-3.

---

## 15. Review gate for a Fable agent

Before implementation passes the milestone "core + one connector + one kind end-to-end", run a
**review subagent on Claude Fable** (`claude-fable-5-1`) with only this document and the code. It
must answer, with evidence from the code:

1. Do `domain` and `app` stay free of framework/IO crates? Is the dependency rule test real?
2. Is the `KindDescriptor` + typed `KindModule` + `attributes jsonb` split sound? Can a new kind
   be added without touching `domain`, `store-pg` migrations or `web` routes? What breaks?
3. Can the connector capability traits be expressed one-to-one as a WIT world (no Rust-only types,
   no callbacks that WIT can't express)? Draft the WIT and list mismatches.
4. Is every outgoing HTTP call routed through governor + HTTP cache + host allowlist (so WASM
   connectors can't bypass them later)?
5. Does any request handler await a connector call or sleep? (grep + reasoning)
6. Storage budget: can any write path bypass `StorageBudget`?
7. Are ports too fine-grained or too coarse (ports with one impl and no test value)?
8. Companion/user model: do promotion, demotion and merge work without rewriting history?

Additionally, as a software architect, give a judgement on the design as a whole:

9. Is hexagonal architecture justified here, crate by crate? Where is it ceremony (KISS/YAGNI
   violations: ports with one adapter and no test value, needless mapping layers)? Where is it
   missing?
10. DRY: duplicated types across `domain`, `contracts` and DB rows - which mappings are worth it,
    which should be collapsed?
11. Crate count and boundaries: merge or split proposals with reasons (compile time, ownership,
    reuse by Tauri).
12. The single biggest risk in the current design and a cheaper alternative.

Findings go to `docs/reviews/<date>-fable-architecture.md` and block the next milestone until
addressed or explicitly waived in `DECISIONS.md`.

---

## 16. Importers (v1)

Trakt, Goodreads, BG Stats (JSON), BGG (plays + collection), generic CSV/JSON (documented
schema of our own), and a **legacy export (JSON)** of the owner's current library.

All importers: streaming parse on the CPU pool, run in the `user_import` lane, idempotent
(re-running an import creates no duplicates; keyed by source + source id), dry-run preview,
per-row error report, resumable after restart, progress over SSE. Catalog matching goes through
external IDs first, then the connectors (queued, never inline).

**Legacy importer:** implement it only from the export file supplied by the owner (stored as
`testkit/fixtures/import/legacy/export.json`) and from observation of that file's structure.
No other description of the format is used.

---

## 17. Notifications (fully configurable)

A small rules engine, not a hard-coded list.

- **Event catalogue** (each event has a stable id, payload schema, default template, and an
  audience: user | admin):
  - Releases: item released, new episode / season, new chapter/volume, release date changed.
  - Library: item in a watched list changed status, metadata changed significantly.
  - Social: companion link request, link accepted, play/session shared with you, list shared.
  - Board games: loan due / overdue, wishlist game now marked owned by a companion's linked user.
  - Integrations: Jellyfin play recorded, integration failing.
  - Imports: import finished / failed (with report link).
  - Admin: storage high watermark / hard cap, connector circuit open, new sign-up, job backlog.
- **Subscriptions** (per user, admin sets instance defaults and can lock some):
  event type + filter (kind, list, item, companion) + channels + delivery mode
  (`immediate` | `digest` with cron schedule) + quiet hours + timezone + per-event rate limit.
- **Channels** (adapters behind the `Notifier` port): in-app inbox (always on), email (SMTP),
  ntfy, Gotify, Discord, Matrix, Apprise, generic webhook (signed), web push / Tauri native.
- **Templates**: per event and channel, i18n keys, overridable by brand and by admin.
- Delivery runs as jobs with retry and dedupe (no duplicate "episode released" for the same
  episode), and a delivery log visible to the user.

---

## 18. Research tasks for a Fable agent (before the board-game milestone)

Run a **research subagent on Claude Fable** (`claude-fable-5-1`). Output:
`docs/research/board-games.md`, ending in a concrete proposal (data model changes against D-9a,
UI flows, stats list, what goes in v1 vs later). Topics:
- What serious board-game loggers record and why: per-player score categories (e.g. points by
  source), tie-breakers, teams vs individual, co-op win/loss, solo modes with automa scores.
- Campaign / legacy games: campaign entity, scenario progression, persistent per-player state.
- Variants and house rules, game setup (player count, expansions, modules), play time per player.
- Locations and game groups (recurring game nights), "game night" as a container of several plays.
- Collection management: insert/sleeves, storage location, value tracking, loans.
- Stats people actually use (H-index, win rates, streaks, "dime" and "nickel" games, cost per play).
- Import formats available from BG Stats and BGG, and what they lose.
Rules: describe ideas in our own words; do not copy UI, text or schemas from any app.

---

## 19. Open items

- Default brand assets (name "kithkeep" is a codename only; white-label config supplies the real one).

