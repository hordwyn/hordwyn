STATUS: COMPLETE (2026-11-09)

# Architecture review: kithkeep at tag ux4 (branch main)

Reviewer: independent architect (read-only review, no builds run). Date: 2026-11-09.
Inputs: docs/HANDOFF.md sections 2 and 4, docs/design/DESIGN.md, xtask/src/arch.rs,
docs/DECISIONS.md (skimmed), every Cargo.toml, Cargo.lock, the source tree under src/rust.
Paths below drop the `src/rust/` prefix for Rust files. Line counts come from `wc -l` on
`src/**/*.rs` (and `tests/**/*.rs` where given) and are rounded; test counts are the number
of `#[test]` / `#[tokio::test]` / `#[sqlx::test]` / `#[wasm_bindgen_test]` attributes found by
grep (they include `#[ignore]` tests). Where I did not verify a claim by reading the code,
I say so ("unsure").

## 0. Summary

The workspace follows DESIGN.md D-3 closely: `domain` and `app` are clean of IO crates, the
adapters sit in their own crates, and the arch test (xtask/tests/architecture.rs:9-10 over
xtask/src/arch.rs) is real. The structure problems are not in the core but at the edges:

1. `web` is one 32 k-line crate (68 files, 344 components, 54 server functions, two embedded
   2100-line Fluent files, 146 unit tests in the same compilation unit) that is compiled three
   times per full build (ssr for the server, hydrate for wasm, ssr again for `cargo test`).
   That, not the workspace shape, is what needs 3-4 GB per rustc.
2. Every UI operation crosses four layers: server fn (web/api.rs) -> `Backend` trait
   (web/backend.rs) -> `WebBackend` (apps/server/src/web_backend.rs) -> `api::ops` -> `app`.
   The two trait definitions plus their two server-side impls are about 1,250 lines of
   forwarding code with no logic (DEC-162 accepted that cost; this review recommends removing
   it).
3. `api` (12.7 k lines) is larger than `app` (7.4 k) and has become a second application layer:
   the admin area (`api/src/admin/*`, about 4.6 k lines) holds its own settings model,
   validation and live-state mutation, with its own store port living in `auth::admin`. It
   bypasses `app` entirely.
4. Stringly-typed contracts (status, group, role, action and error codes as `String`) are
   re-interpreted in `web` with string matches, and the server-fn error channel packs
   `code;retry_after` into one string.
5. A handful of concrete runtime costs: the Fluent bundles are re-parsed on every server-side
   page render (web/src/i18n.rs:76-97 via web/src/lib.rs:81), and four DTO builders in `api`
   issue one or more queries per row (N+1).

None of these is a correctness defect; all of them are cost (build time, memory, latency,
lines to maintain). Section 8 orders the fixes.

## 1. Crate graph as built

### 1.1 Table

Workspace deps = normal (non-dev) dependencies on other workspace crates, from each Cargo.toml.
"(ssr)" / "(pg)" = optional, enabled by that feature. LOC = `src/` lines; "+N t" = `tests/`
lines. Tests = attribute count (src + tests).

| Crate | Purpose | Depends on (workspace) | Depended on by | LOC | Tests |
|---|---|---|---|---|---|
| domain | pure types and rules | - | app, kinds, store-pg, search, realtime, catalog-core, connectors, importers, integrations, notify, auth, testkit, api, server, cli | 2.0 k | 20 |
| app | use-cases + 22 port traits | domain | every adapter, api, server, cli, testkit, kinds | 7.4 k + 4.8 k t | 93 |
| contracts | serde DTOs (+ utoipa under `openapi`), plus `locale` and `auth` rule helpers | - | web, api, auth, notify, server | 3.2 k | 9 |
| kinds | built-in kind modules, `KindCatalog` impl | domain, app | connectors, server; dev: api, importers, integrations | 0.8 k + 0.2 k t | 7 |
| store-pg | Postgres repos, migrations, outbox, change feed | app, domain | server, cli; dev: most adapters | 3.7 k + 2.5 k t | 30 |
| search | Meilisearch + pg fallback | app, domain | server, cli | 0.8 k + 0.3 k t | 6 |
| jobs | Postgres queue, worker, cron | app | api, server | 0.7 k + 0.5 k t | 13 |
| realtime | event bus, SSE, LISTEN/NOTIFY | app, domain | api, server | 0.2 k | 4 |
| catalog-core | connector traits, governed HTTP, cache, merge, refresh, pg stats | app, domain | connectors, api, server | 3.2 k + 1.2 k t | 36 |
| connectors | tmdb, igdb, podcastindex, tvdb (features) | app, domain, kinds, catalog-core | server | 1.7 k + 0.9 k t | 24 |
| assets | blob store, image pipeline, storage budget | app | api, config, server, cli | 2.9 k + 0.9 k t | 38 |
| importers | import formats (features) | app, domain | api, server | 2.4 k + 1.1 k t | 33 |
| integrations | webhooks, pollers (pg feature) | app, domain | api, server | 1.1 k + 0.2 k t | 8 |
| notify | rules engine, channels, templates (pg/http/email features) | app, domain, contracts | api, server | 3.3 k + 0.9 k t | 35 |
| auth | passwords, sessions, TOTP, OIDC, secrets, admin store (pg) | app, domain, contracts | api, server, cli | 9.7 k + 1.4 k t | 64 |
| brand | white-label config | - | web (ssr), server | 0.7 k + 0.2 k t | 17 |
| config | typed config file + env | assets | telemetry, server, cli | 1.2 k | 17 |
| telemetry | tracing init | config | server | 42 | 1 |
| testkit | in-memory fakes, fixtures, OIDC test issuer | app, domain | dev-deps only | 2.4 k | 0 |
| ui | generic Leptos components, icon codegen | - | web | 4.1 k + 1.5 k t | 70 |
| web | Leptos app, pages, server fns, i18n | ui, contracts, brand (ssr) | server | 32.4 k | 146 |
| api | REST `/api/v1`, OpenAPI, SSE, `ops`, admin area | app, domain, contracts, auth, assets, catalog-core, importers, notify, integrations, jobs, realtime | server, cli (via server) | 12.7 k + 1.8 k t | 30 |
| apps/server | composition root, Leptos host, job handlers, web backends | everything above | cli | 3.4 k + 12.5 k t | 107 |
| apps/cli | admin shell commands | server, auth, app, assets, config, domain, store-pg, search | - | 0.5 k | 2 |
| xtask | ci, dev, e2e, services, arch and locale checks | - (cargo_metadata) | - | 1.9 k + 0.3 k t | 28 |

Totals: about 104 k lines of Rust in `src/`, about 33 k in `tests/`; 672 packages in
Cargo.lock. `web` alone is 31 % of all source lines; `web` + `api` + `auth` is 53 %.

### 1.2 Comparison with DESIGN.md D-3 and xtask/src/arch.rs

Matches the design:

- `domain` has no workspace deps, no tokio, no async-trait (crates/domain/Cargo.toml).
- `app` depends on `domain` only, plus serde/chrono/uuid/chrono-tz/base64 (crates/app/Cargo.toml).
- `ui` has no workspace deps (crates/ui/Cargo.toml). `web` depends on `ui` + `contracts`
  (+ `brand` under `ssr`), which is DEC-006's amendment of D-3.
- Adapters depend on `app` (+ `domain`), never on each other, except `connectors ->
  catalog-core, kinds` which is the intended "connector SDK" relation.
- No adapter in `NO_DIRECT_HTTP` (arch.rs:37-46) declares an HTTP client.
- The arch test runs against real `cargo metadata` in `cargo test -p xtask`
  (xtask/tests/architecture.rs), which `cargo xtask ci` includes (xtask/src/main.rs:64-130,
  `cargo test --workspace`).

Deviations and gaps (none of them are caught by arch.rs today):

1. **`web` does not depend on `app` at all.** DESIGN.md and arch.rs:26-34, 58 allow and
   feature-gate `web -> app (ssr)`; the code went a different way (DEC-092): a `Backend` trait
   in `web` that the server implements. The rule `FEATURE_GATED_DEPS` is therefore dead, and
   the comment on arch.rs:31-32 is stale. Not a problem by itself, but see section 2.1.
2. **`api` is not in the DESIGN.md crate list** (DESIGN.md section 3 lists 21 crates; `api`
   was added). It is also the only crate besides `server` that depends on nearly every
   adapter (11 workspace deps). arch.rs has no rule for it.
3. **`config -> assets`** (crates/config/Cargo.toml; crates/config/src/lib.rs:15 `pub use
   assets::StorageConfig`). A leaf "typed config" crate depends on an adapter that pulls in
   `image`, `opendal`, `rayon`, `fast_image_resize`, `webp`, `sqlx`. `telemetry -> config`
   then inherits all of it. Everything that wants `AppConfig` compiles the image pipeline
   first. The arch test does not list `config` or `telemetry` in `ALLOWED_WORKSPACE_DEPS`.
4. **`cli -> server`** (apps/cli/Cargo.toml) only for `server::escape` (apps/cli/src/main.rs:312,
   359). The admin CLI therefore links the Leptos SSR stack, `web` (32 k lines) and axum. Any
   change in `web` rebuilds the CLI.
5. **`auth` and `notify` depend on `contracts`** (crates/auth/src/lib.rs:27, 46-47;
   crates/notify/src/template.rs:10, 19) for username/password rules and locale number
   formatting. `contracts` is thereby a shared-utilities crate, not "DTOs only" as its
   doc comment says (crates/contracts/src/lib.rs:1-3). The rules themselves are right to be
   shared with the browser; the home is wrong (they are domain rules).
6. **`auth` is both a service and an adapter.** It owns the `Auth` service over `App`
   (crates/auth/src/lib.rs), the `AuthStore`/`AdminStore` ports, and their Postgres impls
   (`pg.rs` 1,147 lines, `admin_pg.rs` 878, `oidc/pg.rs` 434: 2.5 k lines of sqlx in a crate
   DESIGN.md describes as "passwords, sessions, tokens, OIDC, TOTP"). `sqlx` is a normal
   dependency, so `api` and every test that pulls `auth` compiles sqlx macros.
7. **`contracts`' `openapi` feature is enabled by `api`** (crates/api/Cargo.toml). In the
   server build cargo unifies features, so `web` (ssr) also compiles `contracts` with the
   utoipa derives. Harmless on wasm (separate target build), a small cost on the server build.
8. arch.rs restricts only five crates (`domain`, `app`, `contracts`, `ui`, `web`). There is no
   rule that nothing but `server` may depend on `web`, that `cli` must not depend on `leptos`,
   that `config`/`telemetry`/`brand` stay leaves, or that `testkit` is dev-only. All of these
   hold today by accident.

## 2. Layering and boundary problems

### 2.1 The four-layer pass-through for every UI operation

Per operation there are: a `#[server]` fn in web/src/api.rs or web/src/admin_api.rs (54 of
them, 1,107 lines together); a method on `web::backend::Backend` (about 80 methods,
web/src/backend.rs, 263 lines) or `web::admin_backend::AdminBackend` (about 50 methods,
116 lines); its implementation in apps/server/src/web_backend.rs (547 lines, 68 async fns)
or apps/server/src/web_admin.rs (327 lines, 46 async fns); and the shared operation in
`api::ops` / `api::browse` / `api::people` / `api::sharing` / `api::admin`, which finally calls
`app`. The server-side impls do nothing but resolve the session token to an `Actor` and map
`ApiError` to `ErrorDto` (web_backend.rs:17-52).

DEC-162 kept this on purpose so that `web` never sees server crates and a Tauri shell could
plug in another backend. Two observations:

- The Tauri case is already served differently: web/src/rest.rs (284 lines) calls the REST API
  from the browser for things server fns cannot do. A CSR client would use REST, not a Rust
  trait; the trait buys nothing for it.
- The cost is not only boilerplate. Every new operation touches five files in three crates,
  and the `Backend` trait is the widest interface in the workspace (about 130 methods across
  the two traits) with exactly one implementation each and no test double (the server tests go
  through HTTP).

Recommendation (WP-B2, section 8): under `ssr`, let the server fns call the shared operations
directly through an `Arc<OpsState>` in the Leptos context; delete both traits and both impls.
This needs the operations to live in a crate `web` may depend on under `ssr`, which is the
reason for WP-B3.

### 2.2 `api` is a second application layer

`api` (12.7 k lines) splits into three kinds of code:

- HTTP adapter proper: routes.rs 1,333, admin_routes.rs 582, oidc_routes.rs 635, sso_routes.rs
  425, people_routes.rs 188, goals_routes.rs 119, usage_routes.rs 72, extract.rs 97,
  request_meta.rs 286, error.rs 150 (about 3.9 k lines; 150 `routes!` entries, 175
  `#[utoipa::path]`).
- Shared operations and DTO mapping used by both REST and the web backends: ops.rs 787,
  browse.rs 611, people.rs 461, sharing.rs 296, notifications.rs 604, bulk.rs 241, goals.rs
  229, sync.rs 173, hooks.rs 123, dto.rs 144 (about 3.7 k lines).
- The admin area: admin/mod.rs 1,791, admin/sources.rs 846, admin/oidc.rs 793, admin/setup.rs
  418, admin/mail.rs 407, admin/probes.rs 159, admin/user_menu.rs 153, usage.rs 415 (about
  5.0 k lines).

The admin area is application logic, not an adapter. `AdminApi` (crates/api/src/admin/mod.rs:
229-250) holds a store port (`Arc<dyn AdminStore>`, the trait is defined in `auth::admin`),
the cipher, key info, live `AtomicBool`/`RwLock` settings that request handlers read, and a
usage rate limiter. Settings validation (`one_of`, `fraction`, `set_stored`, `set_mail`,
`effective`, `warnings`, admin/mod.rs:443-665) and the "apply live values" step
(`apply`, admin/mod.rs:665) are use-cases with a port, living in the HTTP crate, with their
port in the auth crate. `App` is not involved. Consequences: these rules cannot be tested with
`testkit::TestEnv`; the web backends must depend on `api` to reach them; and `api` has to
depend on `assets`, `jobs`, `importers`, `integrations`, `notify`, `catalog-core`, `realtime`
to hold their handles.

`api::browse` also carries presentation rules that belong to neither layer cleanly:
`next_step` label-key selection (browse.rs:219), progress `summary` (browse.rs:270), cursor
encoding (browse.rs:398-404 plus `app::ports::LibraryCursor::encode` used at browse.rs:214).
These are fine in an "ops" layer; they are mislabelled as REST code.

### 2.3 Stringly-typed contracts, re-interpreted in `web`

`contracts` carries enumerations as `String`: `UserDto.role` ("admin"/"member",
contracts/src/lib.rs:32), library status, kind group, `NextStepDto.action`, `ErrorDto.code`.
The browser then matches on literals: web/src/widgets.rs:164 `matches!(status.as_str(),
"planned" | "in_progress")`, web/src/next_step.rs:27-30 `match step.action.as_str()`,
web/src/library_state.rs:17 `SORTS` and :26 `GROUP_IDS` restating `app::LibrarySort` and
`domain::kind::KindGroup`. `domain::kind::Status::COMPLETED` (domain/src/kind.rs:86) is the
one place the literal is defined; nothing ties the copies to it. A renamed status or a new
group is a silent UI bug. `contracts` is wasm-safe and has serde, so `#[serde(rename_all =
"snake_case")]` enums cost nothing and would be shared by `api` (mapping from domain) and
`web`.

Error codes have three encodings: `ApiError.code: &'static str` (api/src/error.rs:12),
`ErrorDto.code: String`, and the server-fn channel that packs `"{code};{retry_after}"` into
`ServerFnError::ServerError(String)` and splits it again (web/src/api.rs:633-659; also
web/src/rest.rs:31-40). Leptos 0.8 supports `Result<T, ServerFnError<E>>` with a custom
serde `E`; `ErrorDto` would travel typed.

### 2.4 Business rules in the database

Three migrations hold logic in PL/pgSQL: 0015_phase1_bounds.sql:13-53 (`library_activity`,
`progress_events_touch_entry`, `library_entries_init_activity`), 0023_stats_cache.sql:55-158
(eight triggers that mark the stats cache dirty), 0029_stats_dirty_gen.sql:14-71 (CREATE OR
REPLACE of five of them to fix a lost-mark race). This is a deliberate performance choice
(DEC-235) and the right one for the stats cache. The boundary problem is that `MemStore`
(crates/testkit/src/store.rs) has no equivalent, so use-case tests on `TestEnv` never exercise
the dirty-mark path, and the only parity check is the one in store-pg/tests/phase1.rs:514-545
comparing `month_amounts` to `app::month_amounts`. Unsure whether `MemStore` reproduces
`last_event_at`/`plays` the way the 0015 triggers do; I did not find a test that says so.

### 2.5 Duplicated logic

- Session-to-actor resolution exists in `api::extract::Authed`, in `WebBackend::actor`
  (web_backend.rs:32-39) and in `WebAdmin::actor` (web_admin.rs:31-39).
- `unauthorized()` ErrorDto is built by hand in web_backend.rs:22-29 while `ApiError::
  unauthorized()` exists (error.rs:38-40) and web_admin.rs uses it.
- `parse_id` (api/src/ops.rs:29) is the one UUID parser; good. But `uuid_field` in
  apps/server/src/handlers.rs:16-22 re-does it for job payloads.
- Web-side `GROUP_IDS`/`SORTS` (above). Web-side `rest::status_code` (rest.rs:17-29) maps HTTP
  status to the same codes `ApiError` produces.
- `notify` pins `hmac = "0.12.1"` and `sha2 = "0.10.9"` directly while the workspace has
  `hmac 0.13` / `sha2 0.11` (crates/notify/Cargo.toml vs the root Cargo.toml); two
  implementations of the same HMAC are compiled (section 5).

### 2.6 God modules

Files over 900 lines: api/src/admin/mod.rs 1,791; api/src/routes.rs 1,333; web/src/library.rs
1,312; testkit/src/store.rs 1,190; auth/src/pg.rs 1,147; web/src/log_sheet.rs 1,024;
web/src/shell.rs 1,017; apps/server/src/lib.rs 907; web/src/history.rs 940; web/src/item.rs
904; auth/src/oidc.rs 911; app/src/ports.rs 899. The per-function 100-line rule (DEC-234) is
kept; the per-file concentration is not addressed by any rule. `app/src/ports.rs` deserves a
split into `ports/repos.rs` and `ports/infra.rs` on its own: it defines 22 traits, the
filter/cursor/event/job/notification types, and `PortError`.

### 2.7 Coupling through `contracts`

`contracts::auth` (290 lines) holds `check_username`, `normalize_username`,
`normalize_display_name`, the length constants and `AuthErrorCode`; `contracts::locale`
(134 lines) holds `format_decimal` and `LOCALES`. `auth` and `notify` import them (section
1.2 item 5). The dependency direction (service -> DTO crate) is tolerable, but these are
domain rules; `domain` is already wasm-safe (serde, uuid, chrono without clock). Moving them to
`domain::auth_rules` / `domain::locale` and having `web` depend on `domain` (allowed: it has no
IO) would make `contracts` DTO-only again. If `web -> domain` is not wanted, a tiny
`rules` leaf crate does the same.

## 3. The `web` crate

### 3.1 Size and shape

- 32,363 lines in 68 files; 344 `#[component]`s; 212 `into_any()` calls; `#![recursion_limit
  = "256"]` (web/src/lib.rs:6, DEC-127) because view types already overflowed the default.
- 54 `#[server]` functions in two files (api.rs 677, admin_api.rs 430).
- Two Fluent files embedded with `include_str!` (web/src/i18n.rs:26-27): en.ftl 2,111 lines,
  de.ftl 2,102 lines.
- 146 unit tests in 40 `#[cfg(test)]` modules inside the crate; the largest are pure-logic
  modules (log_form 11, i18n 9, auth 9, admin_state 9, offline 8, history_state 8,
  episode_plan 8, library_state 7, charts 6).
- Feature-gated code: 112 `cfg(feature = "hydrate")` sites, 14 `ssr`, 3 `gallery`. The
  `gallery` module (581 lines) is already out of production builds. The `csr` feature is
  declared but used by nothing in xtask, CI or the Dockerfile (grep); it exists for Tauri.
- Server-only modules inside the crate: backend.rs, admin_backend.rs, auth_redirect.rs, the
  `server` submodule of api.rs (api.rs:17-62), `server_config` in app.rs:66-122. These pull
  `axum`, `leptos_axum`, `async-trait`, `brand` under `ssr`.
- Natural page areas by file (lines): library/item/episodes/logging (library 1312,
  library_state 714, item 904, episode_list 536, episode_plan 505, log_sheet 1024, log_form
  573, next_step 456, bulk 421, widgets 699, recent 113, nudge 125: about 7.4 k); home/history/
  stats (home 790, history 940, history_state 601, stats_extras 608, charts 404, funnel 622:
  about 4.0 k); people/lists (people 572, people_dialogs 557, lists 683, list_share 190,
  list_drag 126, sharing 60, game_groups 287: about 2.5 k); account/auth/settings
  (auth 620, auth_pages 643, sign_in 614, sign_out 131, reauth 599, account 275, settings 762,
  notifications 407, notify_channels 655, notify_rules 325, notify_rules_view 560,
  integrations 497, imports 654, setup 383, setup_state 193, offline 787: about 8.1 k);
  admin (admin/* 3,315, admin_state 508, admin_api 430, admin_backend 116, source_hints 413:
  about 4.8 k); shell and plumbing (shell 1017, palette 872, keyboard 499, i18n 490, app 269,
  api 677, backend 263, rest 284, events 69, pages 69, lib 102, gallery 581: about 5.2 k).

### 3.2 Compile cost and memory

Observed by the owner: one release `rustc` of `web` needs 3-4 GB (src/docker/Dockerfile
comment above `ARG CARGO_BUILD_JOBS`), one debug `rustc` about 2.9 GB (HANDOFF.md section 8),
and the Docker build was OOM-killed on a 16 GB host until the frontend and server builds were
serialised with `CARGO_BUILD_JOBS=2`. Why `web` specifically:

1. The `view!` macro expands each component into deeply nested generic types; 344 components
   and 212 `into_any()` erasures in one crate make one enormous type-checking and
   monomorphisation unit. The recursion-limit bump (DEC-127) is the symptom. Memory in rustc
   scales with the crate, not with the workspace, so splitting is the only lever that lowers
   the peak.
2. The crate is compiled **three times per full build**: `web` with `ssr` inside the server
   build, `web` with `hydrate` for wasm32, and `web` with `ssr` plus `cfg(test)` for
   `cargo test` (the 146 unit tests live inside it). cargo-leptos runs the first two; CI runs
   all three.
3. The wasm release profile uses `lto = true`, `codegen-units = 1`, `opt-level = "z"` (root
   Cargo.toml `[profile.wasm-release]`). Fat LTO with one codegen unit puts the whole
   program through one LLVM pass; this is the "more" in the Dockerfile comment. It is a size
   choice; it costs peak memory at link time regardless of how `web` is split.
4. The 146 tests and the pure-logic modules compile with Leptos even though most of them do
   not need it (library_state, history_state, episode_plan, offline, funnel, charts,
   admin_state, setup_state, log_form parsing). Every test run rebuilds the whole crate.
5. `web-sys` with 36 enabled features and `wasm-bindgen` are only under `hydrate`; fine.

### 3.3 Split proposal

Target shape (names are suggestions):

```
crates/web-state      pure logic + its unit tests, no leptos:
                      library_state, history_state, episode_plan, offline (pure half),
                      funnel (pure half), charts (data half), admin_state, setup_state,
                      log_form parsing, i18n (I18n, Stamp, formatters), keyboard tables.
                      Deps: contracts, fluent-bundle, unic-langid, serde. ~5 k lines.
crates/web-shell      ClientConfig, provide_page_context, shell, palette, keyboard handlers,
                      events (SSE), pages helpers, widgets, next_step button, the server-fn
                      plumbing module (what api.rs:17-62 is today) and the generic server fns
                      (current_user, sign_in/out, search).  Deps: ui, contracts, web-state,
                      leptos*. ~5 k lines.
crates/web-library    library, item, episode_list, log_sheet, bulk, recent, nudge + their
                      server fns. ~6 k.
crates/web-activity   home, history, stats_extras, charts view, funnel wiring. ~3.5 k.
crates/web-people     people, people_dialogs, lists, list_share, list_drag, sharing,
                      game_groups. ~2.5 k.
crates/web-account    auth, auth_pages, sign_in, sign_out, reauth, account, settings,
                      notifications, notify_*, integrations, imports, setup, offline wiring,
                      rest. ~8 k.
crates/web-admin      admin/*, admin_api, source_hints. ~4.5 k.
crates/web-gallery    gallery (dev only; a normal crate the dev/e2e server depends on
                      behind its `gallery` feature). ~0.6 k.
crates/web            thin: App router (app.rs:191-234), hydrate entry, cdylib. Depends on
                      all of the above. <0.5 k.
apps/server           keeps backend.rs / admin_backend.rs / auth_redirect.rs content
                      (they are server code; today they live in web under cfg(ssr)).
```

Expected effect:

- Peak rustc memory per crate drops roughly with crate size; the largest piece (web-account
  or web-library at 6-8 k lines, 60-80 components) should land well under half of today's
  figure. This is an estimate from the per-component cost, not a measurement; WP-B1 includes
  measuring with `/usr/bin/time -v` before and after.
- `cargo build` of the server after a change in one page area recompiles that area crate and
  the thin `web` only. Today any edit in `web` recompiles 32 k lines twice (ssr + hydrate).
- `cargo test -p web-state` compiles no Leptos at all and holds most of the 146 unit tests.
- Parallelism becomes available: the six area crates compile concurrently (bounded by
  `CARGO_BUILD_JOBS`, which the host sets to 3 for memory reasons; three 1-GB rustcs are
  better than one 3-GB rustc).
- Wasm binary size is unchanged in principle (same code, same LTO). Fat LTO still sees the
  whole program; link-time memory does not drop. If the wasm link is the real OOM point,
  evaluate `lto = "thin"` or `codegen-units = 16` for `wasm-release` and measure the size
  delta (WP-E1).

Things to get right:

- Server functions across crates: `#[server]` registers through `inventory`; the function
  is collected only if its crate is linked into the binary. Every area crate must be a
  dependency of `web` (hydrate) and, transitively, of `server` (ssr); since `web` re-exports
  the routes that reference the page components, this holds automatically. Keep the `prefix
  = "/_fn"` constant in one place (`web-shell`).
- `recursion_limit = "256"` moves to each crate that needs it (per-crate attribute).
- Contexts (`I18n`, `ClientConfig`, `PaletteOpen`, `Toasts`, `OnlineState`) are provided by
  `web-shell` and consumed by every area crate; they must be `pub` types in `web-shell`
  (today several are crate-private, e.g. `palette::PaletteOpen`).
- The two `.ftl` files stay embedded once, in `web-state`. The locale parity check in xtask
  (xtask/src/locales.rs) reads `crates/web/locales`; adjust the path.
- Cargo features: the `ssr`/`hydrate` pair must be declared on every area crate and forwarded
  from `web` (`ssr = ["web-shell/ssr", "web-library/ssr", ...]`). The arch test needs a rule
  that only `web` may enable them.
- `#[cfg(feature = "ssr")]` code that touches `axum`/`leptos_axum` should end up in
  `web-shell` only (the server-fn plumbing). Pages should not need either.

### 3.4 Migration steps

1. Create `web-state` by moving the pure modules and their tests; make `web` depend on it.
   No behaviour change; `cargo test -p web-state` runs without Leptos. (Half a day; this
   step alone removes the Leptos compile from most web unit tests.)
2. Make the shared contexts and helpers `pub` and move them with shell/palette/keyboard/
   widgets/events/pages/api-plumbing into `web-shell`. `web` depends on it. Build, run e2e.
3. Move the server-only modules (backend, admin_backend, auth_redirect, `server_config`) out
   of `web` into `apps/server` (or do WP-B2 first, which deletes most of them).
4. Carve out one area crate at a time, starting with `web-admin` (least coupled to the rest,
   already has its own server fns and state module), then `web-people`, `web-activity`,
   `web-account`, `web-library`. After each: `cargo leptos build`, e2e, `cargo xtask ci`.
5. Move `gallery` to `web-gallery`; the server's `gallery` feature depends on it.
6. Measure: peak RSS per rustc (`/usr/bin/time -v cargo build -p <crate>` on each), total wall
   time of `cargo leptos build --release`, wasm size. Record in a DEC.
7. Extend arch.rs: `web-*` crates may depend on `ui`, `contracts`, `web-state`, `web-shell`
   only; only `web` may depend on `web-*`; only `server` may depend on `web`.

Risk: medium. The code moves are mechanical, but Leptos context/feature plumbing errors show
up only at runtime (missing context panics in SSR). Mitigation: the server tests already render
every page (apps/server/tests/*_page.rs, 107 tests) and e2e covers the flows.

## 4. The `api` and `app` crates

### 4.1 `app` (7.4 k lines, 22 modules, 93 tests)

Shape: `App` (app/src/lib.rs:81-89) holds `Repos` (10 repository ports, lib.rs:66-77) plus
seven infrastructure ports; use-cases are `impl App` blocks spread over 18 modules. Error type
`AppError` has five variants (app/src/error.rs:7-17); `Conflict(String)` is the only
stringly one. Tests: 93, most in `tests/` on `testkit::TestEnv` (fakes for every port), plus
store-pg/tests/use_cases.rs running the same use-cases on Postgres. This is the part of the
system that matches DESIGN.md sections 3 and 4 best.

Observations:

- `ports.rs` (899 lines) is one file for 22 traits with 123 methods and all their value types
  (filters, cursors, events, jobs, notifications, secrets). It is the most-imported file in
  the workspace; every change to any port rebuilds everything. Split into `ports/repos.rs`,
  `ports/infra.rs`, `ports/types.rs` (no semantic change, big incremental-build win).
- Port granularity is right where DESIGN.md asked (one repo per aggregate, each with a Pg and a
  Mem impl). `LibraryRepo` (17 methods, ports.rs:183) is the widest and mixes CRUD with query/
  paging/tags/series; `CatalogRepo` has 14. Acceptable; watch them.
- `SecretCipher` and `SecretRows` (ports.rs:851, 872) are infrastructure concerns of the admin
  area that `app` itself never calls (unsure: I found no use inside `app/src` other than the
  trait definitions). They are in `app` so adapters can share them; a `ports::infra` module
  makes that explicit.
- `catalog.rs`, `episodes.rs` know job names (`REFRESH_JOB`, `EPISODES_JOB`, lib.rs:39-40) and
  enqueue jobs; the handlers live in apps/server/src/handlers.rs. The contract between the
  two is a string and a JSON payload shape with no shared type; `uuid_field` (handlers.rs:
  16-22) parses it by hand. A `JobKind` enum with typed payloads in `app::ports` would close
  this.
- `bulk.rs` + `bulk/undo.rs` (573 lines) and `activity.rs` + `activity/person.rs` (973
  lines) are the biggest use-case families; both fine.
- `chrono-tz` and `uuid` are declared with explicit versions in app/Cargo.toml instead of
  `workspace = true` (same for domain, notify). Harmless today; a drift risk.

### 4.2 `api` (12.7 k lines, 30 tests)

Covered in 2.2. The split I recommend (WP-B3):

```
crates/ops        ops.rs, browse.rs, people.rs, sharing.rs, notifications.rs, bulk.rs,
                  goals.rs, sync.rs, hooks.rs, dto.rs, the ApiError type and its From impls
                  (error.rs minus IntoResponse), ApiState minus axum. ~4 k lines.
                  Deps: app, domain, contracts, auth, assets, importers, integrations, notify,
                  jobs, realtime, catalog-core (what ApiState needs).
crates/app-admin  (or app::admin if it can be expressed over app ports) the settings model,
                  validation, provenance, key info, probes, sources/credentials, setup state,
                  mail settings, usage funnel aggregation: admin/*.rs minus the HTTP bits,
                  usage.rs. AdminStore port moves here from auth::admin (its Pg impl stays in
                  auth or moves to store-pg). ~4.5 k lines.
crates/api        axum only: routes*.rs, extract.rs, request_meta.rs, IntoResponse for
                  ApiError, OpenAPI assembly. ~3.9 k lines. Deps: ops, app-admin, axum,
                  utoipa.
```

Effects: `web` (ssr) may depend on `ops` and `app-admin` (they have no axum) and call them
directly (WP-B2); `api` becomes a thin adapter again; admin rules get `TestEnv`-style unit
tests (today `api/tests/rest.rs` and `sync.rs` build `ApiState` with `admin: None`, and the
admin area is tested only through the server binary's HTTP tests, apps/server/tests/admin.rs
32 KB and instance.rs 39 KB).

Other `api` points:

- `ApiState` (api/src/lib.rs:93-118) has six `Option<...>` fields for subsystems that "small
  test setups" leave out. Every handler that uses one has to produce a 404/500 for `None`
  (e.g. `admin()` at admin/mod.rs:279-286, `api()` in hooks.rs:20). A builder with defaults
  from `testkit` would let the fields be non-optional.
- `From<AppError> for ApiError` (error.rs:79-106) and `From<AuthError>` (108-140) are the only
  status mapping; `app::ports::PortError` maps to 500 (143-150). Good: one place.
- `request_meta.rs` (286 lines, trusted proxies) and `extract.rs` are generic middleware;
  they would fit a `http-common` module if `api` is split, or stay in `api`.

## 5. Dead code, unused features, dependencies, duplicate versions

### 5.1 Unused dependencies

A grep-based check (every `[dependencies]` entry of every crate against `crate_name::`,
`crate_name!` and similar uses in `src/`, `tests/`, `build.rs`) found **no declared dependency
without a use**. The check is loose (a mention in a comment counts; macro-only or
`#[derive]`-only uses could be missed in the other direction), so treat it as "nothing
obvious". `cargo udeps` / `cargo machete` would confirm; both were out of scope (no cargo
runs).

### 5.2 Unused or half-used features

- `web/csr`: declared (crates/web/Cargo.toml) and referenced only in a comment in
  xtask/src/main.rs:57 and .gitlab-ci.yml:15. Nothing builds it; it has never been compiled in
  CI. Either add a `clippy --features csr --target wasm32` step or drop it until the Tauri
  milestone.
- `importers/trakt-import` is a default feature while `server` and `api` enable `generic`
  explicitly; `cli` depends on `importers` indirectly via `server`. No build uses `importers`
  without `trakt-import`, so the feature split has no coverage of the "off" state. Same for
  `connectors`' per-provider features: CI builds `all-connectors` only. Partial feature sets
  compile untested (unsure whether that matters to the owner; it matters if the features are
  meant as a product switch).
- `web/gallery`: fine, used by dev/e2e.
- `contracts/openapi`: fine.

### 5.3 Duplicate versions in Cargo.lock (672 packages)

Each duplicate is one more crate compiled, and for proc-macro and crypto crates that is
noticeable. Owners found by reading the lock file:

| Crate | Versions | Who pulls the older one | Actionable? |
|---|---|---|---|
| reqwest | 0.12.28, 0.13.5 | meilisearch-sdk 0.33 | Yes: replace meilisearch-sdk with a thin client over `HttpFetch` (also routes search traffic through the governed client, per D-5.2) or wait for an upstream bump. Removes a whole second HTTP stack. |
| thiserror (+impl) | 1.0.69, 2.0.21 | gloo-net, oauth2, openidconnect, yaup | Upstream. |
| sha2 | 0.10.9, 0.11.1 | notify (ours), oauth2, openidconnect, rsa, sqlx-core, sqlx-macros-core | Partly: `notify` pins 0.10.9 itself although the workspace is on 0.11; moving `notify` to the workspace version removes one owner but not the duplicate (sqlx/openidconnect keep 0.10). |
| hmac | 0.12.1, 0.13.0 | notify (ours), openidconnect | Same as sha2. |
| base64 | 0.22.1, 0.23.1 | 0.22 is ours plus axum, leptos, sqlx, oauth2, openidconnect, jsonwebtoken, wiremock; 0.23 via lettre, opendal, reqwest 0.13, hyper-util, serde_with | Upstream-bound; our pin cannot collapse it. |
| schemars | 0.9.0, 1.2.3 | serde_with (via openidconnect) | Upstream. |
| config | 0.1.0 (ours), 0.15.27 (leptos_config) | Our crate is named `config`, same as the crates.io crate leptos pulls. Cargo copes, but every `cargo` message about `config` is ambiguous and a future `cargo add config` in a leaf crate would collide. Rename ours to `app-config` (cheap now, costly later). |
| rand / rand_core / rand_chacha / getrandom | 3 + 3 + 2 + 3 versions | openidconnect/oauth2 (0.8), proptest/sqlx-postgres/totp-rs (0.10), ring/jsonwebtoken (getrandom 0.2) | Upstream. |
| hashbrown | 4 versions | indexmap 1 (serde_with), others | Upstream. |
| syn | 2.0.119, 3.0.9 | nearly every proc-macro crate on 2, newer ones on 3 | Upstream; costs one extra large compile. |
| darling | 0.20.11, 0.24.1 | derive_builder (0.20) | Upstream. |
| convert_case | 0.6, 0.8, 0.11 | config 0.15, leptos_macro, convert_case_extras | Upstream. |
| indexmap | 1.9.3, 2.14.2 | serde_with | Upstream. |
| spki/der/pkcs8/signature/const-oid/pem-rfc7468 | two RustCrypto generations | rsa 0.9 (testkit) vs the 0.8-era stack of aes-gcm/hkdf 0.13 | Partly ours: `rsa` is a normal dependency of `testkit` just for the OIDC test issuer; it drags the old RustCrypto generation into every dev build. Make `rsa` optional behind a `testkit/oidc` feature that only the OIDC tests enable. |
| wasm-streams, miniz_oxide, spin, quick-error, pastey, bitflags, windows-sys, r-efi, cpufeatures, block-buffer, crypto-common, digest | 2 each | transitive | Upstream. |

Two more build-cost items that are not duplicates:

- `webp` 0.3.1 (crates/assets/src/image.rs:130) next to `image` with its `webp` feature: the
  `image` crate's WebP encoder is lossless-only, so the separate crate is justified for lossy
  output. Fine; note it in a comment.
- `rsa` + `num-bigint-dig` in `testkit` (above).

## 6. Error handling, configuration, wiring, testability, migrations

### 6.1 Error handling

- The KICKOFF rule (thiserror in libraries, anyhow in apps and xtask) is kept: `anyhow` appears
  only in apps/server, apps/cli, xtask.
- Three error vocabularies meet in the server fns (section 2.3). The `;` packing is the weak
  spot; a typed `ServerFnError<ErrorDto>` ends it. `AuthError::code()` -> `contracts::auth::
  AuthErrorCode` is the model to follow for `AppError` too.
- `ServerFnError::new("unauthorized")` / `"backend missing from context"` (web/src/api.rs:
  26-39) are literals; fine, but they should be the same constants the pages match on
  (`pages::needs_sign_in`).
- Job handlers return `Result<(), String>` (jobs::Handler, see handlers.rs:30-33); the
  queue stores the string. Adequate for retry/dead-letter; loses the error kind for the admin
  "failed jobs" view (unsure whether the view needs it).

### 6.2 Configuration

- `config` (1.2 k lines, 2 files): TOML + `APP__SECTION__KEY` env overrides, schemars for the
  generated reference, 17 tests. Solid. The only structural issue is `config -> assets`
  (section 1.2 item 3): `StorageConfig` and `ByteSize` should live in `config` (or a tiny
  `storage-config` leaf) with `assets` depending on it, not the other way round. That also
  frees `telemetry` from the image stack.
- Runtime-editable settings (OD-10/OD-11) live in `instance_settings` and are merged with
  env provenance in `api::admin` (`ConfigValues`, admin/mod.rs:173; `effective`, :585). The
  precedence logic (env wins and shows read-only) is in the HTTP crate; it belongs with the
  settings model (WP-B3 moves it).

### 6.3 Wiring

`Server::build` (apps/server/src/lib.rs:224-425, about 200 lines, `allow(too_many_lines)`)
plus helpers `notify_engine`/`notify_channels` (78-172), `default_registry` (177-215),
`governed_http` (610), `auth_settings` (648), `oidc(...)`, `search_index(...)`, and
`admin_wiring::with_admin` (483 lines) make the composition root about 1.4 k lines. It is
readable top to bottom and has `Overrides` for tests (58-65). Problems:

- `Server` exposes 14 `pub` fields (37-54) that tests and the CLI reach into; the struct is
  the de-facto service locator. Make them private with accessors where needed, or accept it
  and say so.
- `build` connects, migrates, loads keys, seals plaintext secrets, syncs source counters and
  runs the first health probe (392-403) before returning. Startup side effects in the
  constructor make `Server::build` unusable for "just give me a wired object" in tests;
  the server tests pay for a migration run each (unsure how long; `apps/server/tests` is
  12.5 k lines and 107 tests, the biggest test suite).
- Recommendation (WP-D1): `wiring/{db,notify,catalog,auth,search,admin}.rs`, each a `fn
  (ctx) -> Result<Part>`; `Server::build` becomes the sequence; a `Server::connect_only` for
  tests that do not need probes; startup jobs (probes, counter sync) move to `run_*`.

### 6.4 Testability

- `testkit::TestEnv` wires `App` to `MemStore` (all 10 repo ports, store.rs 1,190 lines),
  `FixedClock`, `SeqIds`, `RecordingEvents`, `MemJobs`, `RecordingNotifications`, `MemSearch`,
  `MemBlobs`, `ReplayHttp`, `FakeCipher`. `auth` has `MemAuthStore` and `MemAdminStore`
  (auth/src/admin.rs:330). Coverage of the ports by fakes is complete.
- MemStore/Postgres parity: `store-pg/tests/use_cases.rs` runs the use-cases on Postgres and
  `phase1.rs:514-545` compares `month_amounts`. There is no shared contract suite that runs
  the same assertions against both stores, and the trigger-maintained columns of 0015 and the
  stats dirty marks of 0023/0029 have no in-memory counterpart (I found no `last_event_at` or
  `plays` handling in testkit/src/store.rs). A macro-generated `repo_contract!(store_factory)`
  suite in `testkit` (WP-D2) would catch drift and define what MemStore must emulate.
- Unit tests inside `web` (146) and `ui` (70, native `ssr` render tests plus wasm tests) are
  sound in approach; their cost is the compile unit (section 3).
- API tests: 30 in `api/tests` with `admin: None`; the admin area is tested only end-to-end.

### 6.5 The 31 migrations

- Forward-only (`sqlx::migrate!("./migrations")`, store-pg/src/lib.rs:20); no down files.
  Acceptable for a greenfield product, but it means every schema mistake is a new migration
  forever. Before the first external deployment, squash 0001-0031 into one baseline (the
  owner's instance is the only one) and start reversible pairs from there.
- Naming: `NNNN_topic.sql` mostly; 0014_phase1, 0015_phase1_bounds, 0017_phase3_instance name
  the project phase, not the change. Minor; adopt "topic only" going forward.
- Risky or notable:
  - 0015_phase1_bounds.sql:27 and :48 run `UPDATE library_entries` over the whole table inside
    the migration transaction (backfill). Fine at today's size; at 50 k entries per user and
    many users it holds locks for the duration. Future backfills should be batched or done by
    a job.
  - 0023 + 0029: trigger logic versioned across two files (CREATE then CREATE OR REPLACE). The
    current function bodies are therefore only readable by applying both in order. Keep a
    `docs/modules/.../stats-cache.adoc` (or a comment block in 0029) with the final text.
  - CHECK constraints enumerate string sets (0001:12 role, :41/:52 link states, :66 entity
    state; 0004:5/15/23/24/28 asset kinds, priorities, classes, tiers; 0016:57/83/84; 0017:10/
    17; 0018:20/21; 0019:9). Every new value is a migration. Library `status` (0001:87) is
    deliberately unconstrained text, so adding a kind-specific status needs no migration,
    which matches DESIGN.md section 15 question 2.
  - Indexes are created without `CONCURRENTLY` (not possible inside sqlx's per-migration
    transaction without `-- no-transaction`). Fine today; a rule for future indexes on
    `progress_events`, `library_entries`, `sessions`.
  - `CREATE EXTENSION IF NOT EXISTS pg_trgm` appears in 0001:5 and 0007:4; harmless.
  - 0028/0030 (TMDB retention purge, DEC-453/466) delete provider data by policy; the policy is
    in the job, the schema only adds columns. Reversible by the owner's ruling, not by SQL.
- 319 offline query files in `.sqlx`; 322 `query!`/`query_as!`/`query_scalar!` macro uses
  against 94 unchecked `query(`/`query_as(`/`query_scalar(` calls. The unchecked ones are
  mostly dynamic SQL (library query builder, unsure for each). Good ratio.

## 7. Performance-relevant structure

Confirmed by reading (not measured):

1. **Fluent bundles parsed per SSR request.** `web::shell` -> `provide_page_context`
   (web/src/lib.rs:81) -> `ClientConfig::i18n()` (app.rs:56) -> `I18n::new` (i18n.rs:132-139)
   -> `bundle()` twice (i18n.rs:76-97): each call runs `FluentResource::try_new` over a
   2,100-line file, so an `en` page parses the English file twice and a `de` page parses both.
   Fix: a process-wide `OnceLock<HashMap<(locale, overrides_hash), Arc<I18n>>>` under `ssr`,
   keyed by locale and brand override set (the overrides are static per brand). Hydration
   parses once per page load; acceptable.
2. **N+1 DTO builders in `api`.**
   - `browse::library_query` (browse.rs:205-207) calls `row()` per row; `row()` calls
     `dto::item()` which calls `cover()` (dto.rs:46-50) which awaits `svc.get(uuid)` once per
     item: one asset query per library row (48 per "Load more" page, `PAGE_SIZE` in
     library_state.rs:20). Same path for Home (`browse.rs:571-580`) and search results
     (`ops.rs:185, 239`). Fix: `AssetService::get_many(&[Uuid])` and a `dto::items()` that
     prefetches.
   - `sharing::sessions` (sharing.rs:179) -> `session_dto` (147-150): three awaits per session
     (participant names, catalog item, owner user). Fix: batch by ids (`CatalogRepo::get_many`
     exists; `UserRepo::get` does not have a many variant - add it).
   - `admin::users` (admin/mod.rs:900) -> `admin_user` (851-876): four awaits per user
     (credentials, identities, sessions, disabled flag). Small tables, admin-only, but it is
     the Users page.
   - `people::person_page` (people.rs:173) -> `list_card` per shared list.
3. **`list_users` scans** at admin/mod.rs:900, :909 and admin/oidc.rs:603 load every user to
   filter admins in Rust. Fine for a home instance; a `count_admins`/`admins()` repo method
   when "hosted" mode has hundreds of users.
4. **`App::library_query`** is bounded to 50 k entries (0015, DEC) and paged by cursor; the
   stats cache (0023/0029) moves aggregation to write time. Both are the right shape.
5. **Startup**: `run_probes` and the counter sync run inside `Server::build` (lib.rs:392-403);
   every process start (and every server test) pays them.
6. **`page_routes` is cached in a `OnceLock`** (lib.rs:832-838); good. `web::shell` is rendered
   per request with `brand.head_css()` (app.rs:140-142); unsure whether `head_css` is cached
   inside `brand`.
7. **In-memory per-process state that assumes one web process**: `SourceStats` (synced every
   `STATS_SYNC_SECS` = 60 s, lib.rs:33) and `usage::Limiter` (`Mutex<HashMap>`, usage.rs:12,
   admin/mod.rs:249). With `--role=web` scaled to two processes the counters diverge for up to
   a minute and the beacon limit doubles. Documented behaviour would suffice.

## 8. Recommendations: prioritised work packages

Format as in docs/handoff/phase4-plan.md: one build at a time; paths relative to `src/rust`.
Effort: S (under a day), M (1-3 days), L (a week). Risk: L/M/H. Each ends with `cargo xtask
ci` green and a DEC entry.

### WP-A1 cache-i18n (effort S, risk L) - first

- Under `ssr`, memoise `I18n` per (locale, brand overrides) in a `OnceLock`-guarded map in
  web/src/i18n.rs; `ClientConfig::i18n()` looks it up. Hydration path unchanged.
- Acceptance: a server test renders two pages and asserts the bundle is built once (expose a
  counter under `cfg(test)`); SSR p95 of Home in `apps/server/tests/latency.rs` drops (record
  before/after in the DEC).

### WP-A2 batch-dto-builders (effort M, risk L)

- `assets::AssetService::get_many`, `dto::items(s, &[CatalogItem])` prefetching covers;
  `UserRepo::get_many`; rewrite `browse::row`/`library_query`/Home, `sharing::sessions`,
  `admin::users`, `people::person_page` to batch.
- Acceptance: a store-pg test counts queries for a 48-row library page (use a
  `sqlx` statement log or a counting `PgPool` wrapper in testkit) and asserts a constant
  number independent of row count; load.rs library/Home p95 recorded.

### WP-C2 arch-rules-and-leaves (effort S-M, risk L) - before any split

- Move `StorageConfig`/`ByteSize` into `config`; `assets` depends on `config` (or a new leaf
  `storage-config`). Remove `config -> assets`.
- Move `server::escape` into a crate without Leptos (`admin-shell`, depending on auth, app,
  store-pg, config); `cli` drops `server`.
- Rename our `config` crate to `app-config` (package name; the directory can stay).
- Extend xtask/src/arch.rs: `config`, `telemetry`, `brand`, `contracts`, `ui`, `web-state`
  (later) are leaves or near-leaves; only `server` depends on `web`; `cli` must not reach
  `leptos`; `testkit` only as dev-dependency; `api` may not be depended on by `web`. Update
  the stale `FEATURE_GATED_DEPS` entry (web/app) to whatever WP-B2 decides.
- Acceptance: `cargo test -p xtask` passes with the new rules; `cargo build -p cli` no longer
  compiles `web`/`leptos` (check `cargo tree -p cli -i leptos` is empty).

### WP-B3 split-api-into-ops-and-app-admin (effort L, risk M)

- As in 4.2: `ops` (shared operations + DTO mapping + ApiError sans axum), `app-admin`
  (settings model, validation, provenance, probes, sources, setup, mail, usage; `AdminStore`
  port moves here), `api` (axum, OpenAPI). `ApiState` becomes `OpsState` + `AdminState`;
  `api::ApiState` wraps them.
- Acceptance: admin settings validation has unit tests on `MemAdminStore` without HTTP
  (at least: env precedence, lock-out rules, mail validation); api/tests still green;
  `cargo tree -p ops -i axum` empty.

### WP-B2 remove-backend-pass-through (effort M, risk M) - after B3

- Under `ssr`, server fns in `web` take `Arc<OpsState>` / `Arc<AdminState>` from the Leptos
  context and call `ops::*` / `app_admin::*` directly; `WebBackend`, `WebAdmin`, `Backend`,
  `AdminBackend` deleted (about 1,250 lines). `ShellSession`/`resolve_shell_session` move to
  apps/server (they are axum middleware). `web` gets optional deps `ops`, `app-admin` under
  `ssr` (arch rule: feature-gated, like the old `app` rule). Record the reversal of DEC-162.
- Acceptance: every server test and e2e green; `rg "trait Backend|trait AdminBackend"` empty;
  line count of apps/server/src drops by about 870.

### WP-B1 split-web (effort L, risk M)

- Section 3.3/3.4 in order: web-state (tests out of Leptos), web-shell, area crates
  (admin, people, activity, account, library), web-gallery, thin web.
- Acceptance: `/usr/bin/time -v cargo build --release -p <each web crate>` peak RSS recorded;
  the largest is at most half of today's `web` figure (measure today's first); `cargo leptos
  build --release` wall time recorded; wasm size within 2 % of today; `cargo test -p web-state`
  completes without compiling `leptos` (`cargo tree -p web-state -i leptos` empty); e2e green.

### WP-C1 typed-contracts (effort M, risk M)

- Enums with `serde(rename_all = "snake_case")` in `contracts` for status, group, role,
  next-step action, sort, error code; `From` impls in `ops::dto`; `web` matches on enums;
  `Result<T, ServerFnError<ErrorDto>>` for server fns; delete `encode_error`/`error_code`/
  `retry_after_secs` string packing. Keep the wire format identical (same strings) so REST
  clients see no change; bump the OpenAPI doc.
- Acceptance: `rg '"in_progress"|"completed"|"planned"' crates/web*/src` matches only
  tests; OpenAPI diff shows enums where strings were; e2e green.

### WP-D1 decompose-server-build (effort M, risk L)

- `wiring/` modules; `Server::connect_only` for tests; probes and counter sync moved to
  `run_*`; `Server` fields private where nothing outside needs them.
- Acceptance: `Server::build` under 100 lines without `allow(too_many_lines)`; the
  server test suite wall time recorded before/after.

### WP-D2 port-contract-suite (effort M, risk L)

- `testkit::repo_contract!` macro generating the same assertions for `MemStore` and
  `PgStore` (including `last_event_at`/`plays` after progress writes, stats dirty marks via
  the `ProgressRepo`/stats API, cursor paging, tag counts). Implement what MemStore lacks.
- Acceptance: the suite runs in `store-pg/tests` (Pg) and `testkit` or `app/tests` (Mem) and
  passes on both.

### WP-D3 migration-conventions (effort S, risk L)

- Topic-only names; `-- no-transaction` + `CONCURRENTLY` rule for new indexes on the three
  large tables; batched backfills; a doc page with the final text of the trigger functions;
  decide and record the baseline squash before the first external deployment.
- Acceptance: a `docs/modules/ROOT/pages/migrations.adoc` exists and xtask `ci` has a lint
  that new migration file names match `^\d{4}_[a-z_]+\.sql$` and contain no `phase`.

### WP-C3 dependency-hygiene (effort S-M, risk L)

- `notify` on workspace `hmac`/`sha2`; `chrono-tz`, `uuid` via `workspace = true` everywhere;
  `rsa` optional in `testkit`; evaluate replacing `meilisearch-sdk` by a thin client over
  `HttpFetch` (removes reqwest 0.12 and brings search under the governed client); drop or CI-
  compile `web/csr`.
- Acceptance: duplicate count from the lock-file script (section 5.3) recorded before/after;
  `cargo deny check` green.

### WP-E1 wasm-link-profile (effort S, risk L)

- Measure `wasm-release` with `lto = "thin"` and with `codegen-units = 16`: peak RSS of the
  wasm build step, size before/after wasm-opt. Keep fat LTO only if the size difference
  matters to the owner (record the numbers).
- Acceptance: a DEC with the four measurements and the chosen profile.

### Order and dependencies

A1, A2 (independent quick wins) -> C2 (rules first, so the later moves are enforced) -> B3 ->
B2 -> B1 -> C1 -> D1 -> D2 -> D3 -> C3 -> E1. B1 can start its first step (web-state) in
parallel with B3 since they touch different crates, but only one cargo build runs at a time on
the owner's host.

### What I would not do

- Do not merge adapter crates to "reduce crate count"; they are small, compile in parallel,
  and the boundaries are what keeps `app` testable.
- Do not move the stats triggers out of Postgres; the dual-write alternative is worse.
- Do not introduce a generic "service layer" trait for `ops`; plain functions over a state
  struct (what `api::ops` already is) are the right shape.
