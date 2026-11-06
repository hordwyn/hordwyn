# Gate G1: architecture review (D-15)

Date: 2026-11-06. State reviewed: `main` after M08 plus the M07 work in progress (one kind,
`movie`, end to end: TMDB search -> catalog -> library -> progress -> web page).

**Reviewer and independence.** KICKOFF asks for a `claude-fable-5-1` reviewer that has not
seen the work. On the owner's instruction (2026-11-06, DEC-051) this session runs the reviews
itself. M00-M03 and M06 were produced by a different session and are reviewed independently;
M04, M05, M07 and M08 were written by this session, so those parts are a **self-review** and
should be re-checked by an independent reviewer when budget allows (listed as an open item in
the report).

Evidence is cited as `path:line` or test names. Findings are numbered `G1-n`; each has an
action and a DECISIONS entry (DEC-098 onwards).

## 1. Do `domain` and `app` stay free of framework/IO crates? Is the rule test real?

Yes. `xtask/src/arch.rs` builds a graph from `cargo metadata` and checks normal dependencies
**transitively**: `domain` and `app` must not reach sqlx, axum, leptos, reqwest, opendal,
meilisearch-sdk or wasmtime; `domain` additionally not tokio or async-trait. It also checks
allowed workspace deps (`ui` none, `web` only ui/contracts/brand and `app` behind `ssr`) and
forbids direct HTTP clients in `connectors`, `importers`, `integrations`, `notify`, `assets`.
The rules have unit tests on hand-built graphs (`xtask/src/arch.rs` tests), so the checker
itself is tested, and `xtask/tests/architecture.rs` runs it on the real workspace in CI.
`app`'s actual deps: domain, async-trait, thiserror, serde, serde_json, uuid, chrono.

## 2. Is `KindDescriptor` + typed `KindModule` + `attributes jsonb` sound? What breaks?

Sound for compiled-in kinds. `crates/kinds/tests/dummy_kind.rs` registers a new kind inside
the test and stores items with validated attributes without touching `domain`, `store-pg`
migrations or `web` routes (routes are generic: `/items/:id`). What breaks or degrades:

- **G1-1** Labels: the UI translates `kind-<id>`; a new kind without a message shows the raw
  key. Action: fall back to a humanised id (applied in M07 follow-up, DEC-098).
- **G1-2** Runtime kinds (Tier 2 WASM plugins) cannot be validated: validation deserialises
  into the Rust type (DEC-016); there is no JSON-Schema validator for kinds that only ship a
  schema. Action: deferred to the WASM milestone; the `KindCatalog` port already returns a
  validated value, so a schema-based validator can be added behind it (DEC-099).
- Connectors map provider data per kind (`connectors` depends on `kinds`), so a new kind
  needs connector code; that is expected.

## 3. Can the connector capability traits be expressed one-to-one as a WIT world?

Almost. Draft:

```wit
package app:connector@0.1.0;

interface types {
  enum lane { interactive, user-import, background-refresh, prefetch }
  record http-request { connector: string, op: string, method: string, url: string,
    headers: list<tuple<string, string>>, body: option<list<u8>>, lane: lane }
  record http-response { status: u16, headers: list<tuple<string, string>>, body: list<u8> }
  variant http-error { blocked(string), circuit-open, rate-limited(u64), network(string) }
  enum capability { search, details, people, groups, trending, translations, images }
  record rate-limit { per-second: u32, burst: u32, max-concurrency: u32 }
  variant auth { none, api-key(string), bearer(string) }
  record attribution { text: string, url: string, licence: string }
  record connector-info { id: string, name: string, kinds: list<string>,
    capabilities: list<capability>, rate-limit: rate-limit, auth: auth,
    attribution: attribution, hosts: list<string>, cache-ttl-secs: u64 }
  record external-id { namespace: string, value: string }
  record search-request { kind: string, query: string, language: option<string>, limit: u32 }
  record details-request { kind: string, id: string, language: option<string> }
  enum image-kind { cover, backdrop, person }
  record image-ref { kind: image-kind, url: string, width: option<u32>, height: option<u32> }
  record snapshot { connector: string, kind: string, external-ids: list<external-id>,
    title: string, original-title: option<string>, year: option<s32>,
    description: option<string>, attributes-json: string, images: list<image-ref>,
    complete: bool }
  variant connector-error { http(http-error), status(u16), not-found,
    missing-credentials(string), parse(string), unsupported(string) }
}

interface host {
  use types.{http-request, http-response, http-error};
  fetch: func(req: http-request) -> result<http-response, http-error>;
}

world connector {
  use types.{connector-info, search-request, details-request, snapshot, image-ref,
    connector-error, lane};
  import host;
  export info: func() -> connector-info;
  export search: func(req: search-request, lane: lane) -> result<list<snapshot>, connector-error>;
  export details: func(req: details-request, lane: lane) -> result<snapshot, connector-error>;
  export images: func(req: details-request, lane: lane) -> result<list<image-ref>, connector-error>;
  export kind-descriptor-json: func() -> option<string>;
}
```

Mismatches:

- `Snapshot.attributes` is a `serde_json::Value`; WIT has no dynamic JSON type -> a JSON
  string (`attributes-json`). Acceptable.
- Capabilities are optional traits (`as_search()` returns `None`); a WIT world cannot have
  optional exports -> every export exists and returns `unsupported`, the host uses
  `info().capabilities` to decide what to call. The host adapter maps this back onto the
  capability traits one-to-one.
- `Ctx { http, lane }` becomes the `host.fetch` import plus a `lane` parameter.
- **G1-3** Credentials: today connectors put the `Authorization` header in the request
  themselves (`Tmdb` holds the token). A plugin would need the secret. Action: the governed
  client injects credentials from `ConnectorInfo.auth` and config, connectors stop holding
  secrets (applied with M09, DEC-100).
- Async: the traits are `async`; with wasmtime's async support a synchronous-looking WIT call
  maps to an async host call. No callbacks are needed.

## 4. Is every outgoing HTTP call routed through governor + cache + allowlist?

For providers, yes: connectors only see `Ctx.http` (the `HttpFetch` port), the architecture
test forbids HTTP client crates in connector-like crates, image downloads use `HttpFetch`
with the connector's policy (`assets/src/service.rs` `fetch`), and OIDC uses `HttpFetch` as
connector `oidc` (`auth/src/oidc.rs`). The governed client rejects unknown connectors and
non-allowlisted hosts before any I/O (`catalog-core/tests/governed.rs`).

**G1-4** Not governed: opendal's S3 traffic and meilisearch-sdk (both bring their own
reqwest). They talk to the operator's own infrastructure, not to providers, and a WASM
plugin cannot reach them. Accepted and documented (DEC-101).

## 5. Does any request handler await a connector call or sleep?

No. Handlers in `crates/api/src/routes.rs` call `api::ops`: item detail reads the store and
enqueues `catalog.refresh` (`App::item_detail`); search answers from the local index and
spawns the fan-out (`ops::search`, `tokio::spawn`); `/assets/...` reads or redirects and
enqueues (`AssetService::serve`). Refreshes, image fetches and eviction run only in worker
handlers (`apps/server/src/handlers.rs`). The only `sleep` outside tests is the worker's
cron ticker. Measured: item detail 5 ms and search 2 ms with a connector that takes 10 s
(`apps/server/tests/latency.rs`).

**G1-5** Exception: the OIDC start and callback handlers await the identity provider
(discovery is cached after the first call). That is inherent to a sign-in redirect and not
a page view; accepted (DEC-102).

## 6. Storage budget: can any write path bypass `StorageBudget`?

No path in the code: the opendal store is `pub(crate)` in `assets` (`backend.rs`), the
budget implements `BlobStore` and is the store handed to the HTTP cache
(`apps/server/src/lib.rs` `governed_http`), uploads and image variants go through
`StorageBudget::write`. Postgres and Meilisearch are reported, not budgeted (D-8).
Residual risk: with several processes the cap can be exceeded by one object per process
(DEC-059). Tests: `assets/tests/budget.rs`, `service.rs` (hard cap: nothing written,
requests redirect).

## 7. Are ports too fine-grained or too coarse?

Mostly right. Repository ports per aggregate have Postgres and in-memory adapters and real
test value (every `app` use-case is tested without Postgres). `ObjectIndex`, `AssetRepo`,
`AuthStore`, `HttpCacheIndex`: memory + Postgres, used by property tests and unit tests.
`KindCatalog` has one production adapter but the dummy-kind test uses it.
`web::backend::Backend` has one implementation and no fake; it exists to keep `web` free of
`api`/`auth` (dependency direction), which is a valid reason. `Notifier` is declared but
unused until M13.

**G1-6** `JobQueue` is a port but `jobs::Worker` is used directly by the server; fine
(worker is infrastructure, not a use-case).

## 8. Companion/user model: promotion, demotion, merge without rewriting history?

Yes. Participants reference companions only (`domain/src/session.rs`). Merge sets
`merged_into` on the removed companion and keeps sessions untouched; readers resolve the
chain (`app/tests/companions.rs::merge_does_not_rewrite_history`). Promotion creates an
invite; accepting creates the user and sets `linked_user` on the existing companion, and the
new user sees shared sessions immediately
(`promote_invite_creates_linked_user_who_sees_shared_sessions`). Deleting a user demotes
other owners' companions to a snapshot (`deleting_a_user_demotes_companions_linked_to_them`).
The M05 sign-up uses the same `accept_invite` (`auth/src/lib.rs`).

## 9. Is hexagonal architecture justified, crate by crate?

- `domain`, `app`: yes; the whole M01 suite runs without IO, and the same core can run in a
  Tauri client against SQLite.
- `store-pg`, `search`, `assets`, `auth`, `jobs`, `realtime`, `catalog-core`: adapters with
  real second implementations (memory) used in tests. Justified.
- `contracts`: plain DTOs, no ports; right.
- `api` + `web::backend`: thin. `api::ops` keeps REST and server functions on one code path.
- Ceremony: `telemetry` (one function) and `brand` (one struct) are tiny crates; harmless.
  `config` depends on `assets` for `StorageConfig`: inverted but simple (config aggregates
  the adapters' sections); accepted (DEC-103).
- Missing: nothing structural for v1.

## 10. DRY: types across `domain`, `contracts` and DB rows

- Domain <-> DB rows (`store-pg` row structs + `TryFrom`): worth it (column types, `jsonb`,
  HLC text encoding differ from domain types).
- Domain <-> contracts (`api/src/dto.rs`): worth it (wire format must stay stable and
  wasm-safe; domain types contain LWW wrappers and HLC stamps clients should not see).
- `assets::AssetRecord` exists only in `assets`; no domain twin. Good.
- **G1-7** Collapse candidate: `api::routes` request bodies (`AddDto`, `StatusDto`,
  `RatingDto`, `NameDto`) are private structs next to their handlers; moving them to
  `contracts` would let the web client and scripts share them. Low value now; deferred
  (DEC-104).

## 11. Crate count and boundaries

24 crates. Compile time is the cost that matters on small hosts (a cold workspace build
takes about 20 minutes on 2 vCPUs). Proposals:

- Keep `domain`/`app`/`contracts` separate (Tauri reuse).
- Keep `ui` separate from `web` (reusable, wasm tests).
- Merge `telemetry` into `config`? No: both are tiny; merging saves nothing measurable.
- `importers`, `integrations`, `notify` are still stubs; keep as planned (features per
  provider).
- No split proposals.

## 12. Single biggest risk and a cheaper alternative

**Build and tooling weight of the full-stack Rust web app** (Leptos SSR + hydrate + wasm +
Tailwind, 24 crates, Postgres and Meilisearch in tests). On the build host a full CI run takes
20 minutes and the target directory filled a 20 GB disk three times (DEC-055, DEC-091).
Every UI change pays a wasm and SSR rebuild. This slows iteration more than any runtime risk.

Cheaper alternatives, in order of disruption:

1. Keep the design, but cache aggressively in CI (per-job target caches, done) and keep the
   dev profile lean (done); run Playwright only in CI and in `cargo xtask e2e`.
2. Build `web` in CSR mode for development (`cargo leptos watch` with `csr`) and SSR only for
   release and e2e.
3. If iteration speed stays the bottleneck: drop SSR for authenticated pages (they need the
   session anyway) and keep SSR only for public pages.

The runtime risk worth naming second is the **Postgres-centric worker** (job queue, outbox,
LISTEN/NOTIFY, search fallback all on one database): simple and transactional, but one hot
database. It is the right trade for a self-hosted v1.

## Findings and actions

| Id | Finding | Action | DEC |
|---|---|---|---|
| G1-1 | Unknown kinds show raw `kind-<id>` labels | Fallback to a humanised id | DEC-098 (applied) |
| G1-2 | No schema validation for runtime kinds | Deferred to the WASM milestone | DEC-099 |
| G1-3 | Connectors hold provider secrets | Governed client injects credentials (M09) | DEC-100 |
| G1-4 | S3 and Meilisearch traffic not governed | Accepted: own infrastructure | DEC-101 |
| G1-5 | OIDC handlers await the IdP | Accepted: sign-in redirect, cached discovery | DEC-102 |
| G1-6 | Worker used directly, not via port | Accepted | - |
| G1-7 | REST request bodies not in `contracts` | Deferred | DEC-104 |
| G1-8 | Self-review of M04/M05/M07/M08 | Independent review when budget allows | DEC-051 |
