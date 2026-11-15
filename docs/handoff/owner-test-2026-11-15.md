# Owner use on prod, 2026-11-15: findings

First findings from the owner using the deployed v1 candidate (main, tracker.example.org).
Numbering continues the owner-test series (OT-21 was the last). Nothing here is built yet.
**Release vehicle is open on purpose:** the owner wants to go through the v1 release
discipline first (patch release vs 1.1.0 vs 2.0.0, see HANDOFF section 0 and
docs/modules/ROOT/pages/releases/) and then decide per finding.

## Closed threads

- **Two accounts after the first OIDC sign-in:** the provider's sign-in created a new
  account instead of offering "Is this your account?" (that step needs a *verified*
  provider email that matches a local account; OD-15). Resolved by the owner without code:
  admin deletes the new account (the `oidc_identities` row goes with it, `ON DELETE
  CASCADE`), then Settings > Account > Linked sign-ins > Link from the old account. Worked.
- **Linking a person entry to an account:** People card menu "Link to account" (username,
  the other side accepts under "Requests for you"). Found and used by the owner; the
  confusion came from OT-23.

## Findings

| ID | Finding | Type | Where to look first | Vehicle |
|---|---|---|---|---|
| OT-22 | Undo of "Watched with..." on a movie leaves the shared session behind: the item is no longer watched, but the person card still says "Together 1", "Last: Harry Potter ..., Today" and "History with kritzl" shows "Session logged with kritzl". No UI or REST route deletes a session, so the owner cannot clean it up | bug (data) | `api/src/ops.rs::log_progress` stores the session (id from `blake3(op, "session")`, then `new_v5(.., user id)` in `App::log_session_event`) and then the progress event (same `op`) with no link between them; `App::undo_progress` (`app/src/library.rs:408`) only deletes sessions of `Progress::Session` (plays). Fix idea: undo derives the session id from the event's `client_op_id` and deletes it (owner + item checked), plus a test that repeats the owner's steps. Without a `client_op_id` there is no link: decide whether to store one. Also decide what happens to the leftover row on prod (one-off cleanup, or a session delete action) | tbd |
| OT-23 | People: "Merge" with only one person: the "Keep" select shows only "Choose...", "Swap" does nothing, the submit stays disabled, no message or toast | bug (UX) | `web/src/people_dialogs.rs` `Merge`, `keep_options`, `swap` (no-op when `keep` is empty); `web/src/people.rs::menu_items` offers Merge unconditionally. Fix idea: hide Merge when there is nobody else, or say so in the dialog; Swap gives feedback or is disabled | tbd |
| OT-24 | "Watched with..." cannot be added after the fact: on a movie or show already watched alone, the action bar shows only "watched" / "up to date" and the button is gone (`StepButtons` renders it only while there is a next step). The episode row menu still offers it, but on a watched episode that logs a new view (a rewatch) instead of adding people to the existing view (not fully traced) | gap (feature) | `web/src/item.rs` `StepButtons`, `web/src/episode_list.rs` row menu; model question first: add participants to an existing view, or log a new view with them | tbd, needs owner decision |
| OT-25 | Rewatch count not visible / not settable: the owner did not find whether a show was watched twice or more. It exists ("Watched N times, last on <date>" in the facts line; shows count whole passes or finished rewatch runs, started from the action bar overflow "Start a rewatch" on a Completed show), but it is easy to miss, and earlier passes that were never logged (before the app, imports) cannot be entered as a count | UX + gap | `web/src/item_words.rs::fact_line`, `ItemPageDto.times`; DEC-537, DEC-587, DEC-590 | tbd |

| OT-26 | Books: the edition is lost. The owner added ISBN 978-1-5476-0417-3 (A Court of Thorns and Roses, **Collector's Edition**, Bloomsbury YA, hardcover 2019; Open Library edition OL28352792M: `number_of_pages` 448, `pagination` "433 pages : map, black and white illustrations"). The item page shows the work (OL17352669W) with the median of all 38 editions, 454 "(estimate)"; the edition name and its pages are dropped. Owner's wish: (a) a search by ISBN shows exactly that edition, because the ISBN is correct; (b) a title search lets the person pick their edition: the work with the regular hardcover first, the other editions below (paperback, special or collector's editions, re-releases), with a filter field for the edition; (c) the page total starts from the edition's **numbered pages** (the readable pages, from `pagination`, else `number_of_pages`), editable by hand as today | gap (model) | `connectors/src/openlibrary.rs::by_isbn` (keeps only the work), `snapshot` (`number_of_pages_median`); one item = one work (module doc, DEC-711..732). Needs a per-entry edition (edition id, name, format, publisher, date, pages) beside the shared work, the edition list (`/works/{id}/editions.json`) in search, and a parser for `pagination`. Same question for other kinds with editions later (board game editions, director's cuts). Also in docs/handoff/v2-notes.md | v2 talk (note only) |

## For the talk

1. Release discipline first: what a patch release may contain (data fixes like OT-22?),
   what waits for 1.1.0, what is 2.0.0 (model changes like OT-24).
2. OT-22 is the only one that leaves wrong data behind; the leftover session on prod needs
   a cleanup path either way.
3. OT-24 and OT-25 need a model decision before any code.
4. Not a finding, for v2 notes if wanted: merging two user accounts (today only the
   delete-and-link path above, which loses the deleted account's data).
