# Review of package v1-manual (lane C), 2026-11-13

Status: COMPLETE. Read-only review: no file but this one was written, nothing was built
or run (no cargo, npm or Playwright), nothing committed. Every statement about a test
result is taken from `docs/fable/round2/v1-manual-progress.md`, not re-measured.

Reviewed: the working tree of `<repo>-lane-polish` (branch
`lane/v1-manual`, HEAD = merge base `5f41f26`, nothing committed): 35 modified or deleted
files and 17 untracked ones. Compared with `main` (tag v1-books) for the provenance code
and the merge.

Inputs: `docs/fable/round2/v1-plan.md` 4a and 7 (B2, B5), DEC-640, `docs/HANDOFF.md` 2 and
4.8, `v1-manual-decisions.md` (LC-23..36), `v1-manual-progress.md`.

Paths below are relative to `src/rust/` unless they start with `docs/` or `src/web/`.

## Summary

| Severity | Count | Ids |
|---|---|---|
| Critical | 0 | - |
| High | 0 | - |
| Medium | 4 | MAN-1, MAN-2, MAN-3, MAN-4 |
| Low | 8 | MAN-5 .. MAN-12 |

Must fix before merge: MAN-1, MAN-2, and the merge notes of MAN-4 (they are instructions
for the merge itself). May defer: MAN-3, MAN-5 .. MAN-12.

No security finding. B2, B5, the `user` provenance writers, the purge and the API errors
hold as far as reading the code can tell (section "Checked, no finding").

## Must fix before merge

### MAN-1 (Medium) The dialog pre-selects the first kind of a scope that has several

`crates/web/src/manual_item.rs:187-204` (`kind_choice`, line 195), test
`src/web/tests/manual.spec.ts:49`.

Problem: with a scope that is a kind group, `chosen` is the first kind of the group in
the order of `GET /kinds`, which is the id order of the registry (a `BTreeMap`,
`crates/kinds/src/registry.rs:19`, `crates/api/src/browse.rs:83`). Every group has
several kinds: Watch = anime, movie, show; Read = book, comic, manga; Play = board_game,
video_game; Listen = audiobook, music, podcast. The plan says "the scope's kind when the
scope has one; else a select". The function's own comment gives the reason for not
choosing in scope "all" ("a film added as a board game by default would be an accident");
the same accident happens inside a group. The browser test accepts either kind of Play
(`/^(video_game|board_game)$/`), so it does not pin the behaviour.

Failure scenario: the owner searches "Holiday 2019" in the Watch scope, takes "Add ... by
hand", sees the title filled in and presses Enter. The item is created as Anime. The kind
cannot be changed ("Edit details" has no kind, LC-28) and an item cannot be deleted
(plan 4a risks), so the wrong item stays in the catalogue and in "In your instance" for
every member; the owner has to remove it from the library and add a second one. Same for a
book scope (a comic added from Read becomes "book", with pages as its progress model).

Fix: choose a kind only when the scope's group holds exactly one enabled kind; otherwise
leave "Choose a kind..." (the client check then stops at the Kind field, which is already
built). Keep the group's kinds first in the list. Pin it in
`manual_item::tests::the_dialog_pre_selects_the_scopes_kind` and make the spec expect the
empty value for Play.

### MAN-2 (Medium) "Already here" can be skipped, and a failed second step makes a duplicate

`crates/web/src/manual_item.rs:326-354` (300 ms timer), `383-438` (`submit`), `357-379`
(`add_and_open`); `crates/api/src/manual.rs:38-59` (no check on the server).

Problem, two ways to the same result (a second item of the same title that nobody chose;
no delete exists):

1. The list of items with the same title is asked 300 ms after the last change of title
   or kind and arrives a round trip later; `submit` does not wait for it and the server
   does not check. The title is pre-filled and the focus is in it, so Enter directly after
   the dialog opens (or directly after choosing the kind) creates the item before the
   list could show.
2. Creating is two calls from the browser (LC-35): `POST /items`, then `add_to_library`.
   When the second fails (network, session expired between the two), the item exists, the
   dialog stays open with an error, `busy` is false and the target is still `New`. The
   next press of "Add to library" sends `POST /items` again: a second item. "Already
   here" does not help, because title and kind did not change and the effect does not run
   again.

Failure scenario (1): "Catan" exists from an import; the owner types Catan in the Play
scope, clicks the row and presses Enter within about half a second: two "Catan" in the
catalogue, the plan's "a second Catan is a choice, not an accident" is not met.
Scenario (2): a phone on weak reception: the first press creates the item and fails on
the add, the second press creates another one.

Fix: (a) in `submit` for a new item, when no answer for the current (kind, title) is in,
ask `GET /items/titled` first and, if it names items, show them and stop once (a second
press saves: "a choice"). Better and independent of the client: `POST /items` answers 409
`conflict` with the ids when the title exists, unless the body carries
`"allow_duplicate": true`. (b) Keep the id of the created item in the form; while it is
set, a retry only repeats `add_to_library`.

## Medium, may defer (with the merge notes of MAN-4 to be followed at the merge)

### MAN-3 (Medium) Import: a member's cover address is dropped without a word; an admin's bad address loses the row; the dry run checks neither

`crates/importers/src/runner.rs:479-498` (`stub`, line 484), `:461` (dry run), LC-34;
`docs/modules/ROOT/pages/import-format.adoc:38-41`.

Problem:

- On a home instance a member may set a cover address in the dialog and over
  `POST /items` (LC-26, LC-30), but the import takes `cover_url` from admins only, because
  the importer "does not know the mode". The row is imported, the address is left out and
  the report says nothing (`a_members_import_keeps_the_rows_and_leaves_the_address_out`
  pins zero errors).
- For an admin, a row whose `cover_url` fails the check (an `http://` address, a blank
  inside) is a row error: title, status, rating and history of that row are not imported
  at all. Losing the picture would be enough.
- A dry run returns before `stub` (`None if r.dry_run => Resolved::New`), so it reports
  such a row as fine and the real run then fails it.
- A row without `cover_url` still goes through `find_or_create_catalog_item` (state
  `partial`, no provenance), a row with one through `create_manual_item` (state `full`,
  `user` provenance): a hand-entered item without a picture loses its `user` marks on an
  export and import, so a later source match overwrites its title and year.

Failure scenario: a family member (role member) moves their library from another home
instance with the export file: all pictures of their hand-entered games are gone and
nothing in the import report says why. The owner (admin) imports a file where one address
is `http://`: that game is missing from the library.

Fix: give the importer the rule instead of the role (a `may_set_cover: Arc<dyn Fn(Actor)
-> bool>` built from `ops::may_edit_items` where the importer is wired, or the hosting
flag read at run time); when the address is refused or invalid, import the row without it
and add a row note ("picture address not taken: ..."); run `checked_cover` in the dry run
too. Optional: create every id-less row of the generic format through
`create_manual_item` so title and year keep `user` provenance.

### MAN-4 (Medium, merge notes) "Keep both sides" is not right for two of the seven conflicts, and two behaviours change once both packages are on main

Progress log, entry "Merge with main"; `crates/api/src/ops.rs:152-157`,
`crates/app/src/catalog.rs:593-622`; main: `crates/api/src/ops.rs:143-171`,
`crates/app/src/catalog.rs:547-582`, `apps/server/src/book_stubs.rs:262-294`.

Problem:

1. `api/src/ops.rs`: `may_edit_items` exists on both sides with the same text. Keeping
   both sides gives two definitions. Take main's block (`may_edit_items` and
   `set_page_count`) and from the lane only the `details_editable` line, the `USER`
   constant, the `source_known` import and the two `item_detail_where` calls.
2. `app/src/catalog.rs`: the lane carries a copy of main's `store_refresh`,
   `USER_PROVENANCE` and `keep_user_attributes` (identical text; the `store_refresh` hunk
   merges by itself). In the conflict at the end of the file keep main's side only
   (`USER_PROVENANCE`, `MAX_PAGE_COUNT`, `PAGE_COUNT`, `keep_user_attributes`, the page
   total functions); the lane adds nothing there that main lacks. `store_refresh` then
   holds its own copy of the `by_user` set next to the lane's `fields_by_user`: use the
   helper in both places.
   Both mistakes end in a compile error (E0428), not in wrong behaviour, so they are
   safe; the log's sentence is still wrong as an instruction.
3. The other five (`app/src/lib.rs`, `ports.rs`, `store-pg/.../catalog.rs`,
   `testkit/src/store.rs`, `catalog-core/tests/credentials.rs`) are additions next to each
   other: keeping both is right. Main did not touch `upsert`, `purge_provider_data`,
   `handlers.rs` or `catalog-core/src/refresh.rs`, so the lane's changes there meet no
   other edit.
4. Behaviour to check after the merge, not visible as a conflict:
   - `cli match-books` (main) takes every book without an Open Library id as a candidate
     (`items_without_namespace`), so also a book a person entered by hand because no
     source knows it. Applied, the item is "sourced": title, year and typed cover stay
     (`user`), authors, description and subjects of the matched work are added, and "Edit
     details" disappears (409). The command prints its matches before `--apply`, so this
     is the operator's decision, but neither docs page says it. Either leave out items
     whose `title` provenance is `user`, or say it in the command's text and on "What v1
     covers".
   - `set_item_page_count` (main) and `edit_item_details` (lane) both read the item,
     change it and write the whole row. Two such writes at the same moment lose one of
     them (the `FOR UPDATE` in `upsert` only covers the title read). Rare on a home
     instance; noted as MAN-9.
   - Migration: rename `0200_catalog_title_lookup.sql` to `0037_...`, run `cargo xtask
     sqlx-prepare`, and re-run `-p store-pg --test manual --test retention` and the
     server `it` tests `manual::`, `retention::`: the lane changed the `upsert` and purge
     statements that main's new code (`apply_catalog_partial`, `set_item_page_count`,
     `requeue_failed`) writes through.
   - `item::overflow`: "Set page total" (main) and "Edit details" (lane) in one menu, as
     the log says.

## Low

### MAN-5 (Low) Tests that were never seen failing

From the progress log, by its own words:

- `store-pg/tests/manual.rs`: `the_purge_keeps_what_a_person_set_on_postgres`,
  `a_purge_without_fields_of_a_person_empties_the_maps_on_postgres`,
  `a_hand_entered_item_is_stored_and_found_by_its_title_on_postgres` ("green on the first
  run"; the same cases were red in memory, but the SQL of the purge is a different
  implementation from the in-memory one).
- `apps/server/tests/it/manual.rs::hosted_mode_keeps_cover_addresses_and_edits_for_admins`
  (first run green).
- `crates/api/tests/rest.rs`: the two new HTTP tests (no red run recorded).
- `crates/importers/tests/cover_url.rs` (3 tests, no red run recorded).
- `web`: `manual_item::tests` other than the kind pre-selection (form check, refusal
  mapping, `titled_path`) and `palette::tests::add_by_hand_is_offered_once_every_source_has_answered`.
- Playwright `manual.spec.ts` (both tests; the log says a red run would have cost a
  second build).

Fix: for the Postgres purge, put the old `UPDATE` back once and see
`the_purge_keeps_what_a_person_set_on_postgres` fail; the same for the 403 of the hosted
test (pass `true` instead of `may_edit_items`). The rest may stay as recorded.

### MAN-6 (Low) A corrected year does not reach the Library's index

`crates/store-pg/src/repos/catalog.rs:137` (LC-36), `crates/search/src/indexer.rs:248-255`.

The library document carries `title`, `original_title` and `year` of the item. The entries
are queued again only when the title changed. "Edit details" that changes the year only
(and a refresh that changes year or original title) leaves the library documents as they
were until the entry itself changes. Scenario: the owner corrects 2018 to 2019; whatever
reads the year from the library index (year filter or sort, if served from the index)
still says 2018. Fix: compare title, original title and year in the same read.

### MAN-7 (Low) Accessibility of the dialog

`crates/web/src/manual_item.rs:565`, `:289-296`, `:590-606`.

- The Kind select gets no `aria-invalid` and no `aria-describedby` for its error (the
  three inputs do). The message is announced once through the alert, but it is not tied
  to the control (WCAG 1.3.1, 3.3.1).
- A refusal from the server sets the field's message but does not move the focus to the
  field (the client check does, line 401). After "Add to library" the focus stays on the
  button while the wrong field may be scrolled out of view at 360 px.
- "Already here" names title and year only. Two items of the same title and year (which
  this dialog exists to prevent, and MAN-2 can produce) give two identical rows and two
  identical button names "Use this one: Catan (1995)". Add something that tells them
  apart (cover thumbnail, "in your library", or a link that opens the item).

### MAN-8 (Low) The search fill no longer recognises a purged item that kept a title

`crates/app/src/catalog.rs:423-441`.

`Fill` skips cover and popularity for an expired item by `title_unavailable()`. Since
LC-24 a purged item can keep a title of a person; then `Fill` writes the search result's
cover address and popularity into it. The write is stamped (`provider_data_at`, the
`EXCLUDED.title <> ''` branch of `upsert`), so the next sweep purges it again and the
retention limit holds; the comment ("would be kept unstamped") and the intent ("gets its
data back through a refetch") no longer match. Only reachable for an item of a
cache-limited source that also has a `user` title, which today needs an item edited while
its source was not registered. Fix when convenient: carry "expired" on the item (the
`provider_refetch_after` column) instead of deriving it from the title.

### MAN-9 (Low) Edits of a shared item leave no trace and can lose a concurrent write

`crates/app/src/manual.rs:152-197`.

On a home instance every member may change title, year and picture of an item everybody
shares (as specified). There is no record of who changed what and the old value is gone;
`edit_item_details` writes the whole row it read (see MAN-4, point 4). For a family
instance acceptable; before hosted mode an audit entry (the admin area has one for other
actions) and a row lock or version check are needed.

### MAN-10 (Low) The watched webhook still queues for ids no source of the instance knows

`crates/integrations/src/lib.rs:532`.

The webhook asks "any external id", not `source_known` (it has no registry). An item that
only holds ids of sources this instance does not have is queued on every play; the job
now returns at once without marking the item, so nothing stops the next one. Harmless
(one no-op job per play, deduplicated), but it is the loop B2 describes in a milder form.
Fix: none needed for v1; or let the job set `fresh_until` for such an item.

### MAN-11 (Low, to verify) `done.run(())` after the item page is gone

`crates/web/src/manual_item.rs:431`.

`done` is a `Callback` owned by the item page. If the person leaves the page while the
`PATCH` is on its way (browser Back; the dialog is modal but Back still works), the
callback is disposed when the answer arrives. If `Callback::run` panics on a disposed
value in the Leptos version in use (0.8.23), the wasm app stops until a reload. The file
already uses `try_set` for the same reason in `show`. Fix: `done.try_run(())` or check
before running. Not confirmed by a run.

### MAN-12 (Low, hosted mode, after v1) A member can fill the shared catalogue

`crates/api/src/manual.rs:38-59`.

`POST /items` has no limit per account and nobody, not even an admin, can delete an item.
On a hosted instance a member can create any number of titles that every other member
finds under "In your instance" (the plan accepts the visibility for home v1). `GET
/items/titled` likewise answers any member with the titles of all. Before hosted mode: a
rate limit, an admin delete for unreferenced `user` items, and a decision on visibility.

## Checked, no finding

- B5 / SSRF: `assets.register(AssetKind::Cover, ..)` has one call site
  (`apps/server/src/handlers.rs:79`), now behind `cover_source`, which returns none for
  provenance `user` (also on an item with source ids) and for an address nobody answers
  for. No other server code reads `cover_url` to fetch (integrations set none, the
  importer never fetches, notifications carry none). The server stores a typed address as
  text only.
- The address check (`app/src/manual.rs:36-64`): `https://` in any case, host present, no
  `@` in the authority, no blank, control character, quote, angle bracket, backslash,
  backtick, brace, bar or caret, 2,000 characters. It is rendered as an attribute value
  of `<img src>` only (`ImageDto` without variants, so no `srcset`; the inline `style` of
  the cover box takes the thumbhash data URL, never this address). Passes `img-src 'self'
  data: https:` on an https instance and on a plain-http one. No referrer leaves the
  instance: `referrerpolicy="no-referrer"` on the cover and palette images and the
  response header `Referrer-Policy: same-origin` for the three `<img>` in `lists.rs`
  that lack the attribute. An address may name a host in the LAN (`https://192.168.1.1/`):
  that is a GET from each viewer's browser, accepted by DEC-640 for home, admins only on
  hosted.
- Hosted mode: `may_edit_items` = admin, or not hosted. `POST /items` with an address by a
  member: 403; `PATCH` by a member: 403 before the item is read; `details_editable` uses
  the same rule. A member cannot overwrite the data of a source for everybody: `PATCH`
  answers 409 whenever `source_known` (a registered connector with details for the kind
  and an id of the item in its namespace, whatever its key or switch). An item is
  editable although it holds source data only when its source is not registered on this
  instance, which is the plan's rule ("no external id in a registered connector's
  namespace").
- B2: `ops::item` and `ops::item_page` queue only when `source_known`; `refresh_item`
  leaves such an item untouched (no `failed`, no retry time); the importer queued only
  with ids before. `App::item_detail` and `item_detail_for` (rule "any external id") have
  no caller outside tests. New items entered by hand are `full`, import stubs `partial`:
  neither shows "Loading details" (only `stub` does, `web/src/item.rs:334`).
- `user` provenance: `store_refresh` in the lane is the same text as on main. The merge
  writer and the fill keep title, original title, year, description, cover address and
  `attributes.<name>`; a removed year or cover stays removed.
- Retention purge (`store-pg/.../catalog.rs:256-283`, `testkit/src/store.rs:386-423`): per
  field. Everything whose provenance is not `user` is emptied, also when another field of
  the same item is the person's; attributes are filtered by `attributes.<name>`;
  `cover_asset`, popularity, `fetched_at` go; only `user` entries stay in the provenance
  map. A `user` value is set only where the typed value differs from the stored one
  (`edit_item_details`), so a source's value is never relabelled as the person's by
  saving the form unchanged; the dialog pre-fills the address only when it is the
  person's. The expiry mark is now ended by a write with `fetched_at`, not by any titled
  write: `mark_catalog_refresh_failed`, `set_cover_asset`, an edit and (after the merge)
  `set_item_page_count` all carry the stored `fetched_at`, which is empty after a purge,
  so the item stays due for its refetch. Postgres and memory agree.
- Export and import: the export writes `cover_url` only for provenance `user`; an item
  with external ids never takes an address from a file (G2-2 holds, pinned by the "Heat"
  row); CSV column and JSON field are optional, old files read; the published schema was
  regenerated.
- API: 201, 422 `invalid` with `field` (title, year, cover_url, kind), 403, 404, 409, 401
  as in LC-27; `/items/titled` does not collide with `/items/{id}` (static segment wins).
- Migration 0200: one expression index on `(kind, lower(btrim(title)))`, the statement
  compares the same expression. Plain `CREATE INDEX` (short lock) is fine at home size.
- Fluent: 26 keys each in `en.ftl` and `de.ftl`, same names; texts are white-label ("this
  instance", no product name). ASCII: no non-ASCII character in any changed or new file
  other than `de.ftl`.
- Functions over 100 lines: none seen in the new code; the log records clippy pedantic
  clean (`SearchPalette` was split).
- The two recorded mistakes (`pkill -f`, the drop of 334 `test_*` databases): nothing in
  the tree looks affected. They touched processes and databases, not files. `.sqlx` is
  consistent with the code: the 2 removed files are the two statements the lane changed
  (`upsert`, purge), the 5 new ones are those two plus the title read, the outbox insert
  and `titled`. No stray or throw-away file is left (the console-printing spec the log
  mentions is gone). Whether another lane lost a test run or a database cannot be seen
  from this tree.

## Not checked

- Nothing was run. Whether a hand-entered show or anime (no seasons, attributes empty) can
  be tracked in a useful way on its page was not tried; the plan's acceptance names board
  game, book and audiobook only.
- The OpenAPI snapshot and the generated schema attachment were not compared line by line.
