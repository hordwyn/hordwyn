# Next packages after the owner's answers of 2026-11-10 (F9)

Status: COMPLETE (a proposal; the orchestrator decides the cut). Docs only, nothing built.

Input: `docs/fable/round2/owner-answers-2026-11-10.md` (the rulings, DEC-530 to DEC-552),
`docs/fable/round2/ux-owner-test.md` (the UX designs), the round-1 reviews on branch
`wip/fable-reviews`, the fix pack log `docs/fable/round2/fix-pack-ot1a-progress.md`.

## 0. Rules for the cut

- **One building package at a time.** The VM has 4 vCPU and 16 GB and the owner adds no RAM
  (OWN-12 a). Every package that touches Rust ends with the orchestrator's `cargo xtask ci`
  and `cargo xtask e2e`; two of those do not fit side by side. Packages marked
  **docs-only** may run next to a building package.
- **Packages that share the `web` crate run one after the other** (as in phases 3 and 4).
  That is all of ot2 and most of the security pack.
- **Migrations are numbered when a package starts**, with the next free number (0032 at
  the time of writing, `crates/store-pg/migrations/` ends at `0031_catalog_audience.sql`).
  Names below are proposals. No migration-adding package may be open while the baseline
  squash (package base) runs.
- **Test first** (rule 4): each package lists the tests to write and see red before code.
  Paths: `crates/` = `src/rust/crates/`, browser tests in `src/web/tests/`.
- **Every string in en and de** (Fluent), WCAG 2.2 AA, density budget OD-01, icons only
  from the vendored sets. The xtask locale parity test guards the first.
- Sizes: S = up to a day, M = a few days, L = a week or more (one agent, test-first, with
  review). They are estimates.
- A release load run (`cargo xtask load --release`, about 40 minutes and 15 GB of disk) is
  needed only where a package changes a measured read path; it is named where it applies.
- "Blocked on the owner" names a question of the answers file section 13 (Q1 to Q9) or a
  delivery. A package with a blocked part is cut so that the rest can ship.

## 1. Proposed order

| # | Package | Content | Size | Builds | Blocked on the owner |
|---|---|---|---|---|---|
| 0 | ot1a (in flight) | fix pack and its review fixes; commit, tag | - | yes | no |
| 1 | ot2a | item page: actions, labels, counts, card rule, sign-up landing | M | yes | no |
| 2 | ot2b | log dialog instead of the side sheet | S-M | yes | no |
| 3 | ot2c | stats led by time, step 1 | M | yes | no |
| 4 | ot2d | colour and page widths | M | yes | no (owner looks at it once) |
| 5 | ot2e | rewatch run (status model A), bulk rewatch | M | yes | no |
| 6 | ot2f | plays: tab on game pages, hours and win rate | M | yes | no |
| - | dlog | decision log in three files | M | docs-only | no |
| - | data-r | Steam, SteamDB, ProtonDB: terms research | S | docs-only | Q8 for what follows |
| - | legal | lawyer package: the questions and texts | S | docs-only | owner engages the lawyer (Q9) |
| 7 | ot1b | carry-over of the fix pack: search ranking, palette covers | S-M | yes | no |
| 8 | sec0 | body limit, webhook error text | S | yes | no |
| 9 | ops1 | image and first run: cli, env keys, compose, runbooks | M | little | Q1 for the key part |
| 10 | sec1 | security headers, CSP, hashed bundle names | M | yes | no |
| 11 | sec2 | outbound confinement, sign-in throttling | M | yes | Q6 for the last step |
| 12 | sec3 | account deletion and records without names | M-L | yes | Q3 for one detail |
| 13 | sec4 | session cap and session list | M | yes | no |
| 14 | prefs | server-side personal preferences | M | yes | no |
| 15 | test1 | serial browser project, flaky tests fail the pipeline | S-M | yes | no |
| 16 | test2 | per-user import lane | M | yes | no |
| 17 | label | the person's own title and year (TMDB purge) | S-M | yes | no |
| 18 | W1 | attribution page | M | yes | no |
| 19 | arch-q | quick wins: translation cache, batched DTOs, rules and leaf crates | M | yes | no |
| 20 | base | export round trip, migration baseline squash | S-M | yes | owner re-imports |
| 21 | ops2 | Dockerfile structure, thin against fat measurement, publish images | M | yes | Q4; owner picks the profile |
| 22 | W2 | performance plan PERF-0 to PERF-9 | L | yes | no |
| 23 | d1 | licence tag per field and asset | M | yes | no |
| 24 | d2 | Wikidata identity for all kinds | L | yes | no |
| 25 | d3 | TVmaze as the default show source | M | yes | no |
| 26 | ot2g | per-episode runtimes in the stats, "Up to date" badge | M | yes | no |
| 27 | d4 | Wikipedia summaries | M | yes | no |
| 28 | arch-b | split `api`, remove the Backend pass-through, split `web` | L+M+L | yes | no |
| 29 | h1 | hub: role, read API, connector, Wikidata seed | L | yes | no |
| 30 | h2 | hub: peers and signed requests | L | yes | no |
| 31 | d5-d7 | Open Library plus DNB; MusicBrainz, Cover Art Archive, ListenBrainz; VNDB | M each | yes | no |
| later | h3 to h7, kinds | contributions, images, lawful, ops, further kinds | L each | yes | lawyer; Q2, Q5 |
| later | W4, W6, W7, importers, steam | anime and manga, web push, "date unknown", sample-bound importers, Steam | - | yes | deliveries, Q7, Q8 |

Why this order: the owner sees ot2 first (their own findings, all answered with the proposed
design). The security and operations fixes follow because public images (OPS-I b) and any
hosted use wait for them. Data and hub come last because they are the largest and because
the hub's lawyer-bound phases cannot start before the owner has engaged one. The
restructuring sits where the owner put it (ARCH-0 b): after ot1, ot2 and W1; it is placed
directly before the hub so that the hub's files are written into the split crates once.

Tags: `ot2` after package 6 (or after 4, with 5 and 6 as `ot2.1`, if the owner should see
the visible part sooner); `ot1` once sec0 to sec4 and ops1 are in; `ws1`, `ws2` as planned.

## 2. UX pack ot2

All six packages touch `crates/web`, so they run in sequence. Each ends with the affected
Playwright specs green under axe in both themes.

### ot2a - item page: actions, labels, counts (OT-4, OT-5, OT-6, OT-7, OT-12, OT-15 web part, OT-16 rest, OWN-7)

- Scope: two buttons on an item not in the library ("Mark as watched", "Want to watch")
  and one shared toast "Added to your library as Planned" with Undo, on the item page and
  in the palette; a status pill with a fact line in the header; imperative action labels
  per kind verb; "Watched 3 times, last on <date>" and the sheet line "This will be watch
  number N"; the key family "Watched / Listened / Read / Played with..."; bulk marks on
  watched episodes ask "Log these N again as a rewatch?" (until ot2e exists the answer
  logs plain extra views, which is what "Again" on a row does today; DEC-538); the play
  dialog's duration accepts `1:30` or `90` and is prefilled from the last play, scores move
  behind "More" for video games (DEC-498 leftovers); the card-footer rule and the sweep
  over hand-styled links (DEC-497 leftovers); sign-up lands on Home (DEC-551).
- Files and crates: `crates/web/src/item.rs`, `next_step.rs`, `palette.rs`, `log_sheet.rs`,
  `log_form.rs`, `episode_list.rs`, `episode_plan.rs`, `people.rs`, `auth_pages.rs`,
  `locales/en.ftl`, `de.ftl`; `crates/ui/src/button.rs`, `toast.rs`; `crates/contracts`
  (a `times` field on the item page DTO; `next_step` for an item without an entry if the
  server does not send it today, which the UX pass could not verify); `crates/api`,
  `crates/app` only for those two fields.
- Tests first: web unit tests for the label and count functions (en and de, plural forms:
  "Watched once", "Watched 3 times"); `episode_plan` test "a fully watched range offers N
  to log again, a mixed range reports the skipped count"; a duration parser test (`1:30`,
  `90`, `1:5`, empty, garbage); contracts round-trip test for `times`; Playwright
  `item.spec.ts`: both add buttons and Undo, the pill, the count line; `shell.spec.ts`:
  palette Ctrl+Enter shows the same toast; `people.spec.ts` and one more card page: one
  size per footer; `auth.spec.ts`: a new account lands on Home.
- Migrations: none.
- Size: M (many small parts; the UX pass sized each S).
- Depends on: ot1a committed.
- Blocked on the owner: nothing.

### ot2b - centered log dialog (OT-10)

- Scope: from 640 px a centered dialog (32 rem for watched, 42 rem for play, inner scroll,
  fixed footer), below 640 px a bottom sheet; remove the full-height grid that stretches
  the fields; focus trap, Escape, return of focus and the discard prompt as today (DEC-539).
- Files and crates: `crates/ui/src/dialog.rs` (and the sheet component it replaces for the
  log flows), `crates/web/src/log_sheet.rs`, `log_form.rs`, `gallery.rs` (the component
  gallery page).
- Tests first: `crates/ui/tests/render.rs` for the dialog variants and their classes; wasm
  tests for focus trap and focus return; Playwright: the dialog is centered at 1280 px and
  a bottom sheet at 390 px, its footer stays visible with ten players, OD-01 counts (5
  inputs as it opens); every spec that opens a sheet (`item`, `library`, `keyboard`,
  `offline`, `people`) updated first so it is red for the right reason.
- Migrations: none.
- Size: S-M.
- Depends on: ot2a (same files).
- Blocked on the owner: nothing.

### ot2c - stats led by time, step 1 (OT-17)

- Scope: a hero tile (Lucide `piggy-bank`, total hours) and four time-first tiles (movies
  hours, shows hours, pages read, hours played) from the per-kind totals the stats cache
  already stores; status counts move below; text equivalents for every figure; no pace
  wording (R-04). Home block and the top of the Stats page (DEC-542).
- Files and crates: `crates/app` (stats read model), `crates/contracts`, `crates/api`,
  `crates/web/src/home.rs`, `charts.rs`, `stats_extras.rs`, `locales/*`;
  `crates/ui/icons/icons.toml` and `config/brand/ICONS.adoc` for the icon (the SVG is
  already in `crates/ui/icons/vendor/lucide/`).
- Tests first: app test "totals per kind add up to the hero figure, kinds without time
  report none"; contracts test; web unit test for the hour formatting in en and de
  (1 hour, 1.5 hours, thousands separator); Playwright `stats.spec.ts` and `home.spec.ts`:
  tiles, their order, the text equivalent; the icon registry test.
- Migrations: none.
- Size: M.
- Depends on: nothing in ot2 (order after ot2b only because of the shared crate).
- Blocked on the owner: nothing. Shows count only what has a runtime until ot2g.
- Load run: one release run, the stats read changes.

### ot2d - colour and page widths (OT-13, OT-14)

- Scope: a violet accent and four group tints as `[theme]` / `[theme_dark]` values in the
  default brand file, neutral fallbacks in the stylesheet, the start-up contrast check
  extended to the new pairs (DEC-540); 96 rem for grid pages and the top bar, 80 rem for
  record pages, forms unchanged (DEC-541, supersedes DEC-496).
- Files and crates: `config/brand/default/`, `crates/brand` (theme values, `contrast.rs`),
  `src/web` (Tailwind input css), `crates/web/src/shell.rs`, `library.rs`, `home.rs`,
  `charts.rs`, `crates/ui` (badge, chip, button variants that take the accent).
- Tests first: brand test "every new foreground and background pair passes 4.5:1 (text)
  or 3:1 (non-text) in light and dark" with the UX pass's values (expected to confirm
  them; if one fails, the value changes, not the threshold); the white-label test still
  passes (no colour outside the brand folder except neutral fallbacks); Playwright
  `shell.spec.ts` widths per page type at a 2560 px viewport; axe on every spec in both
  themes.
- Migrations: none.
- Size: M.
- Depends on: ot2c (the tiles take the tints).
- Blocked on the owner: nothing; the owner should look at it once on a real screen before
  the tag, because nobody has seen the colours rendered.

### ot2e - rewatch run (OT-9 model A, OT-15 server part)

- Scope: an explicit rewatch run for shows: starting it sets In progress, finishing or
  stopping it sets Completed; movies stay Completed; derived from `status_change` events,
  no schema change (DEC-537). The bulk question of ot2a now starts or continues a run
  (DEC-538). First task, before code: trace the run through sync clients, the offline
  queue stamps (DEC-455) and the importers (Trakt, legacy), which the UX pass did not do,
  and record the result as the package's first DEC.
- Files and crates: `crates/domain` (status rules), `crates/app` (episode marks, status),
  `crates/contracts`, `crates/api`, `crates/web/src/episode_list.rs`, `episode_plan.rs`,
  `item.rs`, `offline.rs`, `locales/*`; `crates/importers` only if the trace says so.
- Tests first: domain tests for the state machine (start, mark, finish, stop, start again;
  a movie rewatch never leaves Completed); app tests "a stopped run leaves Completed and
  keeps the views", "the count of ot2a counts runs once"; an offline-queue test with a run
  started offline; Playwright `item.spec.ts`: start a rewatch, mark two episodes, stop.
- Migrations: none expected; if the trace shows the run needs a stored marker, one small
  migration.
- Size: M.
- Depends on: ot2a.
- Blocked on the owner: nothing.

### ot2f - plays on game pages and in the stats (OT-11 rest)

- Added 2026-11-11 from the second owner test (docs/handoff/owner-test-2026-11-10.md): OT-18 co-op play
  without a result ("Played together", the default for co-op; stored as no outcome, additive,
  old rows unchanged), OT-19 the time tiles wrap by the space of their column (container
  query or auto-fit) and the stray line beside "All-time stats", OT-20 a tile with counts but
  no known time says so ("13 episodes, runtimes not known yet") instead of "0 hours".

- Scope: a "Plays" tab on game pages (date, people, scores, winner, duration); hours and
  win rate on the Stats Play tab. The activity sentence exists (DEC-502).
- Files and crates: `crates/app` (plays read, stats), `crates/store-pg` (one aggregate
  read), `crates/contracts`, `crates/api`, `crates/web/src/item.rs`, `stats_extras.rs`,
  `game_groups.rs`, `locales/*`.
- Tests first: app tests for win rate (co-op outcomes, plays without a winner, a person
  who did not take part) and for hours; a Postgres test for the aggregate; Playwright
  `item.spec.ts` (the tab, keyboard reachable) and `stats.spec.ts`.
- Migrations: none.
- Size: M.
- Depends on: ot2c (same stats files).
- Blocked on the owner: nothing.
- Load run: one release run (stats read).

## 3. Docs-only packages that can run alongside

### dlog - the decision log in three files (DLOG-1 a, DLOG-2 to DLOG-6, then OWN-2)

- Scope: `docs/decisions/CURRENT.md`, `DECISIONS.md` (moved, a status prefix per entry),
  `BACKLOG.md`; the two new fields; owner rulings and records marked; a repository check
  for decision references (an xtask test, the one building part: S); every reference to
  `docs/DECISIONS.md` in docs, briefs and code comments updated (DEC-548).
- Tests first: the xtask check "every DEC-n named in docs and code exists; every entry has
  a status" (red on today's file).
- Migrations: none. Size: M.
- Depends on: a quiet moment: no other agent may append DEC entries during the move. Best
  slot: between two building packages.
- Blocked on the owner: nothing. Afterwards the owner reviews `CURRENT.md` by area (OWN-2 a).

### data-r - Steam, SteamDB, ProtonDB (DATA-R b)

- Scope: read terms and documented endpoints; write `docs/connectors/steam.md`,
  `steamdb.md`, `protondb.md` in the KICKOFF section 5 format with a recommendation each
  (build, link out only, or leave); licence check before reading any repository (rule 6).
- Size: S. No build.
- Blocked on the owner: Q8 decides what is built afterwards (library and playtime import,
  metadata, or a link).

### legal - what the lawyer gets (HUB-11 a)

- Scope: one document with the questions and the texts as far as they exist: contributor
  terms (before H3 opens), image terms with CC BY 4.0 and CC0 (before H4), DSA scope,
  hashes of purged TMDB text (HUB-6), the person's own title and year after a TMDB purge
  (OWN-1), movies and shows on a CC0 hub (HUB-4), the deletion rule of DEC-533.
- Size: S. No build.
- Blocked on the owner: engaging the lawyer (Q9).

## 4. Security and operations

### ot1b - carry-over of the fix pack (OT-3 steps 1 and 2, OT-2)

- Added 2026-11-11 from the owner (OT-21): covers flash during a background import. Keep
  card and `img` elements across refetches (keyed lists, same `src` keeps the loaded image),
  coalesce entity events during an import (one refetch per few seconds, not per item), and a
  neutral placeholder instead of alt text while a cover loads. Test: Playwright with an import
  running, the cover `img` of a visible card is not replaced (same element handle).

- Scope: as diagnosed in `docs/fable/round2/bug-diagnosis.md` (search tie-breaks so an
  original ranks above a derivative title; covers in palette rows). Not part of the review
  page; listed so it is not lost. Re-check the ranking again after d2.
- Files and crates: `crates/search`, `crates/web/src/palette.rs`, `crates/ui/src/command.rs`.
- Tests first: `search/tests/suite.rs` tie-break block on both backends; wasm test for the
  leading slot; unit `to_item_keeps_the_cover`.
- Migrations: none. Size: S-M.
- Blocked on the owner: the diagnosis notes a hotlink policy question for OT-2; default:
  only covers already cached by the instance are shown.

### sec0 - body limit and webhook errors (SEC-2, SEC-9; defaults, not owner-reviewed)

- Scope: the REST body limit (2 MiB) on the pages router; generic error text for webhook
  senders.
- Files and crates: `apps/server`, `crates/api`, `crates/integrations`.
- Tests first: a server test posting the limit plus one byte to a server function and
  expecting 413; an integrations test that a failing webhook answers without detail.
- Migrations: none. Size: S.

### ops1 - image and first run (OPS-1, OPS-2, OPS-7, OPS-9, OPS-12 a, OPS-6 a, SEC-6 a)

- Scope: the `cli` binary in the image; the three first-run keys in `config/.env.example`;
  a "Behind a reverse proxy" page with Caddy and nginx snippets; compose requires its
  password variables, binds to 127.0.0.1, warns at start-up, stop grace period 30 s, a
  commented `web` / `worker` pair; the backup, restore and upgrade runbook ("take the dump
  first", dump-restore is the only rollback); the server logs the migrations it is about
  to run and gets a migration dry run; the docs state "one web process plus any number of
  workers" (DEC-536, DEC-543).
- Files and crates: `src/docker/Dockerfile`, `compose.yml`, `compose.podman.yml`,
  `config/.env.example`, `docs/modules/ROOT/` (operations pages), `apps/server`,
  `apps/cli`, `crates/store-pg` (dry run).
- Tests first: a store-pg test "dry run lists the pending migrations and changes nothing";
  an xtask test that every key the first-run page names is in `.env.example`; a compose
  lint test (required variables have no default, the port binds to loopback) that reads the
  YAML. The image itself cannot be started on the build host (no Docker); the owner's
  check "compose healthy within 60 s" (OWN-8) covers it.
- Migrations: none. Size: M, mostly docs and config.
- Blocked on the owner: Q1 (OPS-3+4, where the secrets key lives and how the backup is
  prompted). Until answered the runbook says where the key is and that it must be backed up
  separately; nothing moves.

### sec1 - headers, CSP, bundle names (SEC-1 a, OPS-8)

- Scope: all security headers set by the app; a Content Security Policy with a per-response
  nonce for the hydration and theme scripts; cache directives; hashed file names for the
  app bundle; a config switch to run the CSP report-only (DEC-536).
- Files and crates: `apps/server` (middleware), `crates/web/src/app.rs`, `shell.rs` (the
  inline scripts get the nonce), `crates/config`, the reverse-proxy page of ops1.
- Tests first: server tests per header on a page, a REST answer and an asset; "every
  inline script of the rendered shell carries the response's nonce"; the whole Playwright
  suite with a listener that fails on any CSP violation in the console (this is the test
  that will be red first).
- Migrations: none. Size: M.
- Depends on: ot2 done (the CSP must be tested against the final pages).
- Blocked on the owner: nothing.

### sec2 - outbound confinement and sign-in throttling (SEC-4 b, SEC-5 b)

- Scope: (1) the governed HTTP client follows no redirects (or re-checks the allowlist per
  hop); a private-range filter for OIDC endpoint hosts and the mail host whose default
  follows the hosting mode (allowed in home, blocked in hosted), with
  `notify.allow_private_targets` as the explicit override; categorised errors for
  discovery and mail-test failures (DEC-534). (2) Sign-in: a budget per client address, a
  cap on concurrent password verifications, a progressive delay instead of the hard lock,
  behind a counter interface; in-memory store first; the Postgres store as the last,
  separable step (DEC-535). Break-glass keeps its audit row and admin notification.
- Files and crates: `crates/catalog-core/src/http.rs`, `crates/auth` (`oidc.rs`, `lib.rs`),
  `crates/api/src/admin/` (mail test, provider test), `crates/config`, `crates/store-pg`
  for the last step, `crates/web/src/sign_in.rs` (the "try again in N seconds" text),
  `locales/*`.
- Tests first: http test "a redirect to a private address is not followed"; oidc tests for
  a discovery document naming a private `jwks_uri`, in home mode (allowed) and hosted mode
  (refused, categorised error); auth tests with a fake clock: the delay grows, a correct
  password after the delay signs in, an address over budget is refused for every name,
  the admin name cannot be locked by wrong guesses from another address, the cap queues
  the ninth concurrent verification; Playwright `signin.spec.ts` for the delay text and
  the break-glass path.
- Migrations: one small table for the counters, only with the last step.
- Size: M.
- Blocked on the owner: Q6 (may the Postgres counters wait). Everything else ships
  without the answer.

### sec3 - account deletion and records without names (SEC-3, DEC-533)

- Scope: deleting an account removes the person's search documents (library and people),
  stored objects (import files) through the budget, and invite addresses; `admin_audit`
  loses `actor_name`, gets a random per-user subject for actor and user targets, a typed
  detail without names, and client addresses are blanked at deletion; the admin audit page
  resolves names at read time and shows "Deleted member" otherwise; a list of deleted
  subject ids for restores; the privacy page and the backup runbook state what is kept and
  for how long. The answers file 11.3 has the conditions.
- Files and crates: `crates/app/src/users.rs`, `crates/store-pg` (repos, migration),
  `crates/search` (indexer deletes), `crates/assets`, `crates/api/src/admin/`,
  `crates/web/src/admin/`, `settings.rs` (privacy text), `locales/*`, operations docs.
- Tests first: the audit's end-to-end test: a user with a note, a people entry and a
  dry-run import is deleted; assert no search document on both backends, no stored object,
  no username or address in any audit row, and that the rows of that person still share
  one subject; a test that fails when any audit action writes a key outside its allowlist;
  a migration test on a copy of today's rows (names gone, subjects backfilled per actor);
  Playwright `admin.spec.ts`: the audit page shows a living member by current name and a
  deleted one as "Deleted member".
- Migrations: one (`audit_subjects`, columns on `admin_audit`, backfill).
- Size: M-L.
- Blocked on the owner: Q3 (keep or replace the display-name snapshot in other members'
  people entries). Default until answered: today's behaviour, named in the privacy page.

### sec4 - session cap and session list (SEC-10 a)

- Scope: an absolute maximum session age (default 90 days) editable in the admin UI under
  the OD-11 env-override rule; a block "Sessions" in Settings > Account with device hint,
  last use, "Sign out" per row and "Sign out everywhere else"; counts against OD-01.
- Files and crates: `crates/auth`, `crates/store-pg`, `crates/config`, `crates/api`,
  `crates/contracts`, `crates/web/src/account.rs`, `settings.rs`, `admin/`, `locales/*`.
- Tests first: auth tests with a fake clock (a sliding session ends at the cap; the cap
  set in env shows read-only); api tests "user B cannot list or end A's sessions";
  Playwright `settings.spec.ts`: two contexts, one signs the other out.
- Migrations: one if the sessions table lacks a creation time or a device hint (to check
  first). Size: M.

### prefs - personal preferences on the server (C10 a, DEC-549)

- Scope: a `user_preferences` store for the shortcuts switch, remembered disclosures, the
  remembered group, the nudge snooze and the episode "ask once"; the browser keeps a copy
  for the first paint; one-time upload of existing browser values after sign-in.
- Files and crates: `crates/domain`, `crates/app`, `crates/store-pg`, `crates/contracts`,
  `crates/api`, `crates/web/src/keyboard.rs`, `nudge.rs`, `widgets.rs`, `library_state.rs`,
  `episode_list.rs`, `crates/ui/src/disclosure.rs` (the storage hook).
- Tests first: app tests (set, get, unknown key refused, another user's keys unreachable);
  Playwright: set the shortcuts switch in one browser context, see it in a second.
- Migrations: one table. Size: M.

### test1 - isolated browser specs, flaky tests fail (TEST-2, TEST-1 a, DEC-545)

- Scope: a serial Playwright project for specs that change instance-wide state, per-test
  admin accounts; a JSON reporter and a scan step that fails the run when a test passed
  only on retry.
- Files and crates: `src/web/playwright.config.ts`, `src/web/tests/helpers.ts`,
  `global.setup.ts`, `xtask` (the scan).
- Tests first: an xtask unit test for the scan on two recorded reports (one flaky, one
  clean).
- Migrations: none. Size: S-M. Costs one or two full e2e runs to prove.

### test2 - per-user import lane (TEST-8 a, DEC-545)

- Scope: first settle whether the lane is shared or per user (the audit and the review
  page disagree); then one import lane per member, ordered within the member; lower the
  import timeouts in the browser tests to 60 s.
- Files and crates: `crates/jobs`, `crates/importers/src/runner.rs`, `crates/store-pg`
  (queue claim), `src/web/tests/imports.spec.ts`.
- Tests first: a jobs test "member B's import starts while member A's large import runs;
  A's two imports still run in order".
- Migrations: possibly an index on the queue for the lane key. Size: M.

### label - the person's own title and year (OWN-1 c, DEC-550)

- Scope: a per-person label (title, year) written when the person adds or first logs an
  item, backfilled once; the TMDB purge of DEC-466 leaves it; library, lists, history and
  search documents show the label when the catalogue title is gone; the placeholder stays
  only where no person's label exists.
- Files and crates: `crates/domain`, `crates/app` (library, purge), `crates/store-pg`,
  `crates/catalog-core` (retention), `crates/search` (documents), `crates/contracts`,
  `crates/web` (title fallback in one place).
- Tests first: app test "after a purge the library entry of A shows the title A added it
  under; an item nobody refers to is still deleted"; a retention test that the purge does
  not touch labels; a search test that a purged item is still found by its label for its
  owner only.
- Migrations: one (label columns or table, backfill). Size: S-M.
- Blocked on the owner: nothing; the legal point goes to the lawyer package.

### W1 - attribution page (existing plan; `docs/fable/round2/attribution-and-connectors.md`)

- Unchanged in scope; changed in content by the answers: no BGG entry (DEC-530); TVmaze and
  Wikipedia credit lines join once d3 and d4 exist; the page must be inside the image
  before a public image (DEC-543). Size: M.

### arch-q - quick wins (WP-A1, WP-A2, WP-C2; ARCH-0 b; defaults, not owner-reviewed)

- Scope: cache the parsed translation bundles per locale and brand; batch the DTO builders
  (covers, users); architecture rules and leaf crates, rename the config crate, the cli no
  longer compiles the web crate.
- Tests first: the architecture test extended with the new rules (red first); a query
  count test for a library page of 50 rows.
- Migrations: none. Size: M in total; contains PERF-3 and PERF-4, so W2 gets shorter.

### base - export round trip and the migration baseline (OWN-9 a, DEC-543)

- Scope: prove that the product's own export and import carry everything a member has
  (library, history, plays, people, lists, reviews, tags, goals, preferences after prefs),
  then squash all migrations into one baseline and document the re-creation of an older
  instance.
- Files and crates: `crates/importers` (own format), `crates/store-pg/migrations`,
  `xtask` (sqlx offline data), operations docs.
- Tests first: a round-trip test: seed a full instance from the testkit, export, import
  into an empty one, compare the read models; a schema test: the baseline produces the
  same schema dump as the 31-plus migrations.
- Migrations: replaces all. Size: S-M.
- Depends on: every migration-adding package planned before the first public image is in
  (sec2 last step if confirmed, sec3, sec4, prefs, label, test2).
- Blocked on the owner: the owner exports before and re-imports after; and says whether
  the home instance holds anything the export does not carry.

### ops2 - build profile and public images (OPS-B a, OPS-I b, W0, DEC-543)

- Scope: the Dockerfile structure of the ops review; measure thin against fat link-time
  optimisation (peak memory, bundle size) and put the numbers to the owner; the release
  build check of the image's feature set in CI (W0); a tag job that pushes images to the
  public registry.
- Files and crates: `src/docker/Dockerfile`, `.gitlab-ci.yml`, workspace `Cargo.toml`
  profiles, operations docs.
- Tests first: the CI job itself is the test; locally one release build per profile, one
  at a time, with the host otherwise idle.
- Migrations: none. Size: M.
- Depends on: ops1, sec0, sec1, W1, base.
- Blocked on the owner: Q4 (which registry and image name); the profile choice after the
  measurement.

### W2 - performance (PERF-T b; PERF-0 to PERF-9 are defaults, not owner-reviewed)

- As in `perf-proposals.md` on the review branch, targets accepted. PERF-5 (zone-change
  rebuild) after ot2c and ot2f changed the stats reads; PERF-3 and PERF-4 are already in
  arch-q. Each step ends with one release load run, so the workstream runs alone on the
  host. Two steps have migrations (PERF-8, PERF-9): schedule them before base or after it,
  not across. Size: L in total.

## 5. Data and hub

### d1 - licence tag per field and asset (WP-OD2; a default, precondition of DATA-0 a)

- Scope: each catalogue field value and asset records its source and licence; the
  retention job, the attribution page and later the hub filter key on it.
- Files and crates: `crates/domain`, `crates/catalog-core`, `crates/store-pg`,
  `crates/connectors` (every connector states its tag), `crates/assets`.
- Tests first: per connector "every field it writes carries its tag"; a retention test
  that only cache-limited tags are purged; a store test for the backfill.
- Migrations: one (provenance columns, backfill from today's provenance). Size: M.

### d2 - Wikidata identity for all kinds (DATA-0 a, DEC-546)

- Scope: the generalised `wikidata` connector (search in the user's language, entity
  fetch, a property map per kind, ids to TMDB, TVDB, TVmaze, IGDB, Steam, Open Library,
  MusicBrainz), tagged CC0; field priority: Wikidata identity above every proprietary
  source; TMDB and IGDB become enrichment an operator switches on; the etiquette rules of
  DEC-192 (User-Agent from the brand, serial requests, maxlag, backoff).
  `docs/connectors/wikidata.md` first (KICKOFF section 5).
- Files and crates: `crates/connectors`, `crates/catalog-core` (priority, merge),
  `crates/kinds` (property maps), `crates/search` (ranking with two sources),
  `crates/web/src/source_hints.rs`, admin sources page, `locales/*`.
- Tests first: mapper tests per kind from saved, synthetic entity JSON; a merge test "the
  title comes from Wikidata when both sources have one; the poster from TMDB"; a retention
  test "after a TMDB purge an item with a Wikidata id keeps title and year"; the search
  suite's original-before-derivative case (OT-3) with both sources present; Playwright
  `sources.spec.ts` with TMDB switched off: search and add a film.
- Migrations: possibly none beyond d1. Size: L.
- Depends on: d1. Load run: one (search and item reads).

### d3 - TVmaze as the default show source (WP-OD3 a, DEC-546)

- Scope: shows, seasons, episodes, air dates and episode runtimes from TVmaze (terms in
  `docs/connectors/tvmaze.md`), tagged CC BY-SA 4.0 with a credit link on item pages and
  the attribution page; TMDB and TheTVDB behind it; share-alike values marked "never
  proposed to the hub".
- Files and crates: `crates/connectors`, `crates/catalog-core`, `crates/web/src/item.rs`
  (credit line), `locales/*`.
- Tests first: connector tests on recorded or synthetic fixtures (marked as such); an
  episode merge test (TVmaze episode list wins, user progress is kept when numbering
  differs); the provenance test "a TVmaze value is not hub-shareable".
- Migrations: none expected. Size: M.
- Depends on: d1, d2 (ids from Wikidata map shows to TVmaze).

### ot2g - per-episode runtimes and the "Up to date" badge (OT-17 step 2, DEC-450 items 1 and 2)

- Scope: stats minutes from the runtime of each logged episode; a dirty trigger on
  `catalog_episodes`; a one-time rebuild of the stats cache; the badge from air dates.
- Files and crates: `crates/store-pg` (statement, trigger), `crates/app` (stats),
  `crates/web/src/item.rs`, `stats_extras.rs`.
- Tests first: a Postgres test "two episodes of 22 and 45 minutes count 67, not twice the
  series value; an episode without a runtime falls back to the series value"; a rebuild
  test; Playwright `stats.spec.ts`.
- Migrations: one. Size: M. Depends on: d3, ot2c. Load run: one.

### d4 - Wikipedia summaries (WP-OD7 a)

- Scope: descriptions from Wikipedia through the Wikidata sitelink, CC BY-SA 4.0 with a
  credited link, on by default, per language with fallback; never hub-shareable.
  `docs/connectors/wikipedia.md` first.
- Tests first: connector tests on fixtures; "the description shows its credit link"; the
  provenance test. Migrations: none. Size: M. Depends on: d2.

### arch-b - the three large restructurings (ARCH-0 b, WP-B2 a, DEC-544)

- Order: WP-B3 split `api` (L), WP-B2 remove the Backend pass-through (M, risk M), WP-B1
  split `web` (L). No user-visible change; each ends with the full suite. The architecture
  test is extended first in each (red), and must keep `web` free of server crates under
  `hydrate`. Nothing else builds while one of them is open, and no other package should be
  open on `api` or `web` across them.

### h1 - hub: role, read API, connector, Wikidata seed (HUB-0 a, DEC-531)

- As WP-H1 of `hub-design.md` section 12 on the review branch (scope, files, tests first,
  acceptance are written there), with these changes from the answers: the licence in
  `GET /meta` is CC0 1.0 (HUB-1 a); the BGG id is a plain identifier field (HUB-5 a);
  `docs/connectors/bgg.md` says "not built" (DEC-530); search on Postgres full text and
  trigram (HUB-14, a default); the kind list is written so that further kinds are added by
  descriptor, because the target is every kind (DEC-532); the owned flag and "Mark owned"
  (DEC-420) are part of it; file names follow the crates as arch-b left them.
- Migrations: two (hub tables, hub client tables). Size: L.
- Depends on: arch-b, d1 (licence tags), d2 (the Wikidata client and etiquette).
- Blocked on the owner: nothing for H1.

### h2 - hub: peers and signed requests (WP-H2)

- As WP-H2 of the hub design; registration by admin approval (HUB-9 a). Size: L.
- Blocked on the owner: a second instance to connect (the owner's own; the staging hub of
  HUB-13 can be that instance).

### d5, d6, d7 - new connectors (DATA-K a)

- d5 Open Library plus DNB (books; `docs/connectors/openlibrary.md` exists, DNB's page
  first); d6 MusicBrainz plus Cover Art Archive and the ListenBrainz importer; d7 VNDB
  (free for non-commercial use only: off in any paid offer). Each M, each with its
  connector doc first, fixtures, and a flag that is off by default where terms are unclear
  (Q11). They touch only `crates/connectors`, `crates/importers` and the sources page, so
  they can be moved earlier if the owner wants books before the hub.

## 6. Later, and what waits for the owner

| Item | Waits for |
|---|---|
| h3 contributions (provenance filter, hashes, review queue) | lawyer: contributor terms before it opens beyond the owner's instances; building may start before |
| h4 images (CC BY 4.0 and CC0 only) | lawyer: image terms; Q5 (confirm no revocable licence) |
| h5 lawful (accounts, deletion by the DEC-533 rule, notice-and-action) | lawyer: DSA scope |
| h6 hub operations, staging flag | the owner hosts the staging instance (HUB-13 a) |
| h7 and further kinds; audiobook editions (WP-OD8 a); movies and shows last | Q2 (scope reading, bridge or full kind, order); lawyer for movies and shows |
| Steam, SteamDB, ProtonDB | data-r result, then Q8 |
| W4 anime and manga | the owner's four actions (OWN-4 a); Q7 (MangaDex) |
| Importers: Goodreads, BG Stats, BGG collection, Trakt raw export | the sample exports (OWN-3 a) |
| Re-authentication fallback for admins without a password | the owner's authentik check (OWN-8 a) |
| OPS-3+4 secrets key location and backup prompt | Q1 |
| Postgres sign-in counters | Q6 |
| Name snapshots after deletion | Q3 |
| Public registry | Q4 |
| W6 web push, W7 "date unknown" on events, comics (WP-OD9 b: not now) | nothing; unscheduled |
| Saved views | the funnel review on 2026-12-06 (OWN-5 a) |
| Remaining defaults not placed above (SEC-7, SEC-8, OPS-5, OPS-10, OPS-11, CI cache, WP-C1, WP-C3, WP-D1, WP-D2, WP-E1, TEST-3 to TEST-7, TEST-9, TEST-10, C2, C4, C11, C12, WP-OD10, HUB-8) | nothing; small, to be attached to the package that touches the same files (C4, the History year filter in the user's time zone, is a real bug and should ride with ot2f or sec4) |

## 7. Not verified

- No code was built or run for this plan. File lists are where the work is expected from
  the crate layout and the reviews; a package's first step is to confirm them.
- Sizes are estimates; the UX sizes come from `ux-owner-test.md` section 5, the hub and
  data sizes from the round-1 reviews.
- Whether the sessions table already holds a creation time and a device hint (sec4),
  whether the server sends `next_step` for an item without a library entry (ot2a), and
  whether the import lane is shared or per user (test2) were not checked here.
- The state of ot1a (uncommitted, under review fixes) is taken from its progress log.
