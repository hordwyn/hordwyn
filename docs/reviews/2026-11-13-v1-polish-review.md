# Review of package v1-polish (lane C), 2026-11-13

Status: COMPLETE. Read-only review: no file but this one was written, nothing was built
or run (no cargo, npm or Playwright). Every statement about a test result is taken from
`docs/fable/round2/v1-polish-progress.md`, not re-measured.

Reviewed: the working tree of `<repo>-lane-polish` (branch
`lane/v1-polish`). The branch has no commit of its own (HEAD = merge base `cb2f7a0`), so
the package is the 24 modified and 3 untracked files. Compared with `main` at `16edd29`
plus the uncommitted sec1 changes of the main checkout.

Inputs: `docs/fable/round2/v1-plan.md` section 2, `v1-status.md` package 3, DEC-640,
`v1-polish-decisions.md` (LC-1 to LC-9), `v1-polish-progress.md`, `docs/HANDOFF.md`
sections 2 and 4, `docs/handoff/agent-briefs/rules.md`, `docs/handoff/fable-briefs/LANES.md`.

## Summary

| Severity | Count | Ids |
|---|---|---|
| Critical | 0 | - |
| High | 0 | - |
| Medium | 5 | POL-1, POL-2, POL-3, POL-4, POL-5 |
| Low | 11 | POL-6 to POL-16 |
| Info | 4 | POL-17 to POL-20 |

Must fix before merge: POL-1 (settle the acceptance line and give the menu entry a
visible mark), POL-2 (German wording of the new bulk question). Everything else may be
deferred; POL-3, POL-4 and POL-5 should get an owner (package or follow-up list) at merge.

Overall: P1 to P8 and the two focus items do what the spec and the LC decisions say. No
defect found that loses data, breaks a page or fails WCAG 2.2 AA. The findings are about
one deviation the owner will see (LC-1), one clumsy German sentence, an edge of the
"one rule" for recent items, and tests that prove less than their names say.

## Questions of the brief, answered

- LC-1 (`/stats` marks nothing in the top bar): the deviation is right in substance, the
  acceptance line of the plan was written against a navigation that has no Stats entry.
  Not acceptable as it stands on a desktop: see POL-1.
- LC-8 (year bounds): correct for every zone of today's tz database, including zones in
  daylight time on 1 January (southern hemisphere) and users without a zone (UTC, same
  fallback as the day groups and the statistics buckets: `time_zone::zone_of`,
  `stats::bucket_of`). Two small points: POL-9, POL-10.
- LC-6 (recent items in `localStorage`): the key is per user (`app-recent-<username>`,
  `recent.rs:39`) and the list is dropped at sign-out (`sign_out.rs:56,63,67`); both are
  unchanged. The two new fields add nothing a second person on the browser could not
  already read (the title was there). No new privacy defect. One edge against DEC-640:
  POL-3.
- LC-5 (failed cover keeps the accessible name): yes. The box becomes `role="img"` with
  `aria-label="Cover of <title>"`, the hidden `picture` leaves the accessibility tree, so
  the name is there exactly once, before and after. A link around the cover computes the
  same name from content. Untested path: POL-7.
- LC-7 (focus by selector): the selector is safe (event names and UUIDs hold no quote),
  the matrix rows are unique per event and channel, the channel cards are keyed
  (`notify_channels.rs:228`), so "Edit" survives the refresh after Save. Weak spot for
  pointer users: POL-4.
- Wording en/de: consistent with the ot2a keys (`next-step-episode-listened` = "Mark S1 E1
  listened" / "S1 E1 als gehoert markieren", `logged-step-episode-listened`,
  `history-session` = "Sitzung eingetragen"). One German sentence is poor (POL-2), one
  hint still says "Mark show" on a podcast (POL-6).
- DEC-640 and the CSP of sec1: compatible. sec1 writes `img-src 'self' data: https:` and
  adds `http:` when the instance itself is served over http
  (`apps/server/src/security_headers.rs:120,137` in the main checkout), which is the case
  in the browser tests, so the pictures of the TMDB mock (`http://127.0.0.1:<port>/t/p/`)
  break no policy and the policy guard of `tests/fixtures.ts` stays quiet. Remaining
  edge: POL-11.
- WCAG 2.2 AA: no failure found. Notes in POL-1 (location is only programmatic), POL-4
  (2.4.3 for pointer users), POL-12 (hints at 400 % zoom).
- Tests that cannot fail or were never red: POL-8.
- `sharing.spec.ts:55`: POL-5.
- Functions over 100 lines: none new. Close to the limit after this package:
  `notify_rules_view::Rules` (96 lines raw, was 78) and `app::activity::activity_feed`
  (98). `Command` (ui), `ItemPage` and `AddFiltered` were over 100 raw lines before and
  are not changed here.
- Out of scope: POL-15.
- Merge with main: POL-17.

## Findings

### POL-1 (Medium, must fix before merge) - `/stats` has no visible "you are here" on a desktop, and the plan's acceptance line still says otherwise

Where: `src/rust/crates/web/src/shell.rs:44-49` (`section_of`), `:653-664`
(`AccountMenuItems`), `:536` (`MENU_ITEM`); `docs/fable/round2/v1-plan.md` section 2
"Acceptance (owner)"; LC-1.

Problem: the plan's acceptance says "`/stats` highlights Stats in the top bar". The top
bar has five sections and Stats is not one of them (`shell.rs:109-117`), so LC-1 moves
`/stats` to `/you`: the top bar marks nothing, the phone tab bar marks "You", the account
menu entry gets `aria-current="page"`. That is the same treatment as Settings and Admin
and the right call; a sixth top-bar entry would change an approved navigation. But:

1. `MENU_ITEM` has no style for `aria-current`, so the mark is for assistive technology
   only. On a desktop a sighted person on Stats sees no highlighted place at all, in the
   bar or in the open menu. The defect "marks the wrong section" became "marks none".
2. The owner will test against the acceptance line as written and see it fail.

Failure scenario: owner test, desktop, opens Stats from the account menu, checks "`/stats`
highlights Stats in the top bar": nothing is highlighted; the menu, opened again, shows
four entries that look alike.

Suggested fix: (a) give the current entry of the account menu a visible state, e.g.
`aria-[current=page]:bg-accent aria-[current=page]:font-medium` in `MENU_ITEM` (the
contrast pair bg-accent/accent-foreground is already used for the focused entry; keep the
focus state distinguishable, e.g. by the weight alone for "current"); extend
`shell.spec.ts` "single-key shortcuts ..." by one computed-style assertion. (b) When
LC-1 becomes a DEC, rewrite the acceptance line of the plan to "on `/stats` the top bar
highlights nothing (it used to highlight Home); the account menu marks Stats, the phone
tab bar marks You" and name it in the owner's test notes as a deviation.

### POL-2 (Medium, must fix before merge) - German text of the new bulk question is not idiomatic

Where: `src/rust/crates/web/locales/de.ftl:422-425` (`episodes-again-ask-views`); LC-3;
pinned by `item_words.rs` test `the_bulk_question_names_a_rewatch_only_where_one_starts`.

Problem: "Nochmal als weiteres Mal eintragen?" says "again" twice, and the plural "Nochmal
als weitere Male eintragen?" is not something a German speaker writes ("Male" as a
countable object of "eintragen"). The English pair ("as a further view" / "as further
views") is fine. The owner tests in German; the sentence sits in a question the owner
approved in its other form.

Failure scenario: an In progress show, "Mark season watched" on ten watched episodes:
"Diese 10 Folgen sind schon gesehen. Nochmal als weitere Male eintragen?".

Suggested fix (same meaning, no new noun, matches "Einmal gesehen" / "{n}-mal gesehen" of
`item-times-watch`):

- one: "Diese Folge ist schon gesehen. Ein weiteres Mal eintragen?"
- other: "Diese { $count } Folgen sind schon gesehen. Alle ein weiteres Mal eintragen?"

Update the two German strings of the unit test with it. Name both wordings in the DEC
that LC-3 becomes, since the text is owner-visible.

### POL-3 (Medium, may defer) - a recent item keeps the provider's address after the cover has been stored

Where: `src/rust/crates/web/src/item.rs:257-258` (`remember_recent`),
`src/rust/crates/web/src/recent.rs:19-25`, `src/rust/crates/web/src/palette.rs:173-184`
(`idle_items`); LC-6; DEC-640.

Problem: DEC-640 is "the stored variant when the instance has one, else the provider's
address". A result row follows it on every search. A recent row does not: it shows the
address the item page had when the item was last opened, frozen in `localStorage`. An
item that is opened right after it was added (the usual case: Ctrl+K, Enter) is
remembered with the provider's address, because the asset job has not run yet. From then
on every opening of the empty palette asks the provider's image host for that picture,
until the person opens the item page again. The follow-up of DEC-640 tells operators that
`assets.fetch_policy = eager` reduces the hosts a browser contacts; for the idle palette
it does not.

Failure scenario: eager instance, a member adds eight movies from the palette, opening
each once. For weeks every Ctrl+K sends eight requests to `image.tmdb.org` (cached by
the browser, revalidated when the cache says so), although all eight covers are stored.
Second form: a stored variant that is replaced answers 404; LC-6 names this and the row
falls back to the icon, which is fine.

Suggested fix (one of): (a) when a search answer or a library read contains an item that
is in the recent list, rewrite its `cover` there (no extra request); (b) store only
same-origin addresses (`/assets/...`) in the recent list and show the icon otherwise;
(c) accept and write one sentence into the operations paragraph of POL-13. (b) is five
lines and keeps the one rule honest for the place where no fresh data is at hand.

### POL-4 (Medium, may defer) - the focus is put on the switch also for people who never had it there

Where: `src/rust/crates/web/src/notify_rules_view.rs:90-97` (`refocus_soon`), `:121-138`
(the effect), `:146` (`pressed.set_value`), `:189-195` (matrix), and
`src/rust/crates/web/src/keyboard.rs:169-179` (`focus_is_nowhere`); LC-7.

Problem: "the focus is on no control" is taken as "the switch lost it". That is true for
a keyboard and for a click in Chromium. In Safari and in Firefox on macOS a click on a
button does not focus it; the active element is `body` before and after. There the
package now calls `focus()` on the switch twice (after the save, after the rebuild)
although the person never had the focus on it, and `focus()` without `preventScroll`
scrolls the switch into view.

Failure scenario: Safari, a long matrix. Click a switch, scroll down to "Rules for one
item" while the save runs on a slow line: the page jumps back up to the switch when the
answer arrives, possibly a second time when the matrix is rebuilt. Second scenario:
`pressed` is set before the defaults offer is shown (`:146`) and is cleared only by a
rebuild or an error; the offer is kept across navigation (`kept("nf-offer")`), `pressed`
is not, so after leaving and coming back the answer to the offer does not restore the
focus (it falls on `body`, the old behaviour).

Suggested fix: remember the switch only when it has the focus at the moment it is
pressed (`document.activeElement` is the switch in `toggle`); pass `preventScroll` when
the switch is in the viewport anyway. For the offer: put the selector into `Offer` so it
is kept with it. The proper fix stays the one LC-7 names (keyed rows and cells,
`aria-disabled` instead of `disabled` while saving), which removes the need to restore
anything.

### POL-5 (Medium, may defer; a gate for the orchestrator) - `sharing.spec.ts:55` is at the edge of its 30 s by construction; no evidence that this package slows it

Where: `src/web/tests/sharing.spec.ts:55-140` (not changed by the package),
`src/web/playwright.config.ts` (default timeout 30 s), progress log "Host notes".

Reading: the test signs up two accounts (two password hashes), runs a palette search,
opens an item, logs with two people, opens the invite dialog, a second browser context,
History twice, People four times, two merges and an undo: about 25 navigations or
round trips with no `test.setTimeout`, while its neighbours of that length have 90 to
120 s. Reported times: 13.1 s with nine others, 29.9 s (passed) in full run 1, timeout
in runs 2 and 3 at two different lines (`hydrated` on History; "Copied to your log.")
with load average 6 to 8 on 4 vCPU. Different lines on each failure and a pass at 29.9 s
point to a test that needs about 30 s of a loaded host, not to a hang in the app.

Does the package touch what the test uses? Three things, none of them a new server cost:

- the palette search: each visible row now asks the TMDB mock for a picture and gets a
  404 (about eight small requests per search, to the node mock, not to the server);
- the item page and the History rows render `widgets::Cover`, which now flips to the
  title box when the picture fails (one signal per cover);
- `/history` calls `activity_feed`; the package moved the existing `self.user(...)` read
  three lines up (`app/src/activity.rs:258`), the number of statements is the same and
  without `year` no bounds are computed.

What is missing: a run of the same test on a build without the package under the same
load. All three full runs were with the package, so "the load" is the likely cause, not a
measured one.

Suggested action: the orchestrator runs `sharing.spec.ts` alone and the whole `chromium`
project once on merged main with no other lane building. Passing there closes this. In
any case the test should get `test.setTimeout(90_000)` like its neighbours (TEST-8), in
lane A's hands because sec1 edits every spec; that is not the workaround LANES.md
forbids, the test is simply three times as long as a default test.

### POL-6 (Low, may defer) - the date hints still say "Mark show" on a podcast

Where: `src/rust/crates/web/locales/en.ftl:2076,2081` and the same keys in `de.ftl`
(`episodes-range-hint`, `episodes-release-hint`), used by `episode_list.rs`
(`DateChoice`); LC-2.

Problem: LC-2 renames the bulk buttons of a podcast to "Mark season listened" / "Mark
podcast listened". The hint under the date choice names the buttons by their old short
names: "Mark season, Mark show and Mark up to here ...". On a podcast no button is called
"Mark show".

Failure scenario: a podcast's Episodes tab with the date choice open reads "Mark podcast
listened" on the button and "Mark show" in the hint beside it.

Suggested fix: a `-listen` pair of the two hints ("Mark season, Mark podcast and Mark up
to here ...") in `EpisodeWords`, or reword both hints without button names ("The bulk
marks spread ..."). Also check "as listened" against "listened to" used by
`episodes-again-ask-listen` and `episodes-skipped-listen` ("already listened to"):
"Also mark the 3 earlier episodes as listened?" is understandable, "as listened to" would
match; the plan itself wrote "Mark S1 E1 listened", so this is taste, not a deviation.

### POL-7 (Low, may defer) - the "failed before hydration" path of `Cover` has no test of its own, and the test for a failed cover does not say which path it took

Where: `src/rust/crates/web/src/widgets.rs:117-124` (the `Effect` with `complete()` and
`natural_width() == 0`); `src/web/tests/shell.spec.ts` "a cover that does not load ...";
`src/rust/crates/web/tests/wasm.rs` `a_cover_that_fails_to_load_shows_the_title_in_its_box`.

Problem: two mechanisms set `failed`: the `error` listener and the check at mount. The
wasm test mounts on the client (listener path). The browser test loads `/library` and the
item page with the pictures answered 404; whether the 404 arrives before or after
hydration is a race, and either mechanism alone makes the test green most of the time.
The red run of the progress log switched off "both `error` handlers", not the effect.

Failure scenario: a later change removes or breaks the effect (for example `node.get()`
no longer tracked); server-rendered pages whose pictures fail fast (a blocked host, the
CSP) show empty boxes again and every test stays green.

Suggested fix: in the browser test, hold the hydration back or answer the picture before
the script (`page.route` on `/pkg/*.js` with a delay of one second), then assert the
title box; or a wasm test that inserts the `img` markup, waits for its `error`, and only
then mounts/hydrates `Cover` over it. Also note for v1-manual: `natural_width() == 0`
is true for an SVG without intrinsic size, so a member-typed SVG address would be shown
as "failed" although it loaded.

### POL-8 (Low, may defer) - tests that were never red or cannot fail in a useful way

Rule: rules.md "TEST-DRIVEN: write the failing test, run it, see it fail". The progress
log is honest about each; listed here so the orchestrator can decide.

1. `src/rust/crates/ui/tests/wasm.rs`
   `the_key_hints_of_the_active_row_are_hidden_on_narrow_screens`: asserts that the class
   attribute contains `hidden` and `sm:flex`. It restates the source line, no stylesheet
   is loaded, and it was never seen red. It fails only when someone edits the class
   string. The behaviour is proven by `shell.spec.ts` "palette at 390 px ..." (seen red).
   Suggested: delete it, or keep it with a comment that it only pins the class.
2. `src/rust/crates/web/tests/wasm.rs`
   `a_cover_that_fails_to_load_shows_the_title_in_its_box`: written after the change, not
   seen red (the browser test was). Acceptable, see POL-7 for what it does not cover.
3. `src/rust/apps/server/tests/it/item_page.rs:243`
   `the_episode_rows_of_a_podcast_say_listened`: new, run only on the final code. It can
   fail (it asserts the old strings are absent), but its red was not seen. It also
   returns early without services (`if !services() { return; }`, the house pattern;
   `REQUIRE_SERVICES=1` guards the gate).
4. `palette::tests::thumb_prefers_the_smallest_stored_variant`: green on the old code by
   design (a guard for unchanged behaviour); fine, just not a red-first test.
5. `src/web/tests/shell.spec.ts` "palette: the library leads ...": the assertions on the
   first row's address and on `naturalWidth` sit inside `if (!src.startsWith("/assets/"))`
   and are skipped when another spec has stored that cover first. In the full parallel
   run this branch is order-dependent. The unconditional assertion two lines above it
   ("covers at the provider's address" > 0) keeps the test meaningful. Suggested: pick
   the first row whose address starts with `IMAGES` instead of the first row.
6. `notifications.spec.ts` `toBeFocused()` after `click()`: red was seen, so they work;
   note that they prove the restore for Chromium, where a click focuses the button
   (POL-4 is the case they cannot see).

### POL-9 (Low, may defer) - `year_bounds`: comment and code disagree for a skipped midnight, and a `None` silently drops the filter

Where: `src/rust/crates/app/src/activity.rs:213-232`, `:260`; LC-8.

Problem: the doc comment says "A midnight a zone skips or has twice takes the earliest
instant of that day". The code takes, for a skipped midnight, the midnight of 2 January
(LC-8 says so too), which would put everything of 1 January into the previous year. And
when that also yields nothing, `year_bounds` returns `None` and `q.year.and_then(...)`
turns into "no year filter": the page then lists every year under the heading of one.
No zone of today's tz database skips 00:00 on 1 January for years anyone logs in (the
cases are local-mean-time changes around 1900 to 1920), so this is about the comment and
about failing closed.

Failure scenario: none with real data today; with `year = i32::MAX` (`checked_add`) or a
year chrono cannot build, the feed is unfiltered instead of empty.

Suggested fix: for `LocalResult::None` walk forward in steps of 15 minutes to the first
instant that exists (at most a few steps), make the comment say that; when no bounds can
be built return an empty page (or `invalid_query`), not an unfiltered one.

### POL-10 (Low, may defer) - the year test covers one zone east of UTC only

Where: `src/rust/crates/app/tests/activity.rs:728-769`.

Problem: Europe/Berlin (UTC+1 in winter) and "no zone" are tested. The brief asks about
DST: on 1 January the southern zones are in daylight time (Australia/Sydney UTC+11,
America/Santiago UTC-3) and zones west of UTC put the boundary on the other side of the
UTC midnight. chrono-tz handles both and the code has no branch for them, so I expect
them to pass; they are cheap to pin.

Suggested fix: two more cases in the same test, e.g. `America/Los_Angeles` (a play at
2026-01-01 07:59 UTC is 2025) and `Australia/Sydney` (a play at 2025-12-31 13:00 UTC is
2026).

### POL-11 (Low, may defer) - a provider cover with an `http:` address on an https instance is blocked by the CSP of sec1

Where: `src/rust/crates/web/src/palette.rs:267-278` (`thumb`), `widgets.rs` (`Cover`);
main checkout `apps/server/src/security_headers.rs:120,137`.

Problem: sec1 allows `http:` pictures only when the instance itself is on http. Podcast
artwork addresses from feeds are often `http://`. With DEC-640 the palette and the
recent list now show provider addresses too, so on an https instance such a cover is
refused in three more places (result rows, recent rows, and as before the cards until
the asset is stored). LC-5 turns that into the kind icon or the title box, so nothing
looks broken; each refusal is a line in the browser console.

Failure scenario: https instance, lazy fetch policy, search "radio" in the palette: the
podcast rows whose feed gives `http://` artwork show the icon only; the console lists
one policy violation per row.

Suggested fix: none in this package. For the connector or `api::dto::cover`: hand out the
address with `https:` when the source gave `http:` (browsers do the same upgrade for
mixed pictures), or leave `cover_url` out for `http:` addresses on an https instance.
Write the decision next to DEC-640.

### POL-12 (Low, may defer) - at 400 % zoom a keyboard user on a desktop loses the visible key hints

Where: `src/rust/crates/ui/src/command.rs:336` (`hidden ... sm:flex`); LC-4.

Problem: below 40 rem the hints of the active row are not shown. A desktop at 1280 px
and 400 % zoom is 320 px wide, so a sighted keyboard user who zooms does not see "Enter
Open / Ctrl+Enter Add as Planned". The hints stay in the input's description (screen
readers); whether the shortcut help lists the two palette keys was not checked. Not a
WCAG failure (1.4.10 asks
for no loss of content or function; the functions remain and are documented), but the
one group that needs the hint most at that width is the one that loses it.

Suggested fix: instead of hiding, put the hints on a line of their own under the title
below 40 rem when the device has a fine pointer or a keyboard
(`@media (hover: hover)`), or show them in the palette's footer. LC-4 lists this as an
alternative; fine to defer.

### POL-13 (Low, may defer, needs an owner) - "Which hosts a browser contacts for pictures" is written nowhere and belongs to no package

Where: `docs/modules/ROOT/pages/operations.adoc` (lane and main: no such paragraph),
`docs/modules/ROOT/pages/v1-scope.adoc`; DEC-640 "Consequences written down";
`v1-plan.md` section 2 "Follow-ups written down, not built here".

Problem: P7 makes the browser of every member ask the image hosts of the sources while
typing in the palette (before, only for items already opened or added). The plan lists
the operations paragraph as a follow-up "not built here" and names no package for it;
sec1 changed operations.adoc and did not add it. After the merge of both the behaviour
is live and the page an operator reads does not mention it.

Failure scenario: an operator who chose the instance for privacy learns from the browser's
network tab that a search contacts `image.tmdb.org`.

Suggested fix: three sentences in operations.adoc (hosts: the image hosts of TMDB,
TheTVDB, IGDB, podcast feed hosts; when: search results, cards and recent items until the
cover is stored; how to reduce: `assets.fetch_policy = eager`, with the limit of POL-3)
and a pointer from "Limits to know" of `v1-scope.adoc`. Assign it at merge (v1-polish
itself is the natural place since it ships the behaviour).

### POL-14 (Low, may defer) - "What v1 covers": small points

Where: `docs/modules/ROOT/pages/v1-scope.adoc`.

1. Line 26, "Anime that TMDB lists can be tracked as a show": true, but then the anime
   counts as kind Show in the statistics and filters; one half sentence avoids a
   surprise ("and then counts as a show").
2. "Kinds without a source" says "the search finds nothing new for them". The palette
   footer already says which groups have no source; fine. Books and board games are in
   this list and will move with v1-books and v1-wikidata: the source comment at the top
   says so, but a merge of lane B before this page is updated makes the page untrue for
   books. Put the update of this page into the merge checklist of lane B.
3. "one line per item with the `kind` and a `title` is enough": the example lines also
   carry `source_id`, which `import-format.adoc:20` marks as required. Say "`source_id`,
   `kind` and `title`".
4. The xrefs resolve (`operations.adoc` has `== Backup`, `connectors.adoc` and
   `import-format.adoc` exist). Not built with Antora here.

### POL-15 (Low, may defer) - scope additions to confirm

1. F1 and F2 (focus on the pressed switch; focus back on "Edit"): not in section 2 of
   the plan nor in LANES.md ("Lane C: v1-polish, then v1-manual"). The progress log takes
   them from sec0's "left open" list (`sec0-progress.md:177-178`). Sensible and small,
   but they are 70 lines in two files of another package's area
   (`notify_rules_view.rs`, `notify_channels.rs`, `keyboard.rs`); the orchestrator should
   confirm they were assigned here and that lane A (sec1/sec2) does not touch the same
   functions. In the main checkout's working tree these three files are not modified, so
   no textual conflict today.
2. `aria-current` on every entry of the account menu (Notifications, Settings, Admin),
   not only Stats: wider than P1, consistent, covered by a unit test. Fine; note it in
   the DEC.
3. `palette::thumb` became `pub(crate)` and is called from `item.rs`: fine.
4. Not done and correctly left alone: `sharing.spec.ts`, DECISIONS.md, any commit.

### POL-16 (Low, may defer) - small code notes

1. `src/rust/crates/web/src/widgets.rs:139-141`: `on:load` sets `failed` back to
   `false`. Once `failed` is true the `picture` is `display: none`, so a lazy picture
   will not load again by itself; the handler only matters when the address changes on
   the same element. Harmless, but the sentence "A later `load` brings the picture back"
   in LC-5 promises more than happens: a cover that failed while offline stays a title
   box until the card is rendered anew (same as the broken picture before).
2. `src/rust/crates/ui/src/command.rs:370-383` (`Leading`): `failed` belongs to the row,
   rows are keyed by item id. When the same row gets another picture address (the cover
   is stored while the palette is open) the `img` keeps `hidden` from the earlier
   failure unless a `load` resets it; there is no `on:load` here. Add
   `on:load=move |_| failed.set(false)` for symmetry with `Cover`.
3. `src/rust/crates/web/src/shell.rs:92-106` (`account_entry`): `/settings/notifications/x`
   maps to Settings, `/settings/notifications` to Notifications; fine today (no
   sub-pages), fragile if one is added. A prefix match would be sturdier.
4. `src/rust/crates/web/src/palette.rs:173-184`: the suggestion "Log a play of <title>"
   (`search-cmd-log-play`) still says "play" for a video game although the toast after
   it now says "Session". `RecentItem.kind` is now at hand to choose the word. Outside
   P3 as written ("the toast"); same for the tab "Plays" and "No plays logged yet." of a
   video game. One follow-up line, not this package.

### POL-17 (Info) - merge with main

- The lane has no commit; `git merge-tree main HEAD` is trivially clean. What matters is
  the working tree against main plus sec1's uncommitted changes.
- Three-way merge simulated with `diff3 -m` (lane file, base `cb2f7a0`, main checkout's
  file) for the five files both sides changed: `operations.adoc`, `nav.adoc`,
  `item.spec.ts`, `notifications.spec.ts`, `shell.spec.ts`: zero conflict blocks each.
  In `shell.spec.ts` sec1 changed line 1 (`from "./fixtures"`) and this lane line 3 (adds
  `expectReflow` to the helpers import) with one unchanged line between; git may or may
  not call that adjacent. If it conflicts, the resolution is: line 1 from main, line 3
  from the lane.
- After the merge `tests/global.setup.ts` ("every spec uses the shared fixtures") must
  pass: the lane adds no new spec file and no new import of a Playwright package.
- The new `IMAGES` constant of `shell.spec.ts` assumes the server of the tests is on
  http (so `img-src` carries `http:`). If the e2e server ever moves to https, the three
  palette and cover tests will be failed by the policy guard, not by their own
  assertions; a comment at `IMAGES` would save the next person an hour.
- No Rust source file of the lane is modified in the main checkout's working tree
  (sec1: `web/src/app.rs`, `auth`, `config`, `apps/server`); `item_page.rs` of the server
  tests is not touched by sec1.

### POL-18 (Info) - LC-2 to LC-4, LC-9 checked and fine

- P3: `EpisodeWords::of` covers every label of the tab that names the act (state, button,
  both button names, two bulk buttons, the question, "Remember for this ..."); only the
  two hints of POL-6 are left. Locale parity holds by inspection (15 keys added to each
  file, same names). Board game "Play/Partie", every other kind "Session/Sitzung":
  matches `next-step-log-session`, `history-session`. `ItemInfo.board_game` has one
  constructor (`item.rs:300`).
- P6: `again_ask_key(verb, bulk.start_on_again)` uses the same flag that decides whether
  "They are logged with today's date." is shown, so text and effect cannot drift apart.
- P4: the row is `flex items-center`, the label `min-w-0`, so the title wraps and the row
  grows; rows keep `min-h-11` on phones (2.5.8).
- P2: the cursor logic (`before.min(end)`) is unchanged; the year options come from
  `stats_series(all, year)`, which buckets with the same zone function, so an option
  never points at an empty year because of the zone.
- P8: the page exists, is the second nav entry and is linked from Backup.

### POL-19 (Info) - rules check

- ASCII: the new and changed files outside `de.ftl` are ASCII (checked the three docs,
  the three specs and the touched Rust sources; German test strings use `\u{..}`).
- White-label: no brand string added (`app-recent-` is the existing key prefix).
- No GPL code, no hand-drawn SVG, no AI attribution found in the diff.
- Decisions are in the lane file as LANES.md asks; DECISIONS.md untouched. LC-1, LC-2 and
  LC-3 are owner-visible and say so.
- Nothing committed, as the lane rules ask.

### POL-20 (Info) - not verified here

- No build, clippy or test run (read-only brief). Function lengths were counted from
  the source text with a script, not by clippy.
- Whether `class:hidden` together with a static `class` on the same `img`
  (`command.rs:378-380`) behaves in every Leptos path was taken from the reported green
  wasm and browser tests, not re-run.
- Antora build of the docs page not run.

## Must fix before merge

1. POL-1: visible mark for the current entry of the account menu; acceptance line of the
   plan rewritten and the deviation named for the owner.
2. POL-2: German text of `episodes-again-ask-views` (and the two strings of its test).

## May defer (assign at merge)

POL-3 (recent rows and the one rule), POL-4 (focus restore for pointer users), POL-5
(run `sharing.spec.ts` on merged main without load; `test.setTimeout` via lane A),
POL-6, POL-7, POL-8, POL-9, POL-10, POL-11, POL-12, POL-13 (needs an owner), POL-14,
POL-15, POL-16.
