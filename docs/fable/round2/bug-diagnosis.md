# Bug diagnosis for the owner test findings (F3, read-only)

Date: 2026-11-10. Base: `main` at 225b09d. Nothing was built or run: every claim comes from
reading the code and the existing tests. Paths are relative to the repo root; `crates/` means
`src/rust/crates/`. "Unsure" marks what a workstream agent
must confirm with the first failing test.

Order: by suggested fix order (cheap and certain first).

## OT-8 History tab label shows "History0" (no space before the count)

**Cause (certain).** Not Leptos: CSS. The label is built as three children of the tab button,
a text, a whitespace-only text `" "` and a `<span>` with the count
(`crates/web/src/item.rs:745`). The tab button is a flex container: `TabsTrigger` renders
`<button class="inline-flex items-center justify-center whitespace-nowrap ...">`
(`crates/ui/src/tabs.rs:110`). In a flex container every run of child text becomes an
anonymous flex item, and a run that holds only white space is not rendered at all (CSS
Flexible Box Layout Level 1, section 4 "Flex Items":
https://www.w3.org/TR/css-flexbox-1/#flex-items , quoted from memory on 2026-11-10, not
re-fetched). So "History" and the count sit next to each other with no gap, whatever the
`view!` macro does with the literal. The hint about Leptos whitespace handling is therefore
moot; I did not verify how Leptos 0.8.23 (`src/rust/Cargo.toml:66`) emits the literal.

Introduced with the tabs of UX phase 1: `git log -S'{t("item-history")} " "'` -> add6203
"feat(ux): phase 1 - where am I, what is next".

Same pattern elsewhere: I checked every other `" "` literal in `crates/web/src`
(`history.rs:159`, `item.rs:807`, `item.rs:899`, `notifications.rs:284`, `bulk.rs:181`,
`imports.rs:153`, `imports.rs:500`, `auth_pages.rs:567`, `admin/users.rs:372-381`,
`admin/storage.rs:49,83,311`, `admin/sso.rs:191-192,660`, `admin/mod.rs:289`). Their parents
are `<p>`, `<span>`, `<li>` or a table cell without `flex`/`grid` in the lines I read, so
they render the space. Unsure only for `TableCell` on phones (it takes a `label`, so it may
switch to a flex/grid layout there): check `crates/ui/src/table.rs` when touching it.

**Why no test caught it.** The Playwright specs match the tab with `/^History/`
(`src/web/tests/item.spec.ts:136`, `:374`), and the accessible name is "History 0" (the
text node still exists in the DOM), so even an exact name match would pass. Only a visual
or layout assertion can fail.

**Failing test first.** Playwright, `src/web/tests/item.spec.ts`, new test
`"the History tab shows its count apart from the label"`:

- open an item page with one history entry (reuse the seeded item of the test at line 136);
- `const tab = page.getByRole("tab", { name: /^History/ }); const count = tab.locator("span");`
- assert the gap: `const label = await tab.evaluate(el => { const r = document.createRange();
  r.selectNode(el.firstChild); return r.getBoundingClientRect().right; });`
  `const left = (await count.boundingBox()).x; expect(left - label).toBeGreaterThanOrEqual(3);`
- assert `await expect(tab).toHaveAccessibleName("History 1")` (guards the fix against
  dropping the space from the name).

A cheaper unit guard in `crates/ui/tests/render.rs`: `TabsTrigger` renders the class `gap-1.5`
(only if the fix below goes into `ui`).

**Fix.** Give the trigger a gap instead of relying on a text space: add `gap-1.5` to the
`TabsTrigger` class list (`crates/ui/src/tabs.rs:110`). Keep the `" "` in `item.rs:745`: it
is what puts the space into the accessible name and into copied text. Alternative with no
`ui` change: wrap label and count in one `<span>` in `item.rs:745` (text inside a plain span
keeps its spaces). I prefer `gap-1.5`: every later trigger with an icon or a count gets it
for free.

Files: `crates/ui/src/tabs.rs` (one class), test file. Risk: none for keyboard, palette,
search, bulk dates. `gap` only acts between flex items, and single-text triggers have one.

**Size: S.** No F2 dependency. (F2 may still want a different label, for example
"History (3)"; that is copy, not this bug.)

## OT-1 Palette results cannot be reached with the keyboard

**Short answer.** I could not find a logic fault that stops the arrow keys: the combobox
pattern is implemented and unit-tested. I found four defects that together produce what the
owner describes, and one coverage hole (no test presses an arrow key in the real palette).
The first test below decides which of them is the live cause. Treat "cause" here as
"ranked candidates", not as proven.

**What is implemented (verified in code).**

- `Command` is an APG "combobox with listbox popup": the `<input role="combobox">` keeps DOM
  focus and names the active option with `aria-activedescendant`
  (`crates/ui/src/command.rs:133-145`); ArrowDown/ArrowUp move `active` with wrap-around,
  Enter selects, Ctrl/Cmd+Enter runs the secondary action (`command.rs:56-83`).
- Options are `<li role="option">` without `tabindex` (`command.rs:201-214`), so Tab never
  lands on them. That is what the pattern asks for: "DOM focus remains on the combobox", and
  "the popup, and the popup descendants are excluded from the page Tab sequence"
  (WAI-ARIA APG, Combobox pattern, https://www.w3.org/WAI/ARIA/apg/patterns/combobox/ ,
  read 2026-11-10). So "Tab does not reach them" is by design and must stay; the arrow keys
  are the way in.
- The dialog only handles Escape and Tab (`crates/ui/src/dialog.rs:144-156`); the window
  key listeners do not act while typing or while a layer is open
  (`crates/web/src/keyboard.rs:108-134`, `shell.rs:283-287`), and the library grid listener
  returns unless a grid card has focus (`library.rs:969-976`). Nothing swallows the arrows.
- A wasm test proves ArrowDown moves `aria-activedescendant` and Enter picks that option
  for a static list (`crates/ui/tests/wasm.rs:104-135`), and a second one that the hints
  follow the active row (`wasm.rs:561-615`).

**Candidates, most likely first.**

1. **The active row cannot be seen (WCAG 1.4.11, 2.4.7).** The only mark of the active
   option is `bg-accent text-accent-foreground` (`command.rs:209`). `--accent` is zinc-100
   on a white `--popover` in the light theme and zinc-800 on zinc-900 in the dark theme
   (`src/web/style/input.css:17,25` and `:67,75`). By my calculation that is about 1.1:1
   (light) and 1.2:1 (dark) against the row background, far below the 3:1 that a focus or
   state indicator needs. The key hints do move with the active row (`command.rs:186-198`),
   but they are small text at the far right. An owner pressing ArrowDown sees no selection
   move: "arrow keys do not move through the results". This also fits OT-13 ("very black
   and white").
2. **The active option is never scrolled into view.** The list is `max-h-80
   overflow-y-auto` (`command.rs:155`) and there is no `scrollIntoView` anywhere in
   `command.rs`. "Harry Potter" brings up to 20 rows per source (`api/src/ops.rs:157`,
   `:233` limit 20) plus group headings, so after about eight presses the active row is
   below the fold, and ArrowUp from the first row wraps to the last row, which is invisible.
3. **Every result batch resets the active row to the first one.** `Effect { filtered.track();
   active.set(0) }` (`command.rs:119-122`) runs whenever the filtered list changes. In the
   palette the list changes once per answering source for seconds after typing
   (`crates/web/src/palette.rs:312-318`), so arrow presses made while "Asking tmdb, igdb..."
   is shown are undone. On the owner's network the sources were slow ("retrying").
4. **Hover steals the active row.** `on:mousemove=move |_| active.set(index)`
   (`command.rs:205`). The dialog is centered (`dialog.rs:211`) and grows as results arrive,
   and the active row changes height when the long hint ("Ctrl+Enter Add as Planned or next
   step", `shrink-0`, `command.rs:190`) squeezes the title into more lines, so rows move
   under a resting pointer. Whether a browser fires `mousemove` for a pointer that did not
   move is browser-specific; I believe current Chrome and Firefox do not, so this is the
   least likely. The existing e2e test activates its row with `option.hover()` instead of
   arrow keys (`src/web/tests/shell.spec.ts:291-293`), so the hover path is the only one
   tested in the app.

Not a cause but found on the way (same files, fix in the same package):

- **Client filter drops server hits.** `Command` filters the given items again by substring
  of the typed text (`command.rs:46-51`, `:111-118`). The palette passes server results
  through it (`palette.rs:643-652`), so a typo-tolerant hit ("harry poter" ->
  "Harry Potter ...", DEC-493) or a hit matched on the description is removed in the
  browser. The search can then look empty while the server answered. This matters for OT-3.
- **Lost first batch (race).** `search_id` is set only after the HTTP answer of `search`
  (`palette.rs:397-409`), but the server starts the fan-out before it answers
  (`api/src/ops.rs:206-208`). A `search.results` event that arrives first is dropped by the
  id check (`palette.rs:308-310`) and its source stays in "Asking ..." forever. Unsure how
  often this happens (a cached provider answer can be faster than the response).

**Failing test first.** Playwright, `src/web/tests/shell.spec.ts`, new test
`"palette: arrow keys move through provider results and Enter opens the active one"`:

1. sign up, `Control+k`, fill "fight club" (the fixture query of the test at line 273), wait
   until the "Asking ..." line is empty (all sources answered) and at least 3 options show;
2. `const box = palette.getByRole("combobox"); const options = palette.getByRole("option");`
   expect `options.nth(0)` to have `aria-selected="true"`;
3. press ArrowDown twice; expect `options.nth(2)` `aria-selected="true"`, `options.nth(0)`
   `"false"`, and `box` `aria-activedescendant` equal to `await options.nth(2).getAttribute("id")`;
4. expect the combobox still focused (focus never leaves the input);
5. **visibility:** read `getComputedStyle` of the active and of an inactive option and
   assert they differ in more than the background, for example
   `outline-style !== "none"` or a `box-shadow`/border on the active one (this is the
   assertion that fails today if candidate 1 is the cause);
6. press ArrowUp three times (wraps to the last option); expect
   that option `toBeInViewport()` (fails today: candidate 2);
7. press Enter; expect the URL `/items/<id>` of that option.

If steps 1-4 already fail, the fault is functional and candidate 3 or 4 is live: add
`await page.mouse.move(x, y)` onto the list before typing to tell 4 from 3.

Plus two wasm tests in `crates/ui/tests/wasm.rs` (fast, no server):

- `command_keeps_the_active_option_when_items_are_appended`: mount with a `RwSignal<Vec<_>>`
  of 3 items, ArrowDown twice, push a fourth item, settle; assert `aria-activedescendant` is
  still the third option (fails today: candidate 3).
- `command_shows_items_as_given_when_filtering_is_off`: with the new `filter=false` prop and
  query "xyz", all items stay listed (the client-filter fix).

**Fix.**

- `crates/ui/src/command.rs`:
  - make the active option visible without colour alone: keep `bg-accent` and add a 2 px
    inset ring or a start border in `ring`/`primary` colour (3:1 against the popover), in
    both themes; tokens only, no hard-coded colour (rule 9);
  - after `active` changes by key, call `scroll_into_view_with_bool(false)` (or
    `scrollIntoView({block: "nearest"})`) on the option element by its id; not on hover;
  - remember the active option by **item id**, not by index: on a list change keep the same
    id if it is still listed, else fall back to 0 (replaces the unconditional reset at
    `:119-122`);
  - hover: only take over on a real pointer move (compare `client_x/client_y` with the last
    event) so a list that moves under a resting pointer changes nothing;
  - new prop `filter: bool` (default true for the gallery); the palette passes `false` for
    server results and keeps `true` for the idle list (recent items and commands,
    `palette.rs:616-622`).
  - optional, APG optional keys: Home/End stay with the text cursor; PageUp/PageDown not
    needed.
- `crates/web/src/palette.rs`: pass `filter`; set `search_id` before the request (generate
  the id in the browser and send it, or buffer events that arrive before the answer). The
  second needs an API field (`SearchResponseDto` / query param): keep it as a separate
  small change.
- Fluent: none, unless a status line "N results" is added for screen readers (the APG does
  not require it).

Risk to other behaviour: DEC-426 (palette keys: Enter open, Ctrl+Enter quick add, palette
stays open) is untouched, the key handler keeps its three keys. DEC-422..430 keyboard map:
no global key changes. The ring must not break the 44 px rows on phones
(`command.rs:207`). Id-based active state changes what Ctrl+Enter acts on while results
stream in: it now stays on the row the person chose, which is the safer direction (today a
late batch can move the action to another item between ArrowDown and Enter).

**Size: M** (S for visibility + scroll alone). No F2 dependency; if F2 changes the brand
tokens (OT-13) the ring colour follows the tokens.

## OT-3 Search ranking: a derivative title above the original

**Cause (certain as a design gap; which path hit the owner is unsure).** Nothing in the
search path knows which "Harry Potter" is the well-known one, and three mechanisms can put
a derivative title first:

1. **No popularity or exact-match signal exists anywhere.** A connector result
   (`Snapshot`, `crates/catalog-core/src/model.rs:108-121`) carries title, original title,
   year, description, attributes, images: no popularity, votes or rank. The catalog search
   document is `title, original_title, description, kind, year`
   (`crates/search/src/indexer.rs:164-170`). `grep -rn "popularity\|vote_count"` over
   `crates/` finds nothing outside tests. So the hint is confirmed: neither provider
   popularity nor exact-prefix match is used at all.
2. **Local ranking cannot tell the two apart.**
   - Meilisearch (primary): only filterable and searchable attributes are set
     (`crates/search/src/meili.rs:55`, `:64`); the ranking rules are the engine defaults and
     there is no custom rule. Both titles hold both query words at the start of `title`, so
     they tie on every text rule and the engine's internal order decides. (Engine default
     rule list not re-read today; DEC-493 cites "words, typo, ...".)
   - Postgres fallback: `score = ts_rank(tsv, query) + similarity(body, query)`, ties by id
     (`crates/search/src/pg.rs:127-135`), with `body = title + " " + original_title`
     (`pg.rs:73`). `similarity` is whole-string trigram similarity, so the **shorter** body
     wins, and a title whose original title repeats the words gets a higher `ts_rank`. Both
     effects are unrelated to which film people mean. DEC-493 only ordered "words before
     typos"; it did not add any tie-break among documents that hold all words.
3. **Merge order: old local copies beat fresh provider answers.**
   - Every provider hit of every search is stored as a catalog item
     (`crates/catalog-core/src/search.rs:181-183`, `ensure_catalog_item`) and then indexed.
   - The palette shows the local hits first and appends each source's hits as they arrive,
     in arrival order of the sources (`crates/web/src/palette.rs:401`, `:312-318`;
     `FuturesUnordered` in `catalog-core/src/search.rs:155`, `:177`).
   - The palette searches after every 250 ms pause (`palette.rs:368-414`), so typing
     "Harry Potter" slowly runs several searches ("harr", "harry p", ...). From the second
     one on, the hits of the earlier ones are "local" and are ranked by rule 2 above the
     provider's own (better) order.
   - Within one source the provider's order is kept (`crates/connectors/src/tmdb.rs:236-239`:
     page 1, first `limit` rows, no sort parameter), so a first-ever search shows TMDB's
     order. If the odd title was first in the "tmdb" group on a first search, the order came
     from TMDB itself and only signal 1 can fix it. I cannot tell which case the owner saw:
     the group heading of the row ("Library/local - Movie" vs "tmdb - Movie") decides.

Related defect (see OT-1): the palette filters server hits again by substring
(`crates/ui/src/command.rs:46-51`), so typo hits never show; "words first, typos later" is
invisible in the palette today.

**Failing tests first.**

- `crates/search/tests/suite.rs` (runs on both backends, `:162`, `:170`), new block
  `"the better-known title wins a tie on the words"`: index, in this order, `d2` = title
  "Harry Potter and the Order of the Homeless" (popularity 2), `d1` = title "Harry Potter
  and the Philosopher's Stone", original_title the same (popularity 900), `d3` = title
  "Harry Potter" (popularity 5). Query "harry potter": expect `["d3", "d1", "d2"]` (exact
  title first, then by popularity). Second query "harry poter" (typo): the three still
  come, same order. Insert `d2` first so insertion order cannot make the test pass.
- `crates/connectors/tests/tmdb.rs`: `search_hits_keep_popularity`: the search fixture row
  carries `popularity`; the snapshot exposes it. (Check the field name in the recorded
  fixture and the TMDB reference before writing it: rule 1. I did not verify the TMDB,
  IGDB or Open Library field names today.)
- Rust unit in `crates/web/src/palette.rs`: extract the merge into a pure function
  `merge(local, batch) -> Vec<CommandItem>` and test `library_entries_lead_and_a_source_
  keeps_its_order`.
- Playwright (after the above): `shell.spec.ts`, with the TMDB mock returning the original
  second: the original's option index is lower than the derivative's.

**Fix (three steps, each shippable).**

1. **S - stop the palette from fighting the server:** `filter=false` for server results
   (OT-1 fix). Show as "local" only items that are in the person's library (the entry is
   what makes a local hit more relevant); all other catalog copies merge into their
   source's group or are left to the provider answer. Files: `palette.rs`,
   `api/src/ops.rs:150-190` (the response could flag `in_library`; today the DTO has no such
   field: `contracts/src/lib.rs:78-95`).
2. **S/M - tie-breaks from data we have:** after the text score: exact title match
   (case-insensitive) first, then title prefix. Postgres: add the two booleans to the
   `ORDER BY` of the `exact` CTE (`pg.rs:127-135`) - no extra scan, the typo phase stays
   behind its one-time filter (DEC-493 timing must be re-measured only if the plan
   changes). Meilisearch: move `exactness` up via `set_ranking_rules` in `connect`
   (`meili.rs:39-70`).
3. **M - popularity:** `Snapshot.popularity: Option<f32>` -> `catalog_items.popularity`
   (migration 0032) -> search document field -> Meilisearch custom ranking rule
   `popularity:desc` after the text rules; Postgres `ORDER BY phase, exact, prefix,
   popularity DESC NULLS LAST, score DESC, id`. Each connector maps its own number
   (document the field per connector in `docs/connectors/<id>.md`; values are not comparable
   across providers, so they only order hits of the same kind/source).

Risk: DEC-493 (search budget): steps 1-2 add no statement; step 3 adds a column to the
index row only. A settings change in Meilisearch re-indexes on start (one task per index).
DEC-426 palette keys: untouched. Step 1 changes what "local" means in the palette footer
texts (`search-local`), copy check in en/de.

**Size:** S (step 1), S/M (step 2), M (step 3). **F2 dependency:** none; the owner should
confirm that popularity may be stored per item (TMDB terms 1.C six-month cache already
governs provider data, DEC-453).

## OT-15 "Mark up to here" does nothing on watched episodes

**Cause (certain).** The row menu item "Mark up to here" calls `mark(((season, episode),
true))` (`crates/web/src/episode_list.rs:487-491`). For `up_to` the handler takes
`unwatched_up_to(rows, target)` and hands the list to `bulk`
(`episode_list.rs:337-342`). `unwatched_up_to` keeps only episodes that are **not** in
`SeasonRow.watched` (`crates/web/src/episode_plan.rs:96-110`), and `watched` is the set of
every episode that has any episode event in the item's history, with no notion of a first
or second pass (`episode_plan.rs:28-57`). When everything up to the target is watched the
list is empty; `bulk` passes it on and `log_episodes` returns at once without a message:
`if eps.is_empty() { return; }` (`episode_list.rs:92-94`). No request, no toast, no error:
"does nothing".

The same holds for "Mark season watched" and "Mark show watched"
(`episode_list.rs:332-335`, `:364-367`, `episode_plan.rs:114-131`), which the owner will hit
next. Only the per-row button rewatches: on a watched episode it is labelled "Again" and
logs that one episode again (`episode_list.rs:526-529`, `plan_single`, test note at
`episode_plan.rs:487`).

Introduced with the episode list in UX phase 1 (`git log -S unwatched_up_to` -> add6203);
it is the designed behaviour for a first pass ("Unwatched episodes from the start up to and
including target", `episode_plan.rs:93-94`), the rewatch case was never specified. So this
is a model gap, not a slip.

**Failing tests first.**

- Rust unit, `crates/web/src/episode_plan.rs` (tests module), name
  `up_to_here_on_watched_episodes_is_a_rewatch_not_nothing`: rows = one season of 5 with
  1..=5 watched; the new planner (below) returns `UpTo::Rewatch(vec![(1,1),(1,2),(1,3)])`
  for target (1,3); for 1..=2 watched and target (1,4) it returns
  `UpTo::Log(vec![(1,3),(1,4)])` (today's behaviour, must not change).
- Playwright, `src/web/tests/item.spec.ts`, name
  `"Mark up to here on watched episodes offers a rewatch and logs it"`: seed a show with
  season 1 episodes 1-3 watched (the seed helper at the top of the file, lines 15-30, takes
  a `history` array); open the Episodes tab; open the row menu of episode 3; choose "Mark up
  to here"; expect a visible question (role `group`, like `episodes-ask`) naming 3 episodes;
  confirm; expect the toast and `GET /api/v1/items/{id}` (or the History tab) to show 6
  episode events. Second assertion for the minimum fix: nothing is ever silent - if the
  model says "no rewatch here", a `role="status"` line says why.

**Fix, depending on the F2 model decision** (OT-9/OT-15 are one topic):

- **Outcome A: a rewatch is "the same episodes logged again" (no new state).** S-M, web only.
  - `episode_plan.rs`: add `enum UpTo { Log(Vec<Ep>), Rewatch(Vec<Ep>) }` and
    `plan_up_to(rows, target)`: unwatched ones if there are any, else all episodes from the
    start to the target.
  - `episode_list.rs:337-342`: on `Rewatch` do not log at once; show an inline question in
    the place of `AskPrevious` ("All N episodes up to here are watched. Log them again?"
    with "Log N again" and "Cancel"), inline and not a dialog (addendum-1 D15,
    `episode_list.rs:424-425`); on yes call `bulk`.
  - Same for season and show ("Mark season watched" on a complete season).
  - `log_episodes` keeps its guard, but callers never pass an empty list silently: add an
    `ui.error`/status text for the remaining empty case.
  - Fluent en/de: `episodes-ask-again` (plural), `episodes-ask-again-yes`.
  - Open point for F2: which pass does a partly rewatched season belong to? With A the list
    cannot show "2 of 10 in the rewatch"; chips still say 10/10.
- **Outcome B: a rewatch is a state of the entry (a numbered pass).** L, crosses crates.
  - Domain/app: a pass counter on the library entry or on episode events, a "Start rewatch"
    action, `watched` computed for the current pass; the item page summary
    (`crates/web/src/item.rs:361`, "rewatches" = finished - 1) and stats
    (`app/src/stats.rs:26-28`, counts include rewatches) must agree with it; a migration.
  - Web: `season_rows` gets the pass's events only, then `unwatched_up_to` works unchanged
    and this bug disappears without a special case.
  - The minimum fix (never silent) still applies before B lands.

**Minimum fix that needs no decision (do now, S):** when a bulk mark resolves to no
episodes, say so (`role="status"`, "Every episode up to here is already watched. Use Again
on an episode to log it once more."). It removes the dead click and does not prejudge A/B.

Risk: bulk dates DEC-446..451. "On release dates" must not be offered for a rewatch (it
would put the second pass on the original air dates): `offered` and `missing` are computed
from the unwatched episodes of the season (`episode_list.rs:325-329`), so for a fully
watched season `missing == 0` and "On release dates" **is** offered today; with A the
rewatch question must force `now` or `range`, and `release_dates` must not be called for
`UpTo::Rewatch`. "Spread over a date range" works as is (`spread_over` takes a count).
Offline queue: unchanged, the same `log_progress` calls. Unsure: I did not trace whether the
server collapses two events for the same episode on the same day; the "Again" button uses
the same call (`episode_list.rs:343-345` -> `log_episodes`), so it should not.

**Size:** S (minimum), S-M (A), L (B). **Depends on F2:** yes.

## OT-11 Game score and "won" show nowhere

**Cause (certain): the play is stored completely, and no read path shows score or outcome.**

Write path (works):

- the play sheet builds players with `score` and `winner`, plus `coop` won/lost
  (`crates/web/src/log_form.rs:227-258`);
- `POST /items/{id}/plays` stores a session with participants
  (`crates/api/src/ops.rs:460-505`; `crates/app/src/sessions.rs:64-124`) in `sessions` and
  `session_participants.score/winner` (`crates/store-pg/migrations/0001_core.sql:142-167`),
  and one progress event `Progress::Session { session_id }` (`sessions.rs:116-123`);
- the e2e test asserts exactly this through `GET /api/v1/sessions`, never through the UI
  (`src/web/tests/item.spec.ts:169-172`).

Read paths (the gap):

1. **History feed and Home activity.** The row DTO has no field for a score, a winner, a
   co-op outcome or a duration: `ActivityRowDto` (`crates/contracts/src/browse.rs:241-269`)
   and its people are `companion_id, name, is_me` only (`browse.rs:231-236`; mapped in
   `crates/api/src/browse.rs:412-441`). The row text is a fixed sentence: a session event
   renders `progress-session` = "Session logged" (`crates/web/src/widgets.rs:557`,
   `locales/en.ftl:517`), a session row without progress `history-session` = "Session
   logged" (`crates/web/src/history.rs:52-56`, `en.ftl:663`). People are listed as "with
   ..." **without** the viewer (`history.rs:43-49`, `:101-103`), so a solo play of a video
   game reads "Lego Harry Potter - Session logged" and nothing else. The play is in the
   feed, but it does not look like the play the owner logged.
2. **Item page, History tab.** Built from the item's progress events only, with
   `participants: Vec::new()` and `session_id: None`
   (`crates/web/src/item.rs:817-841`): same sentence, no people at all.
3. **Stats.** The Play group charts `Plays` and `Minutes` (`crates/web/src/charts.rs:76`);
   a session event counts one play and its duration
   (`crates/app/src/stats.rs:145-148`). `Amounts` has no wins or scores
   (`stats.rs:20-35`). A play without a duration adds 0 minutes (see OT-16). Home shows
   status counts per group (OT-17), which a play does not change visibly.
4. **The only reader of `winner`:** the win rate on People cards
   (`crates/app/src/people.rs:34-101`, `crates/web/src/people.rs:37-38`, `:235`), per other
   person. A solo play has no other person, so nothing shows there either.

So the hint's three suspects resolve to: feed source filter - not the cause (the row is
there); stats measure for games - plays are counted, wins are not a measure; group mapping
- not the cause. Unsure (cannot run): whether the owner's Stats page showed the play count
at all; if not, suspect the stats cache (WP4-1, migrations 0023/0029), which I did not
trace.

**Failing tests first.**

- Server integration, `src/rust/apps/server/tests/` (new file `plays_feed.rs`, or next to
  the activity tests): log a play with two players (31 and 42.5, second wins) and 45
  minutes; `GET /api/v1/activity` -> the row has `play.duration_minutes == 45`,
  `play.outcome == null`, and its participants carry `score` and `winner`; a solo play
  with `winner: true` for the viewer -> `participants` contains the viewer with
  `is_me: true, winner: true`; a co-op play -> `play.coop == "won"`.
- Rust unit, `crates/web/src/history.rs` tests (there is a fixture builder at `:852`):
  `a_play_row_says_who_won_and_the_score`: text "Play - you won, 42.5 points" for the
  viewer as winner; "Play with Ben - Ben won (42.5 to 31)"; co-op "Play with Ben - won
  together"; solo without score and winner "Play".
- Playwright, extend the play test at `src/web/tests/item.spec.ts:152-173`: after saving,
  the item's History tab contains "Ben won" and "42.5"; `/history` shows the same row.

**Fix.**

- `crates/contracts/src/browse.rs`: `ActivityPersonDto` gets `score: Option<f64>`,
  `winner: bool` (`#[serde(default)]`); `ActivityRowDto` gets
  `play: Option<ActivityPlayDto { duration_minutes, coop, location }>`.
- `crates/app/src/activity.rs` (+ `activity/person.rs`): the feed already reads the
  participants of every session row (DEC-494 text), so score and winner come with the same
  read; add duration and co-op outcome from the session. No new statement expected; check
  the span reads in `store-pg/src/repos/sessions.rs:219-281` (they select the columns
  already).
- `crates/api/src/browse.rs:412-441`: map the new fields.
- `crates/web/src/history.rs:52-56`, `widgets.rs:557`: a play sentence from the data
  (Fluent en/de: `history-play`, `history-play-won-you`, `history-play-won-name`,
  `history-play-coop-won|lost`, `history-play-minutes`); replace "Session logged" by
  "Play" for kinds with the sessions model (copy: F2).
- `crates/web/src/item.rs:817-841`: the item History tab should use the same rows as the
  feed (call the activity read filtered by item) instead of rebuilding rows without
  people; else it stays the one place without scores.
- Stats (only if F2 wants it): `Amounts.wins`/`decided` for the Play group needs the stats
  cache statement and its dirty triggers (migrations 0023/0029) - M on its own; the People
  win rate logic (`app/src/people.rs:94-101`) is the rule to reuse.

Risk: History filters and counts (`crates/web/src/history_state.rs:286-300`) do not look
at the new fields. The offline queue replays plays with the same DTO (new fields are
response-only). Search DEC-493, palette, keyboard map, bulk dates: untouched. Shared rows
(`shared: true`) would now show other people's scores to the linked viewer: that is the
same audience that already sees the row and its people; confirm with the owner.

**Size:** M (feed + item history), plus M for win stats. **F2 dependency:** partly: the
sentence wording and whether Stats gets wins/"hours played" (OT-17) are F2; carrying the
data in the DTO is not and can start now.

## OT-16 No playtime on a game play

**Cause (certain): the field exists but is hidden.** Storage and API are complete:

- column `sessions.duration_minutes integer` (`crates/store-pg/migrations/0001_core.sql:147`);
- API: `NewPlayDto.duration_minutes`, checked against `MAX_PLAY_MINUTES` and stored
  (`crates/api/src/ops.rs:481-489`);
- form: `PlayForm.duration`, parsed to minutes (`crates/web/src/log_form.rs:126`, `:239-252`,
  error key `log-error-duration` at `:40`).

In the sheet the input sits inside the collapsed disclosure "More" whose summary is
"Co-op, duration, place, notes" (`crates/web/src/log_sheet.rs:643-645`; the field at
`:757-759`; strings `log-more`, `log-more-play`, `log-duration = Duration in minutes` in
`crates/web/locales/en.ftl:240-242,257`). The first screen of the play sheet shows date,
groups, players, one score per player and the winner (`log_sheet.rs:639-642`). For a solo
video game that is exactly "only a score". The split was made for the density budget
(OD-01, 5 inputs) with board games in mind; `PlaySheet` serves board and video games alike
(`log_sheet.rs:572`), and the kinds do not differ in the sheet
(`crates/kinds/src/builtin/play.rs:27`, `:79`, `:124`: all `ProgressModel::Sessions`).

So this is a layout/model question, not a missing column. What **is** missing, as far as I
could see: nothing shows the duration afterwards either (see OT-11: the History row and the
item history do not print it; verify there).

**Failing test first.** Playwright, `src/web/tests/item.spec.ts` (or the spec that has the
play sheet; search for "Log a play"), name
`"a video game play takes a playtime without opening More"`:

- seed a `video_game`, open its page, open the play sheet;
- expect `page.getByLabel(/Playtime|Duration/)` to be visible while the "More" button has
  `aria-expanded="false"`;
- type `90`, save; expect `GET /api/v1/sessions` to hold `duration_minutes: 90` for the item;
- for a `board_game`: the first screen is unchanged if F2 keeps it there (assert the count
  of visible inputs <= 5, OD-01).

Rust unit: `log_form.rs` already covers parsing; add a case if hours:minutes input is
allowed ("1:30" -> 90).

**Fix, by F2 outcome.**

- **A (small): move duration out of "More" for kinds played alone.** `PlayFields` gets the
  kind (from `ItemInfo`); for `video_game` and `visual_novel` render the duration field
  before the scores and drop "duration" from the summary text of "More" (a second Fluent
  key `log-more-play-solo`). For a solo video game the score and winner fields are noise:
  hide the winner select when there is one player (`log_sheet.rs:731-736` shows it whenever
  not co-op). Stays inside OD-01: date, players, playtime, score = 4 inputs.
- **B (model): a video game is tracked by playtime, not by scored plays.** Then the sheet
  for `video_game` leads with playtime (hours and minutes) and "finished the game", and
  scores move behind "More". Needs the F2 decision on what a "play" of a video game is, and
  feeds OT-17 ("hours played"). Stats then need a minutes measure for the Play group (see
  OT-11).
- Either way: label "Playtime (minutes)" in en/de instead of "Duration in minutes"
  (copy, F2), and accept `h:mm`.

Files: `crates/web/src/log_sheet.rs`, `log_form.rs` (if `h:mm`), `locales/en.ftl`,
`locales/de.ftl`. Risk: the disclosure state is remembered per browser
(`disclosure_storage_key("item", "play-more")`, `log_sheet.rs:644`), unaffected. Keyboard:
Ctrl+Enter saves the form (`log_sheet.rs:634-638`), unaffected.

**Size:** S (A), M (B). **Depends on F2:** yes (which fields lead per kind).

## OT-2 No thumbnail in search results

**Cause (certain): the data arrives, the palette drops it.** A search hit is a full
`CatalogItemDto` with `cover: Option<ImageDto>` (`crates/contracts/src/lib.rs:78-95`,
`:55-66`), for local hits and for streamed provider hits alike (both go through
`dto::item`, `crates/api/src/ops.rs:185`, `:239`; `crates/api/src/dto.rs:88-96`).
The palette maps each hit to a `CommandItem`, which only has `id, label, group, keywords`
(`crates/ui/src/command.rs:11-19`); `to_item` copies title, year and original title and
ignores `cover` (`crates/web/src/palette.rs:275-286`). `CommandOption` renders the label
text only (`command.rs:213`).

Where the item page gets its cover: the same `ImageDto` through the `Cover` widget
(`crates/web/src/item.rs:286`, `crates/web/src/widgets.rs:68-124`): processed variants
under `/assets/...` when the image is cached, else the provider URL
(`crates/api/src/dto.rs:45-86`).

Two things to decide in the fix:

- **Fresh search hits usually have no cached asset yet.** A new item starts with
  `cover_asset: None` and the provider's `cover_url` (`crates/app/src/catalog.rs:304-305`;
  `crates/catalog-core/src/search.rs:101-105`); the asset is attached later by a job
  (`src/rust/apps/server/src/handlers.rs:77`), and search results are fetched ahead only in
  the `Eager` budget mode (`crates/assets/src/config.rs:236-242`; default `Library`). So a
  thumbnail in the palette is, for most rows, the provider URL loaded by the browser
  (TMDB/IGDB see the viewer's IP). The item page already does this for an uncached cover
  (`widgets.rs:110-117`, `referrerpolicy="no-referrer"`), so it is not a new exposure, but
  20 rows per keystroke pause is more of it. Options: (a) accept, as on the item page;
  (b) show thumbnails only for cached covers and the kind icon otherwise; (c) a small
  proxied thumbnail endpoint. Default proposal: (b) + (a) behind the existing asset
  setting. Owner decision; the round-1 `security-audit.md` may have a view (not re-read).
- **Size of the picture.** `ImageDto.url` is the Card variant (`dto.rs:65-68`); a 32-40 px
  thumbnail should use the smallest variant via `srcset`/`sizes` (the `Cover` widget takes
  `sizes`, `widgets.rs:73-75`).

**Failing tests first.**

- wasm, `crates/ui/tests/wasm.rs`: `command_option_renders_its_leading_slot`: a
  `CommandItem` with `leading` set renders it inside the option before the label, and the
  option's accessible text is still the label only (the image is decorative, `alt=""`).
- Rust unit, `crates/web/src/palette.rs` tests: `to_item_keeps_the_cover`: a DTO with a
  cover maps to an item whose thumbnail url is the DTO's; without a cover, `None`.
- Playwright, `src/web/tests/shell.spec.ts` in the palette test (`:273` on): the "Fight
  Club (1999)" option contains an `img` (or the kind icon when policy (b) and uncached);
  rows keep one height (no layout shift: compare the option's box before and after the
  image loads).

**Fix.**

- `crates/ui/src/command.rs`: `ui` has no workspace deps (KICKOFF 4, architecture test), so
  it cannot take `ImageDto`. Add a plain field to `CommandItem`, for example
  `thumb: Option<CommandThumb { url: String, srcset: String, square: bool }>` (keeps
  `PartialEq, Eq`, which `Memo<Vec<CommandItem>>` needs, `command.rs:10`, `:111`), and
  render a fixed-size box (for example `h-10 w-7`, square for music and board games) with
  `loading="lazy" decoding="async" referrerpolicy="no-referrer" alt=""`; with `None`, a
  muted box with the kind icon so all rows align.
- `crates/web/src/palette.rs:275-286`: fill it from `i.cover`; recent items
  (`crates/web/src/recent.rs`) have no cover stored: either add the url there or show the
  icon.
- Row height: keep at least 44 px on phones (`command.rs:207`); the thumbnail makes rows
  taller on desktop, so fewer fit in `max-h-80` - together with the missing scroll-into-view
  (OT-1) fix that first.

Risk: OT-1's keyboard behaviour is unaffected (no new tab stops: the image is not
focusable). DEC-426: unchanged. Performance: `dto::item` does one asset lookup per hit
("cover lookups batched in `dto::item`" is an open performance item, HANDOFF section 0,
step 3); the palette already pays for it today, showing the image adds no server work.

**Size:** S/M. **F2 dependency:** none for the mechanism; the hotlink policy (a/b/c) is an
owner decision.

## Summary table

| ID | Cause in one line | First failing test | Size | Needs F2 |
|---|---|---|---|---|
| OT-8 | whitespace-only text is dropped inside the `inline-flex` tab button (`ui/src/tabs.rs:110`, `web/src/item.rs:745`) | Playwright gap between label and count, `item.spec.ts` | S | no |
| OT-1 | combobox logic is there; active row is invisible (about 1.1:1), never scrolled into view, reset to row 1 by every result batch; no test presses arrows in the palette | Playwright arrows + visibility in `shell.spec.ts`; wasm "active survives appended items" | M | no |
| OT-3 | no popularity or exact-match signal; ties decided by engine order or body length; stale local copies rank above the provider's order | `search/tests/suite.rs` tie-break block on both backends | S + S/M + M | no |
| OT-15 | bulk marks take unwatched episodes only; an empty list returns silently (`episode_list.rs:92-94`) | unit `episode_plan.rs` + Playwright `item.spec.ts` | S now, S-M (A) or L (B) | yes |
| OT-11 | score, winner, outcome are stored but no DTO or view reads them; rows say "Session logged" | server integration on `/activity` + unit on the row sentence | M (+M win stats) | partly |
| OT-16 | the duration field exists but sits in the collapsed "More" (`log_sheet.rs:643-645`, `:757-759`) | Playwright: playtime visible without opening More | S (A) or M (B) | yes |
| OT-2 | hits carry `cover`; `CommandItem` has no field for it and `to_item` drops it | wasm leading slot + unit `to_item_keeps_the_cover` | S/M | no (owner: hotlink policy) |

Suggested order for the fix pack "ot1": OT-8, OT-1 (with the `filter` prop, which OT-3
step 1 needs), OT-15 minimum fix, OT-11 DTO part, OT-3 steps 1-2, OT-2; then the F2-bound
parts (OT-15 A/B, OT-16, OT-11 wording and stats, OT-3 step 3).

## Could not verify

- Nothing was run. OT-1 is a ranked list of candidates; the Playwright test in that
  section decides. In particular I could not confirm whether any browser sends `mousemove`
  to a row that moved under a resting pointer.
- Contrast figures in OT-1 are my own calculation from the Tailwind zinc values, not a
  measurement.
- OT-3: which mechanism the owner hit (TMDB's own order on a first search, or a local copy
  ranked first) needs the group heading of the row or a reproduction. Meilisearch's default
  ranking rules and the popularity field names of TMDB, IGDB and Open Library were not
  re-read today.
- OT-11: whether the Stats page counted the play at all on the owner's instance (stats
  cache not traced).
- OT-15: server behaviour for two events of the same episode was not traced beyond the
  existing "Again" path.
- OT-8: how Leptos 0.8.23 emits a whitespace-only literal was not checked; the CSS rule
  makes it irrelevant. The flexbox sentence is cited from memory.
- The DEC that introduced each behaviour was only looked up for OT-8 and OT-15 (commit
  add6203); for the others the code comments name the design sections, not a DEC.
