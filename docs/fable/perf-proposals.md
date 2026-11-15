STATUS: COMPLETE (2026-11-09)

# Performance design proposals (post-ux4 workstream)

Read-only design document. Nothing here was compiled, run or EXPLAINed; every
number quoted comes from DECISIONS.md (DEC-235, 399-404, 416, 459, 460, 476,
493, 494) or docs/HANDOFF.md section 3. Line references are to the tree at
`main` = `ux4` on 2026-11-09 and are of the form `path:line` relative to
`src/rust/`. This builds on docs/fable/architecture-review.md section 7 and
does not repeat it.

Sections are added one at a time; the status line at the top says when the
document is complete.

## 0. Ground truth the proposals stand on

Load harness (`apps/server/tests/load.rs`, DEC-399): one seeded database (50 000
movies in the load user's library, `LOAD_EVENTS` finished events, `LOAD_SESSIONS`
own plays with the companion "Anna" on every tenth, `LOAD_SHARED` sessions of the
user "friend" with a companion linked to the load user), `LOAD_CLIENTS` concurrent
clients each running the mixed set ten times (`load.rs:205-297`), `LOAD_WRITERS`
writers logging a page every ~72 ms (`load.rs:309-316`), one timed `/stats/totals`
after `PUT /auth/me/time-zone` (`load.rs:430-436`). Budget 300 ms p95 per
endpoint, 1000 ms for the zone-change read (`load.rs:28-31`). The reference
shape for every number below is "release, 8 clients + 2 writers, 40k events,
20k sessions, 2k shared" unless marked debug.

Latest numbers on this shape (DEC-494, three runs, p95 ms): History person
45-47, activity 47-55, year 60-68, Home 158-192, query first 207-212, query
last 144-155, library first/last 29-39, search 100-108, totals 53-58, series
42-45, POST 32-35, writers 43-48, zone change 745-755 (once).

Facts about the harness that matter for the design (read from `load.rs`):

- Items are seeded without `cover_asset` (`load.rs:465-489`), so `dto::cover`
  (`crates/api/src/dto.rs:46-50`) returns at the first `if` and the per-row asset
  read costs nothing in the harness. A real library with covers pays it.
- The harness speaks REST only (`/api/v1/...` with a bearer token). No server-side
  page render happens, so the Fluent bundle parse per SSR render (WP-A1) is
  invisible to it.
- `/people` is not in the mixed set (DEC-470 deferred it). The load user has one
  companion (Anna) and no companion linked to "friend" (`load.rs:588-607` links in
  the other direction only), so the shared half of `companion_stats` would read
  the 2 000 shared sessions but attribute them to nobody.
- The time-zone read is taken once, right after the PUT, with the eight clients
  already finished, so it measures the full rebuild alone.

What the architecture review already says (section 7, not repeated here): the
Fluent bundles are parsed twice per SSR request; `dto::item` -> `cover` is an
N+1 on every row list; `sharing::sessions`, `admin::users` and
`people::person_page` have their own N+1s; `library_query` and the stats cache
have the right shape.

## 1. P-1: first statistics read after a time-zone change (~750 ms, target 500)

### 1.1 Current mechanism

- `App::stats_totals` (`crates/app/src/stats.rs:255-300`) reads the user's zone
  (`user_zone`, one `users` read) and calls `ProgressRepo::month_amounts`.
- `PgStore::month_amounts` -> `cached_month_amounts`
  (`crates/store-pg/src/repos/stats_cache.rs:26-71`): one statement reads the
  stamp and whether any dirty mark exists (`:31-38`); if the stamp is another zone
  or a mark exists, `refresh_stats` runs (`:76-167`) inside one transaction that
  locks the user's `stats_users` row `FOR UPDATE` (`:85-91`), so a second reader
  waits for the whole rebuild.
- A full rebuild (`full = stamp.zone != zone`, `:103`) runs `bucket_zone`
  (`pg_timezone_names` lookup, ~47 ms per DEC-400), sets `work_mem` to 32 MB,
  deletes every `stats_item_months` row of the user and subtracts them from
  `stats_months` (`:122-146`), then `insert_item_months` with `items = NULL`
  (`:221-296`) recomputes every effective event of the user (window functions per
  item, join to `catalog_items`, left join to `sessions`) grouped by item, kind
  and month in the new zone, inserts ~42 800 item rows and upserts the rollup.
- DEC-404's EXPLAIN ANALYZE split: about 400 ms computing, about 270 ms writing
  the item rows (after the foreign key on the user was dropped; a month index
  would add 125 ms more). The read before the cache existed cost ~390 ms at 40k
  (DEC-399 (b) "zone change 397").

### 1.2 Why it costs what it costs

The amounts per (item, month) depend on the zone only through the month
boundary, but the cache is keyed by the zone's month, so a zone change
invalidates every row. The read path then does three things a read should not
pay for at once: recompute 40k events, write 42.8k rows with a primary key, and
hold a row lock that serialises every other reader of that user. Nothing in
the statement is wasteful per row; the volume is the cost. Writing is 40 percent
of it and is only needed for the next incremental refresh, not for this answer.

### 1.3 Options

Option P1-A: answer the read from the full statement, rebuild in the background
(DEC-404 and DEC-476's suggestion).

- `cached_month_amounts` sees `stamp != zone`; instead of refreshing it returns
  `month_amounts_full(user, zone)` (`crates/store-pg/src/repos/library.rs:376-454`,
  the statement the cache is tested against) and asks a rebuilder to run
  `refresh_stats` later. Dirty marks (same zone) keep the inline refresh.
- The rebuilder is a port on `App` (`StatsRebuilder::schedule(user, zone)`), the
  server implementation a `tokio::spawn` guarded by a per-user in-flight set
  (`Mutex<HashSet<UserId>>`) so two reads schedule one rebuild. In the in-memory
  store and in tests the implementation runs inline, which answers DEC-404's
  objection that the cache test's steps would race with the rebuild: the test
  calls the rebuilder itself (`rebuild_now`) between steps.
- Expected: first read after a zone change = the full statement, ~390 ms at 40k
  (DEC-399), inside the 500 ms target; the second read ~10 ms once the rebuild
  has committed, meanwhile ~390 ms again (every read until then goes to the full
  statement; a `stats_users.rebuilding` flag is not needed, the stamp says it).
- Cost: one more port and a spawn; the same user's dirty marks set during the
  rebuild are handled by DEC-460's generations already (`take_marks`).

Option P1-B: start the rebuild when the zone is set, not when it is read.

- `App::set_time_zone` (`crates/app/src/time_zone.rs:62-67`) schedules the
  rebuild right after `users.update`. Most users set a zone once in Settings and
  open Stats or Home seconds later; with a 0.8 s rebuild the cache is usually
  ready when the first read arrives.
- Alone it does not meet the harness, which reads within milliseconds of the
  PUT and would wait on the `FOR UPDATE` row lock for the running rebuild
  (`stats_cache.rs:85-91`). Combined with P1-A the waiting read answers from the
  full statement instead and never takes the lock (it only reads the stamp).
- Expected with A: harness first read ~390 ms; a human user almost always ~10 ms.

Option P1-C: make the rebuild itself cheaper.

- C1: the three cache tables (`stats_item_months`, `stats_months`, `stats_users`)
  `UNLOGGED`. Their content is derivable; after a crash Postgres truncates
  unlogged tables, so the stamp row disappears with the data and the next read
  does a full rebuild (the stamp check at `stats_cache.rs:39` already treats a
  missing stamp as "another zone"). `stats_dirty` stays logged (a lost mark is
  the UX4-1 bug). Unlogged tables skip WAL on insert, which is a good part of the
  270 ms write; the exact share needs the measurement in 1.6. `stats_users` must
  be in the same set: an unlogged `stats_item_months` with a logged, clean stamp
  would serve empty statistics after a crash.
- C2: the two `lag()` windows (`stats_cache.rs:247-250`) partition by
  `(item_id, page IS NULL)` and `(item_id, pos IS NULL)`, two sorts of the whole
  event set. Postgres 18 (DEC-002) supports `IGNORE NULLS` on window functions,
  so one window `PARTITION BY item_id ORDER BY occurred_at, id` with
  `lag(page) IGNORE NULLS` and `lag(pos) IGNORE NULLS` would need one sort. The
  semantics differ from today's: today a paged event's previous page is the
  previous *paged* event's page (NULL partition members are the non-paged
  events), which is exactly what `IGNORE NULLS` gives. Must be verified against
  `month_amounts_full` by the existing equality test; gain unknown until
  EXPLAINed, I would not count on more than a fifth of the compute part.
- C3: do not write item rows in the zone-change path at all: a rebuild writes
  only `stats_months` and sets a `stats_users.items_pending` flag; the item rows
  are produced by the background job. Rejected: it duplicates P1-A's background
  job while keeping the compute in the read path.

Option P1-D (not recommended): bucket per (item, UTC day) and re-bucket per
zone at read time. Zone offsets are not whole days, so day rows do not
compose into local months without the events near the boundary; rejected.

### 1.4 Recommendation

P1-A plus P1-B, then P1-C1 as a measured follow-up. A+B remove the write from
the read path and make the common human case a cache hit; C1 shortens the
background rebuild and shortens the window in which reads go to the full
statement. C2 only if C1 leaves the rebuild above one second at 40k.

### 1.5 Risks

- A read during the rebuild returns the full statement, the next the cache:
  both are correct for the same data, but a write that commits between them
  shows in one and not necessarily the other until its mark is refreshed. This
  is already the case for dirty marks; no new inconsistency class.
- The background task dies with the process; the stamp still says the old zone
  and the next read repairs it (schedules again). No stuck state.
- Memory: `month_amounts_full` returns one row per kind and month, small. The
  rebuild's `work_mem` 32 MB per spawn; the in-flight set bounds it to one per
  user.
- Unlogged tables are not replicated; the project runs a single Postgres
  (HANDOFF section 8), and the data is derivable. Document it in the
  migration header.
- Multi-process (`--role=web` twice, architecture review 7.7): two processes may
  both rebuild the same user; the `FOR UPDATE` on `stats_users` serialises them
  and the second finds the stamp set and exits at `stats_cache.rs:103-115`
  (`full` false, `dirty` empty). Fine.

### 1.6 Correctness tests

- `store-pg/tests/phase1.rs` `cached_stats_equal_the_full_statement_after_every_step`
  must keep passing with the rebuilder run inline between steps.
- New: "a read with a stale stamp equals the full statement and does not write"
  (assert `stats_item_months` row count unchanged and `stats_users.zone` still
  the old name after the read).
- New: "after `rebuild_now` the stamp is the new zone, the dirty set is empty and
  the read equals the full statement".
- New: "two concurrent stale reads schedule one rebuild" (counting rebuilder).
- `store-pg/tests/stats_race.rs` both tests unchanged (generations are untouched).
- C1: "after `TRUNCATE stats_item_months, stats_months, stats_users` the next
  read rebuilds and equals the full statement" (the crash simulation).

### 1.7 How to measure

- Keep the existing "stats totals, zone changed" line. Add a second timed
  `/stats/totals` after polling `SELECT zone FROM stats_users WHERE user_id = $1`
  (the harness owns `srv.pool`, `load.rs:362`) until it equals the new zone, with
  the poll time reported as "stats rebuild, background". Tighten
  `ZONE_BUDGET` to 500 ms once A+B are in.
- Acceptance: first read <= 500 ms debug at 40k (DEC-404's target) and <= 400 ms
  release 8 clients; second read <= 60 ms; background rebuild <= 1.5 s debug at
  40k (C1 aims at <= 1.0 s).
- For C1 and C2 alone, time `refresh_stats` directly in the Postgres test with
  40k seeded events, before and after, in the same test binary (no EXPLAIN
  needed, wall clock is what the user sees).

## 2. P-2: library query, first page (~210 ms under 8 clients)

### 2.1 Current mechanism

- `App::library_query` (`crates/app/src/browse.rs:196-235`) builds a
  `LibraryFilter` and calls `LibraryRepo::query`, then `rows()`
  (`browse.rs:301-346`: three batched reads for the page's items, tags and
  events).
- `PgStore::hits_after` (`crates/store-pg/src/repos/library_page.rs:176-210`)
  calls `query_rows` once with `limit + 1` and reads the total from the first
  row's `total` column.
- `query_rows` (`library_page.rs:218-317`) is one static statement: a CTE `hits`
  selects every entry of the user that passes the filters and computes three
  sort keys with `CASE` on the sort parameter `$7`; the outer query applies the
  cursor predicate, orders by eight `CASE` expressions on the direction `$8`,
  limits, and carries `(SELECT COUNT(*) FROM hits) AS total` on every row.
- Indexes available: `library_entries_recent (user_id, added_at DESC, id DESC)`
  (0011), `library_entries_kind_status (user_id, kind, status)` (0015), the
  unique `(user_id, item_id)`.

### 2.2 Why it costs what it costs

- The CTE is referenced twice (`FROM hits` and the count), so Postgres
  materialises it: every one of the 50 000 entries is written to a tuplestore
  with its computed keys before anything is sorted or limited. The cursor
  predicate and the `LIMIT` sit outside and cannot shrink the scan.
- The `ORDER BY` is a list of `CASE` expressions on a parameter, so no index can
  deliver the rows in order; the first page is a top-N sort over the whole
  filtered set, the last page a filter plus a small sort. The debug single-client
  gap first 86-89 vs last 64 (DEC-459) is that sort; the remaining ~50 ms are the
  materialisation, the count and `rows()`.
- `GET /library` on the same data costs 10.7 ms debug (DEC-459): keyset on
  `library_entries_recent` plus an index-only `COUNT(*)` (`library_page.rs:156-162`)
  and a single `get_many`. The difference to 86 ms is the price of the CTE
  shape, not of the data.
- Home pays it three times: `continue_rows` (`browse.rs:352-359`, status
  in_progress, sort last_activity, limit 6), `up_next` twice (`browse.rs:368-412`,
  planned sorted by added, once restricted to the series ids, once limit 12),
  each with a full materialisation and a count nobody displays.

### 2.3 Options

Option P2-A: take the total out of the statement.

- `LibraryFilter` gains `with_total: bool`; `HitRow` loses `total`; the CTE is
  then referenced once and is inlined, so the planner can push the cursor
  predicate into the scan (the keyset page no longer materialises 50k rows).
- The total comes from a second path only when asked:
  - no tag, facet or item filter: sum the matching cells of
    `LibraryRepo::counts` (`library.rs:237-255`, `GROUP BY kind, status` on
    `library_entries_kind_status`, an index-only scan the Library page already
    runs for its chips). `App` has the cells in `library_counts`; the repository
    does not need to know.
  - otherwise one `SELECT COUNT(*)` over the same `WHERE` (a second statement,
    still a filtered scan, but only for selective filters).
- Callers: the web Library asks for the total on the first page only
  (`crates/web/src/library.rs:223` merges `more.total` from later pages; keep the
  first page's), and decides "Load more" by `next.is_some()` instead of
  `total > rows.len()` (`library.rs:884`). Home never asks. `up_next` and
  `continue_rows` pass `with_total: false`.
- Expected: first page falls to roughly the last page's cost or below (the
  materialisation goes, the top-N sort stays): ~130-150 ms under 8 clients,
  ~60 ms debug single; Home loses three materialisations and three counts.
  DEC-416 rejected "drop the total on later pages" for the UI's "Showing N of
  M"; this keeps M from the first page, so the UI is unchanged.

Option P2-B: index-ordered pages (needs a migration).

- A stored generated column
  `library_entries.last_activity timestamptz GENERATED ALWAYS AS
  (GREATEST(added_at, COALESCE(last_event_at, added_at))) STORED` (the
  expression at `library_page.rs:237`), kept by the 0015 trigger that already
  updates `last_event_at`.
- Indexes `(user_id, last_activity DESC, id DESC)` and
  `(user_id, status, last_activity DESC, id DESC)` for the default sort and the
  Home Continue read; `(user_id, status, added_at DESC, id DESC)` for Up next.
  Year, rating and plays sorts are rare and keep the sort; title would need
  `lower(title)` on the entry (a copy kept by a trigger on `catalog_items`),
  which I would not do until a user asks.
- The `ORDER BY` must be literal for an index to serve it. Replace the eight
  `CASE` lines with one of twelve constant `ORDER BY` strings chosen by a
  `match` on `(LibrarySort, descending)` and executed with
  `sqlx::query_as::<_, HitRow>(SQL)` (no input in the SQL text; the filter CTE
  and the cursor predicate stay parameters). The cursor predicate also becomes
  per-sort text (`(last_activity, id) < ($11, $13)`), which the planner turns
  into an index range. The compile-time check is replaced by a test that runs
  all twelve variants (2.6).
- Expected: first and last page both become index range scans of ~100 rows plus
  `rows()`: ~30 ms debug single, <= 60 ms under 8 clients. The Home Continue and
  Up next reads become 6- and 12-row index scans.

Option P2-C: cached totals per filter (DEC-459's first alternative).

- A `library_totals (user_id, filter_hash, total)` row per seen filter,
  invalidated by every entry write of the user. Fixes only the count, adds an
  invalidation path to five write sites. Rejected while P2-A gives the same
  for the common filters at zero write cost.

### 2.4 Recommendation

P2-A first (no migration, one afternoon, fixes the count for Home and the
Library at once), then P2-B for the two sorts Home and the default Library use.
Together they make the query page cost what the plain page costs.

### 2.5 Risks

- P2-A changes a contract detail: `LibraryRowsDto.total` would be optional (or
  0) on later pages. The web is the only client; the harness asserts `total` on
  the first page only (`load.rs:254`) and `next` on the last (`:263`).
- P2-A's count from `counts` must apply the same group/kind/status selection as
  the query (`selected_kinds`, `browse.rs:122-139`): a test compares it with
  `COUNT(*)` for every combination the Library page can send.
- P2-B's generated column rewrites the table once (50k rows per user, a one-off
  migration cost) and adds three indexes to every entry write; the writers' POST
  (p95 32-35 ms) should move by less than 5 ms.
- P2-B's twelve statements lose `sqlx` offline checking; the twelve-variant test
  and `cargo xtask sqlx-prepare` for the rest cover it.
- Numbers with NULL keys (rating, year, plays) keep today's "missing last in
  both directions" rule (`library_page.rs:283-288`) in their variants; the
  keyset tests in `app/tests/browse.rs` and `store-pg/tests/phase1.rs`
  (DEC-416) already walk them.

### 2.6 Correctness tests

- Existing keyset walks (`app/tests/browse.rs`, `store-pg/tests/phase1.rs`) for
  every sort and direction: pages of 1..7 concatenated equal one read of all.
- New: total from `counts` equals `COUNT(*)` of the query for group, kinds,
  statuses and their combinations; falls back to the counting statement with
  tags, facets or items.
- New (P2-B): every `(sort, direction)` statement parses and orders as the
  in-memory store does (the `App` tests run on both stores already).
- New: Home DTO unchanged with `with_total: false` (`library_total` on Home comes
  from `library_counts`, `crates/api/src/browse.rs:604-606`, not from a query).

### 2.7 How to measure

- The harness lines "library query, first 100" and "library query, last 100"
  and "home" as they are. Add one filtered query to the mixed set
  (`{"statuses":["completed"],"limit":100}`, the common status tab) so the
  `counts` path is measured.
- Acceptance: P2-A alone: query first <= 150 ms under 8 clients, Home <= 120;
  P2-A+B: query first and last <= 60 ms under 8 clients, <= 30 ms debug single
  (plain `GET /library` is 10.7 plus `rows()`), Home <= 90 ms under 8 clients.

## 3. P-3: `/people` is not in the load set and reads sessions

### 3.1 Current mechanism

- `GET /api/v1/people` -> `api::people::people_page`
  (`crates/api/src/people.rs:116-147`) -> `people_and_stats` (`:90-114`), which
  calls `App::companions` and `App::companion_stats`, then `CardCtx::new`
  (`:34-58`: sent link requests, one `user_opt` per linked person, the zone),
  `link_requests`, and `goals::game_groups`.
- `App::companion_stats` (`crates/app/src/people.rs:171-220`):
  `companions.by_owner` (`:172`), `self.companions(actor)` (`:175`, which is
  `by_owner` again, `crates/app/src/companions.rs:69-74`), then
  `sessions.by_owner(actor)` (`:186`): every own session with its participants
  (`crates/store-pg/src/repos/sessions.rs:167-177` and `load_sessions`
  `:27-67`, two statements, 20k + 22k rows at the load shape), then
  `shared_sessions` (`crates/app/src/sessions.rs:332-349`:
  `companion_ids_representing` = `linked_to` plus one `by_owner` per linking
  user, `with_participants` = `DISTINCT session_id` over the companion's
  participations then `sessions_where_ids`, one `shares_sessions` user read per
  owner), then `play_items` (`:222-243`, one `get_many` of the distinct items).
- The accumulation is in Rust; for shared sessions the loop at `people.rs:209-218`
  reads `self.user(s.owner)` at `:213` once per shared session (2 000 user statements at
  the load shape; the result is the same for every session of one owner).
- `game_groups` (`crates/app/src/game_groups.rs:40-100`) is already bounded
  (`group_plays`, DEC-470) but reads `companions.by_owner` a third time.

### 3.2 Why it costs what it costs

Linear in the user's sessions: 20k `Session` structs with participants are
built in Rust to produce one line per person (DEC-235 called this out for the
activity feed, which WP4-1 fixed; `companion_stats` kept the full read). On
top, an N+1 of user reads per shared session that nothing in the harness has
measured yet. At 20k own and 2k shared sessions I expect, from the pre-WP4-1
activity numbers (220 ms debug for 22k sessions read once, DEC-399 (b)) plus 2
000 sequential round trips of ~0.2-0.5 ms, a single-client debug p95 around
600-1 200 ms and a failure of the 300 ms assert under 8 clients. This is a
prediction, not a measurement; 3.7 makes it one.

### 3.3 Options

Option P3-A: remove the N+1 and the repeated reads (no migration, no SQL).

- Cache the owner's `self_companion` per owner in `companion_stats` (a
  `HashMap<UserId, CompanionId>` filled by `user_opt` once per distinct owner,
  like `Ctx.owners` in `activity.rs:380-390`), or extend `shares_sessions` to
  return the user so `shared_sessions` already carries it.
- Read `companions.by_owner` once in `people_page` and pass the list into
  `companion_stats`, `game_groups` and `companions` (an `_with(owned)` variant
  each), saving two statements.
- Expected: removes ~2 000 statements per request at the load shape; the 20k
  session read remains. Likely brings the debug single-client number under 300
  ms; not enough under 8 clients.

Option P3-B: aggregate in SQL (`SessionRepo::companion_stats`), Rust keeps the
rules as the default trait implementation for the in-memory store.

- Inputs as arrays: the owner, the `(companion_id, root_id)` pairs from
  `roots(&owned)` (merge chains resolved in Rust, `people.rs:106-120`), the
  kind ids whose progress model is `Sessions` (the `play` flag, from the
  registry), and for shared sessions the `(owner_user_id, root_id)` pairs from
  `by_user` (`people.rs:181-185`).
- Statement 1, own sessions: `sessions s JOIN session_participants p` on the
  owner's sessions, map `p.companion_id` to its root through
  `unnest($pairs)`, `GROUP BY root`: `COUNT(DISTINCT s.id)` as together,
  `COUNT(DISTINCT s.id) FILTER (WHERE c.kind = ANY($play_kinds))` as plays,
  wins as `COUNT(DISTINCT s.id) FILTER (WHERE play AND (s.coop_outcome = 'won'
  OR (s.coop_outcome IS NULL AND p.winner)))` (the `won` rule at
  `people.rs:97-103`, "a win if any merged entry won" at `:192-204` is the
  `bool_or` inside the DISTINCT), decided per the `Acc::add` rule (read it when
  implementing; `people.rs:74-95`), `max(s.started_at)` and the item of that
  session via `DISTINCT ON` in a second CTE for `last`.
- Statement 2, most played: per `(root, item)` counts over play sessions,
  `DISTINCT ON (root) ORDER BY root, n DESC, last DESC, item_id` (the tie rule
  at `people.rs:255-259`).
- Statement 3, shared sessions: the same two over sessions whose owner is in
  `$owner_pairs` and that have the owner's `users.self_companion` as a
  participant, joined to `users` once (replaces the per-session user read).
- Expected: three index-driven aggregates over `sessions_owner_recent` and
  `session_participants_companion`, ~5-15 ms at 20k sessions, no Rust
  materialisation; `/people` becomes the sum of small reads (<= 40 ms under 8
  clients).

Option P3-C: an incremental `companion_stats` cache kept by triggers, like
0023. Rejected: the aggregate over one user's sessions is cheap in SQL; a
cache is warranted only if hosted instances show users with hundreds of
thousands of sessions.

### 3.4 Recommendation

P3-A at once (it is a bug-shaped cost, not a design), then P3-B. Put `/people`
into the harness before either, so the before number exists.

### 3.5 Risks

- P3-B moves rules into SQL that `app/tests/people.rs` tests on the in-memory
  store; the Postgres implementation needs an equivalence test on the same
  seed (3.6), the pattern DEC-470 used for `group_plays`.
- Merge chains: roots are resolved in Rust and passed as pairs, so the SQL
  never follows `merged_into`; a companion not in the pairs (deleted,
  foreign) contributes nothing, as today's `people.contains(r)` filter
  (`people.rs:197`).
- A shared session counts for the companion that stands for its owner
  (`by_user`), with the owner's self participation for the win; a session the
  owner logged without themselves as participant counts as together but not as
  a win, as today (`p` is `None`, `won` reads `coop_outcome` only).
- Array parameters: `unnest` of two arrays of equal length; the repository
  asserts the lengths.

### 3.6 Correctness tests

- `store-pg/tests/people_stats.rs` (new): seed owned and merged companions,
  own sessions with winners, co-op outcomes, ties, a game group, a linked user
  who shares and one who does not, shared sessions with and without the
  owner's self participation; assert `SessionRepo::companion_stats` on Postgres
  equals the in-memory default implementation (`App` on both stores answers the
  same `Vec<CompanionStats>`).
- `app/tests/people.rs`: a counting repository asserts the People page issues
  no per-session user read (`users.get` calls <= distinct owners) and reads
  `companions.by_owner` once.
- Existing `people.spec.ts` for the page contract.

### 3.7 How to measure

- Add to the mixed set: `GET /api/v1/people`, asserting Anna's `together ==
  sessions / 10` and her `plays` equal (every seeded game is a `Sessions`
  kind). Add a companion owned by the load user and linked to "friend" with the
  friend's sessions having the friend's self companion (already seeded at
  `load.rs:601-607`), so the shared half attributes the 2 000 sessions and the
  win path runs; assert `together == shared` for that card.
- Run the current tree once (debug single, debug 40k shape, release 8 clients)
  to record the before number; expect a failing assert under 8 clients.
- Acceptance: P3-A: `/people` p95 <= 300 ms under 8 clients at 20k/2k;
  P3-B: <= 40 ms under 8 clients and within 1.5x between `LOAD_SESSIONS=500` and
  `20000` (the "does not grow with sessions" check DEC-404 used for activity).

## 4. Home (~140-190 ms under 8 clients)

### 4.1 Current mechanism

`api::browse::home` (`crates/api/src/browse.rs:567-611`) runs five parts under
`tokio::try_join!` (`:603`):

1. `library_counts` -> `LibraryRepo::counts` (one grouped index scan).
2. `continue_rows` -> `library_query` (status in_progress, sort last_activity,
   limit 6): the full CTE materialisation and count of section 2, then
   `rows()` for 6 rows, then `row()` per row with `dto::item` (a cover read per
   row when covers exist).
3. `up_next` -> `next_in_series` (`library.rs:201-235`, bounded) plus two
   `library_query` calls (planned restricted to the series ids; planned, limit
   12): two more materialisations and counts.
4. `activity_feed` with limit 5 (`crates/app/src/activity.rs:205-301`): `user`,
   `companion_ids_representing` (`linked_to` + `by_owner` per linking user),
   `feed_context` (`linked_to` again), one window of 11 rows per source, the
   pairing reads, names and items; then `user_zone` once more (`browse.rs:600`).
5. `stats_totals` (`stats.rs:255-300`): `counts` again, `user_zone` (a third
   `users` read in the same request), `month_amounts` (cache hit: two small
   statements).

### 4.2 Why it costs what it costs

Home is the slowest part, not the sum (DEC-404), and the slowest part is any
of the three library queries: each is a 50k-row materialisation plus a top-N
sort. Under eight clients they compete for the same four cores and the same
pool (20 connections), so Home's p95 tracks the query page's (207-212 vs
158-192 in DEC-494). The repeated `counts` and the three `users` reads are
small but add round trips under contention.

### 4.3 Options

- H-A: P2-A with `with_total: false` on the three Home queries. No Home code
  changes beyond the flag. Expected Home ~ the activity part (~50 ms) plus the
  remaining top-N sorts; p95 <= 120 under 8 clients.
- H-B: P2-B's `(user_id, status, last_activity DESC, id DESC)` and
  `(user_id, status, added_at DESC, id DESC)` indexes turn Continue and Up next
  into 6- and 12-row index scans. Expected Home <= 90 under 8 clients, debug
  single ~30 ms (from 52).
- H-C: read the `User` once in `home` and pass the zone into the activity and
  stats parts; compute `stats_totals` from the `counts` already read
  (`App::stats_totals_from(counts, months)`), so one `counts` statement serves
  both. Saves three to four round trips per Home. Small, independent of A and B.
- H-D (not recommended): a per-user Home cache with invalidation on every write.
  Home is derived from five bounded reads once A and B are in; a cache adds a
  staleness class for the page the owner opens most.

### 4.4 Recommendation

H-A comes free with P2-A; H-C in the same change; H-B with P2-B.

### 4.5 Risks and tests

- `try_join!` cancels the other parts when one fails; unchanged.
- Test: the Home DTO for a seeded user is byte-equal before and after
  (`apps/server/tests` has the Home assertions the harness uses at
  `load.rs:243-249`: 6 continue rows, 6 up next with `next_in_series` first, 5
  activity rows).
- Counting-repository test: Home issues one `counts` and one `users` read.

### 4.6 Measurement

The "home" line of the harness. Acceptance as in 4.3: <= 120 ms under 8
clients after H-A/H-C, <= 90 after H-B; debug single <= 40 ms.

## 5. The optional `sessions (started_at DESC, id DESC)` index

### 5.1 Current mechanism

Three reads order sessions by time without an owner:

- `SessionRepo::with_participants_window` (`sessions.rs:241-271`): `EXISTS`
  over `session_participants` for the companions, cursor and bounds, `ORDER BY
  started_at DESC, id DESC LIMIT`. Used for the shared stream of every feed
  window (`activity.rs:321-328`) and of Home.
- `with_participants_spans` (`sessions.rs:306-336`): the same with span bounds
  (DEC-494's person walk).
- The DEC-494 anchors read (the person's sessions newest first).

DEC-494's EXPLAIN: a sequential scan of all 22 000 sessions hashed against the
companion's 2 000 participations, then a top-N sort, 6.4 ms per window; with
the index 0.9 ms. It was left out as "not needed for the budget".

### 5.2 Why it costs what it costs

`sessions_owner_recent (owner_id, started_at DESC, id DESC)` leads with the
owner, so a read for "sessions some companion took part in" has no ordered
path; the planner reads every session and sorts. The cost is per window and
grows with every user's sessions on the instance, not with the viewer's.

### 5.3 Options

- S-A: the plain index `sessions (started_at DESC, id DESC)`. Newest-first
  walk probing `session_participants` per row until `LIMIT` rows pass. Fast
  when the companion's sessions are recent and dense (the harness: Anna on
  every tenth play, the friend's share every 20 hours); slow when they are old
  and sparse on a busy instance (the walk passes every newer session of every
  user). One migration, no code.
- S-B: participant-driven reads, no index: `FROM session_participants p JOIN
  sessions s ON s.id = p.session_id WHERE p.companion_id = ANY($1) AND
  (s.started_at, s.id) < ($2, $3) ... ORDER BY s.started_at DESC, s.id DESC
  LIMIT $n`. `session_participants_companion (companion_id)` (0001) finds the
  2 000 participations, 2 000 primary-key probes on `sessions`, a top-N sort of
  2 000. Cost scales with the companion's participations, never with other
  users' sessions. Expected ~2-3 ms per window at 2k participations; the same
  rewrite fits the spans and anchors reads.
- S-C: denormalise `started_at` into `session_participants` (kept by a trigger
  on `sessions`) with an index `(companion_id, started_at DESC, session_id
  DESC)`: an exact keyset for the shared stream, O(limit) per window
  regardless of participations. One migration plus a trigger; the right shape
  if a companion accumulates tens of thousands of participations (a spouse on
  every play for years).

### 5.4 Recommendation

S-B now (no migration, removes the sequential scan from the feed's per-window
cost and from Home), S-C when a hosted instance shows a companion with more
than ~20k participations. S-A only if S-B's EXPLAIN surprises; its worst case
is the one hosted mode produces.

### 5.5 Risks and tests

- S-B changes three statements; `store-pg/tests/windows.rs`
  (`span_reads_return_the_full_reads_rows_in_the_spans_on_postgres`,
  `the_person_feed_matches_the_full_read_on_postgres`) and
  `app/tests/activity.rs` `windowed_pages_return_the_full_reads_rows` already
  compare windowed reads with the full read; they cover the rewrite.
- A companion in several `ANY($1)` entries (merged duplicates) must yield one
  row per session: `DISTINCT ON (s.started_at, s.id)` or `EXISTS` kept inside
  the join; the merge-chain test in `activity_person.rs` covers it.

### 5.6 Measurement

The harness lines "activity, 20 rows", "home" and "history, person" at
`LOAD_SHARED=2000`, plus one run at `LOAD_SHARED=20000` (the shared sessions
are the only ones this touches). Acceptance: activity and person p95 within
noise at 2k and no more than 1.3x at 20k shared; the friend's sessions do not
need an owner index change.

## 6. Cover lookups per row in `dto::item`

### 6.1 Current mechanism

`dto::item` (`crates/api/src/dto.rs:88-108`) awaits `cover(state, i)`
(`:46-86`), which awaits `AssetService::get(uuid)`
(`crates/assets/src/service.rs:500-502`, one `assets` row by id) when the item
has a `cover_asset`. Callers build DTOs in sequential loops: `browse::row`
(`browse.rs:298-308`, used by `library_query`, Home Continue and Up next),
`ops.rs:185`, `:239`, `:307` (search hits, already read in one `get_many` by
DEC-493, then one cover read each), `sharing.rs:251`, `people.rs:292`, `:346`.

### 6.2 Why it costs what it costs

One round trip per row, awaited in sequence: 48 for a Library page
(`PAGE_SIZE` in the web), 100 for the harness page, 20 per search. Each is a
primary-key read (~0.2 ms in release alone, several ms under contention and
pool pressure with 8 clients on 20 connections). It is invisible today because
the seeded items have no covers (section 0); a real library has them on most
items.

### 6.3 Options

- C-A: batch. `AssetRepo::get_many(&[Uuid]) -> Vec<AssetRecord>` (one `WHERE
  id = ANY($1)`), `AssetService::get_many`, and `dto::items(state, &[&CatalogItem])
  -> Vec<CatalogItemDto>` that prefetches the records into a `HashMap` and
  builds covers from it; `browse::rows(s, &[LibraryRow])` replaces the per-row
  `row` at the seven call sites. `cover` keeps its single-item form for the
  item page. Expected: one statement per list instead of N; a 48-row page saves
  47 round trips (10-25 ms under load with covers).
- C-B: a per-process cache of `AssetRecord` by id (small, immutable after the
  fetch completes; a `Mutex<HashMap>` with a TTL or a `moka`-style LRU).
  Invalidation across processes is by TTL only (architecture review 7.7); a
  refetched cover (DEC-453) could show the old variant URL for the TTL. Adds a
  dependency or a hand-made cache for a problem C-A solves without state.
- C-C: carry the cover variants on `catalog_items` (denormalised JSON kept in
  step with the asset). Rejected: a second owner for the asset's fields.

### 6.4 Recommendation

C-A. It is the architecture review's WP-A2 item made concrete; no state, no
migration.

### 6.5 Risks and tests

- The DTO must be identical: a test builds `dto::items` and `dto::item` for the
  same items (with an asset record with hash, one without hash, one without
  `cover_asset`, one with a malformed id) and compares.
- Order preserved: `dto::items` returns in input order (the search path relies
  on hit order, DEC-493).
- Counting test: a library page with covers issues one asset statement.

### 6.6 Measurement

Harness knob `LOAD_COVERS=1`: `seed_movies` inserts one `assets` row per movie
(table `assets`, migration `0004_storage.sql:3`; `hash` set, no stored object
needed since `cover` reads the record only) and sets `catalog_items.cover_asset`.
Record "library query, first 100", "home" and "search" with and without the
knob on the current tree, then after C-A. Acceptance: with covers, each of the
three lines within 10 percent of its no-cover number after C-A (before, the
harness page of 100 rows should show the N+1 as a clear step).

## 7. i18n bundles parsed per render (architecture review WP-A1)

### 7.1 Current mechanism

`web::shell` (`crates/web/src/app.rs:135-146`) calls `provide_page_context`
(`crates/web/src/lib.rs:79-85`), which calls `ClientConfig::i18n()`
(`app.rs:55-57`): `Arc::new(I18n::new(locale, strings.clone()))`. `I18n::new`
(`crates/web/src/i18n.rs:132-139`) builds two bundles, `bundle(locale, ..)` and
`bundle("en", ..)` as fallback; each `bundle` (`:76-97`) runs
`FluentResource::try_new` over the embedded file (`en.ftl` 2 111 lines, `de.ftl`
2 102 lines) and adds the brand overrides as small resources. So every SSR
page render parses two 2 100-line files (English twice for an `en` page), and
the brand overrides are re-parsed each time although they are fixed per
process.

### 7.2 Why it costs what it costs

Fluent parsing is a few milliseconds per file in release and tens in debug;
it is pure CPU on the request thread, before any data is read, and the output
is the same for every request with the same `(locale, overrides)`. The
hydration side parses once per page load in the browser, which is fine.

### 7.3 Options

- I-A: process-wide cache under the `ssr` feature:
  `static BUNDLES: OnceLock<Mutex<HashMap<(String, u64), Arc<I18n>>>>` keyed by
  locale and a hash of the `BTreeMap<String, String>` overrides (deterministic
  order, so a stable hash). `ClientConfig::i18n()` looks up or builds. `I18n`
  holds `fluent_bundle::concurrent::FluentBundle` (`i18n.rs:15`, `:78`), which
  is `Send + Sync`, so one `Arc<I18n>` serves every request thread. Two
  entries in practice (en, de) per brand.
- I-B: cache the parsed `FluentResource`s only and build bundles per request.
  `FluentBundle::add_resource` takes the resource by value, so it would clone
  2 100 messages per request; most of the saving is lost. Rejected.
- I-C: compile the messages at build time (a `fluent-templates`-style static
  loader). Loses the runtime brand overrides (DEC-090 chose Fluent for them).
  Rejected.

### 7.4 Recommendation

I-A; effort S as the review says.

### 7.5 Risks and tests

- A brand changed at runtime: overrides come from `brand.toml` read at start
  (`app.rs:134`, `server_config`), so the key covers the cases that exist; if a
  SIGHUP reload of the brand is added later the cache must be cleared there.
- Tests that construct `I18n::new` directly (`i18n.rs:335-456`) are unaffected;
  add one that two `ClientConfig`s with different overrides get different
  `Arc`s and two with equal ones the same.
- The locale checker (HANDOFF section 8, "Fluent key prefix") is build-time and
  unaffected.

### 7.6 Measurement

The harness does not render pages. Two honest ways:

- A unit timing in `web` (`#[ignore]`, printed): 200 x `I18n::new("de", ..)`
  average, versus 200 x `ClientConfig::i18n()` with the cache, in a release
  test build. Acceptance: the cached call is under 50 microseconds.
- A harness line "page, /library (SSR)" with a cookie session. The REST
  sign-up at `load.rs:139-154` returns a bearer token; whether the SSR routes
  accept it or need the session cookie is to be confirmed in
  `apps/server` before wiring this. Acceptance: SSR p95 under 8 clients falls
  by at least the two parses (expect 10-30 ms in debug, a few ms in release).

## 8. Prioritised plan

Order by (budget risk) x (effort), smallest migrations last so each release
load run (about 40 minutes and 15 GB, DEC-459) measures one package.

| Step | Package | Items | Migration | Effort | Acceptance (release, 8 clients + 2 writers, 40k / 20k / 2k, p95 ms) |
|------|---------|-------|-----------|--------|-------------------------------------------------------------------------|
| 0 | perf-0 measure | `/people` in the harness (3.7), `LOAD_COVERS` (6.6), filtered query line (2.7), second zone read (1.7) | no | S | a before record for every new line; `/people` expected to fail the 300 ms assert |
| 1 | perf-1 people-n1 | P3-A | no | S | `/people` <= 300; `users.get` calls <= distinct owners |
| 2 | perf-2 query-total | P2-A, H-A, H-C | no | M | query first <= 150, Home <= 120, query last <= 150; totals on the first page unchanged |
| 3 | perf-3 batch-covers | C-A | no | S | with `LOAD_COVERS=1`: query first, Home, search within 10 percent of their no-cover numbers |
| 4 | perf-4 i18n-cache | I-A | no | S | cached `i18n()` under 50 us; SSR line (if wired) down by the two parses |
| 5 | perf-5 zone-rebuild | P1-A, P1-B | no | M | zone change first read <= 500 debug 40k and <= 400 release; second read <= 60; `ZONE_BUDGET` 500 |
| 6 | perf-6 shared-reads | S-B | no | S | activity, person, Home within noise at 2k shared, <= 1.3x at 20k shared |
| 7 | perf-7 people-sql | P3-B | no | M | `/people` <= 40; <= 1.5x between 500 and 20 000 sessions |
| 8 | perf-8 stats-unlogged | P1-C1 (C2 if needed) | yes (0032) | S | background rebuild <= 1.0 s debug 40k; truncate-recovery test passes |
| 9 | perf-9 library-indexes | P2-B, H-B | yes (0033) | M | query first and last <= 60, Home <= 90, debug single query <= 30; POST writers p95 +5 ms at most |

Steps 1-7 need no migration and can each ship behind one `cargo xtask load
--release --clients 8` run; 8 and 9 are the only schema changes and come last
so that the indexes are added once the statement shapes are final (P2-B's
`ORDER BY` texts decide the index column order).

Overall acceptance for the workstream, same shape: every mixed-set line <= 150
ms (today's worst is the query first page at ~210), Home <= 90, `/people` <= 40,
zone change <= 400, and the debug single-client set at 40k all <= 60 except the
zone change (<= 500) and search (unchanged, ~107).

What I would not do in this workstream: per-filter total caches (P2-C), a Home
cache (H-D), a companion-stats cache (P3-C), an asset record cache (C-B), the
global `sessions (started_at DESC, id DESC)` index (S-A) before S-B has been
measured. Each adds state whose invalidation is the next review's finding.

## 9. Summary per item

| Item | Recommended option | Where |
|------|--------------------|-------|
| P-1 zone-change read | P1-A read-through from `month_amounts_full` plus P1-B rebuild scheduled at `set_time_zone`; P1-C1 UNLOGGED cache tables as follow-up | section 1 |
| P-2 library query first page | P2-A total out of the statement (counts from `LibraryRepo::counts` when no tag/facet/item filter; first page only), then P2-B generated `last_activity` column, two status indexes, per-sort `ORDER BY` | section 2 |
| P-3 `/people` | harness line first; P3-A remove the per-shared-session user read and the triple `by_owner`; then P3-B SQL aggregates with Rust rules kept as the default implementation | section 3 |
| Home | H-A (P2-A with `with_total: false`), H-C one `counts` and one `users` read, H-B with P2-B | section 4 |
| sessions time index | S-B participant-driven reads without a new index; S-C denormalised `(companion_id, started_at)` only for very large companions; S-A not recommended | section 5 |
| cover lookups | C-A `AssetService::get_many` + `dto::items`, `LOAD_COVERS` knob to make it visible | section 6 |
| i18n bundles | I-A process-wide `OnceLock` cache keyed by (locale, overrides hash) | section 7 |

STATUS: COMPLETE (2026-11-09)
