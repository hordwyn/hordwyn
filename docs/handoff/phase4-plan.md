# UX phase 4 plan: fluency and measurement (2026-11-08)

Written by a planning agent from HANDOFF.md (sections 0, 2, 3, 4, 7, 8), phase3-plan.md
(format), design.md 9 "Phase 4" and the sections it points to (3.2 keyboard map, 3.4 next step
and dates, 3.5 bulk bar, 4.1 fair share, 5.3/5.4 palette and offline, 6.1-6.4, 6.6, 6.7, 6.15,
8 Keyboard), addendum-1 1.2 D16 and 3.4 (probes, Users, stacked tables), research.md R-04 and
R-54, and every DEC deferring to phase 4: 235, 237, 247, 285, 286, 312, 316, 326, 335, 336,
344, 353, 355, 363, 382, 383, 384 (UX3-25), 388, 390 (closed by 397), 394, 396.
Eight packages, run in order, one cargo build at a time. Paths are relative to `src/rust`
unless they start with `src/web` or `docs`.

## Already in the code (do not rebuild)

- Keyboard: `shell.rs` `shortcut_keys` (~240) has `?` help and `N` (add); `ShortcutHelp` (~757)
  lists Ctrl+K, N, ?, Esc; `palette.rs:171` Ctrl/Cmd+K; `library.rs:206` `/` focuses the filter;
  log sheets save on Ctrl+Enter (`log_sheet.rs:295,546`). No G-chords, J/K, W/T/L/R, roving
  tabindex, palette Ctrl+Enter or recent items.
- Palette: scope segments, source footer, no-results card; `PaletteTarget(RwSignal<Option<(String,String)>>)`
  (`palette.rs:29`) scopes it to list add; import matching uses `MatchTarget`; `ui::command.rs`
  has no `on_secondary`.
- Offline: `shell.rs:723` `provide_online` drives `OfflineBanner` and disables forms; client op
  ids sent by `next_step.rs:124`, `episode_list.rs:113`, `log_form.rs:98,245`; `POST /sync` push
  idempotent by op_id (`api/src/sync.rs`; commands Add, SetStatus, Rate, SetNotes, LogProgress;
  no session command).
- Library: list view uses `ui::Table` without selection; `QuickMenu` (`widgets.rs` ~220) has the
  next step, statuses, tags. NO remove-from-library use case anywhere (app, api, store) and NO
  "owned" flag.
- Stats: `app/src/stats.rs` totals/series via `ProgressRepo::month_amounts`
  (`store-pg/src/repos/library.rs:497`), one CTE over all the user's effective events in the
  user's zone with the pages/positions forward-move rules; `store-pg/tests/phase1.rs` compares
  it with `app::month_amounts`. Stats page `home::Stats`, month/year tabs per group; no
  per-person view, year-over-year or goals.
- Activity: `app/src/activity.rs` windows events (`HistoryWindow`, index
  `progress_events_user_recent`) but reads ALL own sessions (`my_sessions` ->
  `SessionRepo::by_owner`) and all shared sessions (`with_participants`); `feed_context` looks
  up companions one by one.
- Load test `apps/server/tests/load.rs` (`#[ignore]`): 50 000 movies, `LOAD_EVENTS` (default
  5 000), 500 books in series, 50 games with 500 plays, 300 ms p95 budget, single client, debug
  only. ux3 p95: search 106, Home 164, stats 90, library 64-89, activity 13 ms; ux1 with 40 000
  events: stats 197, Home 270 ms.
- Jobs: lanes, cron (`cron_schedules`), `lane_counts`; `catalog-core/src/gate.rs` orders waiters
  by lane then FCFS; `Ctx` (`catalog-core/src/connector.rs:16`) carries `user_key`, no user id.
- Admin: `HealthRowDto` (`contracts/src/admin.rs:233`) computed in the request; `SourceStats`
  (`catalog-core/src/stats.rs`) in memory per process; `password_resets` and the mailer exist;
  CLI `admin reset-totp`; `GET /auth/me/identities` and self-unlink exist; `ui::Table` has a
  focusable scroll wrapper (DEC-337).
- Episodes: `SeasonInfo{number, episode_count}` and `first_air_date` only
  (`kinds/src/builtin/screen.rs`); TMDB `details` reads `/tv/{id}` seasons without episode lists
  (`connectors/src/tmdb.rs:165-297`); `BulkDates` offers Now and date range
  (`web/src/episode_plan.rs`).
- People: `companion_stats`, merges, lists; `log_sheet.rs:454` prefills the last group that
  played a game; no game-group entity.
- Telemetry: metrics are `tracing` events with target `metrics` (`crates/telemetry`, DEC-060);
  `ui::Disclosure` has `storage_key`; `blake3` is a workspace dependency.

## WP4-1 measure-perf (server only; measure first, then optimise)

- Load harness (HANDOFF 7): extend `load.rs` with `LOAD_CLIENTS` (concurrent clients running the
  mixed set), `LOAD_SESSIONS` (own plays, default 500), `LOAD_SHARED` (sessions a linked user
  shares), optional writers logging progress during reads; new measured endpoints
  `/stats/series`, History with person and year filters, `POST` progress; one summary line per
  endpoint for a DEC. New `cargo xtask load [--release] [--clients N]` wrapping `cargo test
  [--release] -p server --test load -- --ignored --nocapture`; not in `ci`; refuses below 15 GB
  free disk (release is a second tree); delete `target/release` afterwards.
- Baseline before any change, four runs recorded in a DEC: (a) debug 5 000 events; (b) debug
  40 000 events + 20 000 sessions + 2 000 shared; (c) release, data as (b); (d) release
  `LOAD_CLIENTS=8` with 2 writers.
- Incremental stats totals (DEC-235): table `stats_item_months` (user, item, kind, month in the
  user's zone, the `Amounts` columns, seconds) + `stats_dirty (user_id, item_id)` + a user-level
  zone stamp. Triggers (as migration 0015) mark items dirty on progress event insert (undo
  incl.), session duration update/delete, `catalog_items.attributes` change (runtimes, for every
  library user of the item), `users.time_zone` change (invalidates the user), new library entry.
  `month_amounts` keeps its signature; reads recompute only dirty items with the existing CTE
  restricted to `item_id = ANY(...)`, then `SUM ... GROUP BY kind, month` over the cache; full
  rebuild when the zone stamp differs. Imports and sync are covered by database triggers.
- Paged activity (DEC-235): `SessionRepo::by_owner_window` and `with_participants_window`
  (before, since, limit) on `sessions_owner (owner_id, started_at DESC)` (+ `(started_at DESC,
  id)` index if EXPLAIN needs it); `activity_feed` pulls sessions in the same window as events;
  batch companion/owner names (`companions.get_many`).
- Fair share (design.md 4.1 "rate-limit protection", 9): hosted mode only, interactive waiters
  served round-robin per user then FCFS; `Ctx` gains `user: Option<UserId>` set by search; the
  gate reads `instance.hosting` live.
- DEC-286 half: TheTVDB token cache keyed on `blake3(pin)`.
- Acceptance (debug, 4 vCPU, 50 000 items; budget assert stays 300 ms p95): stats totals p95 <=
  40 ms at 5k events (ux3 90) and <= 60 ms at 40k (ux1 197), ratio p95(40k)/p95(5k) <= 1.5,
  `/stats/series` <= 60 ms at 40k; Home <= 120 ms at 5k (164) and <= 150 ms at 40k (270);
  activity 20 rows <= 30 ms with 20 000 own + 2 000 shared sessions and within 2x of the
  500-session number; History person filter <= 100 ms; POST progress <= 100 ms with the trigger
  adding <= 10 ms; first stats read after a zone change (full rebuild) <= 500 ms at 40k once;
  release single client every endpoint <= 100 ms (target); release 8 clients + 2 writers every
  endpoint <= 300 ms (asserted); debug 4 clients reported only. A missed target still under 300
  ms: record numbers and reason in a DEC and continue (rule 2).
- Files: `apps/server/tests/load.rs`, `xtask/src/main.rs`,
  `crates/store-pg/src/repos/{library.rs,sessions.rs}`,
  `crates/app/src/{stats.rs,activity.rs,sessions.rs,ports.rs}`,
  `crates/catalog-core/src/{gate.rs,connector.rs,http.rs}`, `crates/connectors/src/tvdb.rs`,
  migration `0023_stats_cache.sql`, `.sqlx/`.
- Tests first: `store-pg/tests/phase1.rs` cache-backed `month_amounts` equals the from-scratch
  CTE and `app::month_amounts` after each step of a script (logs, undo, page moving back,
  session duration edit, runtime change via refresh, zone change, entry removed and re-added,
  import batch); `app/tests` activity pages of size 1..7 over interleaved events, own and shared
  sessions return exactly the old full read's rows in order with `next` correct at the year
  bound; gate unit test (1 permit; A queues 5 interactive, B 1; B second in hosted, sixth in
  home); TheTVDB cache key is not the PIN.
- Defaults: lazy dirty-set recompute on read (log the per-(user, kind, month) rollup
  alternative if the target is missed); activity `year` filter stays UTC; fair-share user from
  the search `Actor`, background lanes unchanged.

## WP4-2 admin-ops (server, then web admin and sign-in)

- Probes table (DEC-285, addendum-1 3.4): `health_probes (id, status, detail, args,
  checked_at)` written by cron `admin.probes` on `background_refresh` every 60 s (database,
  search, storage, jobs, mail last test, identity provider discovery/cache age/last callback,
  secrets active version/rows per version/locked count, data sources); Overview reads the
  table, never probes in the request; rows older than 3 minutes say "not checked recently".
- Persisted source counters (DEC-247, 326): each process flushes hourly buckets every 60 s into
  `source_stats (connector, hour, calls, rate_limited, last_ok_at, last_error_at, last_error,
  last_401_at, circuit_open_until)` additively; status = database + local unflushed deltas. The
  back-channel `jti` replay cache moves to a table with expiry (`oidc_used_jti`).
- DEC-286 rest: `locked_totp_count` by stored key version without decrypting.
- Users row menu (DEC-336): "Reset authenticator" `POST /admin/users/{id}/reset-totp` (gated,
  audited in the transaction, notifies the person); "Unlink provider" `DELETE
  /admin/users/{id}/identities/{identity}` (gated, audited, 409 `last_sign_in_method`); web row
  menu items with a confirm naming the consequence.
- Linked accounts column (DEC-388 (1)): count of `oidc_identities` per provider in `GET
  /admin/oidc` and the providers table.
- Localised health times (DEC-336): args carry `at` plus a server-computed local time in the
  admin's zone; web shows it with the i18n date formatter.
- Stacked admin tables (DEC-388 (2)): `ui::Table` `stacked` mode below 47.5rem (row -> card with
  labelled fields) for providers, Users, delivery log; the rules matrix keeps the scroll wrapper
  (log why).
- Emailed one-time link (DEC-312, 316; addendum-1 4.2 step 3): `/sign-in/link` offers "Email
  me a link" when the account has no password and mail works; to the stored verified email; 30
  min, single use, hashed (password_resets pattern, new purpose), must be opened in the browser
  holding the link ticket cookie; throttled, audited; without mail "ask your admin" stays.
- OD-21 "Keep email in sync": not built - the owner ruled "no email sync" (HANDOFF 4.3); record
  that HANDOFF section 0's "email sync" is closed by that ruling.
- Files: `crates/api/src/admin/{mod.rs,oidc.rs,mail.rs}`, `crates/catalog-core/src/stats.rs`,
  `crates/auth/src/{oidc.rs,pg.rs,admin_pg.rs}`, `crates/jobs`,
  `apps/server/src/{lib.rs,admin_wiring.rs}`, `crates/contracts/src/admin.rs`,
  `crates/ui/src/table.rs`, `crates/web/src/admin/{overview.rs,users.rs,sso.rs}`,
  `crates/web/src/{notify_channels.rs,sign_in.rs}`, Fluent en/de, migration `0024_admin_ops.sql`.
- Tests first: admin.rs (Overview reads probe rows and makes zero provider calls during the
  request; the probe job writes 8 rows; stale row reported); two `SourceStats` (web and worker
  roles) on one database show the worker's 429 on the web role; admin reset-totp/unlink gated,
  audited, last-method refusal; linked-accounts count; oidc.rs emailed link links once, second
  use/expiry/other browser refused, no mail -> no offer, jti replay via a second router refused;
  admin.spec (row menu confirm, stacked Users at 360 px with axe, localised Overview time);
  signin.spec emailed link via the mail sink.
- Defaults: probe 60 s, stale 180 s; flush 60 s; link 30 min. If short on time, cut at the seam:
  server half first, web half as WP4-2b.

## WP4-3 bulk-lists (server and web)

- Remove from library (needed by the bulk bar, quick menu 6.2, item overflow 6.3):
  `App::remove_from_library` deletes the entry and its tags, keeps progress events, sessions and
  list memberships ("delete keeps history", DEC-348); re-add restores counts; stats keep counting
  history; undo restores status, rating, notes, tags from a snapshot returned to the client.
- Bulk API `POST /library/bulk {item_ids, op}` (set_status, add_tag, remove_tag, add_to_list,
  remove), one transaction per op, <= 500 ids, per-id results + one undo token; server fn in api.
- Bulk bar (design.md 3.5, 5.6, 6.2, 7.1): checkboxes in list view only, "Select all on this
  page", sticky bar with count, Set status, Add tag, Add to list, Remove (confirm with count),
  toast with Undo; selection survives background refresh (as DEC-398), cleared on filter change.
- Quick menu "Add to list" and "Remove"; item overflow "Remove from library".
- Drag handles on the list page (DEC-353): pointer drag on top of keyboard Move up/down; handle
  `aria-hidden`, buttons stay (WCAG 2.5.7).
- Files: `crates/app/src/{library.rs,browse.rs,lists.rs}`, store-pg library/list repos,
  `crates/api/src/{routes.rs,ops.rs}`, `crates/contracts`, `crates/ui/src/{table.rs,datagrid}`,
  `crates/web/src/{library.rs,library_state.rs,widgets.rs,item.rs,lists.rs,api.rs}`.
- Tests first: app library tests (remove keeps events and stats; re-add; undo restores);
  bulk over mixed ids (a foreign id fails alone); `library_page.rs` bulk 500 ids, over the limit
  400; library.spec (select 3, Add tag, Undo; Remove with confirm and Undo; selection kept
  through refresh; axe and reflow with the bar); lists.spec (drag reorder persists, keyboard
  still works).
- Defaults: no "Mark owned" (no owned flag until board games; log); bulk only in list view;
  removal keeps sessions and list items.

## WP4-4 keyboard-palette (web, small server additions)

- Global map (3.2): `G then H/L/Y/T/P` -> Home, Library, History, Lists, People; 1.5 s chord
  window; visible "G..." hint in a polite live region; never while typing, in a dialog or with a
  modifier; `?` help lists everything, per page too.
- Home (6.1): J/K between Continue cards, Enter runs the focused card's next step.
- Library (6.2, 8): roving tabindex in the grid (arrows by row/column from the rendered column
  count, Home/End), Enter opens, W next step, T tags, L list picker.
- Item page (6.3): W next step, R focuses rating, T tag, L list.
- Palette (5.3, 6.4, 3.4): Ctrl+Enter on a provider hit adds as Planned, keeps the palette open,
  toast "Added to Planned" with Undo (WP4-3 remove); Ctrl+Enter on a library hit runs its next
  step; active row shows Enter / Ctrl+Enter hints; `ui::Command` gains `on_secondary`.
- Empty palette (< 2 chars): recent items (last 8 opened, localStorage per user, never sent) and
  suggested commands ("Go to Lists", "New list", "Log a play of <last game>", "Import a file").
- `PaletteTarget` becomes an enum (list add, import match, notification rule subject, log with a
  person): "Add a rule for one item" (DEC-344; API takes `filter.items`), "Log with <person>" on
  People cards (DEC-353).
- Channel edit in place (DEC-344) via `PUT /notifications/channels/{id}`; secrets write-only,
  blank keeps the stored value.
- Files: `crates/web/src/{shell.rs,palette.rs,home.rs,library.rs,item.rs,widgets.rs,next_step.rs,people.rs,notify_rules_view.rs,notify_channels.rs,lists.rs,imports.rs}`,
  `crates/ui/src/{command.rs,kbd/copy.rs}`, Fluent en/de.
- Tests first: new `keyboard.spec.ts` (G+L lands on /library with focus on the H1; G ignored in
  a text field; `?` lists chords; arrows move focus, one card tabindex 0; W logs and toasts; J/K;
  W/R on the item page); shell.spec (Ctrl+Enter on a mock TMDB hit adds Planned, palette stays,
  Undo; recent item shown); notifications.spec (rule for one item; edit channel name keeping
  the secret); people.spec (Log with a person); wasm unit tests for roving index and the chord
  state machine.
- Defaults: recent items local only; shortcuts on, single-key ones only outside text fields,
  plus a "Keyboard shortcuts" switch in Settings > Account (WCAG 2.1.4); chord window 1.5 s.

## WP4-5 offline-funnel (server and web)

- Offline queue (DEC-237, design.md 5.4, 2.4, 6.15): offline or on a network error, next step,
  episode marks, status and rating go into a per-user localStorage queue of
  `SyncCommandEnvelope`s with the existing client op ids and client-side HLC (check `domain::hlc`
  builds for wasm); toast "Saved offline, will sync"; Undo removes from the queue; flush via sync
  push on `online` and page load; 4xx dropped with a toast naming the item, network failures
  stay; banner "N changes waiting"; sheets and bulk stay disabled offline.
- Logging funnel (design.md 9, R-54): client records flows from intent (palette open, next-step
  click, sheet open, episode mark, bulk action) to logged/abandoned with taps and elapsed time;
  one beacon to `POST /api/v1/usage/funnel` (signed in, rate-limited, no item ids); server
  aggregates into `usage_funnel_daily (day, flow, outcome, taps_bucket, ms_bucket, n)` with no
  user id; daily cron emits a `metrics` tracing event per flow (DEC-060); retention 180 days;
  `GET /admin/usage` + CSV and a "Usage measurement" disclosure on Admin > Storage and jobs;
  setting `usage.funnel` (default on, env override, admin-editable); one line on Settings >
  Privacy saying what is counted.
- D16 disclosure metrics (addendum-1 1.2 D16, 1.4): `ui::Disclosure` reports
  opened-within-2-s-of-load and open-at-save per `storage_key` into the same aggregate (flow
  `disclosure`, keyed by page/section, never by user).
- Add "review the funnel after four weeks, cut what is not used" to HANDOFF section 6 with date.
- Files: new `crates/web/src/{offline.rs,funnel.rs}`,
  `crates/web/src/{shell.rs,next_step.rs,log_form.rs,log_sheet.rs,episode_list.rs,palette.rs,library.rs,settings.rs}`,
  `crates/ui/src/disclosure.rs`, `crates/api/src/{sync.rs,routes.rs}`, new
  `crates/api/src/usage.rs`, `crates/config/src/lib.rs`, migration `0025_usage_funnel.sql`,
  `crates/web/src/admin/storage.rs`.
- Tests first: `apps/server/tests/usage.rs` (aggregate without user; schema has no user or item
  column; unsigned refused; rate limit; setting off drops; retention); sync replay twice changes
  nothing (with client HLC SetStatus); new `offline.spec.ts` (setOffline, mark next step, toast
  and banner count, online -> History once; Undo offline leaves nothing); home.spec beacon via
  route interception; wasm unit tests for the queue and buckets.
- Defaults: queue only for existing sync commands (plays/sessions online-only; DEC); taps
  buckets 1..5+, time < 2, 5, 10, 30, 60 s and over; funnel on by default (first party,
  aggregated, no user id), admin can turn it off.

## WP4-6 stats-goals-people (server and web)

- Opt-in goals (design.md 9, R-04): `goals (id, user_id, year, group or all, metric
  completions|finished|episodes|pages|plays|minutes, target)`, private; progress from the WP4-1
  cache; Stats page "Set a goal" behind a disclosure; "7 of 12 in 2026" with ProgressBar and
  text; never "behind/ahead", never on Home (R-04); edit, delete.
- Stats polish (6.1, 6.6, R-04): "this year so far vs the same period last year" on Stats and in
  the History side panel; per-person section ("most often with" from `companion_stats`); split
  `home::MeasureChart` under 100 lines (DEC-394).
- Game groups (design.md 6.7, 9): `game_groups (id, owner_id, name, removed_at)`,
  `game_group_members (group_id, companion_id, position)`, sessions get nullable
  `game_group_id`; People tab "Game groups" (cards, plays together, last played, "Log a play
  with" via the WP4-4 scoped pick); PlaySheet group chips fill the players; CRUD dialogs; member
  merges follow companion merges.
- Files: `crates/app/src/{stats.rs,people.rs,sessions.rs,companions.rs}`, store-pg repos,
  `crates/api/src/{routes.rs,sharing.rs}`, `crates/contracts`,
  `crates/web/src/{home.rs,charts.rs,history.rs,people.rs,people_dialogs.rs,log_sheet.rs}`,
  migration `0026_goals_groups.sql`.
- Tests first: app stats (goal = year sum; not visible to a linked user; same-period at Feb 29
  and year start in the user's zone); app people (group plays; merged member stays; removed group
  keeps sessions); new stats.spec (goal set/edit/delete, not on Home, axe + reflow); people.spec
  (group, log a play, count); item.spec (PlaySheet group chip).
- Defaults: yearly goals only; one per (year, group, metric); same period = Jan 1 to the same
  local day.

## WP4-7 episode-air-dates (connectors, kinds, web)

- TMDB: per season `GET /3/tv/{series_id}/season/{season_number}` keeping only
  `episodes[].episode_number`, `air_date`, `runtime`, plus the season `air_date` from `/tv/{id}`.
  VERIFY endpoint and field names against developer.themoviedb.org "TV Seasons > Details" first
  (rule 1), record in `docs/connectors/tmdb.md`, and check the terms allow caching these fields.
  One extra call per season, only in the detail-refresh job on `background_refresh`, through the
  http cache; skip seasons whose `episode_count` and `air_date` did not change.
- TheTVDB v4: series episodes endpoint (`/series/{id}/episodes/{season-type}`, field `aired`),
  verified against the official swagger first.
- Fixtures recorded with `cargo xtask record tmdb` when a key is set, else synthetic from the
  docs examples and marked so. New table `catalog_episodes (item_id, season, number, air_date,
  runtime_minutes)`; no titles or overviews. `SeasonInfo` gains optional `air_date`.
- Web: `BulkDates` third option "On release dates", offered only when every chosen episode has a
  past air date (else a hint how many are missing); future episodes refused. Optional: runtime
  feeds stats minutes; "Up to date" badge (3.4) from air dates - else follow-ups.
- Files: `crates/connectors/src/{tmdb.rs,tvdb.rs}`, `crates/kinds/src/builtin/screen.rs`,
  `crates/app/src/catalog.rs`, store-pg catalog repo,
  `crates/web/src/{episode_plan.rs,episode_list.rs}`, `docs/connectors/{tmdb,tvdb}.md`, migration
  `0027_catalog_episodes.sql`, `src/web/tests/tmdb-mock.mjs`.
- Tests first: connector fixture tests (season parsed; unchanged season not fetched again); app
  refresh test (stored, idempotent); item.spec (mark a season on release dates, History per date;
  option hidden when dates missing).
- Default: doubt about TMDB terms/docs -> episode fetch behind a feature flag off (Q11), UI option
  shipped disabled.

## WP4-8 hardening (test debt, UX3-25, final measurement)

- UX3-25 (DEC-384): e2e captures the server log (Playwright webServer stdout to a file) and fails
  the run on any `panicked at` line; run full e2e three times; fix if reproduced, else keep the
  guard and record it. Do not retry the `SignalSetter::map`/`try_set` approach that deadlocked.
- Deferred tests (DEC-383, 396, 335), each separate: rotate/prune with `APP__SECRETS__KEYS`;
  `reload_keys` edges; reset-link expiry and disabled account; reset-totp enrolled; promote of an
  admin; mail test without `to`; locked SMTP password; `PgMailSecret::rewrap_batch`; `mail.*`
  validation; source switch blocking detail refresh; `ClientConfig.today` over HTTP; History
  `year`; back-channel 429; `/sign-in?error=` with the loop guard; DEC-338 500; `health()` states;
  cooperative win and most-played tie-break; suggestions across accounts; link-only list not in
  "Shared with me"; legacy watchlist, skips and `undated_history`; cross-user import run/match;
  upload over 64 MiB; Trakt cases; mail step via the re-auth dialog (DEC-397); axe at 360 px
  with a provider panel open in both themes; server start with encrypted rows and no key over
  HTTP, `key_unavailable`, `last_tested_at` over HTTP, silent wizard skip on a reused server;
  candidate source Test with a live key as `#[ignore]`.
- Splits: long scenario tests `apps/server/tests/{instance.rs,cli.rs}` (DEC-382, drop file-level
  allows) and `ui::DropdownMenu` (DEC-394).
- Final numbers: re-run the WP4-1 load set (debug and release, 8 clients) on the finished tree;
  numbers into HANDOFF section 3.
- Files: `src/web/playwright.config.ts`, `xtask/src/main.rs`, `apps/server/tests/*`,
  `crates/importers/tests/*`, `crates/ui/src/dropdown.rs`, `src/web/tests/{admin,signin}.spec.ts`.

## Dependencies

WP4-1 before WP4-6 (goals read the cache) and WP4-8 (final load run); WP4-3 before WP4-4
(Ctrl+Enter Undo needs remove); WP4-4 before WP4-5 (funnel instruments palette paths) and WP4-6
(group "Log a play with" uses the scoped pick). WP4-2 and WP4-7 are independent.

## Suggested deferrals (out of phase 4)

Saved views (conditional in design.md 9; decide after the four-week funnel review); "Date
unknown" on events (DEC-355, R-08; touches occurred_at across domain, sync, importers and the
stats cache - own package after phase 4, migrate legacy estimates); "Mark owned" and "unplayed
owned games" (owned flag comes with board games M10/M11); avatar upload (DEC-353); fuzzy
duplicate suggestions, restoring a removed person, revoked-link 200 (DEC-351) - kept recorded;
OD-21 email sync closed by the owner ruling; web push, attribution page, further connectors,
Goodreads/BG Stats/BGG importers - parallel workstreams; wider uses of per-episode data (next
release sort, airing notifications) - connectors workstream after WP4-7.
