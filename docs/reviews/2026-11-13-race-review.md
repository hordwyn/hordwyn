# Review: race fix (lane/race, uncommitted on 3647f8f)

Date: 2026-11-13. Independent read-only review of `git diff HEAD` plus the untracked files
(`crates/web/tests/server_memos.rs`, `docs/fable/round2/race-decisions.md`,
`race-progress.md`). Nothing was built or run; findings come from reading the diff, the
surrounding code and the framework sources (`leptos-0.8.23`, `tachys-0.2.19`,
`reactive_graph-0.2.16`). Paths are relative to `src/rust/crates/` unless they start with
`src/` or `docs/`.

## Verdict

The six steps do what they say for the cases the tests cover, the one test-side change is a
legitimate fix, and the patch applies to current main without a textual conflict. Two points
should be closed before the merge (RACE-1, RACE-2); the rest may be deferred with a note.

Counts: 0 critical, 2 high-medium "must fix before merge", 5 medium "may defer", 7 low.

What was checked and found in order (no finding):

- F2 marker: `hydrate_body` is synchronous (`leptos-0.8.23/src/mount.rs:23-30`), a Suspense
  hydrates in the first run of a render effect (`suspense_component.rs:568-600`), so
  `hydration::done()` after it is reached only when the walk went through. A panic traps the
  wasm call, `done()` never runs.
- F2 loop: a reload needs "failed and no key"; the key is removed only by a hydration that
  went through. fail -> reload -> fail stays; fail -> reload -> ok clears and does not
  reload. No sequence reloads twice in a row. No storage (`storage()` is `None`) or a failed
  `set_item` means no reload at all.
- F3 equality: every DTO derives `PartialEq` (no manual impl in `contracts`, the only manual
  ones are `ui/toast.rs:33` and `catalog-core/gate.rs:22`, neither is a `loaded` value). A
  changed answer cannot compare equal. After an error the next value always gets a new key.
- F5: an own refresh runs at once as before; it only sets `Pace::ran`. Nothing is dropped:
  an event inside the quiet time sets `waiting` and is answered by the timer. Compared with
  the base, an event of a follow-up job (a cover that ends after an add) is answered at the
  same time as before (base: about t0+2.05 s, now t0+2 s).
- F6 item page on the server: `lists` is read one level behind `data`; the boundary's second
  walk (`suspense_component.rs:398-423`, "additional resource reads nested inside a
  conditional") registers it, so the server waits for it. On a refetch the item render does
  not wait for `lists` (the keyed `each` does not read it; the test with the held `lists`
  shows the new rating first).
- The test-side change (`src/web/tests/library.spec.ts:549-552`): the first page sets
  `data-hydration-failed` before it reloads, so waiting for that marker could read the first
  page's state. Waiting for the second "panicked at" is waiting for the app's own event, not
  a longer timeout or a retry. Legitimate.
- Merge: each changed file applies to main (32ddb98) cleanly (checked with a temporary index).
  Main's changes since 3647f8f in `web/src` (admin/contact.rs, log_sheet.rs, log_form.rs,
  item.rs 4 lines, home.rs 1 line, ...) add no `loaded(` call, no `Memo::new`, no caller of
  `refetch_on_update`/`refresh_on_updates`, no `<Suspense>` and no `DropdownMenu`.

## Must fix before merge

### RACE-1 (high-medium) The cure of C1 is unproven: the server detector was never red

- Where: `src/rust/apps/server/tests/it/library_page.rs:140`
  (`every_page_of_many_at_once_has_its_header_rows`); `race-progress.md` "was not run before
  F1"; diagnosis 2.5 T1b asked for the run with "F1 reverted" under `stress -c 4`.
- Problem: the diagnosis marks the stale memo as DERIVED, not proven. The detector has only
  ever run on the fixed code, so nobody knows whether it can see the defect at all, and no
  run shows that `derived` removes it. The source-rule test proves that the three memos are
  gone, not that they were the cause.
- Failure scenario: the real cause is elsewhere (for example the boundary's task set seen
  empty between two walks). The Library is again sent without its status tabs under load.
  With F2 a person now sees one reload instead of a dead page, so the defect is quieter than
  before; in the suite the console guard turns some unrelated test red now and then, and the
  detector stays green and is trusted.
- Fix: run the detector on the base code (or with the three `derived` calls turned back into
  `Memo::new`) a number of times under `stress -c 4`, as T1b says. If it turns red: note the
  rate in the test's doc comment and in LR-1. If it never does in N runs: say so in the doc
  comment and in LR-1 ("never seen red, N runs"), and keep C1 open in the handoff as
  "cure derived, watched by the console guard". Either result is fine to merge; the missing
  run is not.

### RACE-2 (high-medium) A menu kept open takes the focus on every differing refetch

- Where: `ui/src/dropdown.rs:118-124` (the effect focuses item `active` whenever `open` is
  true and the menu exists) together with the new `open=` at `web/src/people.rs:385`, `:469`,
  `game_groups.rs:149`, `integrations.rs:208`, `admin/users.rs:507`, `item.rs:634`, `:977`.
- Problem: the menu has no close on an outside click or on focus leaving (only Escape, Tab,
  a pick, the trigger). A menu opened with the pointer and then left stays open. Before the
  lane a refetch closed it; now every refetch with a different answer builds a new menu with
  `open == true`, and its effect moves the focus to the first item, wherever the focus was.
  The new instance also starts with `active = 0`, so a keyboard user on the third item is
  put back on the first.
- Failure scenario: People, an import is running (an event per item, a different answer
  every 2 s). The person opens "More actions for Zed" with the mouse, clicks into another
  field (the Library filter on a page with cards, the name field of an open sheet, a dialog
  opened by another control) and types. At the next answer the focus jumps into the menu;
  the next Enter activates "Edit" or "Remove", arrow keys move in the menu. For a screen
  reader the focus moves without a cause (WCAG 3.2.x).
- Fix: in the effect, focus the item only when the focus is nowhere (`document.activeElement`
  is `body` or null: the old menu item was just removed) or inside this dropdown; leave it
  alone otherwise. Keep `active` with `open` (one kept struct, or a second optional prop) so
  the same item gets the focus back. A test: open the menu, focus a field outside the
  `loaded` view, change from elsewhere, expect the field focused.

## May defer (with a note in the handoff)

### RACE-3 (medium) The source rule catches only the direct patterns

- Where: `web/tests/server_memos.rs:62` (`typed_names`), `:103` (`READERS`), `:167`.
- Problem: a new memo over a resource is found only when the resource is a `let x =
  Resource::new(` in the same top-level `fn`, a name typed `: Res<` / `: Resource<`, or one of
  three hard-coded function names. Not found:
  1. the crate's other aliases: `data: Data` (`imports.rs:69`, `admin/security.rs:28`,
     `admin/sso.rs`, `admin/storage.rs`), `MailData` (`admin/instance.rs:246`), `Sources`
     (`admin/sources.rs:23`), `SourcesResource` (`setup.rs:295`, `let data: X =
     Resource::new(` is found, a parameter of that type is not);
  2. a resource from the context: `let user = expect_context::<UserResource>()`
     (`account.rs:169`, `settings.rs:255`, `sharing.rs:34`) then `Memo::new(move |_|
     user.get())`;
  3. an indirect read: a memo over a `Signal::derive` or a closure that reads a resource
     (`library.rs` `known`, a `Signal` prop such as `widgets.rs:188` `of`), or a new helper
     like `kinds_now` whose name is not in `READERS`;
  4. `Memo::new_owning(`, `Memo::new_with_compare(`;
  5. a method in an `impl` block (indented `fn`): `item_around` cuts at left-margin `fn`
     only, so the method's parameters are looked up in the wrong item.
- Failure scenario: the next header row is written as `Memo::new(move |_|
  user.get().and_then(..))` or as a memo over `known`; the test is green and the C1 family is
  back.
- Fix: collect the aliases from the sources (`type X = Resource<` / `type X<T> = Resource<`)
  and use them in `typed_names`; treat `expect_context::<..Resource..>` / the collected
  aliases as a binding; match `Memo::new` with any suffix up to `(`; build `READERS` from
  the functions whose body reads a known resource (one pass, per file); say in the module
  doc what the test does not see (indirect reads through a `Signal`).

### RACE-4 (medium) A second failure leaves a dead page with nothing for the person

- Where: `web/src/lib.rs:160-173` (`failed`).
- Problem: after the one reload (or when `sessionStorage` is not available, or `set_item`
  fails) the page is marked `data-hydration-failed` and nothing else: it looks complete and
  does not react below the place of the panic. Nothing is shown or announced. LR-2 rejected
  "an error screen instead of a reload", but not a line in addition to it.
- Failure scenario: a server defect that is the same on every answer (not the C1 race): every
  person gets one reload and then a page whose buttons do nothing, with no hint to reload or
  report it. Same for a browser with storage blocked, on the first failure.
- Fix: in the branch that does not reload, insert one plain element at the top of `body`
  (`role="alert"`, a sentence and a "Reload" link, no framework needed; text from the
  page's config or a `data-` attribute for the locale). Assert it in the "panics again"
  test.

### RACE-5 (medium) An equal answer no longer resets what the render holds

- Where: `web/src/settings.rs:155-172`, `:178`; callers that refetch after a refused or
  failed change, for example `admin/security.rs:161` (the callback sets the message and
  refetches for `Ok` and `Err`), `lists.rs:516` (after a failed removal).
- Problem: before, a refetch was also "render again from the server's values". Now an equal
  answer keeps the render, with its local signals. For a refused save that is the better
  behaviour (the typed text stays next to the error). It is wrong where the server accepts
  a change and stores the old value (clamping, normalising, a setting pinned by the
  environment): "Saved." next to a field that shows what was typed, not what is stored,
  until a different answer or a navigation. Relative texts computed at render time ("3 days
  ago", `day_words`) also stay as rendered across equal answers.
- Fix: nothing in `loaded`. Note the changed contract at the call sites that relied on it:
  a caller that wants "show the stored value" after a save sets its fields from the answer
  itself, or `loaded` gets an explicit "render again" handle for those few callers. Check
  the settings forms whose server side clamps (`auth.session_days`).

### RACE-6 (medium) `kept` open flags are never cleared and one key is shared across persons

- Where: `web/src/people.rs:385` (`pp-menu-<id>-open`), `:469` (`pp-page-menu-open`),
  `game_groups.rs:149`, `integrations.rs:208`, `admin/users.rs:507`; `settings.rs:132`.
- Problem: (a) a flag that is `true` when its card leaves the answer stays `true` in the
  `Kept` map for the life of the page. (b) `pp-page-menu-open` has no person id, and
  `PersonPage` keeps one `loaded` (one `Kept`) across a change of `:id`. The item page got
  an explicit `menus.close()` on a change of the route; the person page did not.
- Failure scenarios: (a) the menu of Zed is open here, Zed is removed elsewhere (other tab,
  merge); the card goes. "Undo" brings Zed back with the same id: the card appears with its
  menu open and, with RACE-2, takes the focus. (b) the menu on `/people/A` is open, the
  person goes back/forward in the history to `/people/B` (keyboard, mouse button): B's
  page shows with the menu open, its items act on B.
- Fix: (a) in the card, `on_cleanup` cannot tell a refetch from a removal; instead let
  `loaded` drop the slots that the new render did not ask for (mark on `kept()`, sweep after
  the render), or close the flag when the id is missing from the new answer. (b) key the
  flag with the person id (`pp-page-menu-<id>-open`).

### RACE-7 (medium) The suite on the merge result has never run under the console guard

- Where: `src/web/tests/fixtures.ts:36-58`, `:104`; `race-progress.md` (one whole run,
  `chromium` only, 2 workers, on the lane's base).
- Problem: the guard fails any test whose page prints a hydration error or a panic. That is
  the intent, but it is global, and it has run once on one browser project. Main has pages
  and a spec since 3647f8f that never ran under it (`books.spec.ts`, the log sheet changes),
  and the other Playwright projects (if the gate runs them) were not run at all. The
  progress log also lists the final checks after the last code change as fmt, clippy and
  `cargo test -p web` only; it does not say that the browser run came after that change.
- Failure scenario: the merge is green in text and red in the gate for a panic that is real
  but old and unrelated (any `panicked at` after hydration now fails its test).
- Fix: none in the code. Plan the full gate on the merge result, all projects, and treat a
  guard failure there as a finding of its own, not as a reason to narrow the pattern.

## Low

### RACE-8 (low) First load of the item page now waits for the lists

- Where: `web/src/item.rs:927-930`, LR-6.
- Problem: the server and a client-side navigation show the item when `item_page`, `kinds`
  and `lists` are all there (latency is the slowest of them; before, the lists cell came in
  on its own). A `lists` call that hangs keeps the skeleton (and the stream) for the whole
  page; an error just leaves the cell out, as before.
- Acceptable: `lists` is one indexed read per user and the cell is above the fold. Note it
  in LR-6 as the price; if `lists` ever gets slow, read it into a signal by an effect in
  the browser instead.

### RACE-9 (low) "No reload loop" is checked with a fixed 500 ms

- Where: `src/web/tests/library.spec.ts:556-558`.
- Problem: a reload cycle under load takes longer than 500 ms (document, wasm, hydration),
  so a third load would mostly not be seen; the check can pass over a loop. It is also the
  one fixed wait of the package.
- Fix: assert the state that makes a loop impossible instead: `sessionStorage`
  `app-hydration-retry` is still `"1"`, and count main-frame navigations (`framenavigated`)
  equal to 2 after waiting for the second page's `load` event.

### RACE-10 (low) Negative checks behind two animation frames can pass falsely

- Where: `src/web/tests/helpers.ts:322-325` (`twoFrames`), used in `library.spec.ts`,
  `people.spec.ts`, `item.spec.ts`, `home.spec.ts` before "is still there".
- Problem: "still visible / still the same node" two frames after the request started or
  the response arrived proves little when the render comes later under load. The tests were
  red on the base, so they do detect the defect on a quiet host; under load they lean to
  green. No false red.
- Fix: none needed now. Where the answer is released by the test, the assertion after the
  release (same marked node still there, new value shown) is the strong one; keep both.

### RACE-11 (low) `watchPolicy` drops failures when called with two arguments

- Where: `src/web/tests/fixtures.ts:43` (`failures: string[] = []`).
- Problem: a caller outside the fixture that passes only `found` gets a private list nobody
  reads. No such caller exists today.
- Fix: make the parameter required.

### RACE-12 (low) The reload test depends on a debug build's wording

- Where: `src/web/tests/library.spec.ts:535`, `:560` (exactly one / two "hydration error
  occurred").
- Problem: the detailed hydration message is printed by debug builds; a run against a
  release build has only the panic line and these two assertions fail. The guard itself
  still works (`panicked at`).
- Fix: assert on `panicked at` only, or note the debug requirement next to the test.

### RACE-13 (low) The reload itself: typed text and no announcement

- Where: `web/src/lib.rs:169-172`.
- Problem: a person who typed into the server-rendered page before the wasm ran (sign-in,
  the Library filter) loses the text on the reload unless the browser restores it; assistive
  technology reads the page twice without a reason. Rare (one reload per tab and failure)
  and better than the dead page.
- Fix: none required; covered when RACE-4 adds a visible line for the non-reload case.

### RACE-14 (low) `ItemPage` is over 100 lines and grew

- Where: `web/src/item.rs:100` (105 lines at 3647f8f, 107 now).
- Problem: over the limit before the lane; the lane added two lines.
- Fix: move the event effect and the route-change effect into a helper
  (`follow_item_events(id, refetch)`), outside this package or with it.

## Notes for the merge

- LR-1..6 need DEC numbers above DEC-732 (main's last).
- Run `cargo test -p web --test server_memos` on the merge result: it scans main's new files.
- Transition states have no busy mark: during a refetch the old chips, tabs and counts stay
  usable and show the old numbers until the answer is there. That is the intent of LR-6 and
  fine for assistive technology (nothing is announced, nothing disappears).
