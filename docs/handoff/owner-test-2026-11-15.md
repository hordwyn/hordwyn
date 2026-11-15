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

| OT-26 | Books: the edition is lost. The owner added ISBN 978-1-5476-0417-3 (A Court of Thorns and Roses, **Collector's Edition**, Bloomsbury YA, hardcover 2019; Open Library edition OL28352792M: `number_of_pages` 448, `pagination` "433 pages : map, black and white illustrations"). The item page shows the work (OL17352669W) with the median of all 38 editions, 454 "(estimate)"; the edition name and its pages are dropped. Owner's wish: (a) a search by ISBN shows exactly that edition, because the ISBN is correct; (b) a title search lets the person pick their edition: the work with the regular hardcover first, the other editions below (paperback, special or collector's editions, re-releases), with a filter field for the edition; (c) the page total starts from the edition's **numbered pages** (the readable pages, from `pagination`, else `number_of_pages`), editable by hand as today | gap (model) | `connectors/src/openlibrary.rs::by_isbn` (keeps only the work), `snapshot` (`number_of_pages_median`); one item = one work (module doc, DEC-711..732). Needs a per-entry edition (edition id, name, format, publisher, date, pages) beside the shared work, the edition list (`/works/{id}/editions.json`) in search, and a parser for `pagination`. Same question for other kinds with editions later (board game editions, director's cuts). Also in docs/handoff/v2-notes.md, with the owner's addition: the edition is optional and changeable on the entry (Want to read the book, later read a used special edition bought at a book fair: same entry, same history) | v2 talk (note only) |

| OT-27 | Shows from TheTVDB list only the watched episodes: "Unforgettable (2011)" (source TheTVDB) shows "Season 1 2/2" with Episode 1 and 2 after logging two, no further seasons or episodes, while the next step says "Mark S1 E3 watched". "Mark season watched" and "Mark show watched" have nothing to mark | bug (data, v1 source) | `connectors/src/tvdb.rs` `details`: every season is stored with `episode_count: 0` (the `/series/{id}/extended` answer has no counts); `web/src/episode_plan.rs::item_seasons` drops seasons with count 0, then `season_rows` builds the seasons only from the logged episodes. Fix idea: read the episode counts (and titles, air dates, runtimes) from TheTVDB's episodes endpoint, aired order; a test with a recorded answer. Also check why this show came from TheTVDB rather than TMDB (source order), and refresh the stored TVDB shows after the fix | tbd |
| OT-28 | Search does not find "Pokemon Indigo League" (Netflix's title). On TMDB it is season 1 ("Indigo League") of "Pokemon" (1997); the search knows show titles only | gap (search) | search documents hold titles only; idea: index season names and alternative / regional titles (TMDB `alternative_titles`, season names) so a season or streaming title leads to the show and season | tbd |
| OT-29 | Per-episode view count: each episode row should show how many times that episode was watched (a small tag, e.g. "2x") | feature | `web/src/episode_list.rs` `EpisodeRow` ("Watched" / "Again"); the counts are in the effective history already. Also in docs/handoff/v2-notes.md | v2 talk (note only) |

| OT-30 | Episode names: the rows say "Episode 1", "Episode 2"; the owner wants the real titles, kept in sync for the shows in a library (watched or planned) | feature (reverses part of DEC-446) | DEC-446/448 keep only number, air date and runtime per episode ("no names, overviews, stills, people"; "also keep episode names" was the rejected alternative). The `catalog.episodes` job already fetches every season of a library show from TMDB and refreshes seasons with missing or future dates, so names ride along: a `title` column on `catalog_episodes` (migration 0002+, so not a patch under the frozen baseline), the TMDB season answer's `name` (asked in the instance or member language, open), TheTVDB episodes once OT-27 reads them, and the row label "1. Pilot". Recheck TMDB terms for storing names | tbd (needs a migration) |

| OT-31 | Acceptance step 14: Admin > Instance > Mail saves and "Send a test" reports success, but the mail never arrives. **Resolved 2026-11-16: it was in the spam folder** (relay: Proton). Left: sender domain SPF/DKIM/DMARC (owner, DNS), and the app logs nothing on a successful send (a success line with the relay answer would have helped) | setup + small gap | needs the app log around the test send (`dc logs app | grep -iE 'mail|smtp'`) and the relay's log; check the sender address against the relay (SPF/DKIM, a relay that accepts and drops), and whether "success" means "handed to the relay" | not blocking |
| OT-32 | Acceptance step 16: no sign-in row in the audit log. Only break-glass sign-ins write one (`auth.local_sign_in`); sign-ins through the provider write none | gap + checklist error | `api/src/sso_routes.rs` (local only); the checklist step assumes every sign-in is audited. Check step 16 with `/sign-in/local` from outside; decide whether provider sign-ins get an audit row | tbd |

## Acceptance run on prod, 2026-11-15 (owner, checklist artifact; Claude ran 15, 17, 18 from outside)

- Pass: 1-13, 19a-19g, 21, 22; 15 and 17 checked by Claude (headers once each, ports closed, cookie Secure/HttpOnly; 401 x4 then 429 with Retry-After on the break-glass route). Remarks: HSTS already one year (the name is final); nginx shows its version (`server_tokens off`).
- 14 passes (mail landed in spam, OT-31). 24 passes (backup taken and copied off the host).
- Host remark: the server runs Ubuntu 20.04 (standard support ended 2025-05; ssh warns about missing post-quantum key exchange, nginx 1.18). Not an app finding; plan an OS upgrade to 24.04 after the release.
- 16 passes: the break-glass sign-in notice named the public address 62.155.192.104, not 172.x (the proxy is believed); OT-32 stays for provider sign-ins.
- 25 passes: restore copy beside live on a spare port (3001 is used by another service on the host), same user count, secrets v1 loaded (9 values, 0 locked), healthz ok; the copy reused the live image. Checklist fix: step 25 should reuse the live image and say to pick a free port.
- 18 passes (live updates checked by the owner).
- Open: 26 staging dry run (optional with the in-place upgrade).
- Owner, 2026-11-16: the instance is production with personal data; no test or fake data there. Steps that need data (19a-19g, 21) move into the Playwright e2e suite for the next releases ("we should add UI tests"), so a later acceptance run on prod only reads and runs server commands.
- Skipped: 20 podcasts, 23 notifications.
- Step 3 owner question: secrets in `config/.env` vs the encrypted store in the database.
- Decisions: rc.1 is confirmed by **upgrading prod in place** (backup first), no fresh install; OT-22..30 **do not block** the tag; F-8 and F-9 **accepted** (household accounts); imported Completed books **count their pages** (owner: the later edition fix must migrate existing entries, the instance is productive now); operator contact **set**; F-1 **publish the website** (with the website in its own repository, DEC-840, its publication need not wait for the app tag - to confirm); DEC-725 (does a Completed set by hand count in the year and goals) **unanswered**, owner asked what it means.

## For the talk

1. Release discipline first: what a patch release may contain (data fixes like OT-22?),
   what waits for 1.1.0, what is 2.0.0 (model changes like OT-24).
2. OT-22 is the only one that leaves wrong data behind; the leftover session on prod needs
   a cleanup path either way.
3. OT-24 and OT-25 need a model decision before any code.
4. Not a finding, for v2 notes if wanted: merging two user accounts (today only the
   delete-and-link path above, which loses the deleted account's data).
