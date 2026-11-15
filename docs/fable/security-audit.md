STATUS: COMPLETE (2026-11-09)

# kithkeep whole-system security audit

Auditor: independent (did not write any of the code). Scope: repository at branch `main`
(= tag `ux4` plus commit f1eeefa, Docker fix), read-only review of source, migrations,
Docker/compose files, CI and docs. Nothing was compiled or executed. Paths are relative to
the repo root; Rust paths drop the `src/rust/` prefix. Line numbers were read from the
files on 2026-11-09; where a statement rests on library behaviour outside the repo
(server_fn, reqwest, axum) the registry source or the documented default is named and the
statement is marked "not executed".

Earlier per-phase reviews live in `docs/reviews/*.md`; this audit does not repeat findings
that were fixed there, but re-verifies the large ones (section 6).

## Contents

1. System overview and trust boundaries
2. Threat model per boundary (hosted vs home mode)
3. Route authorization matrix
4. Deep checks
5. Findings (SEC-n)
6. What was checked and holds; what was not checked
7. Prioritised fix plan

## 1. System overview and trust boundaries

One Rust binary (`apps/server`, roles `all|web|worker`) serves:

- the Leptos SSR app and its server functions (`/_fn/*`, `apps/server/src/lib.rs:798-826`),
  with the web crate's `Backend`/`AdminBackend` traits implemented by `web_backend.rs` and
  `web_admin.rs` over the same `api::ApiState` the REST layer uses;
- the REST API `/api/v1` (`crates/api/src/routes.rs:61-145` plus the admin, people, goals,
  oidc, sso and usage route sets), OpenAPI at `/api/v1/openapi.json`;
- SSE at `/api/v1/events` (`realtime::Bus::sse`), immutable assets at `/assets/{hash}/{file}`,
  brand fonts, `/healthz`;
- the job worker and cron (imports, refreshes, re-wrap, retention, eviction, notifications);
- a CLI (`apps/cli`) for shell-only escape hatches (promote, reset-password, reset-totp,
  reindex, secrets rotate/prune).

Stores and peers: Postgres (all state, job queue, outbox, audit, secrets), Meilisearch or
the Postgres fallback (search), filesystem or S3 (assets, HTTP cache blobs, import files),
outbound HTTP through one governed client (`catalog_core::GovernedHttp`, allowlisted per
connector: TMDB, TheTVDB, IGDB, Podcast Index, OIDC) plus a second "plain" client for
user-configured notification targets (ntfy/gotify/Discord/Matrix/Apprise/webhooks,
`notify::channels::http`), SMTP (lettre), OIDC providers (discovery, token, JWKS, UserInfo,
back-channel logout), and inbound webhooks (`POST /api/v1/hooks/watched`, bearer `itk_`
token).

Trust boundaries (B1..B10):

| Id | Boundary | Trust |
|---|---|---|
| B1 | Browser (signed-out visitor) -> server | untrusted; may hold share links, invite tokens, reset tokens |
| B2 | Browser (member) -> server | authenticated by session cookie or bearer; must not reach other members' data |
| B3 | Admin -> server | trusted for instance operation; gated actions need a fresh proof (DEC-191); must not read members' private notes or credentials |
| B4 | Server -> Postgres / Meilisearch / S3 | trusted infrastructure, same operator |
| B5 | Server -> providers (TMDB etc.) | semi-trusted data source; responses are untrusted input |
| B6 | Server -> OIDC provider | the provider is trusted to assert identities for *its* issuer only |
| B7 | OIDC provider / integration sender -> server (callback, back-channel logout, webhook) | untrusted until the signature / token verifies |
| B8 | Server -> user-chosen URLs (notification channels, admin-tested base URLs, SMTP host, provider endpoints) | hostile by default: SSRF surface |
| B9 | Hosting operator (compose, env, key file, volumes) | fully trusted; the key file and `APP__SECRETS__KEYS` are the root of trust |
| B10 | Other members on the same instance (hosted mode) | mutually untrusted; sharing is opt-in (link requests, instance-visible lists, shared sessions) |

Modes: `instance.hosting = home` (a household, everyone is semi-trusted) or `hosted` (many
mutually untrusted users; sign-up by invite, fair-share lanes). The code does not branch on
the mode for authorization (good: the same ownership checks apply in both), so the hosted
threat model is the one audited against.

## 2. Threat model

STRIDE per boundary, with the controls found and the gaps (gaps are numbered in section 5).

**B1 visitor -> server.**
- Spoofing: login CSRF is stopped by the Origin guard (`apps/server/src/csrf.rs:20-40`);
  password guessing is throttled per name in memory (`crates/auth/src/lib.rs:192-249`);
  username enumeration is limited to 30 checks/min per instance (DEC-184). Gap: throttles are
  per process and per name, not per client address, every attempt costs one argon2
  verification, and anyone can lock a name for 15 minutes (SEC-5).
- Tampering: OIDC pending state is HMAC-signed and single use (`crates/auth/src/oidc/cookie.rs`).
- Information disclosure: `/api/v1/sources`, `/notifications/web-push-key`, `/openapi.json`,
  `/auth/oidc/providers`, `/setup` state are public by design (low value). Share links expose a
  list by a 256-bit token (4.10).
- Denial of service: JSON bodies 2 MiB, imports 64 MiB on the REST router; the server-function
  router has no body limit (SEC-2); no per-IP rate limiting anywhere (SEC-5).

**B2 member -> server.**
- Elevation: every REST handler takes `Authed` and passes the `Actor` into `app` use-cases,
  which check `owner == actor.user_id` (section 3, 4.7). No gap found.
- Tampering: HLC last-writer-wins in sync (`crates/api/src/sync.rs`); undo tokens are signed,
  user-bound and expire (`crates/api/src/bulk.rs:33-45`).
- Information disclosure: search documents carry `owner` and private indexes are filtered
  server-side (`crates/search/src/meili.rs:174-183`, `pg.rs:112-143`); SSE delivers `admin.*`
  only to admins (`crates/realtime/src/lib.rs:102-107`).
- Repudiation: audit rows for sign-in related and admin changes with client address and
  request id (`crates/api/src/request_meta.rs`); member actions are not audited (by design).

**B3 admin -> server.** Admin functions call `require_admin` (`crates/api/src/admin/mod.rs:289`);
sign-in related changes call `fresh` (DEC-191). The admin cannot read member credentials
(TheTVDB PIN is write-only, per user). Gap: an admin can point the server at any host through
the SMTP settings and OIDC issuers, and `Test`/discovery calls it from the server (SEC-4).

**B5/B6/B7 providers and OIDC.** PKCE S256, `state`, `nonce`, issuer pinning in the signed
cookie, RFC 9207 `iss`, single-use claim table, back-channel logout token verified against the
JWKS with an asymmetric-only `alg` allowlist, `jti` replay cache and a per-process budget. Gap:
the governed client follows redirects and the OIDC allowlist is extended with whatever hosts the
discovery document names, including private addresses (SEC-4).

**B8 server -> user chosen URLs.** `notify.allow_private_targets` (default false) blocks
private ranges for notification channels at connect time via a resolver
(`crates/catalog-core/src/plain.rs:17-32`) and for IP literals (`notify/src/channels/http.rs:81-108`),
redirects are off. Holds. The admin's SMTP host and the OIDC endpoints are not subject to it
(SEC-4).

**B9 operator.** Key file generated 0600 under `/data` (`crates/auth/src/secrets.rs:591-606`);
compose ships default credentials for Postgres, Meilisearch and S3 and publishes port 3000 on
every interface without TLS (SEC-6); the application sets no `HSTS`/`CSP`/frame headers and the
docs have no reverse-proxy section (SEC-1), so TLS termination and headers are silently left
to the operator.

**B10 other members.** Sharing is opt-in; per-user data is filtered by `user_id` in every
query reviewed. Deletion of a user cascades through FKs for the relational data but leaves
search documents, audit names and import files behind (SEC-3).

## 3. Route authorization matrix

Legend. Auth: `-` none, `S` session (cookie or bearer, `Authed`), `S+T` session with token for
a gate (`AuthedToken`), `I` integration bearer token, `C` signed cookie/ticket, `K` share/
invite/reset token in the URL or body. Role: `any`, `member`, `admin`. CSRF: `G` = covered by
the Origin guard (`apps/server/src/csrf.rs`, every state-changing method on both routers,
`apps/server/src/lib.rs:440,827`), `n/a` for GET/HEAD. Re-auth: `F` = `require_fresh`
(DEC-191). RL: rate limit (`-` none; `name` = per-username sign-in throttle; `inst` = per
instance per minute; `user` = per user per minute; `proc` = per process per minute). Scope:
whose data the handler can touch. Owner: where ownership is enforced (`app` = in the `app`
use-case by `actor.user_id`; `sql` = in the repository query; `n/a` = no per-user data).

### 3.1 Server-level routes (`apps/server/src/lib.rs`)

| Route | Auth | Role | CSRF | Re-auth | RL | Scope | Owner |
|---|---|---|---|---|---|---|---|
| `GET /healthz` | - | any | n/a | - | - | none | n/a |
| `GET /assets/{hash}/{file}` | - | any | n/a | - | - | provider images by content hash | n/a; hash must exist in the asset repo (`crates/assets/src/service.rs:481`), variant/format allowlisted (475-480) |
| `GET /brand/fonts/{fp}/{file}` | - | any | n/a | - | - | brand fonts declared in `brand.toml` | n/a |
| `ANY /_fn/{*fn}` (109 server fns) | per fn | per fn | G | per fn | as REST | as REST | delegates to the same `api::*` functions through `Backend`/`AdminBackend`; see 3.9. **No body limit (SEC-2).** |
| pages (Leptos routes, fallback 404) | shell session resolved | any | n/a | - | - | SSR of the signed-in shell | pages fetch data through server fns |
| `GET /api/v1/openapi.json` | - | any | n/a | - | - | none | n/a |

### 3.2 `crates/api/src/routes.rs`

| Route | Auth | Role | CSRF | Re-auth | RL | Scope | Owner |
|---|---|---|---|---|---|---|---|
| `POST /auth/sign-up` | -/K(invite) | any | G | - | - | creates own account | `Auth::sign_up` gate (`auth/src/lib.rs:471-488`) |
| `POST /auth/sign-in` | - | any | G | - | name | own session | throttle (`lib.rs:548-557`), `sso_only` in auto mode |
| `POST /auth/username-available` | -/K | any | G | - | inst 30/min | none | gate first (`lib.rs:508`) |
| `POST /auth/sign-out` | S (optional) | any | G | - | - | own session | token hash of the caller |
| `GET /auth/me` | S | any | n/a | - | - | own user | actor |
| `POST /auth/totp` | S+T | any | G | F | - | own credentials | `admin::totp_begin` requires fresh |
| `POST /auth/totp/confirm` | S | any | G | - | - | own credentials | actor |
| `GET /items/{id}` | S | any | n/a | - | - | shared catalog + refresh queue | n/a (catalog is shared) |
| `GET /search` | S | any | n/a | - | governed client | catalog index + connectors with the caller's own keys | search `viewer` |
| `GET /sources` | - | any | n/a | - | - | attribution texts | n/a |
| `GET /library`, `POST /library` | S | any | G | - | - | own library | `app` (`library_page(a)`, `add_to_library(a)`) |
| `PUT /library/{item}/status`, `/rating`, `/note` | S | any | G | - | - | own entry | `app` |
| `GET/POST /items/{id}/progress`, `POST /items/{id}/plays` | S | any | G | - | - | own history; companions must be own (`app/src/sessions.rs:219-220`) | `app` |
| `GET/POST /companions`, `PATCH/DELETE /companions/{id}` | S | any | G | - | - | own people | `app` (`companions.rs:57,312,439`) |
| `GET/POST /lists`, `POST /lists/{id}/items` | S | any | G | - | - | own lists | `app` |
| `POST /companions/{id}/link-requests`, `GET /link-requests`, `POST /link-requests/{id}/accept|decline` | S | any | G | - | - | own companion / requests addressed to caller | `app` (`companions.rs:185-245`) |
| `POST /companions/{id}/unlink`, `/merge`, `/invite` | S | any | G | - | - | own or linked companion | `app` (`companions.rs:312,439`); admins may promote any companion (439) |
| `GET /sessions`, `GET /sessions/shared`, `POST /sessions/{id}/claim` | S | any | G | - | - | own sessions; sessions shared with caller honour `share_sessions` (`sessions.rs:176,254`) | `app` |
| `PUT /auth/me/privacy`, `/locale`, `/time-zone` | S | any | G | - | - | own user | actor |
| `POST /auth/reset-password` | K | any | G | - | - | the token's user | token hash, single use (`auth/src/recovery.rs:47-68`) |
| `POST /lists/{id}/share` | S | any | G | - | - | own list | `app` |
| `GET /shared/lists/{token}` | K | any | n/a | - | - | one link-shared list (names, items; no notes) | token lookup |
| `GET /notifications/events` | S | any | n/a | - | - | catalogue | role filters admin topics |
| `GET /notifications/inbox`, `POST .../{id}/read`, `/read-all` | S | any | G | - | - | own inbox | `notifications::*` by actor |
| `GET/POST /notifications/channels`, `PUT/DELETE .../{id}`, `POST .../{id}/test` | S | any | G | - | - | own channels; URLs checked (B8) | `own_channel` (`api/src/notifications.rs:235`) |
| `GET/PUT /notifications/subscriptions`, `DELETE .../{id}` | S | any (admin for `instance_default`) | G | - | - | own subscriptions / instance defaults | `notifications::put_subscription` |
| `GET /notifications/deliveries` | S | any | n/a | - | - | own deliveries | actor |
| `GET/PUT /notifications/templates` | S | admin for PUT | G | - | - | instance texts | `notifications::set_template` |
| `GET /notifications/web-push-key` | - | any | n/a | - | - | public key (none yet) | n/a |
| `GET/POST /integrations`, `DELETE /integrations/{id}`, `POST /integrations/{id}/test` | S | any | G | - | - | own integrations | `Integrations::*(a, ...)` |
| `POST /hooks/watched` | I | sender | n/a (no cookie) | - | - | the integration owner's library | token hash lookup (`integrations/src/lib.rs:364-371`, only `token_hash` is stored) |
| `GET /sync/changes` | S | any | n/a | - | - | own change feed; entries re-filtered by `user_id` (`sync.rs:155`) | sql + check |
| `POST /sync/commands` | S | any | G | - | - | own library | `app` |
| `POST /imports` (64 MiB), `GET /imports`, `GET /imports/{id}`, `POST .../run`, `/cancel`, `/match` | S | any | G | - | storage budget | own imports (admins excluded, `importers/src/runner.rs:162`) | runner owner checks |
| `GET /export` | S | any | n/a | - | - | own library; CSV formula guard | actor |
| `GET /events` (SSE) | S | any | n/a | - | - | events addressed to the caller; `admin.*` to admins | `visible_to` |
| `GET /kinds`, `/library/counts`, `POST /library/query`, `GET /tags`, `PUT /tags/{tag}`, `POST/DELETE /library/{item}/tags[/{tag}]` | S | any | G | - | - | own library | `app`/sql by `actor.user_id` |
| `POST /progress/{event}/undo` | S | any | G | - | - | own events | `app/src/library.rs:278` filters `user_id` |
| `GET /activity`, `/stats/totals`, `/stats/series`, `/home`, `/items/{id}/page` | S | any | n/a | - | - | own data (+ shared sessions) | `app` |
| `POST /library/bulk`, `POST /library/bulk/undo`, `DELETE /library/{item}` | S | any | G | - | 500 ids | own library | `app::bulk(a)`; undo token user-bound (`bulk.rs:42-45`) |

### 3.3 `crates/api/src/admin_routes.rs`

| Route | Auth | Role | CSRF | Re-auth | RL | Scope | Owner |
|---|---|---|---|---|---|---|---|
| `GET /sources/status` | S | any (vocabulary by role) | n/a | - | - | source states | n/a |
| `POST /sources/ask-admin` | S | any | G | - | once per source/day | admin notification | n/a |
| `GET /me/credentials`, `PUT/DELETE /me/credentials/{source}` | S | any | G | - | - | own per-user key (encrypted) | `user_credentials(actor)` |
| `GET /auth/reauth` | S+T | any | n/a | - | - | own session | token |
| `POST /auth/reauth` | S+T | any | G | (is the proof) | name | own session; regenerated id | token |
| `GET /setup` | -/S | any before the first user, admin after | n/a | - | - | wizard state | `setup_state` (`admin/setup.rs:111-123`) |
| `POST /setup/account` | - | any, **only while no user exists** | G | - | - | first admin | `setup.rs:195-200` checks `user_count() > 0`; 404 after completion |
| `POST /setup/hosting`, `/sources`, `/complete` | S | admin | G | - | - | instance | `require_admin` |
| `GET /admin/overview`, `/settings`, `/users`, `/invites`, `/sources`, `/storage-jobs`, `/notification-defaults`, `/brand`, `/security`, `/audit`, `/audit.csv`, `/mail` | S | admin | n/a | - | - | instance | `require_admin` in each `admin::*` |
| `PUT /admin/settings` | S+T | admin | G | F for sign-in related keys | - | instance | `update_settings` |
| `PUT /admin/users/{id}/role`, `/disabled`, `DELETE /admin/users/{id}`, `POST .../reset-totp`, `DELETE .../identities/{identity}` | S+T | admin | G | F | - | any user | `require_admin` + lock-out guards (`self_*`, `last_admin`) |
| `POST /admin/invites`, `DELETE /admin/invites/{id}` | S | admin | G | - | - | invites | `require_admin` |
| `PUT/DELETE /admin/sources/{id}/credential`, `POST .../test`, `PUT .../licence`, `PUT .../enabled` | S | admin | G | - | governed client | instance keys (encrypted) | `require_admin` |
| `PUT /admin/mail/password` | S+T | admin | G | F | - | SMTP secret | `set_mail_password` |
| `POST /admin/mail/test` | S | admin | G | - | - | sends mail from the server to any address at the configured host (B8) | `require_admin` |
| `POST /admin/storage/evict`, `POST /admin/jobs/{id}/retry`, `DELETE /admin/jobs/{id}` | S | admin | G | - | - | storage/jobs | `require_admin` |
| `POST /admin/secrets/rewrap` | S | admin | G | - | - | every encrypted row | `require_admin` |

### 3.4 `crates/api/src/people_routes.rs`

| Route | Auth | Role | CSRF | Re-auth | RL | Scope | Owner |
|---|---|---|---|---|---|---|---|
| `GET /people`, `GET /people/{id}` | S | any | n/a | - | - | own people; foreign ids are `not_found` (`api/src/people.rs:155-157`) | `app` |
| `PATCH/DELETE /companions/{id}`, `POST /companions/{id}/unmerge` | S | any | G | - | - | own people | `app` |
| `GET /lists/overview` | S | any | n/a | - | - | own + instance-visible lists | `app/src/lists.rs:140-168` |
| `GET /lists/{id}` | S | any | n/a | - | - | own or `Instance` visibility (`lists.rs:43-50`) | `app` |
| `PATCH/DELETE /lists/{id}`, `DELETE/PUT .../items/{item}[/position]`, `PUT .../visibility` | S | any | G | - | - | own list | `app` |
| `POST /lists/{id}/library`, `POST /shared/lists/{token}/library` | S | any | G | - | - | readable list -> own library | `app` |

### 3.5 `crates/api/src/goals_routes.rs`

All seven routes (`/stats/goals[/{id}]`, `/stats/compare`, `/game-groups[/{id}]`) are `S`,
any role, `G` on writes, no gate, no rate limit, own data only
(`app/src/goals.rs:172`, `app/src/game_groups.rs:113,128`). Holds.

### 3.6 `crates/api/src/oidc_routes.rs`

| Route | Auth | Role | CSRF | Re-auth | RL | Scope | Owner |
|---|---|---|---|---|---|---|---|
| `GET /auth/oidc/providers` | - | any | n/a | - | - | enabled provider buttons | n/a |
| `GET /auth/oidc/start` | - (`interactive`,`silent`), S+F (`link`), S (`reauth`), S admin (`probe_*`) | per mode | n/a (GET sets the pending cookie only) | F for `link` | governed 5/s to provider | pending cookie | `oidc_routes.rs:169-186` |
| `GET /auth/oidc/callback` | C (signed pending cookie) + provider code | any | n/a | - | - | sign-in / link / reauth of the cookie's user | `take_pending` state + single use (`auth/src/oidc.rs:744-764`), `link`/`reauth` bound to the session (`resolve.rs:270`, `sso.rs:591`) |
| `GET /auth/oidc/link`, `POST /auth/oidc/link` | C (link ticket) | any | G | - | name (sign-in throttle) | the matched account, password required | `confirm_link` (`resolve.rs:436-472`) |
| `POST /auth/oidc/link/email`, `POST .../email/confirm` | C | any | G | - | per account (`EMAIL_LINKS_PER_WINDOW`) | emailed one-time link to the account's stored address | ticket + code, browser-bound |
| `GET /auth/me/identities`, `DELETE /auth/me/identities/{id}` | S / S+T | any | G | F | - | own identities | `unlink_identity` with `last_sign_in_method` guard |
| `GET /admin/oidc` | S | admin | n/a | - | - | providers | `require_admin` |
| `POST /admin/oidc/providers`, `PUT/DELETE .../{id}`, `PUT .../{id}/secret` | S+T | admin | G | F | - | providers (secret encrypted) | `require_admin`, env provider read-only |
| `POST /admin/oidc/providers/{id}/test` | S | admin | G | - | - | discovery fetch from the server (B8, SEC-4) | `require_admin` |

### 3.7 `crates/api/src/sso_routes.rs`

| Route | Auth | Role | CSRF | Re-auth | RL | Scope | Owner |
|---|---|---|---|---|---|---|---|
| `POST /auth/logout` | S (optional) | any | G | - | - | own session, then provider end-session | token |
| `POST /auth/oidc/backchannel-logout` | provider JWS | provider | n/a | - | proc 60/min | sessions by `sid`/`sub` of that provider | JWKS signature, `iss`+`aud` provider match, claims, `jti` (`sso.rs:616-660`) |
| `POST /auth/sign-in/local` | - | any (admins only succeed in auto mode) | G | - | name | own session; audited, admins notified | `password_sign_in(local=true)` |
| `PUT/GET /auth/reauth/pending` | S+T | any | G | - | 16 KiB | own session secret | token |

### 3.8 `crates/api/src/usage_routes.rs`

| Route | Auth | Role | CSRF | Re-auth | RL | Scope | Owner |
|---|---|---|---|---|---|---|---|
| `POST /usage/funnel` | S | any | G | - | user 30/min, allowlisted words | daily totals without user ids | n/a |
| `GET /admin/usage`, `/admin/usage.csv` | S | admin | n/a | - | - | totals | `require_admin` |

### 3.9 Server functions (`crates/web/src/api.rs`: 64, `admin_api.rs`: 45)

Every server function reads the session cookie from the request headers on the server
(`web/src/api.rs:31-38`) and calls the same `api::*`/`admin::*` functions through
`web_backend.rs`/`web_admin.rs`, so role and ownership are those of 3.2-3.8. The nine
functions without a session requirement mirror the public REST routes exactly:
`current_user`, `sign_in`, `sign_up`, `username_available`, `sources`, `shared_list`,
`setup_state`, `setup_account`, `sign_in_page`. Gated actions pass the token so
`require_fresh` applies. CSRF: the pages router is wrapped by the same guard
(`apps/server/src/lib.rs:827`). Gap: no request body limit on `/_fn/*` (SEC-2).

### 3.10 Gaps flagged by the matrix

- No route has a per-client-address limit; the only limits are per name / per user / per
  process (SEC-5).
- `/_fn/*` has no body limit (SEC-2).
- Admin-only outbound calls (`/admin/oidc/providers/{id}/test`, `/admin/mail/test`, provider
  discovery during `oidc_start`) reach any host the admin names (SEC-4).
- `POST /hooks/watched` returns storage error text to the sender on a 500 (SEC-9).

## 4. Deep checks

### 4.1 Passwords, sessions, cookies

- argon2id with the crate defaults (m=19456 KiB, t=2, p=1; RFC 9106 second option),
  `crates/auth/src/lib.rs:283-293`; a dummy hash is verified for unknown users (`lib.rs:575-583`),
  so user existence is not visible through timing. Minimum length 10, maximum 1024
  (`crates/contracts/src/auth.rs:17-18`). Password change ends every other session
  (`lib.rs:783`).
- Session tokens: 32 random bytes from `getrandom`, base64url, only the blake3 hash stored
  (`lib.rs:661-677`); sliding expiry extended at most once an hour (`lib.rs:699-703`), no
  absolute cap (SEC-10). Sign-in always creates a new token (no fixation); re-auth regenerates
  the id and moves the session secrets (`fresh.rs:138-157`).
- Cookie: `HttpOnly; SameSite=Lax; Path=/; Max-Age`, `Secure` when `auth.secure_cookies`
  (default true, admin-editable per OD-11) (`crates/api/src/routes.rs:147-158`). OIDC pending
  and link cookies are scoped to `Path=/api/v1/auth/oidc` (`oidc_routes.rs:30,55-61`). Holds.
- The session token is also returned in the sign-in JSON body for API clients; both the
  cookie and `Authorization: Bearer` are accepted (`extract.rs:16-26`). Acceptable; the
  bearer path is not subject to CSRF because no cookie is involved.

### 4.2 TOTP and recovery codes

- 20-byte seed (`lib.rs:724`), stored encrypted (purpose `totp_secret`, `auth/src/pg.rs:23-24`),
  plain column emptied by the start-up migration; replay of the same or an older step is
  refused (`lib.rs:631-634`). Enrolment needs a fresh proof (`routes.rs:250-255`).
- Recovery codes: 8 codes of 50 bits, Crockford base32, argon2-hashed, single use
  (`fresh.rs:43-70, 276-304`).
- `TotpLocked` when the seed's key version is not loaded: the account cannot sign in rather
  than skipping the factor (`lib.rs:605-607`). Good.

### 4.3 Re-auth window, throttles, lock-out guards

- 15 minutes sliding, 60 minutes cap (`fresh.rs:17-20, 81-88`); OIDC round trip needs
  `auth_time` within 300 s and the identity of the session's account (`fresh.rs:205-223`).
- Throttle: 10 failed sign-ins per lowercased name per 15 minutes, in-memory per process,
  capacity 10 000 names (`lib.rs:192-249`); the same budget covers password change, re-auth
  and "Is this your account?". No per-address budget, every attempt runs argon2 (SEC-5).
- Lock-out guards: `self_role_change`, `self_disable`, `self_delete`, `last_admin`,
  `last_sign_in_method`, and the provider role rule never demotes the last admin
  (`resolve.rs:231-234`). Holds.

### 4.4 OIDC / SSO

- Authorization code + PKCE S256 always, `state` and `nonce` random, kept in an HMAC-SHA256
  signed cookie derived by HKDF from the key ring (`cookie.rs:19-38, 154-213`); single use
  through `claim_once` (`oidc.rs:758-762`); `return_to` restricted to local paths
  (`cookie.rs:226-239`); mix-up defence by pinning the issuer in the cookie and checking RFC
  9207 `iss` (`oidc.rs:785-793`); ID token verified by `openidconnect` with the nonce; a UserInfo
  `sub` mismatch is refused (`oidc.rs:868-870`).
- Account resolution never auto-links by email (OD-15): a verified email match leads to a
  password prompt or an emailed one-time link for password-less accounts, throttled per
  account and bound to the browser's ticket (`resolve.rs:291-335`, `email_link.rs`).
- Back-channel logout: JWS with asymmetric `alg` only (`sso.rs:208-220`), provider found by
  `iss` and `aud` = client id, claims checked (`exp`, `iat`, `events`, no `nonce`, `sub` or
  `sid`), `jti` replay cache in memory and in a table (`sso.rs:616-660`), 60/min per process.
  Holds.
- Silent sign-in guard cookie, hint cookie (signed, 400 days, holds `login_hint`, SEC-11).
- `issuer_allowed` accepts `http://` issuers when the instance itself runs on `http://`
  (`oidc.rs:250-267`); fine for development, a foot-gun in production only if `public_url` is
  left at http (then everything is already unprotected).
- Gap (SEC-4): the OIDC connector's host allowlist is extended with every host named by the
  provider's discovery document (`oidc.rs:399-428, 546`) without a private-address check, and
  the governed `reqwest::Client` follows redirects by default (`catalog-core/src/http.rs:211-216`
  sets no redirect policy; `allows_host` runs only on the initial URL, `http.rs:504-518`).

### 4.5 Secrets at rest

- Envelope: AES-256-GCM, per-value DEK wrapped by the KEK version, format byte and version in
  the blob, AD bound to the slot (`crates/auth/src/secrets.rs:11-40, 289-370`). Key file
  written with mode 0600, warning when group/other readable (`secrets.rs:591-623`). Keys from
  file first, then `APP__SECRETS__KEYS`, else generated (OD-06/07). `secrets.ephemeral` never
  seals stored plain text (`apps/server/src/admin_wiring.rs:68-87`).
- Coverage: one `SecretRows` per purpose (`totp_secret`, `session_secret` (ID token + pending
  action), `channel_secret`, `connector credential`, `smtp_password`, OIDC `client_secret`),
  with a test that every `impl SecretRows` in the workspace is registered
  (`admin_wiring.rs:94-113, 420-472`). The nightly re-wrap job and `cli secrets prune` use the
  same list (UX3-1 fixed). Holds.
- Rotation: `cli secrets rotate` + SIGHUP reload, refused when a version in use would be
  dropped (`apps/server/src/main.rs:65-82`). Cookie/undo keys are HKDF-derived from the ring,
  every loaded version verifies (`cookie.rs:33-38`, `bulk.rs:47-65`). Holds.
- The key file lives on the same volume as the filesystem asset store (`/data`, Dockerfile);
  the database is a separate volume, so a copy of `/data` alone does not expose plaintext.
  Documented as one backup unit (`docs/modules/ROOT/pages/operations.adoc:30-31`).

### 4.6 SSRF

- Notification channels: `check_target` refuses non-http(s) schemes and private/loopback/
  link-local/CGNAT/unspecified addresses for IP literals; names are resolved by a `PublicOnly`
  resolver at connect time (no TOCTOU), redirects disabled, 5 s connect / 15 s total
  (`plain.rs:17-51`, `notify/src/channels/http.rs:75-108`, `domain/src/net.rs:8-28`). G2-7 fixed.
  `allow_private_targets` (default false) is the documented LAN switch.
- Connectors: fixed host allowlists per connector (`http.rs:129-138`); `connectors.*.base_url`
  is file/env configuration only (operator), not editable in the admin UI.
- OIDC: see 4.4 (SEC-4). SMTP: host/port editable by an admin, `send_test_mail` connects from
  the server (`crates/api/src/admin/mail.rs:247-275`) - an admin can probe internal SMTP ports;
  the result text is categorised before storage (UX4-22) but shown to the admin.
- Image fetches go through the governed client to allowlisted provider hosts only, with a
  25 MiB cap and decoder limits (`assets/src/service.rs:24,328`, `image.rs:76-83`).

### 4.7 Multi-tenancy isolation

- Library, tags, notes, progress, sessions, companions, lists, goals, game groups,
  integrations, channels, subscriptions, imports: every `app` use-case takes the `Actor` and
  filters by it (`app/src/companions.rs`, `sessions.rs`, `goals.rs`, `game_groups.rs`,
  `lists.rs:43-50`, `library.rs:278`); repository queries carry `user_id`/`owner_id`.
- Lists: `Instance` visibility is readable by every member (by design); `Link` visibility by
  token; private otherwise (`lists.rs:43-50`). Shared list DTO carries names and items, no notes
  (`sharing.rs:245-259`).
- Sessions: a linked participant sees another user's session only when that user's
  `share_sessions` is true (`sessions.rs:176,254`, `store-pg/src/repos/catalog.rs:424`).
- Search: `library` and `people` indexes are private; the server adds `owner = <viewer>` to
  every query and strips `owner` from hits (`meili.rs:174-183, 234-236`); the Postgres fallback
  does the same (`search/src/pg.rs:112,131,143`). Catalog index is shared by design.
- Sync: change rows are per user; entries are re-checked for `user_id` (`sync.rs:155`).
- SSE: `admin.*` to admins, otherwise audience list or everyone for shared catalog events
  (`realtime/src/lib.rs:102-107`).
- Usage funnel: no user or item column (`migrations/0025_usage_funnel.sql`), allowlisted words
  (`usage.rs:127-181`), 30 beacons/min per user. UX4-14 fixed.
- Admin reach: admins cannot read members' per-user credentials (write-only), import reports
  (`runner.rs:162`), or private notes through any admin route found. Admins may promote any
  companion to an invite (`companions.rs:439`); harmless.

### 4.8 Injection

- SQL: every query reviewed uses `sqlx::query!`/bound parameters; the only non-literal
  `sqlx::query(sql)` iterates three constant strings (`search/src/indexer.rs:134-146`). No
  `format!` with SQL keywords in the workspace. Meilisearch filters are quoted
  (`meili.rs:22-26`) with a test for the injection case.
- HTML: Leptos escapes text nodes; the three `inner_html` uses are the config JSON with `<`
  escaped as `<` (`web/src/app.rs:122-130,156`), the brand theme CSS from the operator's
  brand directory (`app.rs:161`) and vendored icon bodies (`ui/src/icon.rs:36`). The SSO
  interstitial escapes provider name and URL (`web/src/auth_redirect.rs:13-43`). No
  user-controlled raw HTML found.
- CSV: `importers::cell::guard` prefixes formula characters; audit, usage and library exports
  use it (`admin/mod.rs:1187-1194`, `usage.rs:269`). UX3-5 fixed.
- Headers: redirect targets come from `return_path`-validated local paths or provider URLs
  parsed by `url` (`oidc_routes.rs:64-73`); cookie values are base64url or signed blobs;
  `HeaderValue::from_str` failures fall back to an empty header rather than panicking.
- Logs: no `tracing` call found that prints a password, token or secret; `SessionToken`,
  `PendingLogin` and `Credentials` have redacting or no `Debug` (`lib.rs:261-265`,
  `cookie.rs:98-107`, `store.rs:14-15`). Provider error details are logged with
  `detail = %f.detail` (`oidc.rs:738`) - these are provider status lines, not tokens.
- Email: addresses and subjects go through lettre's typed builders; admin-chosen test
  recipient only.

### 4.9 File uploads and importers

- Imports: raw body up to 64 MiB on `/imports` only (`routes.rs:30,126-130`), stored in the
  storage budget (quota) and parsed in the worker; formats are an allowlist
  (`importers/src/lib.rs:346-358`); rows are collected in memory (a 64 MiB JSONL file becomes
  a `Vec` of rows - bounded by the body limit). No archives are accepted (no zip/tar/gzip
  crates in the workspace outside `ui`), so no decompression bombs.
- No user image uploads exist (avatars are provider images); `/assets` serves only hashes
  known to the asset repository with allowlisted variants/formats, so no path traversal.
- Import files of dry runs stay in storage until a real run (`runner.rs:135`); on user
  deletion the storage index row keeps the object with `owner_id = NULL`
  (`migrations/0004_storage.sql:14`) - see SEC-3.

### 4.10 Tokens, randomness, timing

- All tokens (session, invite `inv_`, reset `pwr_`, share, integration `itk_`, link ticket id,
  undo) come from `getrandom` (`lib.rs:267-276`, `apps/server/src/infra.rs:62-66`) with 16-32
  bytes and are stored as blake3 hashes where they act as credentials. Lookups by hash are
  constant-time by construction; HMAC verification uses `verify_slice` (`cookie.rs:193-197`).
- Undo tokens: signed, user-bound, 1 hour TTL (`bulk.rs:18-45`); UX4-12 fixed. Keyset cursors
  (`parse_cursor`) are unsigned but only select within the caller's own rows.
- DEK and nonces are fresh per value (`secrets.rs:697`); HKDF-SHA256 for derived keys.

### 4.11 HTTP security headers, CORS, caching

- No layer in the workspace sets `Content-Security-Policy`, `X-Frame-Options`/
  `frame-ancestors`, `Strict-Transport-Security`, `Referrer-Policy` or
  `X-Content-Type-Options` (grep over `crates` and `apps`; `apps/server/src/lib.rs:445-454` adds
  only the CSRF guard and request meta). `Cache-Control: no-store` is set on OIDC redirects and
  logout only; authenticated JSON and SSR pages have no cache directive. No CORS layer exists
  (good: cross-origin reads are blocked by default). SEC-1.

### 4.12 Docker image and compose

- Multi-stage build on `rust:1.99.1-slim-trixie`, runtime `debian:trixie-slim`, non-root
  `app` (uid 10001), `/data` owned by it, key file at `/data/secrets.key`, port 3000, `curl`
  kept for the healthcheck (`src/docker/Dockerfile`). Extra CA certificates from
  `config/certs/` are trusted at build and run time (documented, operator-controlled).
- `compose.yml`: default Postgres password `app`, default Meilisearch master key, default
  RustFS root keys when the env file is missing (`compose.yml:14,16,41,53,80-81,102-103`);
  `ports: "3000:3000"` (`compose.yml:22`) binds every interface, plain HTTP. Postgres, Meilisearch and S3 publish
  no ports (good). SEC-6.

### 4.13 Supply chain

- `cargo-deny` with advisories (two ignores with written rationale), licence allowlist and
  registry pinning (`src/rust/deny.toml`); `flake.lock` pins the toolchain; `package-lock.json`
  pins the two Playwright dev dependencies; CI runs `cargo deny` and `npm ci`.
- `src/sh/ci-tools.sh:11-27` downloads cargo-deny, cargo-leptos, wasm-bindgen and the
  tailwind binary from GitHub releases over TLS without checksum verification; the Dockerfile
  runs it (`Dockerfile:10-11`). SEC-8.
- Vendored icons and the font are recorded with licences (`config/brand/ICONS.adoc`, not
  re-audited here).

### 4.14 Backups, retention, deletion

- Backup docs cover the dump and the key file as one unit and the reindex after restore
  (`operations.adoc:20-68`).
- Retention jobs: audit 365 days, usage totals 180 days, password resets purged daily,
  TMDB retention (DEC-466).
- Deleting a user (`app/src/users.rs:108-132`) demotes linked companions, then
  `DELETE FROM users` cascades (`store-pg/src/repos/people.rs:206-218`, FKs in
  `migrations/0001_core.sql`, `0005`, `0008`, `0010`, `0012`, `0014`, `0016`, `0017`, `0023`,
  `0026`, `0031`). Left behind (SEC-3): Meilisearch documents of the user's library entries
  (title, status, rating, **notes, review**) and people (**name, notes**) and the same rows in
  `search_docs` (no FK, `0007_search.sql`), because the cascade writes no outbox rows (outbox
  inserts exist only in `store-pg/src/tx.rs:66` and `repos/catalog.rs:254`); audit rows keep
  `actor_name` and `detail.username` (`0016_admin.sql:105-116`, `admin/mod.rs:1039`); storage
  objects (dry-run import files) keep the bytes with `owner_id = NULL` (`0004_storage.sql:14`);
  `signup_invites.email` and `created_by/used_by` rows stay.

## 5. Findings

### SEC-1 (medium) No HTTP security headers and no cache directives on authenticated responses

Evidence: `apps/server/src/lib.rs:445-454` (the only cross-cutting layers are the CSRF guard
and request meta); a grep for `content-security-policy|x-frame-options|strict-transport|
referrer-policy|x-content-type` over `crates/` and `apps/` finds nothing;
`docs/modules/ROOT/pages/operations.adoc` has no reverse-proxy or TLS section (the only "proxy"
section is about TLS-inspecting build proxies, line 134).

Scenario: (a) a same-site page (another app on a sibling subdomain of a hosted instance, where
`SameSite=Lax` cookies are sent) frames `/admin/users` or the re-auth dialog and overlays it
(clickjacking); (b) any future XSS runs without a CSP backstop; (c) a misconfigured front proxy
or an intermediate cache may store signed-in pages and JSON (no `Cache-Control: private,
no-store`); (d) without HSTS a first visit over http is downgradeable.

Fix: add one `axum::middleware::from_fn` in `guarded()` that sets
`Content-Security-Policy: default-src 'self'; img-src 'self' data: https:; frame-ancestors
'none'; base-uri 'self'; form-action 'self' <provider end-session hosts if needed>`,
`X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`,
`Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy` as appropriate, and
`Cache-Control: private, no-store` for everything except `/assets`, `/brand/fonts`, `/pkg`
and the public shared-list page; emit `Strict-Transport-Security` when `public_url` is https.
Leptos' hydration script needs a nonce or `'wasm-unsafe-eval'`; the theme boot script
(`web/src/app.rs:155`) needs a hash or nonce. Add a server test that asserts the headers on
`/`, `/api/v1/auth/me` and `/_fn/current_user`. Document the reverse-proxy requirement.

### SEC-2 (medium) Server-function requests have no body limit

Evidence: the pages router (`apps/server/src/lib.rs:798-826`) has no `DefaultBodyLimit` or
`RequestBodyLimitLayer`; the REST router sets 2 MiB / 64 MiB (`crates/api/src/routes.rs:28-30,
125-130`). `server_fn-0.8.13/src/request/axum.rs:49-51` reads the body with
`body.collect().await` (whole body into memory) - not executed, read from the cargo registry.
axum's `DefaultBodyLimit` only applies to its own extractors, not to a raw `Request`, so even
the axum default does not protect this path.

Scenario: an unauthenticated client posts a multi-gigabyte body to `/_fn/sign_in` (the body is
read before any session check); a few parallel requests exhaust memory on the 4 vCPU / 16 GB
build host class of machine the project targets. Also reachable signed-in on every server fn.

Fix: `.layer(tower_http::limit::RequestBodyLimitLayer::new(api::routes::JSON_BODY_LIMIT))` on
the pages router (imports already go to `/api/v1/imports` through `web/src/rest.rs:64`), or
`axum::extract::DefaultBodyLimit` plus a `Limited` body wrapper before
`handle_server_fns_with_context`. Add a test posting `JSON_BODY_LIMIT + 1` bytes to a server
fn and expecting 413.

### SEC-3 (medium) Deleting a user does not remove all of their personal data

Evidence: `crates/app/src/users.rs:108-132` and `crates/store-pg/src/repos/people.rs:206-218`
delete by FK cascade; no outbox rows are written for the cascaded library entries and
companions (outbox inserts only at `store-pg/src/tx.rs:66` and `repos/catalog.rs:254`), so the
`library` and `people` search documents - which hold `notes`, `review`, companion `name` and
`notes` (`crates/search/src/indexer.rs:199-211, 238-255`) - stay in Meilisearch and in
`search_docs` (`migrations/0007_search.sql`, `owner_id` without FK). `admin_audit.actor_name`
and `detail.username` survive (`migrations/0016_admin.sql:105-116`,
`crates/api/src/admin/mod.rs:1039`, `sso_routes.rs:124`). `stored_objects.owner_id` is set to
NULL and the object (dry-run import file with the person's library) stays
(`migrations/0004_storage.sql:14`, `importers/src/runner.rs:135`).

Scenario: a hosted-mode member asks for deletion; the operator believes the account is gone,
but the person's notes and reviews remain searchable in the Meilisearch database (readable by
anyone with the master key or a backup), their username stays in the audit log for a year, and
their uploaded export stays in object storage. The Postgres fallback keeps the same rows.

Fix: in `App::delete_user`, before `users.delete`, enqueue `outbox (entity, id, 'delete')`
rows for every library entry and companion of the user (or move the delete into
`PgStore` with a transaction that does both), and `DELETE FROM search_docs WHERE owner_id =
$1`; delete `stored_objects` with `owner_id = $1` through the budget; pseudonymise audit rows
(`actor_name = NULL`, `detail - 'username'`) or record in the privacy page that audit rows are
kept 365 days as legitimate interest; delete `signup_invites.email` for invites used by the
user. Add a test: create user with a note, companion and dry-run import; delete; assert no
search document, no object, no username in audit.

### SEC-4 (medium) Outbound requests to admin-chosen hosts are not confined (OIDC endpoints, redirects, SMTP)

Evidence: `crates/auth/src/oidc.rs:399-428` adds the `token_endpoint`, `jwks_uri` and
`userinfo_endpoint` hosts of each provider's discovery document to the OIDC allowlist, and
`oidc.rs:546` does so before the JWKS fetch; neither applies `domain::net::is_private_ip`.
`crates/catalog-core/src/http.rs:211-216` builds the governed `reqwest::Client` without a
redirect policy (reqwest's default follows up to 10 redirects - documented default, not
executed), and `allows_host` is checked only once on the request URL (`http.rs:504-518`).
`issuer_allowed` accepts `http://` issuers whenever `public_url` is http (`oidc.rs:250-267`).
`crates/api/src/admin/mail.rs:247-275` connects to any SMTP host/port the admin saved.

Scenario: a hosted-mode admin (not the operator) registers a provider whose discovery
document names `jwks_uri: http://10.0.0.5:9200/_cluster/health`; `Test` fetches it from the
server and the failure detail (`Failure::failed(format!("jwks: {e}"))`) tells them whether the
host answered and what the parse error was - a port scanner and blind-ish SSRF against the
operator's network. A compromised legitimate provider can redirect any allowlisted call to an
internal address. SMTP "send a test" probes internal mail relays.

Fix: set `.redirect(reqwest::redirect::Policy::none())` on the governed client (or a custom
policy that re-runs `allows_host` per hop); in `Oidc::allow`, skip hosts that are IP literals in
private ranges and resolve names through a `PublicOnly`-style resolver unless
`notify.allow_private_targets` (reuse `catalog_core::plain::PublicOnly` for the OIDC connector's
client); apply the same rule to `mail.host`; show only categorised errors to the admin for
discovery failures. Keep the `http://` issuer exception for loopback only when `public_url` is
https.

### SEC-5 (medium) Sign-in throttling is per name and per process; every attempt costs an argon2 verification; a name can be locked by anyone

Evidence: `crates/auth/src/lib.rs:192-249` (map keyed by lowercased username, 10 failures /
15 minutes, process memory), `lib.rs:548-557` (checked before, counted after the verification),
`lib.rs:575-580` (a dummy argon2 verification for every attempt, including unknown names),
`crates/api/src/request_meta.rs` provides the client address but no handler uses it for limits.
No per-address limit exists on `/auth/sign-in`, `/auth/sign-in/local`, `/auth/reauth`,
`/auth/oidc/link`, `/auth/reset-password`, `/auth/username-available` (instance-wide 30/min,
`lib.rs:160-162`).

Scenario: (a) lock-out DoS: ten wrong passwords for `admin` lock the break-glass path for 15
minutes, repeatable forever from one IP; (b) CPU DoS: a flood of sign-ins with random names
bypasses the per-name budget and runs one argon2id (19 MiB, t=2) per request; (c) password
spraying across many usernames is not slowed; (d) one client can exhaust the 30/min
username-check budget for everyone; (e) with several `web` replicas the budgets multiply.

Fix: add a second budget keyed by `request_meta::current().client_addr` (e.g. 30 failures /
15 minutes, and a global concurrency semaphore around `verify_password`, e.g. 8), prefer a
progressive delay over a hard lock so a victim is slowed rather than locked out, and move the
counters to Postgres (or accept per-process with documentation) for multi-replica
deployments. Rate limit `/auth/reset-password` and `/shared/lists/{token}` per address as a
matter of hygiene.

### SEC-6 (low) Compose ships default credentials and binds plain HTTP on every interface

Evidence: `src/docker/compose.yml:14,41` (`POSTGRES_PASSWORD:-app`), `:16,53`
(`MEILI_MASTER_KEY:-change-me-please-32-bytes-minimum`), `:80-81,102-103` (RustFS
`change-me`), `:22` (`ports: "3000:3000"`). `config/.env.example` leaves the values
empty, so a copy-paste start works with the defaults.

Scenario: a quick-start deployment on a VPS exposes the app over plain HTTP to the internet
(cookies marked `Secure` are then never sent and the admin will toggle `secure_cookies` off);
the database and search services are reachable only on the compose network, so the default
passwords matter mainly when another container on that network is compromised or when the
operator later publishes those ports.

Fix: use `${POSTGRES_PASSWORD:?set POSTGRES_PASSWORD in config/.env}` syntax so compose
refuses to start without secrets; bind `127.0.0.1:3000:3000` and add a "Behind a reverse
proxy" section (TLS, `server.trusted_proxies`, forwarded headers, the SEC-1 headers if the app
does not set them); print a start-up warning when `public_url` is http and `bind` is not
loopback.

### SEC-7 (low) `RequestBodyLimit`-independent memory use of imports and bulk endpoints is bounded only by the body limit

Evidence: `crates/importers/src/lib.rs:346-358` parses the whole file into a `Vec` of rows;
`crates/api/src/routes.rs:30` allows 64 MiB per upload; the storage budget bounds what is
kept, not what is parsed concurrently. `MAX_BULK` 500 ids per request (`bulk.rs:109`).

Scenario: several members upload 64 MiB JSONL files at once; each worker parse holds the
file plus its rows in memory. Bounded, so low.

Fix: stream JSONL/CSV parsing (iterate rows, do not collect) in the worker, or cap concurrent
import jobs per worker (the queue lane already serialises per user; make the global limit
explicit).

### SEC-8 (low) Build tools are downloaded without integrity verification

Evidence: `src/sh/ci-tools.sh:11-27` fetches cargo-deny, cargo-leptos, wasm-bindgen and the
tailwind binary from GitHub releases and pipes them into `tar`/the PATH with no checksum;
`src/docker/Dockerfile:10-11` runs the script during the image build; `.gitlab-ci.yml:74`
runs it in CI.

Scenario: a compromised release asset or a MITM on a TLS-inspecting corporate proxy (which the
Dockerfile explicitly supports via `config/certs/`) injects a build tool that alters the
produced binary.

Fix: pin a `sha256` per artifact in the script and verify before extracting; or install the
cargo tools with `cargo install --locked` at pinned versions. Also consider removing `curl`
from the runtime image (healthcheck via a tiny built-in `--healthcheck` flag) to shrink the
attack surface.

### SEC-9 (low) Storage error text is returned to webhook senders

Evidence: `crates/api/src/hooks.rs:48-50` maps `IntegrationError::Port(e)` to a 500 whose
message is `e.0` (the port error string, which wraps sqlx/opendal errors); the generic
mapping in `crates/api/src/error.rs:139-147` logs and returns "internal error" instead.

Scenario: a sender with a valid `itk_` token (or the owner of one) learns database/connection
details from a transient failure.

Fix: log the error and return the generic message like `error.rs:139-147`.

### SEC-10 (info) Sessions slide without an absolute lifetime; no session list/revocation for members

Evidence: `crates/auth/src/lib.rs:699-703` extends `expires_at` on use; `session_days` (default
30) is a sliding window only. Password change, disable and reset revoke sessions; there is no
"sign out everywhere" or device list for members.

Fix: add `created_at + N days` as a hard cap (e.g. 90) and a Settings > Account list of
sessions with revoke.

### SEC-11 (info) The SSO hint cookie carries the login hint for 400 days

Evidence: `crates/auth/src/oidc/sso.rs:296-311`, `cookie.rs:109-117`: `preferred_username`
or `email` signed (not encrypted) in an `HttpOnly` cookie, cleared on explicit sign-out
(`sso_routes.rs:206-210`).

Fix: acceptable per addendum-1 4.2; consider encrypting the value or storing only the
provider id and a short opaque hint reference on the server.

### SEC-12 (info) `Origin`-less state-changing requests pass the CSRF guard

Evidence: `apps/server/src/csrf.rs:28-30` allows a request without `Origin` and without
`Sec-Fetch-Site: cross-site`. Every current browser sends `Origin` on cross-origin POSTs, and
`SameSite=Lax` withholds the cookie on cross-site POSTs anyway, so this is defence in depth
only; it keeps curl/bearer clients working.

Fix: none required; optionally require `Sec-Fetch-Site` or `Origin` when a session cookie is
present and the request carries a `Content-Type` of a form.

## 6. Checked and holds / not checked

Verified fixed from earlier reviews (evidence in section 4): G2-1 login/logout CSRF (Origin
guard on both routers); G2-2 shared catalog poisoning (DEC-153, `find_or_create_catalog_item`);
G2-4 brute force (per-name throttle, with the residual SEC-5); G2-7 SSRF DNS rebinding
(connect-time resolver, no redirects); G2-10 body limits on the REST router (residual SEC-2 for
server fns); UX2-S5/S6 (candidate key rejection not recorded, password change throttled);
UX3-1 (secret purpose registry with a workspace test); UX3-5 (CSV formula guard); UX3-9/20
(X-Forwarded-For walked from the right, mapped IPv6 networks); UX3-10/11 (allowlist for
disabled provider Test, refetch cool-down); UX3-12 (password reset atomic, purged); UX4-12
(signed, user-bound undo token); UX4-14 (funnel subjects allowlisted, no ids); UX4-22
(SMTP test text categorised before storage).

Checked and holds: argon2id parameters and dummy verification; session token entropy,
hashing and cookie flags; TOTP replay protection, encrypted seed, lock when the key is
missing; recovery code entropy and hashing; re-auth window and OIDC `max_age=0` round trip
bound to the session; OIDC PKCE/state/nonce/iss/single-use/return-path; back-channel logout
verification and replay cache; no automatic email linking; envelope encryption, key file mode,
rotation, re-wrap coverage test; notification-target SSRF guard; per-user isolation in every
`app` use-case reviewed and in both search backends; SSE audience filtering; undo token
signing; setup wizard first-account guard; admin gates and lock-out rules; CSV guards; no
dynamic SQL; HTML escaping of the only hand-built page; log redaction of secrets; asset
serving by known hash only; import format allowlist and absence of archive parsing; Docker
non-root user; cargo-deny configuration and lockfiles.

Not checked (read-only audit, no execution): the actual response headers and body limits at
run time (statements rest on code reading and library defaults); the Leptos client bundle for
DOM sinks (the wasm code was read only where it builds requests); the TheTVDB/TMDB/IGDB/
Podcast Index connector parsers for pathological responses beyond the size caps; the job queue
lease and fair-share logic in depth (`crates/jobs`); the Postgres roles/grants the migrations
assume (single application role); the brand directory loader for path handling of
operator-supplied files; the Playwright tests; third-party advisory status as of today beyond
`deny.toml`; the behaviour of `secure_cookies` toggling at run time in browsers; the
`handlers.rs` job handlers line by line.

## 7. Prioritised fix plan

1. SEC-2 body limit on `/_fn/*` (one layer, one test) - smallest change, removes an
   unauthenticated memory DoS.
2. SEC-1 security headers + `Cache-Control` middleware and the reverse-proxy docs section;
   emit HSTS on https `public_url`.
3. SEC-3 complete deletion: outbox deletes for library entries and companions, `search_docs`
   purge, owned storage objects, audit pseudonymisation; one end-to-end test.
4. SEC-4 confine outbound calls: `redirect(Policy::none())` on the governed client, private-
   range filter for OIDC endpoint hosts and `mail.host` (honouring `allow_private_targets`).
5. SEC-5 per-address budget and a global argon2 concurrency cap; progressive delay instead of
   a hard lock; rate limit the public token endpoints.
6. SEC-6 compose hardening (`:?` required variables, loopback bind, start-up warning).
7. SEC-8 checksum pinning in `ci-tools.sh`; SEC-9 generic 500 text in `hooks.rs`.
8. SEC-7, SEC-10, SEC-11, SEC-12 as hygiene items when the surrounding code is next touched.
