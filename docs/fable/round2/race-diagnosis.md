# Race diagnosis: two browser tests that fail only under load

Date: 2026-11-13. Read-only diagnosis: no code was changed, nothing was built or run (a gate
was running in the checkout). Everything below comes from reading the app code, the crate
sources in `~/.cargo/registry/src/index.crates.io-*/` and the log of the failed run
(`.../scratchpad/e2e-site-run1.log`, called "run log" below).

Paths: the web crate is `src/rust/crates/web/src/` (short: `web/`), the UI crate
`src/rust/crates/ui/src/` (`ui/`), the specs `src/web/tests/`. The browser log prints the web
crate as `crates/web/src/`. Framework crates are cited as `leptos-0.8.23/...`,
`tachys-0.2.19/...`, `reactive_graph-0.2.16/...`, `leptos_server-0.8.8/...`.

What is proven from the sources and the log is marked PROVEN; what is the best explanation
but was not reproduced is marked DERIVED.

## 1. Short answer

| # | Cause | Seen as | Confidence |
|---|-------|---------|------------|
| C1 | The server sent the Library header without the status tabs, the browser rendered them: hydration panic. The three header memos (`chips`, `all_count`, `tabs`) cache a resource read on the multi-threaded server and can stay stale | `library.spec.ts:427` | symptom chain PROVEN, the stale memo DERIVED |
| C2 | The marker `body[data-hydrated]` is set although hydration panicked, and a dead page does not recover | the 180 s timeout instead of a clear failure after 20 s | PROVEN |
| C3 | `settings::loaded` renders every answer of a refetch afresh, also an equal one; menus inside it keep their open state inside the rebuilt view | `people.spec.ts:91` | PROVEN |
| C4 | A page's own refresh does not tell its `Pace`: the event that announces the own action refetches a second time at once | makes C3 hit right after every own action | PROVEN |
| C5 | The Library header sits in `<Suspense>`, not `<Transition>`: every refetch of `counts`/`tags` swaps chips, tabs and count for their skeleton until the answer is there | not one of the two failures; same family, found on the way | DERIVED from the framework source |

The warnings named in the task are not the cause of either failure:

- "reading a resource in hydrate mode outside a Suspense" at `library.rs:161`,
  `history.rs:432`, `item.rs:151`: all three are reads after hydration (a dialog that opens,
  a callback run from a timer). See 6.1.
- "Non-fatal error at tachys-0.2.19/src/renderer/dom.rs:131:9 ... insertNode": a view was
  updated while its nodes were detached; the state is kept and mounted later. See 6.2.

## 2. Failure 1: `library.spec.ts:427` (the bulk bar at 360 px)

### 2.1 What happened (PROVEN)

The step that failed is `library.spec.ts:451-452`: `open(page, "/library?view=list&sort=title")`
then `getByRole("checkbox", { name: "Select all on this page" }).check()`. The run log shows
for that page load (run log lines 578-605):

1. `A hydration error occurred while trying to hydrate an element defined at
   crates/web/src/library.rs:454:18. The framework expected an HTML <div> element, but found
   this instead: [object Comment]`
2. `panicked at tachys-0.2.19/src/hydration.rs:184:9: Unrecoverable hydration error`
   (the panic is `tachys-0.2.19/src/hydration.rs:170-187`, `failed_to_cast_element`)
3. `Uncaught (in promise) RuntimeError: unreachable`
4. Playwright: `locator resolved to <input disabled type="checkbox" id="lib-select-all" ...>`,
   330 times "element is not enabled".

`library.rs:454:18` is the `<div class="-mx-4 mb-3 overflow-x-auto px-4">` of `StatusTabs`
(`web/library.rs:453-457`). So at the place of the status tabs the server's HTML had a
comment node, the placeholder the renderer writes for "nothing" (`None`), while the browser
had `tabs = Some(..)` and wanted the `<div>`.

Why the checkbox stayed disabled: hydration walks the page top down and stopped at the status
tabs. `Results`, the table and its select-all box come later and were never hydrated. The box
is rendered `disabled` until an effect of `ListView` runs (`web/library.rs:1062-1066`,
`1150-1152`, DEC-485); that effect never existed. The page was dead from the status tabs
down, for good.

What it was not:

- Not a refetch. `refresh_on_updates` (`web/library.rs:158`) and every other refetch run in
  effects, and effects run after the synchronous hydration pass. The panic is inside that
  pass.
- Not a failed server function. `KindChips` (rendered just before, `web/library.rs:185-187`)
  reads the same two resources (`kinds`, `counts`) and hydrated: an error or a missing value
  would have stopped hydration at its `<ul>` (`web/library.rs:393`), not at 454.
- Not the browser starting too early. The hydration script is an inline
  `<script type="module">` (`leptos-0.8.23/src/hydration/mod.rs:205`): it runs after the
  document is parsed, that is after the last streamed chunk, so the browser always holds the
  final value of every resource (`leptos_server-0.8.8/src/resource.rs:303-304`,
  `hydration_context-0.3.2/src/hydrate.rs:133-138`). The browser side is deterministic; the
  HTML is the part that varied.

So: in one and the same response the server serialized `counts = Ok(..)`, rendered the kind
chips from it, and rendered the status tabs as `None`. The only thing between the resources
and the status tabs that can differ per place is the memo `tabs`.

### 2.2 Cause C1: a memo between a resource and the view, on the multi-threaded server (DERIVED)

The code (`web/library.rs`):

- `:55-61` `kinds_now()` / `counts_now()` are `self.kinds.get().and_then(Result::ok)`.
- `:349-352` `chips`, `:353-360` `all_count`, `:437-440` `tabs` are
  `Memo::new(move |_| { let (k, c) = (lib.kinds_now()?, lib.counts_now()?); ... })`.
- `:361`, `:441` the views are `move || memo.get().map(..)` inside the `<Suspense>` of
  `:185-190`.

How the server decides that a Suspense is ready
(`leptos-0.8.23/src/suspense_component.rs:347-432`): it calls the children once
(`dry_resolve`, `:348`) so that resource reads register a task with the Suspense
(`reactive_graph-0.2.16/src/computed/async_derived/arc_async_derived.rs:640-670`), waits
until the task set is empty, calls the children once more to find reads that only appear
after the first ones resolved (`:399-411`), and then renders the HTML by calling the
children a last time. A resource read that happens directly in the view is repeated on each
of these calls. A read hidden in a memo is repeated only when the memo is dirty.

The memo can stay clean with a stale value when the resource resolves on another worker
thread while the memo computes. `reactive_graph-0.2.16/src/computed/inner.rs:106-189`:

- `:126` the memo sees "dirty", `:141-145` runs the closure with no lock held (the closure
  subscribes to the resource and reads `None`),
- `:160` sets `state = Clean`, unconditionally.

`arc_async_derived.rs:418-457` on the other thread: `:418` writes the value, `:432`
`loading = false`, `:450-453` `mark_dirty()` on the subscribers, `:456-458` wakes the tasks
that release the Suspense. If `:418-453` falls between the memo's read and its `:160`, the
"dirty" is overwritten by "clean": the memo holds `None`, nothing will mark it again (the
resource has resolved), the Suspense's tasks drain, the second pass reads the clean memo
(no resource read, no new task), and the HTML is rendered from `None`: a comment. The
serialized resource says `Ok`, the browser renders the tabs: the mismatch of 2.1.

The server runs on a multi-thread runtime (`src/rust/apps/server/src/main.rs:92`); resource
futures are spawned onto it, the page render runs on the request's task. The window is a few
instructions wide, which is why it needs load (a thread preempted exactly there) and why
only one of the three memos was hit in the failed run. The same window exists for `kinds`
on the very first pass (the memo then never reads `counts` at all, because of the `?`).

This is the third defect of one family in this code base; the first two were fixed in the
app, by not letting the server read a resource through a second, cached path:

- DEC-398, `web/library.rs:765-783` (`latest_page`): the memo over `rows` exists only under
  `cfg(feature = "hydrate")`; the server uses the first page.
- DEC-456, `web/library.rs:798-814`: `set_pending` only in the browser.
- The same shape in `web/home.rs:111-118` and `web/history.rs:768-775`.

`chips`, `all_count` and `tabs` are the only memos left that read a resource and are
evaluated on the server (checked: all 59 `Memo::new` in `web/`; the others read signals, or
are behind `cfg(feature = "hydrate")`, or read a resource only in the browser).

Honest limit: the stale memo was not reproduced; the interleaving cannot be forced from
outside the framework. It is the one explanation that fits all facts of 2.1 (same response,
same resources, chips right, tabs wrong, server side, only under load). A stress test can
show it (2.5 T1b).

### 2.3 Cause C2: the hydration marker lies, and the page stays dead (PROVEN)

`hydrated(page)` (`src/web/tests/helpers.ts:13-15`) waits for `body[data-hydrated]`. The
marker is set by an effect of `App` (`web/app.rs:186-195`). Effects are queued as
microtasks when they are created; `App`'s effect is created before the panic. A wasm panic
unwinds the call `mod.hydrate()` as a JS exception ("Uncaught (in promise) RuntimeError")
but does not stop the instance: the queued effect still runs and sets the marker. In the
failed run `open()` at `library.spec.ts:451` therefore passed, and the test waited 180 s on
a checkbox that could never be enabled.

For a person the same event is worse than for the test: a Library page that looks complete
and does not react below the kind chips, until they reload.

### 2.4 Fix

F1 (C1), `web/library.rs`, about 40 lines. No memo over a resource on the server, and both
resources read on every call:

```rust
/// Kinds and counts for the header rows. Both are read on every call, never one behind the
/// other's `?`: each read registers with the Suspense on the first pass.
fn header(lib: Lib) -> Option<(KindsDto, LibraryCountsDto)> {
    let (k, c) = (lib.kinds.get(), lib.counts.get());
    Some((k?.ok()?, c?.ok()?))
}

/// A memo in the browser; on the server a derived signal that is computed on every read.
/// The server renders on several threads, and a memo that caches a resource read there can
/// miss the resource's answer (DEC-398, DEC-456, DEC-new).
fn derived<T: Clone + PartialEq + Send + Sync + 'static>(
    f: impl Fn() -> T + Send + Sync + 'static,
) -> Signal<T> {
    #[cfg(feature = "hydrate")]
    return Memo::new(move |_| f()).into();
    #[cfg(not(feature = "hydrate"))]
    Signal::derive(f)
}
```

`chips`, `all_count` and `tabs` become `derived(move || header(lib).map(|(k, c)| ..))`.
The browser keeps its memos (single thread, no window), so nothing re-renders more often
than today. `derived` belongs next to `latest_page` or in `web/pages.rs`; `latest_page`,
`home.rs:111-118` and `history.rs:768-775` can use it later, they are not wrong today.
Add the rule to the handoff: "on the server no `Memo` reads a `Resource`".

F2 (C2), `web/lib.rs:94-106`, `web/app.rs:186-195`, `src/web/tests/fixtures.ts`, about 40
lines:

- Set `data-hydrated` in `hydrate()` after `leptos::mount::hydrate_body(..)` has returned,
  not in an effect. A panic inside hydration then never sets it, and `hydrated()` fails
  after 20 s with the page's real state.
- In the panic hook (`console_error_panic_hook::set_once()` at `web/lib.rs:95` becomes an
  own hook that calls it): while a flag "hydrating" is set, mark
  `body[data-hydration-failed]` and reload the page once (a `sessionStorage` key against a
  loop; cleared after a hydration that completed). A person gets a working page after one
  reload instead of a dead one.
- `fixtures.ts` already collects console messages per context for the security policy
  (`fixtures.ts:30-37`). Collect two more and fail the test on them, whatever the test was
  about: `/hydration error occurred|Unrecoverable hydration error/` and `/panicked at/`.
  With the reload of the second bullet a test would otherwise pass over a real C1; with this
  guard it cannot. This is not a workaround in a test: it makes an app defect fail every
  test whose page shows it.

### 2.5 Deterministic tests

T1a (C2 and the browser half of C1; red today, no load needed), `library.spec.ts`: give the
browser the answer of the failed run. Route the document, fetch it, and replace the status
tabs by the placeholder the server writes for "nothing":

```ts
await page.route("**/library?view=list*", async (route) => {
  const res = await route.fetch();
  const html = (await res.text()).replace(
    /<div class="-mx-4 mb-3 overflow-x-auto px-4"><div[^>]*id="lib-status"[\s\S]*?<\/div><\/div>/, "<!>");
  await route.fulfill({ response: res, body: html });
}, { times: 1 });
```

(the exact pattern is to be taken from a saved response). Today: the console shows
"Unrecoverable hydration error", `body[data-hydrated]` is attached all the same, and
`#lib-select-all` stays disabled. After F2: the page reloads once (the route answers only
once), `#lib-select-all` becomes enabled, and the fixture guard reports the hydration error
(the test expects exactly that one report). A second, smaller test: with the reload switched
off by the `sessionStorage` key, `body[data-hydrated]` is never attached and
`body[data-hydration-failed]` is.

T1b (C1, server half; a detector, not deterministic),
`src/rust/apps/server/tests/it/library_page.rs`: the test at `:134-175` already asserts the
invariant once ("Planned 4" in the HTML, multi-thread runtime, 2 workers). Add a loop: 4
workers, 300 `GET /library?view=list` in batches of 16 concurrent calls, each body must
contain `id="lib-status"`, the kind chips' `<ul>` and the serialized counts. Before F1 it
should turn red now and then (how often is unknown; run it with the mutation "F1 reverted"
a few times under `stress -c 4` to learn the rate); after F1 never.

T1c (C1 by construction; deterministic): a unit test in `web/` that reads the sources and
fails on a `Memo::new` whose closure names `kinds_now`, `counts_now` or a `Res<..>` field
outside `cfg(feature = "hydrate")`. Crude, but it keeps the rule of F1 from being broken by
the next header row.

## 3. Failure 2: `people.spec.ts:91` (add, edit, delete a person)

### 3.1 What happens (PROVEN from the code)

`people.spec.ts:107-111`: `addPerson(page, "Zed")`, click "More actions for Zed", click the
menu item "Edit".

1. `People` renders its data with `settings::loaded` (`web/people.rs:63-66`).
   `loaded` (`web/settings.rs:153-184`) is `<Transition><For each=move ||
   data.get().map(|r| (fresh(renders), r)) key=|(k, _)| *k ..>`: `fresh`
   (`web/pages.rs:62-69`) gives every answer a new key, so every answer of a refetch removes
   the whole `PeopleBody` and builds a new one: every `PersonCard` and its `DropdownMenu`
   (`web/people.rs:346-396`, the menu at `:380`). This also happens when the answer is equal
   to the one on screen.
2. The menu's open state is a signal made inside `DropdownMenu`
   (`ui/dropdown.rs:109`, `open.unwrap_or_else(|| RwSignal::new(false))`); the panel is
   `<Show when=move || open.get()>` (`ui/dropdown.rs:164`). A rebuilt card has a closed
   menu. The component has the prop for exactly this case (`ui/dropdown.rs:104-107`: "a menu
   inside a view that a background refresh rebuilds stays open when the caller keeps this
   signal outside that view"); `PersonCard` does not pass it.
3. Two refetches follow "Add person":
   - the page's own: `Ui::report` (`web/people_dialogs.rs:126-129`) runs `refresh`, which is
     `data.refetch()` (`web/people.rs:50`);
   - the event of the same action: creating a person publishes `entity.updated`
     (`src/rust/crates/app/src/companions.rs`), `refetch_on_update` (`web/people.rs:25-27`,
     `:49`) hands it to `events::refresh_on_updates` (`web/events.rs:248-263`), whose `Pace`
     has not been told about the own refetch (`Pace::own`, `web/events.rs:171-173`, is only
     called for `OwnAction`, `:250-260`), so `Pace::event` answers `Refetch::Now`
     (`web/events.rs:151-159`). This is C4.
   More `entity.updated` events arrive from the jobs the test's seed started (import,
   `catalog.refresh`, `assets.fetch`; the server log of the failed run shows them running
   for the whole run), each paced to one refetch per 2 s.
4. `addPerson` (`src/web/tests/helpers.ts:136-143`) returns when Zed's card is visible, that
   is after the first answer. Without load the second answer follows within milliseconds,
   before Playwright has clicked the menu button. Under load it arrives after the click:
   the menu opens, the answer rebuilds the cards, the new menu is closed, and
   `getByRole("menuitem", { name: "Edit" })` waits for an item that will not come back
   (Playwright does not click the button again): the 120 s timeout lane C reported.

For a person: open a card's menu within a moment of any change (their own, or one from
another tab, an import, a job), and it closes under the pointer. Keyboard users lose the
focus as well: it was on a node that is gone.

The Library has the cure and its test: the card shells are keyed and outlive answers
(`web/widgets.rs:196-223`, `keyed`), the quick menu's `open` lives in the shell
(`web/widgets.rs:332`, `:502`, `web/library.rs:1239`), and
`library.spec.ts:233-262` ("a background refresh keeps an open quick menu") guards it.
The item page's dialogs were moved out of the refetched view by DEC-569. People, the lists
and the settings pages still render afresh.

### 3.2 Fix

F3 (C3), `web/settings.rs:153-184`, about 25 lines: an equal answer keeps the render.
`loaded` remembers the last `Ok` value and its key; `each` returns the old key when the new
value is equal, a fresh one otherwise:

```rust
let last = StoredValue::new(None::<(u32, T)>);
// each:
data.get().map(|r| {
    let key = match (&r, last.get_value()) {
        (Ok(v), Some((k, old))) if *v == old => k,
        _ => fresh(renders),
    };
    if let Ok(v) = &r { last.set_value(Some((key, v.clone()))); }
    (key, r)
})
```

This needs `T: PartialEq` at the 27 call sites of `loaded`; most DTOs in `contracts` derive
it (the build will name any that does not).
It takes away every rebuild for the echo of an own action and for events about other
things, which is most of them.

F4 (C3), about 30 lines: answers that do differ must not close a menu. Give each menu
inside a refetched view an `open` signal that outlives the render, with the helper made for
it, `settings::kept` (`web/settings.rs:130-149`):
`let open = kept(&format!("pp-menu-{}", p.id), || false);` and `open=open` on the
`DropdownMenu`. The effect at `ui/dropdown.rs:118-124` then focuses the menu's item again
in the new card. Places: section 5.

F5 (C4), `web/events.rs:248-263` and the pages, about 40 lines: `refresh_on_updates`
returns the callback for the page's own refreshes: it calls `Pace::own(now)` and then
`refresh`. `People` passes that to `Ui::new` instead of the bare `data.refetch()`
(`web/people.rs:49-50`); the same for `lib.refresh` (`web/library.rs:141-145`, `:158`),
Home (`web/home.rs:69`), History (`web/history.rs:431-439`), the person page
(`web/people.rs:416`) and the two list pages (`web/lists.rs:55`, `:217`). The event of an
own action is then answered once at the end of the quiet time (DEC-614 wanted exactly this
and reached it only for the palette's actions), and with F3 that answer changes nothing on
screen.

Order: F4 is the fix for the failure, F3 and F5 take away most of the occasions.

### 3.3 Deterministic tests (no load, red today)

T2a (C3, the failure itself), `people.spec.ts`, modelled on `library.spec.ts:233-262`:

```ts
await page.goto("/people"); await hydrated(page);
await page.getByRole("button", { name: "More actions for Zed" }).click();
const menu = page.getByRole("menu", { name: "More actions for Zed" });
await expect(menu).toBeVisible();
// a change from elsewhere: the answer differs, the cards are rendered again
const refetched = page.waitForResponse((r) => r.url().includes("/_fn/people_page"));
expect((await page.request.post("/api/v1/companions", { data: { name: "Elsewhere" } })).status()).toBe(201);
await refetched;
await expect(card(page, "Elsewhere")).toBeVisible();
await expect(menu).toBeVisible();                       // red today
await menu.getByRole("menuitem", { name: "Edit" }).click({ timeout: 5_000 });
await expect(page.getByRole("dialog", { name: "Edit Zed" })).toBeVisible();
```

The same block for the person page's menu (`pp-page-menu`) and a game group's menu.

T2b (C3, an equal answer): mark Zed's card (`el.dataset.same = "1"` through
`page.evaluate`), change something that does not concern the page (a tag on a library
entry through the API, it publishes `entity.updated`), wait for the `/_fn/people_page`
response, expect the mark to be still there. Red today (the card is a new node), green
with F3.

T2c (C4): count the `/_fn/people_page` requests with `page.on("request")`. Add a person
through the dialog; expect one request within the first second and one more after the
quiet time (2 s), not two at once. Red today. The same count for Library after a status
change from a card's quick menu (`/_fn/library_query`).

T2d (unit, `web/settings.rs`): the key rule of F3 as a pure function (equal value: same
key; other value or error: new key).

## 4. Cause C5: the Library header flips to its skeletons on every refetch (DERIVED)

Not one of the two failures, found while reading; same family (a view detached by a
refetch), and it is the likeliest source of part of the `dom.rs:131` warnings.

`<Suspense>` shows its fallback whenever one of the resources read under it is loading:
`show_b = !none_pending.get() && (!TRANSITION || nth_run < 2)`
(`leptos-0.8.23/src/suspense_component.rs:193`, `:588`), and a resource that loads again
registers a task with every Suspense that read it
(`arc_async_derived.rs:368-377`). `<Transition>` does that only for the first load, which
is why `Results` and `loaded` use it. The Library header uses `<Suspense>` five times:

- `web/library.rs:174-178` the count next to the title (`counts`),
- `:185-187` `KindChips` (`kinds`, `counts`), fallback a skeleton,
- `:188-190` `StatusTabs` (`kinds`, `counts`), fallback a skeleton,
- `:495-497` `FacetFilters`, `:612` `TagFilter` (`tags`), fallback a skeleton.

`refresh` refetches `counts` and `tags` (`web/library.rs:141-145`). It runs after every own
action on a card, after every paced event and on reconnect. Each time the kind chips and the
status tabs are taken out of the document (kept alive, `tachys-0.2.19/src/view/either.rs:497-515`),
their skeletons are shown, and the chips and tabs are put back when the answer is there.
On a fast machine that is a flicker of a few milliseconds; under load it is as long as the
request. A click on a chip or tab in that time finds nothing, and the keyboard focus on a
chip or tab is lost with every refresh (the node is detached, focus falls to the body).

The same on History: `web/history.rs:484-491` reads only `kinds` (never refetched: fine);
`:494-496` `FilterFields` and `:908` (`Side`) need a look at which resources they read.
Home: `web/home.rs:614`, `:621`, `:633`, `:662` (Stats). Stats extras:
`web/stats_extras.rs:247`, `:274`.

Fix F6, about 10 lines: `Transition` instead of `Suspense` at `web/library.rs:174`, `:185`,
`:188`, `:495`, `:612` (and wherever the look at History, Home and Stats finds a resource
that is refetched). The first load still shows the skeleton; a refetch keeps the rows until
the answer is there.

Test T3 (deterministic, with the hold of DEC-569's test, `item.spec.ts:377`):

```ts
await open(page, "/library");
let release = () => {};
await page.route("**/_fn/library_counts*", async (route) => {
  await new Promise<void>((r) => (release = r));
  await route.continue();
});
await page.locator("#lib-status").evaluate((el) => ((el as HTMLElement).dataset.same = "1"));
await page.getByRole("tab", { name: /^Planned/ }).focus();
// a change from elsewhere starts the refetch; its counts are held
expect((await page.request.put(`/api/v1/library/${id}/status`, { data: { status: "completed" } })).ok()).toBeTruthy();
await page.waitForRequest((r) => r.url().includes("/_fn/library_counts"));
await expect(page.locator('#lib-status[data-same="1"]')).toBeVisible();   // expected red today
await expect(page.getByRole("tab", { name: /^Planned/ })).toBeFocused();
release();
```

If T3 is green on today's code, C5 is wrong and F6 is dropped; the test is cheap either way
and belongs before the fix.

## 5. Every place with the same pattern

Memo over a resource, evaluated on the server (C1): `web/library.rs:349`, `:353`, `:437`.
No other (the others are browser-only by `cfg`, or read signals).

Menus inside a view that a refetch renders afresh, without a kept `open` (C3, fix F4):

| Place | Rendered by | Refetched by |
|-------|-------------|--------------|
| `web/people.rs:380` person card menu | `loaded`, `people.rs:63` | own actions, `entity.updated` (`:49`) |
| `web/people.rs:463` person page menu | `loaded`, `people.rs:425` | own actions, `entity.updated` (`:416`) |
| `web/game_groups.rs:147` group card menu | inside `PeopleBody` (`people.rs:107-109`) | as People |
| `web/integrations.rs:207` integration menu | `loaded`, `integrations.rs:128` | own actions, `integration.call`, `stream.lagged` (`:112`) |
| `web/admin/users.rs:506` user row menu | `loaded`, `admin/users.rs:104`, `:275` | own actions |
| `web/item.rs:604` item actions, `:940` lists menu | `For` keyed by `version` (`item.rs:159`, `:189-192`) | every refetch of the item page: own actions, `entity.updated` for the item, for example the cover job that ends just after an item was added |

Already safe: `web/widgets.rs:332`/`:502` and `web/library.rs:1239` (quick menu, `open` in
the keyed shell), `web/episode_list.rs:846-847` (`open=open`), `web/bulk.rs:347` (outside
the rows).

Everything else inside a `loaded` render loses what is not in `kept` when an answer
differs: focus, text typed into an inline field, an open disclosure. The 27 call sites of
`loaded`: `account.rs:71`, `:78`; `imports.rs:109`; `integrations.rs:128`; `lists.rs:71`,
`:236`; `notifications.rs:291`; `people.rs:63`, `:425`, `:497`; `settings.rs:462`;
`setup.rs:297`; `admin/audit.rs:96`; `admin/instance.rs:57`, `:67`; `admin/mod.rs:221`;
`admin/overview.rs:133`, `:177`; `admin/security.rs:45`, `:60`, `:98`;
`admin/sources.rs:51`; `admin/sso.rs:51`, `:59`; `admin/storage.rs:39`;
`admin/users.rs:104`, `:275`. F3 covers all of them for equal answers. The pages that
refetch for events from elsewhere are the ones that matter: People, the person page, the
two list pages (`lists.rs:55`, `:217`), Notifications (`notifications.rs:280`), Imports
(`imports.rs:92`), Integrations (`integrations.rs:112`). Keyed shells for the People cards
and the list rows, as the Library has them, would be the complete cure; that is a larger
change and not needed for the failure.

Own refreshes that bypass the page's `Pace` (C4): `people.rs:50`, `:416`;
`library.rs:141-145`; `home.rs:69`; `history.rs:431-439`; `lists.rs:55`, `:217`.

`<Suspense>` over a resource that is refetched (C5): `library.rs:174`, `:185`, `:188`,
`:495`, `:612`; to check: `history.rs:494`, `:908`; `home.rs:614`, `:621`, `:633`, `:662`;
`stats_extras.rs:247`, `:274`; `item.rs:897`; `bulk.rs:177`.

## 6. The warnings in the run log

### 6.1 "reading a resource in hydrate mode outside a Suspense"

The message comes from `leptos_server-0.8.8/src/resource.rs:232-256`. It is printed in
debug builds only, for any read that is neither under a Suspense nor in an effect, whenever
it happens; it does not know whether hydration is still going on. What a read outside a
Suspense really does: on the server it sees `None` (nothing waits for it), in the browser
it sees the value, so if it decides what is rendered during hydration the two differ. None
of the three places does that:

- `web/library.rs:161` and `web/item.rs:151`: `tags.get()` / `known.get()` inside the
  derived signal `known` / `known_tags`. It is read by `TagEditorBody`, which is rendered
  only when a row was picked (`web/widgets.rs:580-588`, `target.get().map(..)`): after
  hydration, from a click.
- `web/history.rs:432`: `first.get_untracked()` in the `refresh` callback, when it runs from
  the pace's timer (`web/events.rs:205-214`) rather than from its effect.
- Not in the log, same kind: `web/item.rs:232` (`data.with_untracked` in a key handler),
  `web/library.rs:66` (`kinds.get_untracked()` for the quick menu).

Harmless, but they bury a real warning of this kind in noise. Cure (optional, about 15
lines): the tag suggestions from a signal that an effect fills from the resource; in
`history.rs:432` read the rows from `more`/a mirror signal.

The six warnings "you access a ... ArcMemo ... outside a reactive tracking context" at
`web/library.rs:471:36` (run log lines 497-562) belong to the two axe tests before the
failed one; `active()` is called somewhere outside a tracked closure when the Filters panel
opens. Not followed up here.

### 6.2 "Non-fatal error at tachys-0.2.19/src/renderer/dom.rs:131:9 ... insertNode"

`tachys-0.2.19/src/renderer/dom.rs:125-136`: `insert_node` is
`parent.insert_before(new_child, anchor)`; the macro at `:131` logs the `DOMException`
and goes on. `insertBefore` throws when the anchor is not a child of the parent. The
renderer gets there when a view is updated while its nodes are out of the document but its
state still remembers the old parent: a keyed list keeps `parent` from the time it was
mounted (`tachys-0.2.19/src/view/keyed.rs:149-176`) and inserts new rows before its own
marker, which is detached. The new row is kept in the list's state and mounted with the
rest when the list is mounted again (`keyed.rs:464-473`), so nothing is lost; hence
"non-fatal".

Who detaches a live view: a `<Suspense>` whose resource loads again
(`either.rs:497-515`, C5), and every other keep-alive switch. The 18 warnings of the run
come in three bursts (07:34:27, 07:36:00, 07:40:39; parents `HTMLElement`, that is a
`main`/`section`, and `HTMLDivElement`); the log does not say which page. They are not part
of failure 1 (the last burst is a minute before the panic, in another test). With the
console guard of F2 extended to report these with the page URL (report first, fail later)
the next run names the pages; F6 should remove the ones that come from C5.

## 7. Proposed fix package

One package, "races", in this order (each with its test first, red, then the fix):

| Step | Fix | Files | Size |
|------|-----|-------|------|
| 1 | F2: honest hydration marker, one reload after a hydration panic, console guard in the fixture; T1a | `web/lib.rs`, `web/app.rs`, `tests/fixtures.ts`, `tests/helpers.ts`, `tests/library.spec.ts` | about 40 lines app, 50 lines tests |
| 2 | F1: no memo over a resource on the server; T1b, T1c | `web/library.rs`, `web/pages.rs`, `apps/server/tests/it/library_page.rs` | about 40 lines app, 60 lines tests |
| 3 | F4: kept `open` for the 7 menus; T2a | `web/people.rs`, `web/game_groups.rs`, `web/integrations.rs`, `web/admin/users.rs`, `web/item.rs`, `tests/people.spec.ts` | about 30 lines app, 60 lines tests |
| 4 | F3: `loaded` keeps the render for an equal answer; T2b, T2d | `web/settings.rs` (and `PartialEq` where the build asks for it) | about 30 lines app, 40 lines tests |
| 5 | F5: own refreshes tell the pace; T2c | `web/events.rs`, `web/people.rs`, `web/library.rs`, `web/home.rs`, `web/history.rs`, `web/lists.rs` | about 40 lines app, 30 lines tests |
| 6 | T3 first; if red, F6: `Transition` for the Library header (and the places of section 5 that read a refetched resource) | `web/library.rs`, maybe `web/history.rs`, `web/home.rs`, `web/stats_extras.rs` | about 10-20 lines app, 30 lines tests |

Together about 200 lines of app code and 270 lines of tests: one lane, about a day with the
gate. Steps 1-3 are the two failures; 4-6 take away the occasions and the flicker and can
be a second commit. Decisions to log: the rule "on the server no memo reads a resource"
(with DEC-398 and DEC-456 as its earlier cases), the reload after a hydration panic, and
"`loaded` keeps the render for an equal answer" (it changes the sentence "each new value
afresh" of `settings.rs:151`).

Not proposed: longer timeouts, retries, or waits in the two specs. Both specs are right as
they are; with F2 the first one would have failed in 20 s with the hydration error in its
report.
