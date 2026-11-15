STATUS: COMPLETE (2026-11-09)

# Decision log: current state, supersessions, contradictions, open deferrals

Scope: `docs/DECISIONS.md` (DEC-001..494) read in full against `docs/HANDOFF.md` sections 0, 2,
3, 4, 6, 7 at tag `ux4` (main) on 2026-11-09. Owner rulings in HANDOFF section 4 win over any
DEC. Where a DEC's currency was in doubt the code was checked (grep only, nothing compiled);
those checks are cited as `path:line` (Rust paths relative to `src/rust/`). This file changes
nothing; it reads the log and proposes.

Reading guide: a DEC named without qualification is current. "Superseded by" means the later
DEC replaces the choice; "amended by" means the choice stands with a correction; "done" means
a deferral was carried out and the DEC is now history. Section 4 is the backlog.

Owner-review status (HANDOFF 4.5, DEC-174): owner-owned are DEC-001..007, 009..018, 089, 090,
plus the rulings DEC-169..174, 190, 191; DEC-019..088 and 091..168 are in force but not owner-
approved; DEC-175..494 were made under the standing instruction of HANDOFF section 0 and are
in force the same way. Two orchestrator decisions are flagged for the owner in HANDOFF 6:
DEC-466 (TMDB strict purge) and the funnel review date (DEC-431).

## 1. Current state, by area

Each statement is true today and names the DECs that make it so.

### 1.1 Repo, toolchain, engineering standard

- Owner VM with Nix, `nix develop`; no Docker on the build host, Postgres / RustFS /
  Meilisearch run as local processes through `cargo xtask services up` (DEC-001 as amended by
  DEC-064, DEC-032, DEC-202). Versions: Rust 1.99.1, Leptos 0.8.23, axum 0.8.9, sqlx 0.9.0,
  Postgres 18, Meilisearch 1.56, edition 2024 (DEC-002, DEC-012; `rust-toolchain.toml`,
  `Cargo.lock:2952`). GitLab CI on `rust:1.99.1-slim-trixie` with `ci-tools.sh`, kaniko image
  build with `--no-push` (DEC-003, DEC-042; `.gitlab-ci.yml:6,166`).
- Crates exactly as D-3 plus `api` (DEC-004); `web` may depend on `brand` (DEC-006), never on
  `domain` (DEC-433 repeats the HLC in the browser for that reason); `domain` has no tokio or
  async-trait; dev-deps exempt (DEC-010). Architecture test also forbids direct HTTP clients in
  connectors, importers, integrations, notify (DEC-040, DEC-145).
- Unprefixed crate names, env prefix `APP__`, no codename in code (DEC-007); white-label scan
  skips README, LICENSE, flake.lock (DEC-163).
- Licences: MPL-2.0 file-level allowed (DEC-005), BSL-1.0 (DEC-044), 0BSD (DEC-144),
  CDLA-Permissive-2.0 for webpki roots (DEC-036, DEC-083), NCSA for libfuzzer-sys (DEC-065);
  advisories ignored: `paste` (DEC-044), RUSTSEC-2023-0071 rsa (DEC-082; the only private RSA
  key is the test issuer's, DEC-308). Postgres TLS off (DEC-030; `Cargo.toml:56`).
- Dev profile `line-tables-only`, deps no debug, `strip = "debuginfo"` (DEC-055, DEC-091);
  8 MiB stacks for SSR in debug (DEC-188); `recursion_limit = 256` in `web` (DEC-127);
  `store-pg/build.rs` reruns on migrations (DEC-133).
- Clippy pedantic as errors, functions at most 100 lines with reasoned per-function allows
  (DEC-234, DEC-382, DEC-394, DEC-444, DEC-458, DEC-472 lists what still carries an allow).
- Commits: Conventional, ASCII, no AI attribution, commit-lint also rejects non-ASCII
  (DEC-013, DEC-014). Decisions file stays Markdown (DEC-009).
- Reviews: independent review per phase (HANDOFF 0); earlier self-reviews recorded in DEC-051;
  G1 and G2 findings in DEC-098..105 and DEC-152..164.

### 1.2 Domain model

- Time: chrono `DateTime<Utc>` (DEC-015). Rating 0..=100 (DEC-018). HLC stored as sortable text
  (DEC-028); server clock merges client stamps (DEC-124); client stamps more than 5 min ahead
  are replaced (DEC-126); default fields start at stamp `0-0-0` (DEC-128).
- Kind attributes validated by typed structs with committed JSON Schemas; no runtime validator
  for WASM kinds yet (DEC-016, DEC-099). Kind groups watch/read/listen/play/other on the
  module (DEC-204). Connectors fill the typed structs (DEC-037).
- Companions: merge keeps history and resolves chains (DEC-017); delete of a referenced
  person is a removal with `removed_at` (DEC-348 supersedes that part of DEC-017); unmerge
  from a merge record (DEC-347); merged entries fold into stats, filters and group members
  (DEC-346, DEC-376, DEC-442); links end on both sides (DEC-377); a foreign person is 404
  (DEC-470). Game groups soft-removed, plays belong to a group only when logged as it
  (DEC-442).
- Progress: `next_step` per progress model (DEC-207); undo is a compensating event within 24 h
  (DEC-208); entry note = `library_entries.notes` (DEC-209); plays with players, scores,
  winner, co-op; the op id is the idempotency key of the whole command (DEC-232, DEC-236);
  server bounds on times and sizes (DEC-238, DEC-161). Sessions can be logged for any kind
  (DEC-023). Claim = copy to my log (DEC-129).
- Tags on entries, private, case-insensitive identity (DEC-206). Remove from library keeps
  events, sessions, reviews, list items (DEC-417). Yearly goals, private, Stats page only
  (DEC-439).
- Catalog: cover URL and provenance columns (DEC-039); refresh outcomes and retry after
  failure (DEC-041, DEC-114); `ensure_catalog_item` for search results, `find_or_create` for
  user data, never attaching user ids to shared items (DEC-079, DEC-153); all creators go
  through `insert_new`, which claims external ids in one transaction (DEC-452, retry only on
  `CLAIM_RACE`, DEC-471). Episodes per season: number, air date, runtime only (DEC-446..448).
  TMDB retention: `provider_data_at`, hourly sweep, refresh when possible, else strict purge
  including title and year, placeholder, refetch later, unreferenced items deleted (DEC-453 as
  superseded by DEC-466; owner may reverse, HANDOFF 6).
- Catalog change audience: owners, history, lists, sessions, linked participants while
  sharing, and recent item-page watchers; a detail request that queues a refresh watches
  (DEC-490, DEC-492).

### 1.3 Storage, jobs, assets

- One migration set in `store-pg` (DEC-025), 0001..0031; compile-checked queries with a
  committed `.sqlx` cache (DEC-027); fresh database per test, `REQUIRE_SERVICES=1` makes
  skips fail (DEC-026).
- Jobs: own Postgres queue, lanes, `SKIP LOCKED`, leases renewed every third (DEC-029,
  DEC-154), polling 250 ms, no LISTEN/NOTIFY wake-up (DEC-029; none in `crates/jobs/src`).
  Roles `all|web|worker`, bus over `pg_notify` across processes (DEC-076, DEC-077). Server
  retries the database connection 60 s (DEC-167).
- Assets: WebP/AVIF/thumbhash pipeline (DEC-057); weighted LRU eviction (DEC-058); admission
  serialised per process (DEC-059); `stored_objects` index, no HEAD before PUT (DEC-054);
  pinning idempotent, no unpin sweep yet (DEC-066; no caller of `unpin` outside
  `crates/assets/src/service.rs:461`); blocked image hosts fail permanently (DEC-112);
  `forget` keeps files another record shares (DEC-471). HTTP cache bodies are blobs under the
  budget, index in Postgres (DEC-035; `crates/catalog-core/src/pg.rs`).
- Storage budget events and `metrics` tracing target instead of an exporter (DEC-060).
- Statistics cache per item and month with dirty marks carrying a generation, lazy recompute,
  zone stamp (DEC-400, DEC-460); windowed activity reads (DEC-401, DEC-469); person-driven
  History reads (DEC-494); keyset paging for the library (DEC-416).

### 1.4 Search

- Meilisearch with scoped keys, tenant filter added by adapters (DEC-084, DEC-085); Postgres
  fallback with tsvector + pg_trgm, words before typos with `word_similarity` (DEC-086 as
  amended by DEC-493); fall back at start when Meilisearch is down (DEC-088); indexer over the
  outbox (DEC-087). A catalog title change still does not re-index existing library documents
  (DEC-087; `crates/search/src/indexer.rs:158-173` upserts the catalog doc only), except the
  purge path, which queues them (DEC-466). Library text filter answers `search_unavailable`
  (DEC-240). Search streams connector results in-process (DEC-075).

### 1.5 Catalog connectors and sources

- Governed client: token bucket, priority gate with per-user fair share in hosted mode,
  circuit breaker, Retry-After, singleflight and cache for GET (DEC-034, DEC-402, DEC-461,
  DEC-462); runtime host allowlists (DEC-315); no request bypasses it (DEC-040); secrets
  never in URLs or errors (DEC-113, DEC-272, DEC-280).
- Every compiled-in, enabled connector is registered; credential state per connector, keys
  editable in the admin UI, env wins and shows as shadowed (DEC-246, DEC-264); member
  credentials (TheTVDB PIN) used for that member's searches only, never blamed on the instance
  key (DEC-269, DEC-273); token cache keyed by a derived key of the PIN (DEC-403, DEC-475).
  Source Disabled switch persisted in `source_switches` (DEC-304, DEC-333). Counters shared
  across processes through `source_stats*` (DEC-406 supersedes DEC-247's in-memory note).
- Built: TMDB (7-day TTL, show details without `external_ids`, DEC-038;
  `crates/connectors/src/tmdb.rs:304`), IGDB, Podcast Index, TheTVDB (DEC-116); TMDB and
  TheTVDB episode lists (DEC-446, DEC-447, page 0 only). Not built: Open Library, MusicBrainz,
  VNDB, Metron (unblocked by DEC-169), BGG (terms DEC-190, role DEC-193, waits for the token
  and logo), board games on Wikidata with Commons images and "Suggest to Wikidata" (DEC-192,
  DEC-194..196), anime/manga (MAL, Kitsu, MangaUpdates, aniSearch later; DEC-213..221),
  Hardcover (DEC-116). Rejected: AniList (DEC-108), Jikan (DEC-218), MangaDex for v1
  (DEC-217), Ludopedia (DEC-198), the sources of DEC-197.
- Icons: Lucide 1.69.0 + Tabler meeple, allowlisted and generated (DEC-043, DEC-052,
  DEC-183, DEC-419); service logos not vendored, names as text (DEC-043;
  `crates/ui/icons/vendor/` holds lucide and tabler only). TMDB logo may be committed
  (DEC-170) but the attribution page that would show it is not built (HANDOFF 5 item 5).

### 1.6 Auth, OIDC, SSO

- Credentials and sessions in `auth` tables, token hashes only (DEC-067); argon2id, 10..=1024
  chars, dummy hash for unknown users (DEC-068); sliding 30-day session, cookie
  `<ident>_session`, bearer for API (DEC-069); first account is admin, then
  open/invite/closed (DEC-070); TOTP with recovery codes (DEC-071, DEC-251); sign-in throttle
  10 per name per 15 min, in process memory (DEC-155; `crates/auth/src/lib.rs:200`), shared
  with re-auth and password change (DEC-275); username availability check per instance, 30
  per minute (DEC-184); one error code per rule in `contracts::auth` (DEC-185); CSRF guard on
  Origin / Sec-Fetch-Site (DEC-152).
- Re-auth window 15 min sliding, 1 h cap (DEC-191, DEC-249); gated server-side: sign-up mode,
  session length, cookie security, roles, disable/delete, TOTP start, OIDC providers and
  secrets, SSO mode, mail settings and the SMTP password (DEC-249, DEC-314, DEC-321, DEC-397),
  self-unlink and admin reset/unlink (DEC-312, DEC-409, DEC-467). Not gated: hosting mode,
  storage, data-source keys, invites, Test, test mail (DEC-249, DEC-397;
  `crates/api/src/admin/mod.rs:84-87`). Lock-out guards (DEC-250, DEC-259, DEC-274,
  DEC-467). Proof for password-less admins is the OIDC `max_age=0` round trip, pending action
  kept encrypted per session (DEC-323, DEC-331).
- OIDC: several providers in `oidc_providers`, env provider as a read-only row (DEC-309
  supersedes DEC-072); signed pending cookie with modes (DEC-310); own discovery, PKCE S256,
  RFC 9207 mix-up check, 24 h cache with a 60 s refetch cooldown (DEC-311, DEC-372); account
  resolution without auto-link, "Is this your account?" with password or an emailed one-time
  link (DEC-312, DEC-413, DEC-468); groups and provider roles, never the last admin
  (DEC-313); admin API and Test with silent and max_age probes (DEC-314, DEC-324); local-first
  logout, RP-initiated logout, back-channel logout with a shared `jti` table (DEC-318,
  DEC-319, DEC-407), no front-channel (OD-18); silent sign-in, loop guard, outage cap per
  process (DEC-320); `auth.sign_in_mode` page|auto with preconditions and the 302 of
  signed-out visits (DEC-302 as replaced by DEC-321); break-glass with password only, TOTP
  if enrolled (DEC-322, OD-17 as changed); no email sync (DEC-414, OD-21).
- CLI escape hatches, password reset links, SIGHUP key reload (DEC-301 as corrected by
  DEC-367, DEC-368, DEC-373; DEC-303). Client address and request id behind
  `server.trusted_proxies` (DEC-299 as amended by DEC-371).

### 1.7 Secrets at rest

- `auth::secrets` module, no separate crate (DEC-243); AES-256-GCM envelope, key file then
  env then generate (DEC-244); ephemeral key only with `secrets.ephemeral` (DEC-270); re-wrap
  by keyset paging (DEC-276); one registry of every `SecretRows` purpose: TOTP seeds, channel
  secrets, connector credentials, SMTP password, OIDC client secrets, session secrets
  (DEC-367; DEC-245's gaps closed by DEC-300, DEC-309, DEC-317). Config `Secret` newtype,
  empty env values mean unset (DEC-061, DEC-277). Rotation reminder after 2 years (OD-09).
  HKDF-derived keys for the pending cookie and the bulk-undo token (DEC-310, DEC-465).

### 1.8 Admin area and instance settings

- Single-row `instance_settings`, env/file values fixed and shown read-only (DEC-248); audit
  log with 365-day retention and CSV guarded against formulas (DEC-253, DEC-279, DEC-370);
  audit rows in the change's transaction for most store writes, after the change for roles,
  deletion and operations (DEC-298). Eight pages of addendum-1 3.4: Overview, Instance,
  Sign-in and SSO, Users, Data sources, Notification defaults, Storage and jobs, Audit log;
  `/admin/brand` and `/admin/security` are 404 (DEC-328 reverses DEC-255). Members see
  "Admins only", signed-out visitors the same without the side nav (DEC-257 as amended by
  DEC-295). Health rows from `health_probes` written every minute, localised times
  (DEC-405, DEC-411 supersede DEC-285). SMTP in the database, write-only password, test mail,
  categories only in the stored result (DEC-300, DEC-378, DEC-475). Users row menu with
  reset authenticator and unlink (DEC-409); providers table with linked accounts (DEC-410);
  stacked tables except the rules matrix (DEC-412 supersedes DEC-265 for three tables).
  Setup wizard with resumable steps, 303 to `/setup` before the first account, done flags
  (DEC-252, DEC-266, DEC-305); Home admin nudge with a browser-side snooze (DEC-268,
  DEC-294). Usage funnel setting on Storage and jobs (DEC-436).

### 1.9 Notifications

- `Notifications` port in `app`, engine in `notify` (DEC-134); inbox always on, dedupe per
  event key (DEC-135); instance defaults with locking (DEC-136); per-channel rate limit
  (DEC-137); recipient's language (DEC-201 supersedes DEC-138); SSRF guard with
  `allow_private_targets`, redirects not followed, resolver check at connect (DEC-139,
  DEC-158); channel secrets encrypted (DEC-244 reverses DEC-140); Standard Webhooks signing
  (DEC-142); `PlainHttp` for user-configured targets (DEC-145). Emitted today: social, import,
  admin sign-up, storage, source requested, local sign-in, authenticator reset, sign-in
  unlinked, sign-in link mail (DEC-143, DEC-254, DEC-322, DEC-409, DEC-413); the release and
  library-change events are catalogued but not emitted (DEC-143; `crates/notify/src/
  catalogue.rs:39-43`, no emitter in app/api/server). Web push not built, channel fails with
  "not available" (DEC-141; `crates/notify/src/channels/mod.rs:40`). Settings > Notifications
  with Inbox, Channels (edit in place, secrets write-only), Rules (incl. one item), Log
  (DEC-342, DEC-343, DEC-428, DEC-429).

### 1.10 Integrations

- Generic "watched" webhook with `itk_` bearer tokens only valid on `/hooks/*` (DEC-149,
  DEC-151); idempotency by sender id or a 15-minute window (DEC-159); the last call per
  sender remembered with a redacted body and the offending field (DEC-338, DEC-339 as
  extended by DEC-369); `integration.call` event and a test call that records nothing
  (DEC-340); connect panel with the 60 s "check the token" state (DEC-341). Radarr/Sonarr
  read-only and Jellyfin polling are unblocked (DEC-169) but not built.

### 1.11 People, lists, sharing

- Privacy: one `share_sessions` switch (DEC-130, DEC-160; `crates/domain/src/user.rs:27`),
  on Settings > Privacy (DEC-256). Invites single-use 14 days, hashed (DEC-022); list share
  tokens plain, kept on re-save, revoked by private/instance (DEC-131 as refined by DEC-349);
  "Shared with me" from links in either direction (DEC-349). People page with stats, merge
  suggestions by `name_key`, dialogs outside the refetched data (DEC-346, DEC-347, DEC-351);
  lists with keyboard reorder and drag handles (DEC-351, DEC-386, DEC-419); the revoked
  public page answers 404 (DEC-389 replaces the 200 of DEC-351); game groups tab and "Log a
  play with" (DEC-443). People page reads sessions once (DEC-470).

### 1.12 Imports, exports, sync

- Generic JSON Lines / CSV, version 1 with optional `lists` and `event_id` (DEC-118,
  DEC-356); idempotent rows and cursor resume, whole file parsed per run (DEC-119); catalog
  matching by external id, queued refresh, unmatched list with manual match and replay
  (DEC-120, DEC-357); reports owner-only (DEC-121, DEC-361); preview per run, cancel per
  checkpoint, files deleted after the real run (DEC-358, DEC-359, DEC-375); CSV export with
  formula guard (DEC-370). Legacy importer from the owner's file only (DEC-354, DEC-355,
  DEC-374); Trakt file importer on by default with the credit line on its card (DEC-297,
  DEC-360). Goodreads, BG Stats, BGG wait for samples (DEC-362, DEC-365).
- Sync: `GET /sync/changes` and `POST /sync/commands`, library entries with per-field HLC,
  other entities as refetch hints (DEC-123, DEC-156); `LogProgress.hlc` for the implied status
  (DEC-455). Tags not in sync commands, the export or the search index (DEC-206; no `tags` in
  `crates/api/src/sync.rs` or `crates/importers/src/generic.rs`).

### 1.13 Offline queue and measurement

- Offline queue in `localStorage` per user for one-click logs, status, rating; one push in
  queue order with queue-time stamps; cap 500; refusals named; sign-out flushes or asks
  (DEC-432, DEC-433, DEC-455 supersedes DEC-434, DEC-478, DEC-480). Pages hold event
  refreshes while offline and keep the last answer (DEC-486).
- Funnel: first-party, aggregated per UTC day, no user or item column, ten disclosure
  subjects, 180 days, one `metrics` event per flow nightly, admin page and CSV, Privacy line
  (DEC-431, DEC-435..437, DEC-481). Owner review on 2026-12-06 (HANDOFF 6).

### 1.14 Performance

- Budget 300 ms p95; load harness `cargo xtask load` with clients and writers, not in `ci`
  (DEC-399); final numbers in DEC-476, DEC-493, DEC-494 (all asserts pass in release with 8
  clients and 2 writers). Known: first stats read after a zone change ~770 ms (target 500),
  library query first page ~210 ms (full scan for the count) (DEC-404, DEC-459, DEC-476).
  Server-side render hang fixed by listing routes once (DEC-384); SSR Transition without
  `set_pending` and an e2e panic guard that fails on a missing or empty log (DEC-456,
  DEC-463, DEC-487).

### 1.15 Web, UI, i18n, accessibility

- Leptos server functions through a `Backend` trait for reads; changes from the browser go
  to REST through `web::rest` (DEC-092 as amended by DEC-327); JSON input for nested
  arguments (DEC-146, DEC-227); hydration config in the shell (DEC-093); shell session from a
  middleware (DEC-186). Rows render afresh per answer, or keyed by entry id where state must
  survive a refetch (DEC-241, DEC-398, DEC-477, DEC-485, DEC-486, DEC-491).
- Tokens per design 2.7, brand overrides win by specificity, contrast check warns at start
  (DEC-048, DEC-177, DEC-178, DEC-179); Atkinson Hyperlegible Next vendored, served
  fingerprinted, no preload (DEC-175, DEC-176); theme key `app-theme` (DEC-095).
- Routes: Home, `/library`, `/history`, `/people`, `/lists`, `/stats`, `/you`, `/settings/
  {account,notifications,integrations,sources,imports,privacy}`, `/admin/*`; old routes are
  404 without redirects (DEC-171, DEC-256). Library: query object, sorts, "Load more"
  following the cursor, remembered group per browser, facets as datalists, Filters button
  holds tags and facets (DEC-205, DEC-222..225, DEC-416); list view bulk bar with undo
  (DEC-418, DEC-464, DEC-465). Home charts in inline SVG with text equivalents (DEC-229);
  Stats with goals and "so far vs last year" (DEC-439..441). Item page action bar, sheets,
  episode list with ask-once in `localStorage`, bulk dates incl. release dates (DEC-231,
  DEC-233, DEC-363, DEC-449). Keyboard map `G then ...`, roving grid, palette Ctrl+Enter,
  recent items, scoped targets, shortcuts switch per browser (DEC-422..428, DEC-482).
- i18n: Fluent, per-user locale with Accept-Language then brand default, own number and date
  formatter, checks in xtask (DEC-090, DEC-199..201, DEC-203). Per-user time zone, UTC
  fallback, local day and time on Home, History, inbox; History `year` filter still UTC
  (DEC-306, DEC-332, DEC-342; `crates/app/src/activity.rs:49`).
- Accessibility: WCAG 2.2 AA required (DEC-171); rating radiogroup, error summary focus,
  disclosure patterns (DEC-180..182); data tables keep a focusable scroll wrapper (DEC-337);
  sticky bars raise scroll padding (DEC-233, DEC-482); hover contrast (DEC-489).

### 1.16 Testing and CI

- TDD records per package (DEC-024, DEC-316, DEC-336, DEC-344, DEC-352, DEC-365, DEC-415,
  DEC-421, DEC-430, DEC-438, DEC-445, DEC-451). Playwright with a TMDB mock, OIDC mock, mail
  sink, setup project through the wizard, `sso` project (DEC-096, DEC-262, DEC-334,
  DEC-413); every server test request times out at 90 s (DEC-384); `cargo xtask e2e` and the
  GitLab job scan the server log (DEC-456, DEC-463, DEC-487). Deferred tests tracked in
  DEC-281 -> DEC-307/335 -> DEC-383/396 -> DEC-457/473/483.

### 1.17 Ops and Docker

- Image built by kaniko in CI, compose with an optional `s3` profile on RustFS, podman
  override, TLS-inspecting proxy certificates (DEC-042, DEC-166, DEC-168, DEC-202;
  `src/docker/compose.yml:76-95`). Compose "healthy within 60 s" never verified (DEC-165).
  Operational logging of secrets never (DEC-113, DEC-163). Build host notes in HANDOFF 8.

### 1.18 Licences and legal

- Rule 6 as ruled: public API docs and terms of GPL services may be read, code never
  (DEC-169 supersedes DEC-106, DEC-148, DEC-149). Provider-required logos may be committed
  (DEC-170 supersedes DEC-164's logo item). TMDB terms: 7-day TTL, 6-month limit enforced by
  the retention job, no AI use, commercial deployments need an agreement (DEC-038, DEC-053,
  DEC-453, DEC-466). BGG terms (DEC-190), Trakt policy (DEC-297), Wikidata CC0 and Commons
  licence tags (DEC-192, DEC-194), ODbL file never vendored (DEC-220). Attribution page is
  the open workstream that carries the Trakt line, the Trakt/TMDB/TheTVDB notices, the font,
  icons and crate licences (DEC-170, DEC-256, DEC-360; HANDOFF 5 item 5).

## 2. Supersession table

Every DEC that is superseded, reversed, amended, corrected or partly replaced. "Remains valid"
says what of the entry still holds.

| DEC | Status | By | Remains valid |
|---|---|---|---|
| 001 | amended | DEC-064 (no Docker on the build host) | Nix dev shell, owner VM |
| 008 | superseded | DEC-056 -> DEC-202 (RustFS), owner DEC-174 | the reason MinIO is out |
| 016 | amended | DEC-099 (validator deferred to WASM kinds) | typed validation, committed schemas |
| 017 | partly replaced | DEC-348 (delete keeps history by removal), DEC-376 (filters follow merges) | merge semantics, chain resolution |
| 019 | superseded | DEC-033, DEC-034 (connector ports designed in M03) | nothing |
| 021 | superseded | DEC-051 (self-reviews), then HANDOFF 0 (independent review per phase) | nothing |
| 031 | done | DEC-160 (shared sessions reach linked users while sharing) | owner feed rule |
| 035 | done | Postgres index exists (`crates/catalog-core/src/pg.rs`) | blobs under the budget |
| 038 | amended | DEC-453, DEC-466 (the 6-month limit is now enforced) | TTL 7 days, no `external_ids` on shows |
| 051 | superseded | HANDOFF 0 (independent reviews since ux0) | the record of which milestones were self-reviewed |
| 056 | superseded | DEC-202 | nothing |
| 072 | superseded | DEC-309..326 (multi-provider, modes, logout) | openidconnect crate, PKCE S256 |
| 075 | superseded | DEC-084..088 (search crate wired) | in-process fan-out |
| 087 | amended | DEC-466 (purge queues library docs) | indexer design; the title-change gap stays |
| 092 | amended | DEC-327 (browser changes go to REST) | reads through server functions, dependency rule |
| 104 | open | no later DEC; request bodies still private in `api` | - |
| 106, 148, 149 | superseded | DEC-169 (owner R-P1) | nothing (the generic webhook of 149 stays as built) |
| 107 | superseded | DEC-190, DEC-192, DEC-193 | nothing |
| 110 | superseded | DEC-246 (always registered, credential state) | nothing |
| 115 | open | no later DEC (host-side credentials for WASM) | connectors hold their keys |
| 117 | done | DEC-354 (legacy importer built) | Q4 rule: from the file only |
| 122 | partly done | DEC-297, DEC-360 (Trakt); Goodreads, BG Stats, BGG still wait | the waiting list |
| 138 | superseded | DEC-201 (recipient's language) | brand overrides then admin templates |
| 140 | reversed | DEC-244 (channel secrets encrypted) | "never returned", shown once |
| 143 | partly done | DEC-254, DEC-322, DEC-409, DEC-413 added events | release and library-change events still not emitted |
| 150 | superseded | DEC-416 (keyset), DEC-235, DEC-399 (harness) | 50 000 items in the load set |
| 164 | partly superseded | DEC-170 (TMDB logo may be committed) | the other documentation fixes |
| 172 | amended | DEC-191 (OD-12 15 min; the 10-minute interim is gone) | OD-10, OD-11, OD-17 as changed |
| 184 | amended | DEC-299 (trusted proxies exist now) | per-instance limit; per-client limit is possible but not built |
| 187 | superseded | DEC-256 (settings area, `/admin`), DEC-422 (chords), DEC-432 (offline) | `/you` hub, menu order |
| 189 (6) | open | no endpoint returns the inviter yet | the rest of 189 |
| 200 | amended | DEC-306, DEC-332 (per-user zone, no "UTC" marker on local values) | formatter, plurals |
| 201 | amended | DEC-260, DEC-332 (Settings > Account exists) | fallback order |
| 205 | partly open | sorts Progress / Next release / Priority still not built | query object, sorts |
| 210, 211 | superseded | DEC-306 (zone), DEC-400 (cache), DEC-401 (windows), DEC-494 (person) | row shapes, measures |
| 226 | superseded | DEC-233 (log sheets exist) | one-click steps with Undo |
| 230, 239 | superseded | DEC-306, DEC-332 (local days and times) | History layout, filters in the URL |
| 232 | amended | DEC-236 (op id = whole command) | play DTO and sheet |
| 233 | partly open | DEC-417 (Remove built); Rewatch, Report wrong match, Related, Plays table, Publish as review not built | layout and sheets |
| 235 | superseded | DEC-400, DEC-401, DEC-416 | triggers for `last_event_at`/`plays`, `next_in_series` |
| 237 | done | DEC-268, DEC-363, DEC-364, DEC-422..425, DEC-432 | Filters panel as inline disclosure |
| 245 | done | DEC-298, DEC-300, DEC-309, DEC-310, DEC-317 | locked channel behaviour |
| 246 | amended | DEC-269 (member PIN used) | credential model |
| 247 | superseded | DEC-406 (counters persisted) | states vocabulary |
| 249 | amended | DEC-314, DEC-321, DEC-397 (more gates) | window rules, proofs |
| 250 | amended | DEC-321, DEC-322 (SSO-only exists, break-glass) | guard codes |
| 252 | amended | DEC-266 (redirect built) | wizard routes |
| 255 | reversed | DEC-328 | nothing |
| 257 | amended | DEC-295 (signed-out visitors) | members' page |
| 258 | amended | DEC-331 (provider round trip) | dialog mechanics |
| 265 | partly superseded | DEC-412 (three tables stacked) | rules matrix keeps the wrapper |
| 281 | done / cut | DEC-307, DEC-335, DEC-457 | the cut list in DEC-457 |
| 282 | done | DEC-328 (Instance page) | - |
| 283, 284 | done | DEC-301, DEC-303, DEC-299 | - |
| 285 | superseded | DEC-405 | mail row from stored fields |
| 286 | done | DEC-403, DEC-408, DEC-475 | - |
| 289 | done | DEC-304, DEC-333 | key guide lines |
| 292 | done | DEC-305 | `backup_key` never marked |
| 299 | amended | DEC-371 (chain walk, mapped IPv6) | trusted peers, request id |
| 300 | amended | DEC-397 (mail settings gated), DEC-378, DEC-475 | storage and routes |
| 301 | corrected | DEC-367 (prune registry), DEC-368 (fixed mode), DEC-373 (atomic resets) | the hatches |
| 302 | replaced | DEC-321 | the setting definition |
| 312 | amended | DEC-413, DEC-468 (emailed link) | steps 1-4 |
| 316, 326 | done | DEC-327..336, DEC-406, DEC-407, DEC-413 | signed `UserInfo` still unsupported |
| 319 | amended | DEC-372 (cooldown), DEC-407 (shared jti table) | validation rules |
| 336 | done | DEC-409, DEC-411, DEC-413 | - |
| 337 | corrected | DEC-385 (it was a bug, not a race) | focusable wrapper, `rel=external` |
| 339 | extended | DEC-369 | - |
| 344 | done | DEC-428, DEC-429 | - |
| 346 | amended | DEC-381 (twins), DEC-470 (one read) | definitions |
| 348, 349 | amended | DEC-377 (both sides) | - |
| 351 | amended | DEC-389 (404), DEC-419 (drag) | pages |
| 353 | partly done | DEC-376, DEC-419, DEC-428, DEC-443; avatar upload, fuzzy duplicates, restore removed person still open | - |
| 356 | amended | DEC-374 (legacy event ids) | version 1 kept |
| 363 | done | DEC-449 ("On release dates") | Spread over a range |
| 382, 394 | done | DEC-458, DEC-444; DEC-472 lists what still carries an allow | - |
| 383, 396 | done / cut | DEC-457 | the named cuts |
| 384 | amended | DEC-456 (UX3-25 reproduced and fixed) | route list once per process |
| 388 | done | DEC-410, DEC-412 | - |
| 390 | amended | DEC-397 | web gating |
| 400 | amended | DEC-460 (generation per mark) | cache layout |
| 401 | amended | DEC-469 (pair spans), DEC-494 (person path) | windows for the generic walk |
| 402 | amended | DEC-461, DEC-462 | per-user fair share |
| 403 | amended | DEC-475 (derive_key) | hashed key |
| 404, 459 | superseded as records | DEC-476, DEC-493, DEC-494 | the targets and the follow-ups |
| 413 | amended | DEC-468 | - |
| 417 | partly replaced | DEC-464 (per-op inverse), DEC-465 (signed token) | remove semantics, bulk ops |
| 433 | corrected in place | UX4-21 (DEC-474) | - |
| 434 | superseded | DEC-455 | the problem statement |
| 445 | done | DEC-452 | - |
| 450 | partly done | DEC-453, DEC-466 (purge built) | runtimes, badge, paging, airing still open |
| 453 | superseded | DEC-466 (title and year purged too) | stamp, sweep, refresh path |
| 456 | amended | DEC-463, DEC-487 | the SSR fix |
| 482 | corrected | DEC-485 | - |
| 483 | corrected | DEC-486 | - |
| 486, 488 | amended | DEC-490 (audience), DEC-491 (History pages) | `Shown`, hold while offline |
| 490 | amended | DEC-492 | - |
| 086 | amended | DEC-493 | tsvector weights, pg_trgm |

Count: 93 entries listed (superseded or reversed 29, amended/corrected/extended 43, done
deferrals recorded as history 21).

## 3. Contradictions that still stand

| # | Between | Conflict | Recommended resolution |
|---|---|---|---|
| C1 | DEC-453 vs DEC-466 (vs owner) | 453 keeps title and year as the person's record; 466 purges them and deletes unreferenced items. 466 is in force; HANDOFF 6 asks the owner. | Owner decides; whichever stands, mark the other superseded in the log. If 466 stands, consider copying title and year into the library entry at purge time (466's alternative) so libraries never show a placeholder. |
| C2 | DEC-328/331 vs DEC-249 and code | 328 says the Instance hosting radios are "gated"; 331's `gated_settings` lists hosting. The server has `instance.hosting` `gated: false` (`crates/api/src/admin/mod.rs:84-87`, DEC-249). DEC-191 (owner) does not list hosting. | Correct DEC-328/331 wording: the web routes the call through the re-auth helper but the server needs no proof. Or gate it (hosting changes fair share and the hosted banner) - a server decision, not an owner one. |
| C3 | DEC-201 vs owner ruling 4.1 | The ruling: "per-user language with the brand default as fallback". DEC-201 puts Accept-Language before the brand default for users without a choice and for visitors. | Keep DEC-201 (better signal) but record it explicitly as a deviation the owner accepts, or ask in the next review page. |
| C4 | DEC-306/401 vs DEC-239/332 | Per-user zone applies to months, days, times and "today", but the History `year` filter and the activity `year` bound are UTC (`crates/app/src/activity.rs:49,190`). A person in UTC+13 sees New Year's Day under the wrong year. | Make the year filter use the user's zone (local Jan 1 bounds, as DEC-440 already does for the comparison) and close DEC-306's note. |
| C5 | DEC-191 vs DEC-249 | 191 lists "reopen wizard" and "e-mail, revoking other sessions" as gated. Reopen exists only as a CLI hatch (actor `cli`, no proof); there is no email update or session-revocation feature. | Record in DEC-249: the CLI is the proof by host access; email and sessions are open deferrals (D-AUTH below). |
| C6 | DEC-092/162 vs DEC-327 | 162 rejected removing the Backend layers because `web` must not know server crates; 327 lets the browser call REST directly for changes, so the UI now has two paths (server functions for reads, REST for writes) while 162's "four layers" argument still shapes reads. | Accept the split as the architecture and say so in one DEC: reads = server functions (SSR), writes = REST (cookies and codes validated once). Mark DEC-162 "revisit" closed. |
| C7 | DEC-189 (4) vs design.md 5.2 | Sign-up lands on `/library` (`crates/web/src/auth_pages.rs:481`), the design says Home. Owner approved design.md. | Either change the landing page (and the e2e suites) or record the deviation as accepted on the review page. |
| C8 | DEC-155/320/413/436 vs multi-process deployment | Sign-in throttle, OIDC outage cap, emailed-link throttle and the funnel beacon limit are per process; `--role web` can run several processes, and DEC-406/407 moved the counters that mattered into Postgres. | Decide once: either document "throttles are per process; run one web process or accept the multiplied budget", or move them to a shared table (DEC-468 already defers the link throttle). |
| C9 | DEC-184 vs DEC-299 | 184 says per-client limits are impossible because there is no trusted client address; 299 added `trusted_proxies` and the client address is in the audit. | Amend DEC-184: per-client throttling is now possible; decide whether to build it (D-AUTH). |
| C10 | DEC-423 vs owner i18n/a11y framing | The shortcuts switch, disclosure memory, remembered group, nudge snooze, episode ask-once live in `localStorage` per browser (DEC-182, DEC-223, DEC-231, DEC-268, DEC-423) while the design treats them as user preferences. Not a ruling conflict, but five DECs each say "a server preference later". | One decision on a `user_preferences` store (or an explicit "browser-only by design") instead of five open notes. |
| C11 | DEC-043 vs DEC-170 | 043 keeps service logos out until the pages need them; 170 allows the TMDB logo and asks for an attribution page; nothing shows the logo yet, and the TMDB attribution on item pages is text. | Close with the attribution workstream: vendor the TMDB logo per R-F3 and record it in ICONS.adoc. |
| C12 | DEC-066 vs DEC-417 | Pins are never released; "Remove from library" deletes the entry but no caller unpins the cover (no `unpin` call in app/api/server). | Build the sweep (or unpin in `remove_from_library`) and close DEC-066's note. |

Count: 12 standing contradictions (C1 needs the owner; C3 and C7 are owner-visible deviations;
the rest are engineering).

## 4. Open deferrals (the backlog)

Every item a DEC defers that is not done at `ux4`, grouped by who or what it waits for. A
DEC number in brackets is where the deferral was made; "HANDOFF" marks items the handoff lists
without a DEC of their own.

### 4.1 Waiting for the owner

- D-OWN-1 BGG token and the "Powered by BGG" logo files (DEC-190, DEC-173; HANDOFF 6).
- D-OWN-2 Sample exports: Goodreads, BG Stats, BGG collection, Trakt "Raw Export" ZIP
  (DEC-122, DEC-297, DEC-362, DEC-365).
- D-OWN-3 MAL developer agreement and `KK_MAL_CLIENT_ID` (DEC-213); Kitsu terms saved into
  owner-inbox (DEC-214); MangaDex metadata-only yes/no (DEC-217); AniDB rules read in a
  browser (DEC-221); open product databases as a barcode source (HANDOFF 4.6).
- D-OWN-4 TMDB strict purge: keep or reverse DEC-466 (HANDOFF 6; C1).
- D-OWN-5 Funnel review on 2026-12-06: cut unused flows, promote disclosures opened on more
  than half of their visits, decide saved views (DEC-431; HANDOFF 6; phase4-plan deferrals).
- D-OWN-6 Review of DEC-019..088 and 091..168 (DEC-174; they stay in force meanwhile).
- D-OWN-7 Live check that authentik honours `max_age=0` (`live_provider_honours_max_age_zero`,
  DEC-325); fallback od-12 4.4 applies if not.
- D-OWN-8 Compose "healthy within 60 s" on a Docker host (DEC-165); the Tauri spike on a host
  with disk (DEC-125).
- D-OWN-9 Accept or change the two deviations C3 (Accept-Language before the brand default)
  and C7 (sign-up lands on /library).

### 4.2 Attribution page (parallel workstream, HANDOFF 5 item 5)

- D-ATT-1 Crate licences from cargo metadata / cargo-deny, the font (OFL, DEC-175), icons
  from `config/brand/ICONS.adoc` (DEC-043, DEC-183, DEC-419), data sources with their notices
  (DEC-170).
- D-ATT-2 The Trakt line moves from the Imports card to the page (DEC-297, DEC-360); the TMDB
  notice incl. the committed logo (DEC-053, DEC-170; C11); TheTVDB, IGDB, Podcast Index
  notices (DEC-116).
- D-ATT-3 The `#attribution` section of Settings > Data sources moves to the public About page
  (DEC-256).

### 4.3 Connectors and catalog sources (parallel workstream)

- D-CON-1 Board games on Wikidata: search, entity fetch, property map, per-value source
  marker, Commons images with licence tags, matching keys, "Suggest to Wikidata"
  (DEC-192, DEC-194..196); brings the owned flag, then "Mark owned" in the bulk bar
  (DEC-420), Owned filter (DEC-225), "unplayed owned games" and the H-index on History
  (DEC-440). BGG as an optional enrichment store with purge, once the token exists (DEC-193).
- D-CON-2 Anime and manga: MAL primary behind `mal`, Kitsu fallback and id mapper behind
  `kitsu`, MangaUpdates for publishers, legacy importer id chain, anime-offline-database at
  import time, aniSearch later (DEC-213..216, DEC-219, DEC-220).
- D-CON-3 Open Library, MusicBrainz, VNDB, Metron (DEC-106 -> DEC-169); Radarr/Sonarr
  read-only, Jellyfin polling (DEC-148, DEC-149 -> DEC-169); Hardcover research (DEC-116).
- D-CON-4 TMDB show details with `external_ids` (DEC-038); TheTVDB episode paging past page 0
  (DEC-447); per-episode runtimes into stats minutes, "Up to date" badge from air dates,
  next-release sort, airing notifications (DEC-450, DEC-207, DEC-205, DEC-143).
- D-CON-5 Host-side credential injection so a WASM plugin never sees secrets; JSON-Schema
  validation of runtime kinds (DEC-100, DEC-115, DEC-099, DEC-016).
- D-CON-6 Old duplicate catalog items from pre-DEC-452 races are not migrated; operator SQL
  only (DEC-452).

### 4.4 Auth and SSO (auth workstream)

- D-AUTH-1 Web push: RFC 8291/8292 on p256 0.14, tested against the RFC example, then the
  service worker and subscribe button (DEC-141; owner R-P5 says build it).
- D-AUTH-2 Audit rows inside the `link_identity` transaction; a database-backed throttle for
  the emailed link (DEC-468); shared (not per-process) throttles for sign-in, outage cap and
  the funnel beacon, or an explicit "one web process" statement (DEC-155, DEC-320, DEC-436;
  C8); per-client limits now that trusted proxies exist (DEC-184, DEC-299; C9).
- D-AUTH-3 Account page: email update API, delete account, QR image for enrolment (DEC-260);
  "revoking other sessions" as a gated action (DEC-191; C5); WebAuthn (DEC-071 "add later").
- D-AUTH-4 Signed `UserInfo` responses (DEC-311, DEC-316); front-channel logout stays out
  (OD-18).
- D-AUTH-5 `/invite/:token` naming the inviter and prefilling the display name (DEC-189 (6);
  design 6.14, addendum D12).
- D-AUTH-6 Deferred tests: `link/email` 503 and 409, confirm on a disabled account,
  `purge_password_resets` pruning `oidc_link`, `oidc_used_jti` expiry (DEC-473); the cut list
  of DEC-457 that is auth (reload_keys cases, locked SMTP password, `PgMailSecret::
  rewrap_batch`, back-channel 429, loop-guard cookie on `/sign-in?error=`, server start with
  encrypted rows over HTTP, `key_unavailable`, the mail step through the re-auth dialog).

### 4.5 Catalog, search, data model (hardening)

- D-CAT-1 Re-index library documents when a catalog title changes (DEC-087).
- D-CAT-2 Unpin covers when the last library reference goes (sweep or on remove)
  (DEC-066, DEC-417; C12).
- D-CAT-3 Tags in sync commands, the export file and the search index (DEC-206); sync DTOs
  for entities beyond library entries (DEC-156); game groups are not in the sync feed
  (DEC-442).
- D-CAT-4 REST request bodies moved to `contracts` when a client needs them (DEC-104).
- D-CAT-5 A game group left with no members (DEC-484, not touched); a 500 of a webhook call
  attributed with its status (DEC-338/457).
- D-CAT-6 `revoke_signup_invite` writes from a read (a revoke racing a claim can clear the
  claim) (DEC-274); candidate key probes share the HTTP cache with the live key (DEC-273).
- D-CAT-7 Deferred tests: the 48-hour hour-row delete, TVDB episodes through
  `refresh_episodes`, "fewer stored than count", `purge_stale_before` through the handler, the
  `insert_new` claimed-and-released path (DEC-473).

### 4.6 Performance (performance workstream after `ux4`, DEC-476)

- D-PERF-1 First stats read after a zone change (~770 ms, target 500): answer from
  `month_amounts_full` and rebuild in the background (P-1; DEC-404, DEC-476).
- D-PERF-2 "Library query, first 100" (~210 ms under 8 clients): cached totals per filter or
  materialised sort keys (P-2; DEC-459, DEC-476).
- D-PERF-3 `/people` in the load set (P-3; DEC-470, DEC-476); `companion_stats` and
  "most often with" are linear in the caller's sessions (DEC-346, DEC-441).
- D-PERF-4 Optional index `sessions (started_at DESC, id DESC)` for the anchors and shared
  reads (DEC-494); cover lookups batched in `dto::item` (HANDOFF 0 step 3).
- D-PERF-5 LISTEN/NOTIFY wake-ups for the job queue (DEC-029); advisory lock on storage
  admission across processes (DEC-059); a metrics exporter instead of `tracing` events
  (DEC-060); release runs twice and averaged (DEC-459).
- D-PERF-6 Deferred tests: delete triggers' "user gone" branch, UTC `bucket_zone` reuse,
  `hits_after` empty-page recount, `fair_share` flipping live (DEC-473).

### 4.7 Web and UX

- D-WEB-1 "Date unknown" on events (R-08): an unknown-date flag, own package (DEC-355,
  DEC-233; HANDOFF 0 step 4).
- D-WEB-2 Saved views, decided after the funnel review (phase4-plan; HANDOFF 0 and 6).
- D-WEB-3 People: avatar upload, fuzzy duplicate suggestions, restoring a removed person
  (DEC-353); "Log with" for page/position steps and items not in the library (DEC-428,
  DEC-443).
- D-WEB-4 Item page: Rewatch, Report wrong match, Related, the Plays table, the companions
  row, "Publish as review" with the spoiler flag, History tab Edit (DEC-233); `1-5` on a
  focused rating (DEC-425); a single-session read for the play sheet prefill (DEC-240).
- D-WEB-5 Library: sorts Progress, Next release, Priority (DEC-205); Rating filter
  (DEC-225); a facet-values server function (DEC-224); roving tabindex in the list view
  (DEC-424).
- D-WEB-6 Admin: audit "older" paging with `before` (DEC-263); stacked rules matrix stays
  out by decision (DEC-412).
- D-WEB-7 Server-side user preferences for what now lives in `localStorage`: shortcuts
  switch, disclosure state, library group, nudge snooze, episode ask-once (DEC-423, DEC-182,
  DEC-223, DEC-268, DEC-231; C10).
- D-WEB-8 History `year` filter in the user's zone (DEC-306, DEC-401; C4); brand or instance
  default time zone (DEC-306, not built).
- D-WEB-9 Font preload link (DEC-176); a Tailwind class-merge step if conflicts appear
  (DEC-047).
- D-WEB-10 Two flakes seen once and not analysed: History "Show more" losing pages (DEC-488,
  partly addressed by DEC-491) and the 360 px Library Filters panel closing after the quick
  menu's Escape (DEC-488).
- D-WEB-11 Deferred tests: a whole-request 401 keeping the offline queue in e2e, two
  accounts on one browser end to end (DEC-483); timed position inline through the browser,
  "beyond the end" for pages, person and shared History filters through the UI (DEC-242);
  axe at 360 px with a provider panel open in both themes, the silent wizard skip on a reused
  server, the candidate source Test with a live key as `#[ignore]` (DEC-457).

### 4.8 Ops, CI, docs

- D-OPS-1 Postgres TLS (DEC-030). Database-level append-only grant for the audit table in
  the deployment docs (DEC-253).
- D-OPS-2 Independent review of M04, M05, M07, M08 (DEC-105; the phases since were reviewed,
  those milestones' code was not reviewed as such).
- D-OPS-3 Docs: the webhook JSON Schema published as a file; optional sections rendering `-`
  (DEC-164); the wrong comment in `0017_phase3_instance.sql` is left by design (DEC-379) and
  should be noted in the ops page.
- D-OPS-4 Long test files still under reasoned allows: `load.rs`, `phase1.rs`,
  `app/tests/activity.rs`, `Server::build` (DEC-472).

### 4.9 Platform (later milestones)

- D-PLAT-1 Tauri desktop shell, `csr` feature kept for it (DEC-125).
- D-PLAT-2 Tier-2 WASM plugins: schema validator, host credentials (DEC-016, DEC-099,
  DEC-115).

Count: 58 backlog items from 9 owner items, 3 attribution, 6 connectors, 6 auth, 7 catalog,
6 performance, 11 web, 4 ops, 2 platform (plus the test lists inside them).

## 5. Log hygiene

### 5.1 "How to reverse" missing or not a reversal

The column is a forward step (a follow-up), not a way back, in: DEC-066 ("add a periodic
sweep"), DEC-071 ("add later"), DEC-072 ("add an HS256 test issuer"), DEC-087 ("add a library
fan-out"), DEC-102 ("warm the cache"), DEC-115, DEC-117, DEC-119, DEC-120, DEC-122, DEC-123,
DEC-141, DEC-143, DEC-156, DEC-165, DEC-237, DEC-242, DEC-245, DEC-252, DEC-253, DEC-420,
DEC-450, DEC-457. These belong in a "follow-up" field (section 6).

The column is `n/a` or `-` where a reversal exists and could be named: DEC-026 (go back to
testcontainers), DEC-035 (a disk cache), DEC-054, DEC-061, DEC-067, DEC-069 (JWT), DEC-073..078,
DEC-084..086, DEC-092..094, DEC-121, DEC-128, DEC-250, DEC-264 ("none"), DEC-316, DEC-326,
DEC-336, DEC-344, DEC-353, DEC-361. `n/a` is right for environment facts (DEC-001, DEC-064),
bug fixes (DEC-079..081, DEC-097, DEC-109, DEC-114) and records.

The column is too vague to act on: DEC-470, DEC-471, DEC-475 ("revert"), DEC-484 ("revert per
item"), DEC-466 ("see the alternatives").

The column is stale because the reversal path no longer exists: DEC-150 (`LibraryRepo::page`
offset path replaced by DEC-416), DEC-235 ("revert migration 0015" would now also break the
0023 cache), DEC-255 (already reversed), DEC-285/286/247 (replaced), DEC-434 (replaced by
DEC-455), DEC-453 (DEC-466 changed `purge_provider_data`).

### 5.2 Entries that are records, not decisions

They are valuable but hide the decisions between them. Candidates for a separate append-only
log (test evidence, measurements, review dispositions):

- Test-driven evidence: DEC-024, DEC-316 (part), DEC-336 (part), DEC-344 (part), DEC-352,
  DEC-365, DEC-415, DEC-421, DEC-430, DEC-438, DEC-445, DEC-451.
- Measurements: DEC-150 (numbers), DEC-399, DEC-404, DEC-459, DEC-476, the number blocks of
  DEC-416, DEC-493, DEC-494.
- Re-checks and gate outcomes: DEC-050, DEC-051, DEC-052, DEC-053, DEC-105, DEC-116.
- Test inventories: DEC-242, DEC-281, DEC-307, DEC-335, DEC-383, DEC-396, DEC-457, DEC-473,
  DEC-483.
- Review dispositions that only say "applied" or "rejected" without a new rule: DEC-271,
  DEC-287, DEC-293, DEC-294, DEC-395 (rejected parts), DEC-454, DEC-474.
- Root-cause write-ups whose decision is small: DEC-288, DEC-296, DEC-337, DEC-366, DEC-384,
  DEC-385, DEC-398, DEC-456, DEC-477, DEC-485, DEC-486, DEC-488, DEC-489 (keep the one-line
  rule each contains; move the investigation to the log).
- Owner rulings (DEC-169..174, DEC-190, DEC-191, DEC-414) are the opposite: first-class, and
  should be marked as such so a reader sees which entries nobody but the owner may change.

Entries that are decisions but carry a wrong or outdated fact in their text: DEC-187 (menu
points to `/sources`, no `/admin`), DEC-200 ("no per-user time zone"), DEC-211, DEC-230
(UTC), DEC-245 (OIDC/SMTP from env only), DEC-247 (counters in memory), DEC-255 (nine pages),
DEC-285 (probes in the request), DEC-312 (emailed link is phase 4), DEC-319 (jti in memory),
DEC-351 (200 for a revoked link), DEC-363 ("On release dates" not built), DEC-433 (corrected in
place), DEC-453 (title and year kept). Each is listed in section 2; none has a "superseded by"
note in the file itself.

## 6. Proposal: how to restructure the decision log (not done here)

1. Split into three files under `docs/decisions/`:
   - `CURRENT.md`: the current-state summary by area (section 1 of this file, maintained),
     each statement citing the DEC ids. This is what a new agent reads first.
   - `DECISIONS.md`: the append-only history, one entry per line as now, never edited except
     to add a `status` field at the front: `active | superseded by DEC-n | amended by DEC-n |
     done | record | owner`. The KICKOFF format stays valid (the rules quote it); the status
     field is a prefix, so old tooling that splits on `|` still works if it reads from the
     right.
   - `BACKLOG.md`: the open deferrals (section 4), each with its DEC source and its owner or
     workstream; removed when done, with the closing DEC noted in DECISIONS.
2. Add two optional fields to the entry format: `follow-up` (what is deliberately not built
   yet) and `supersedes`. Today both hide inside "choice" or "how to reverse" (section 5.1).
3. Mark owner rulings with `owner` in the status field and keep HANDOFF 4 as their source of
   record; mark review dispositions and TDD/measurement records with `record` so the counts in
   reports can skip them.
4. Make it a repository check (xtask, like the locale and architecture tests): every DEC id
   referenced in code comments, HANDOFF and docs exists; every entry with `superseded by`
   names an existing later id; no `active` entry's text contains "phase N" or "deferred"
   without a BACKLOG line. That keeps the three files from drifting.
5. Rule for new work: a package adds its decisions to DECISIONS.md, moves the lines it
   closes in BACKLOG.md, and edits the affected sentences in CURRENT.md in the same commit;
   the review checks all three.
6. One-time migration: apply the status field from section 2 of this file, move the backlog
   from section 4, and write CURRENT.md from section 1. Measurement blocks of DEC-399, 404,
   416, 459, 476, 493, 494 can move to `docs/perf/load-runs.md` with one line each left in
   DECISIONS.

## 7. Counts

- Entries read: 494 (DEC-001..494).
- Superseded, reversed or replaced: 29; amended, corrected or extended: 43; deferrals done
  and now history: 21 (section 2, 93 rows).
- Standing contradictions: 12 (section 3), of which 1 needs the owner (C1) and 2 are
  owner-visible deviations (C3, C7).
- Open backlog items: 58 (section 4), 9 of them waiting on the owner.
- "How to reverse" that is a follow-up instead: 23; vague or stale: 11; records rather than
  decisions: about 60 entries (section 5).
