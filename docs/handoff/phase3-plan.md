# UX phase 3 plan: sign-in and the stale pages (2026-11-07)

Written by a planning agent from HANDOFF.md (sections 0, 2, 4, 7), design.md 3.6, 3.7,
5.8-5.11, 6.5-6.14 and 9 "Phase 3", addendum-1.md 3.2-3.4 and 4.1-4.7,
docs/research/od-12-reauth-window.md 4.4, and the DEC entries that defer work to phase 3
(DEC-072, 118, 155, 211, 230, 231, 237, 239, 245, 247, 249, 250, 255, 258, 260, 281-292).
Seven packages, run in order (one cargo build at a time). WP3-1..3 are server only. Paths are
relative to `src/rust` unless they start with `src/web` or `docs`.

## Already in the code (do not rebuild)

- OIDC single env provider: `crates/auth/src/oidc.rs` (discovery cached per process, PKCE
  S256, `finish()`, identity lookup by (iss, sub), auto-create, `sanitize_username`); routes
  `GET /auth/oidc/start` and `/auth/oidc/callback` in `crates/api/src/routes.rs` (~257-310);
  `config::OidcConfig`; `ConnectorPolicy::oidc` in `catalog-core/src/http.rs`; table
  `oidc_identities`. No OIDC button in the web crate.
- Re-auth: `crates/auth/src/fresh.rs` (DEC-249), `Auth::confirm_oidc_reauth(token, iss, sub,
  provider, auth_time)` with the 300 s limit; `web/src/reauth.rs` dialog, `gated()`,
  `intercept_or` (DEC-258, DEC-291).
- Secrets at rest: `crates/auth/src/secrets.rs` (`KeyRing`, `SecretCipher`), `SecretRows`,
  rewrap job, `seal_plaintext` (DEC-243..245, 270, 276).
- Instance settings with provenance, `SETTINGS` registry in `crates/api/src/admin/mod.rs`
  (~69); `PUT /admin/settings` already accepts `instance.hosting` (DEC-282).
- `admin_audit` already has `client_addr` and `request_id` columns (written NULL); audit
  query, CSV, retention; `auth::Throttle` (DEC-155); lock-out guards (DEC-250).
- Overview health rows `mail`, `identity_provider` (configured/off only, `api/src/admin/mod.rs`
  ~1453); wizard checklist has `identity_provider` (`admin/setup.rs:138`).
- Notifications server complete (channels with encrypted secrets and Test, subscriptions with
  locked instance defaults, deliveries log, inbox/read, templates, web-push-key stub; REST in
  `crates/api/src/notifications.rs`). Web page `web/src/notifications.rs` (old layout) has
  Inbox, Channels, Subscriptions, Log. Admin Notification defaults page exists.
- Integrations: token create/list/delete, `POST /hooks/watched`, `last_seen_at`/`last_error`
  (`crates/integrations`). Web page old (`web/src/integrations.rs`).
- Companions (`app/src/companions.rs`): create/update/delete, link requests accept/decline,
  unlink, merge, promote/invite, shared sessions and claim, privacy switch. Old web page
  `web/src/sharing.rs::Companions`.
- Lists (`app/src/lists.rs`): create, update, delete, add, remove, move_in_list, share_list,
  list_by_token; `domain::list::Visibility` Private/Instance/Link. REST/server fns expose only
  list, create, add, share, shared. Web: flat `pages::Lists`, `sharing::SharedList`; no
  `/lists/:id` route.
- Importers: idempotent resumable runner with `dry_run`; generic JSONL and CSV; `legacy.rs`
  stub; `POST /imports`, `GET /imports/{id}`, `GET /export` (JSONL), SSE `import.progress`.
- Episode list: Mark season, Mark up to here, ask-once (DEC-231).
- Account page: password change, TOTP with recovery codes, language, theme.
- E2E: setup project creates `e2eadmin`; `helpers.ts` `signIn`, `expectReflow`;
  `tmdb-mock.mjs` webServer pattern is the model for an OIDC mock.

## WP3-1 instance-server (server only, foundation)

- Audit rows in the same transaction as the change (DEC-245, addendum 3.4): store write
  methods taking the audit entry; settings, secrets, credentials, invites, roles,
  disable/delete; anything left outside gets a DEC.
- Client address and request id in audit rows (DEC-284): `server.trusted_proxies` (CIDR list,
  default empty); socket address, or `X-Forwarded-For` only from a trusted peer; UUIDv7 per
  request or `X-Request-Id` from a trusted proxy; in `Caller`; both columns in the CSV.
- SMTP in the database with an encrypted password (DEC-245, addendum 3.2): settings `mail.*`,
  password as a `SecretRows` purpose, env `[notify.smtp]` wins read-only, mailer swappable at
  runtime (today built once in `apps/server/src/lib.rs` ~116), "Save and send a test" writes
  `mail.last_test_at` + result for the Overview mail row.
- CLI escape hatches (DEC-283, addendum 3.3, 4.3), audit actor `cli`: `cli admin promote
  <username>`, `cli admin reset-password <username>` (one-time link; table `password_resets`,
  `POST /auth/reset-password`), `cli admin reset-totp <username>`, `cli admin
  disable-sso-only` (sets `auth.sign_in_mode=page`; key defined here, enforced in WP3-3),
  `cli setup reopen`, `cli secrets status|rotate|prune`.
- SIGHUP re-reads the key set (DEC-283): reloadable cipher behind `Arc<dyn SecretCipher>`;
  refuse a set that drops a version still in use, keep the old ring, log why.
- Source Disabled switch (DEC-289): `set_source_enabled` with audit; registry honours it live.
- Wizard checklist done flags (DEC-292): `SetupStateDto.done: Vec<String>` from the server.
- Per-user time zone, server half (DEC-239, 211, 230): `users.time_zone` (IANA, NULL = UTC),
  `PUT /auth/me/time-zone`; stats buckets, activity feed, History days, `ClientConfig.today` in
  the user's zone; DTOs carry a precomputed local day key and local time (no tz db in wasm).
- Phase 2 tests deferred here (DEC-281): re-auth throttle and a recovery code via `POST
  /auth/reauth`; cron registration of `secrets.rewrap`/`admin.audit_retention` and handlers;
  `/setup/*` as a member and after completion; settings effective without restart;
  `shadowed_value`; audit `before`/`after`; `secret.replaced` detail.
- Files: `crates/auth/src/{admin.rs,admin_pg.rs,pg.rs,secrets.rs}`,
  `crates/api/src/admin/{mod.rs,setup.rs}`, `crates/api/src/{extract.rs,lib.rs}`,
  `apps/server/src/{lib.rs,main.rs,admin_wiring.rs,handlers.rs}`, `apps/cli/src/main.rs`,
  `crates/config/src/lib.rs`, `crates/notify/src/channels/email.rs`, `crates/app/src/stats.rs`
  and activity/history queries, migration `0017_phase3_instance.sql`.
- Tests first: `apps/server/tests/admin.rs` (audit row rolled back with its change via fault
  injection; client address only via trusted proxy; SMTP env read-only, password never
  returned, send-test recorded); new `apps/server/tests/cli.rs` (promote, reset-password link
  use, disable-sso-only, secrets status, audit actor `cli`); `crates/auth/tests` (SIGHUP reload
  swaps the ring, refuses to drop a version in use); time zone test (event at 23:30 UTC is the
  next day for Europe/Berlin in History, activity, stats).
- Defaults: `chrono-tz` (MIT OR Apache-2.0, server only behind `ssr`; check deny.toml and the
  tzdb data licence), fallback brand default zone then UTC, account page suggests the browser
  zone; `std::sync::RwLock` for the reload; reset link 1 h, single use, page in WP3-4.

## WP3-2 oidc-providers (server only)

Addendum 4.2 "Data model", "Redirect and callback", "Account resolution", "Group and claim to
role mapping"; supersedes DEC-072.
- Tables: `oidc_providers` (slug, display name, issuer, client id, client secret as a
  `SecretRows` purpose, scopes, enabled, is_default, auto_create, groups claim,
  allowed_groups, admin_groups, role_source, discovery cache + fetched-at, capabilities, last
  test results, HLC); `oidc_identities` + `provider_id`, `sid`, `linked_at`,
  `last_claims_hash`; `auth_sessions` + `provider_id`, `oidc_sid`; env provider shown read-only
  with provenance Env.
- Multi-provider `Oidc`: discovery per provider with refresh, PKCE S256 always, one callback
  URI. Pending cookie (provider id, issuer, state, nonce, verifier, return path, mode
  interactive|silent|link|reauth), HMAC-signed with the HKDF `cookie_key` purpose, single use.
  Mix-up defence: `iss` response parameter when advertised, else ID token `iss` (RFC 9207,
  RFC 9700).
- Account resolution steps 1-4: step 3 "Is this your account?" (password once; emailed link
  deferred); Link from Settings = mode `link`; unlink refused when no sign-in method would
  remain; OD-15 never auto-link; OD-21 email sync off.
- Groups/claims: groups from ID token else UserInfo; `allowed_groups` refusal with a reason;
  `role_source=provider` only with `admin_groups` (OD-16); never demote the acting admin;
  provider-managed role badged "via group" and locked in Users.
- Admin API: provider CRUD, re-auth gated (DEC-249), audit; Test = discovery, logout
  capabilities, "groups claim seen". Runtime-mutable oidc host allowlist
  (`GovernedHttp::add_connector` is build-time today). Overview `identity_provider` row from
  stored fields (discovery age, last callback result), no probes (DEC-285).
- Files: `crates/auth/src/{oidc.rs,store.rs,pg.rs,lib.rs}`, `crates/catalog-core/src/http.rs`,
  `crates/api/src/routes.rs`, new `crates/api/src/admin/oidc.rs`, `crates/contracts/src/admin.rs`,
  `crates/config/src/lib.rs`, migration `0018_oidc.sql`, testkit in-process test issuer.
- Tests first (new `apps/server/tests/oidc.rs`, testkit issuer): full code exchange; state and
  iss mismatch refused; two providers; link mode; verified email never links without the
  password; auto-create username collision suffix; allowed_groups denial; role_source=provider
  promotes and refuses to demote the acting admin; unlink refused for the last method; client
  secret encrypted and never returned; env provider read-only; unit tests for the signed
  pending cookie (tamper-proof, single use).
- Defaults: RS256 test issuer with the `rsa` crate (dev-dependency of testkit; check on docs.rs
  whether HS256 works and log the choice); plain-http issuers only for loopback or while
  public_url is http; unknown provider id on callback -> generic "sign-in failed" page.

## WP3-3 sso-logout-modes (server only)

Addendum 4.2 "Logout", "True SSO", 4.3, 4.4; OD-17..20; DEC-191.
- Local-first sign-out, then RP-initiated logout via `end_session_endpoint` (`id_token_hint`,
  `client_id`, `post_logout_redirect_uri=/signed-out`, `state`).
- Back-channel logout `POST /api/v1/auth/oidc/backchannel-logout` (public, rate-limited,
  no-store; validate signature, iss, aud, iat, exp, `events`, no `nonce`; `jti` replay cache 10
  min; end sessions by `sid` else all of the (iss, sub) identity; audit row).
- Silent sign-in: top-level redirect `prompt=none`, `id_token_hint` from an `oidc_hint`
  cookie, `login_hint` for Entra, `oidc_silent_attempt` loop guard (5 min); login_required /
  interaction_required / account_selection_required / consent_required -> interactive;
  server_error / temporarily_unavailable / network -> outage path, 3 failures in 10 min stop
  auto-retry.
- Sign-in modes `auth.sign_in_mode` page|auto (re-auth gated, audit); preconditions on the
  server: default provider with a Test in the last 24 h, at least one admin with password and
  authenticator, fresh re-auth. In auto, signed-out page visits 302 to the provider with a
  `role=status` interstitial body.
- Break-glass `/sign-in/local` (OD-17 as changed): password, TOTP only if the account has it;
  admins only in auto mode; throttle, audit, admin notification (new notify event
  `admin.local_sign_in`, with the client address).
- Re-auth round trip (DEC-191, od-12 4.4): mode `reauth` with `max_age=0`, callback calls
  `confirm_oidc_reauth`; the pending gated action is kept server side and shown again.
- Provider Test adds the silent probe and the max_age probe (OD-19): silent_ok,
  silent_login_required, silent_unsupported; max_age supported yes/no.
- `/signed-out` never redirects (OD-20), server-side exemption.
- Files: `crates/auth/src/{oidc.rs,fresh.rs,lib.rs}`, `crates/api/src/routes.rs`,
  `apps/server/src/lib.rs` (redirect middleware next to the /setup redirect), `crates/notify`,
  `crates/api/src/admin/`.
- Tests first (extend `apps/server/tests/oidc.rs`, cases from addendum 4.6 "9 Phase 2"): silent
  success; login_required fallback; loop guard; back-channel by sid and by sub; jti replay
  refused; token with nonce refused; RP-initiated logout parameters; outage and retry cap;
  auto refused without preconditions; /sign-in/local in auto refuses members, admits an admin
  with password only, writes audit and notification; re-auth with fresh auth_time passes, old
  or missing refused; /signed-out never redirected.
- Defaults: store the id_token_hint encrypted per `auth_sessions` row (deviation from the
  addendum's `last_id_token` on the identity; two browsers would overwrite each other; log it;
  drop after logout); jti cache and silent counter in memory per process (like DEC-155);
  authentik max_age=0 check as an `#[ignore]` live test with `KK_OIDC_LIVE_*` for the owner,
  fallback od-12 4.4 step 4.

## WP3-4 signin-admin-web (web crate)

- Auth pages (addendum 4.5, 4.6; design 5.2, 6.14): `/sign-in` with provider buttons first
  (display name, no logos, rule 11), password form in a `details` disclosure (open when no
  provider); states 2-6 (interstitial, outage `role=alert`, denied/group rule, session ended);
  `/sign-in/local`, `/signed-out`, "Is this your account?" page, `/reset-password/:token`;
  account menu Sign out local-first then `/signed-out`.
- Admin area restructured to addendum 3.4's eight pages (reverses DEC-255 as its "how to
  reverse" says): Overview (Brand card folded in), Instance, Sign-in and SSO, Users, Data
  sources, Notification defaults, Storage and jobs, Audit. Instance: hosting mode (DEC-282),
  sign-up mode moved from Users, public address read-only, disclosures Mail (Save and send a
  test), Secrets at rest, Advanced (sessions and cookies from Security). `/admin/brand` and
  `/admin/security` become 404, no redirects (DEC-171). Sign-in and SSO: provider table, Add a
  provider, detail with write-only SecretField, the exact URIs to register, Test outcomes, mode
  radios with consequences, account rules, "Front-channel logout is not used ...". Data
  sources: Disabled switch (DEC-289). Wizard: done rows (DEC-292), step 4 rows "Back up
  secrets.key" and "Connect an identity provider".
- Re-auth dialog "Confirm with <provider>" (DEC-258).
- Settings > Account: Linked sign-ins with Link/Unlink (6.12, addendum 4.6); time zone picker
  with the browser suggestion (DEC-239); then drop the "UTC" markers on Home, History, Stats.
- Phase 2 tests deferred here (DEC-281): axe at 360 px on admin pages and reflow on
  `/settings/*`; password change server fn; privacy toggle; nudge Snooze click; palette
  scope_group; PIN entry through Settings > Data sources.
- Files: `crates/web/src/{auth_pages.rs,pages.rs,shell.rs,reauth.rs,settings.rs,admin_state.rs,admin_api.rs,app.rs,setup.rs,home.rs,history.rs}`,
  `crates/web/src/admin/*` (new `instance.rs`, `signin.rs`; security and brand folded in),
  Fluent en/de, `src/web/tests/`, new `src/web/tests/oidc-mock.mjs`.
- Tests first: `oidc-mock.mjs` (discovery, authorize, token, JWKS, end-session; RS256 with
  node:crypto, no npm deps; webServer entry; provider created through the admin UI); new
  `signin.spec.ts` (provider button, silent sign-in in auto, break-glass, /signed-out stays,
  outage alert, link/unlink, axe + reflow); `admin.spec.ts` (hosting switch, Mail test, Sign-in
  and SSO gate via re-auth, source Disabled switch); SSR in `auth_pages.rs`/`admin_pages.rs`,
  `/admin/security` 404.
- Default: side nav order = addendum 3.4 table order.

## WP3-5 notifications-integrations (server and web)

- Notifications (design 6.8, 5.8): tabs Inbox / Channels / Rules / Log; inbox grouped by day
  in the user zone, "Mark all read"; channel cards with health from the delivery log, "Add and
  send a test" inline per type, secrets as password fields; rules matrix events x channels,
  mode in words, instance defaults labelled, locked rows with a lock, first switch of a new
  channel offers the defaults, "Add a rule for one list, item or person" via the existing
  Filter; log table with expandable error; bell label "9+".
- Integrations (design 6.9, 5.9; R-38): sender cards with health and last recorded item;
  `integrations` + `last_status`, `last_error_field`, `last_payload` (redacted, <= 4 KiB);
  three-step connect panel with SSE `integration.call` per user turning "Waiting for the first
  call..." green; "Send a test call from here" (validates and matches, logs nothing, marked
  test); revoke in the card menu with confirm; refusals explained inline, never as a toast.
- Files: `crates/notify/src/{store.rs,pg.rs,engine.rs}`, `crates/api/src/notifications.rs`,
  `crates/integrations/src/{lib.rs,store.rs,pg.rs}`, `crates/api/src/hooks.rs`,
  `crates/web/src/{notifications.rs,integrations.rs,api.rs,events.rs}`, migration
  `0019_integrations_payload.sql`.
- Tests first: `apps/server/tests/webhook.rs` (422 records field + redacted payload; test call
  writes no progress; SSE on an accepted call); new `apps/server/tests/notifications.rs`
  (channel health, mark all read); rewrite `notifications.spec.ts`; new
  `integrations.spec.ts` (panel goes green after a call, refusal explained inline).
- Default: a 401 cannot be tied to an integration (token unknown); only 422s are attributed;
  after 60 s without a call the panel says "check the token"; token format unchanged; DEC.

## WP3-6 people-lists (server and web)

- People (design 3.6, 6.7, 5.11): `companion_stats` in `app` (together, plays, win rate,
  last, most played game); request cards on top; tabs All / Linked accounts; person cards with
  initials; dialogs invite (consequence text, Copy, 14 days per DEC-022), link by username,
  unlink, merge (suggested for names equal after normalisation, undoable via the merge
  record); "Add a person" sheet; `/people/:id` with header stats, History filtered to the
  person, lists shared by them, actions incl. "delete keeps history".
- Lists (design 6.5, 5.10): `set_list_visibility(Private|Instance|Link)` (make private again;
  re-share replaces the link and says so); queries shared with me / on this instance; REST and
  server fns for get, rename, delete, remove, move, visibility; overview tabs Mine / Shared
  with me / On this instance with cards; `/lists/:id` with editable title/description,
  keyboard move up/down, remove, Add via the palette scoped to the list, "Add all to my
  library"; public page polish (brand header, no ratings/notes, sign-in CTA, friendly
  `shared-list-missing` 404).
- Files: `crates/app/src/{companions.rs,lists.rs}` + stats helper, store-pg list/companion
  repos, `crates/api/src/{sharing.rs,routes.rs}`, `crates/contracts`,
  `crates/web/src/{sharing.rs,pages.rs,palette.rs,app.rs}`, new `people.rs`, `lists.rs`.
- Tests first: app unit tests (companion_stats incl. ties; visibility transitions, Private
  revokes the token and the old link 404s); `apps/server/tests/sharing.rs` (instance-visible
  list visible to another user, private not; move order persists); extend `sharing.spec.ts`;
  new `people.spec.ts`, `lists.spec.ts` (stats, keyboard reorder, axe + reflow).
- Default: "Shared with me" = Instance-visible lists owned by linked people; "On this
  instance" = all Instance-visible lists of others.

## WP3-7 imports-history (server and web)

- Legacy importer (HANDOFF 0 and 4.6; Q4 and rule 6: derived only from the fixture
  `crates/testkit/fixtures/import/legacy/export.json`, never from Ryot docs or code; commit the
  fixture with it): top-level keys metadata, collections, people, workouts, exercises,
  measurements, metadata_groups, workout_templates; `lot` anime/book/movie/show/video_game ->
  kinds; `source` + `identifier` -> external ids (tmdb, igdb, openlibrary, google_books,
  myanimelist, anilist); `seen_history` (state, progress, ended_on, started_on,
  show_season_number/show_episode_number, anime_episode_number) -> history and status;
  built-in collections -> statuses/watchlist, other collections -> lists (optional ImportRow
  field `lists`); workouts, measurements etc. reported as "not imported"; `#[ignore]`
  full-export test on `owner-inbox/legacy/export.json`.
- Trakt: research agent first writes `docs/connectors/trakt.md` (licence and terms first, then
  API docs, R-P4); parse uploaded JSON in the documented `/sync/history`, `/sync/ratings`,
  `/sync/watchlist` shapes, fixtures marked synthetic; no OAuth pull; unclear terms -> feature
  flag off.
- Imports page `/settings/imports` (design 6.11): cards per format (generic, legacy, Trakt;
  Goodreads, BG Stats, BGG "waiting for a sample"); drop zone or picker; dry-run preview
  (counts per kind, 5 sample rows, mapping and what is lost, R-35; `ImportRecord.preview`);
  run with progress "x of y" over `import.progress`, cancel via `cancel_requested` checked per
  checkpoint; result imported/skipped/unmatched with a persistent unmatched list, manual match
  through the palette replaying the row, retry; `GET /imports`; export one button per format
  (JSONL + new generic CSV writer).
- History H1 text link to Export (DEC-237). Episode list "Mark show" and backfill "Spread over
  a date range" and "On release dates" (DEC-237).
- Files: `crates/importers/src/{legacy.rs,lib.rs,row.rs,generic.rs}` + new `trakt.rs`,
  `crates/importers/tests/legacy.rs`, `crates/api/src/{routes.rs,ops.rs}`,
  `crates/web/src/{settings.rs,history.rs,episode_list.rs,episode_plan.rs}` + new `imports.rs`.
- Tests first: `crates/importers/tests/legacy.rs` (replace the ignored stub: exact counts per
  kind, episodes and dates, lists created, lost entries listed, second run creates nothing);
  Trakt fixture test; `apps/server/tests/imports.rs` (dry run writes nothing and returns the
  preview, cancel stops, manual match); new `imports.spec.ts` (upload legacy fixture, preview,
  run, results, export download, axe + reflow); `item.spec.ts` (Mark show, backfill range).
- Defaults: optional `lists` field keeps GENERIC_VERSION 1 (DEC); "On release dates" needs
  per-episode air dates - if the catalog has none, build Mark show and the range now and defer
  release dates; rows without a usable external id count as unmatched.

## Suggested deferrals to phase 4

Probes table every 60 s (DEC-285; status from stored last-Test/last-callback fields); emailed
one-time link for linking a password-less account (addendum 4.2 step 3; fallback "ask your
admin to link it"); OD-21 "Keep email in sync" switch (default off); People "Game groups" tab
(game groups are phase 4); drag handles on the list page (keyboard move satisfies WCAG);
account display name/email editing, delete account, TOTP QR image (DEC-260); backfill "On
release dates" if no per-episode air dates exist; web push (parallel workstream); persisted
source counters across processes (DEC-247) and hashed TheTVDB token cache key (DEC-286);
Goodreads, BG Stats, BGG importers (samples not delivered); candidate source Test with a live
key and the silent wizard skip on a reused server (DEC-281).
