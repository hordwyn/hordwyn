# Owner test, 2026-11-10: findings and plan

The owner's first hands-on test of `ux4` (quick and dirty, at home). Findings are numbered
OT-1..OT-17 in the owner's order and words, then triaged. Nothing here is built yet.

## Closed threads

- **Podman build on the company laptop:** ignore. The laptop runs Cisco SSE (TLS
  interception); at home the image builds and runs. No code change.
- **TMDB "retrying":** same cause. At home TMDB and IGDB both work.

## Findings

| ID | Owner's finding (condensed) | Type | Where to look first |
|---|---|---|---|
| OT-1 | Palette: after typing "Harry Potter" the arrow keys do not move through the results, Tab does not reach them either; Enter / Ctrl+Enter exist but the items cannot be reached in any meaningful way | bug (a11y, WCAG 2.1.1) | `ui` Command / palette, `web/src/palette*` |
| OT-2 | Search results have no thumbnail | feature | palette result row, search hit DTO (cover url) |
| OT-3 | "Harry Potter and the Order of the Homeless" ranks above the original Harry Potter | bug (ranking) | `search` crate, DEC-493 (words first, typos later), provider order |
| OT-4 | "Add to library" sets Planned; why? No explanation of what it does | UX | item action bar, palette Ctrl+Enter (DEC-426) |
| OT-5 | Hard to find the Watched button after "Add to library"; unclear whether the item is now watched | UX | item action bar, status pill |
| OT-6 | "With ..." is confusing; should be "Watched with ..." (per kind verb) | copy | log sheet, Fluent keys |
| OT-7 | Rewatch shows "watched once + X rewatches"; check whether that confuses people | UX check | item page history/summary |
| OT-8 | Movie page History tab shows `history0`, `history1` (no space before the count) | bug | `web/src/item.rs:745` (`" "` text node) |
| OT-9 | Movie status model: completed after one watch; a stopped rewatch is "both"? Re-watching and tracking a rewatch is not intuitive | UX (model) | domain status / rewatch, DEC-2xx on statuses |
| OT-10 | Side sheets: would rather have a centered dialog; they look odd (much empty space at the top, uneven clustering of fields) | UX (layout) | `ui` Sheet, log sheets |
| OT-11 | Game: logged a score and "won" for Lego Harry Potter; it shows nowhere on History or Stats | bug or gap | play sheet -> sessions/plays, History feed, Stats per group |
| OT-12 | Buttons on the person cards have different sizes | bug (visual) | `web` People cards |
| OT-13 | Very black and white, not much colour | visual design | brand tokens (`config/brand/default`), design 2.7 |
| OT-14 | Does not use a 2k screen; much room left and right (not important) | visual | AppShell max width |
| OT-15 | Bulk rewatch of a series: "Mark to here" works once; on already watched episodes a second time does nothing (should log a rewatch) | bug / model | episode list `BulkDates`, `App` episode marks |
| OT-16 | Cannot log playtime on a game, only a score | gap | play sheet fields, `sessions` duration |
| OT-17 | Stats: completed/planned counts are uninteresting. Lead with time: hours of movies, hours of shows, pages read, hours played; item counts secondary. Fun primary tile: a pig icon (piggy bank) with the total hours | UX (stats) | Home all-time charts, Stats page, stats cache (WP4-1) |

## Triage

1. **Plain bugs, fix test-first (orchestrator + workstream agents):** OT-1, OT-8, OT-12, OT-3
   (after the diagnosis), OT-11 (after the diagnosis).
2. **Need a model decision first (Fable UX pass, then the owner):** OT-4, OT-5, OT-7, OT-9,
   OT-15 are one topic: what "in the library", "completed" and "rewatch" mean and how the
   item page shows them. OT-6 and OT-16 belong to the log sheets of the same pass.
3. **Visual / layout pass (Fable UX pass, then the owner):** OT-10, OT-12, OT-13, OT-14.
4. **Stats redesign (Fable UX pass, then the owner):** OT-17. Icon: Lucide `piggy-bank`
   (ISC, already the UI library, rule 11 holds; not vendored yet, add it from Lucide). Hours need runtimes: per-episode runtime in
   stats is deferred in DEC-450, so OT-17 pulls that item forward.
5. **Small features:** OT-2 (cover in palette rows; covers are already cached per item).

## Plan (replaces section 0 "Plan after that" of HANDOFF until the owner finalises it)

- **P0 Fable round 2 (read-only, no builds):** briefs in `docs/handoff/fable-briefs/`.
  F1 owner review page content (8 round-1 reviews + this file), F2 UX pass (OT-4..7, 9, 10,
  12..17), F3 code diagnosis (OT-1, 2, 3, 8, 11, 15, 16), F4 attribution and connector
  research (W1, W5). Not started yet: another agent on the VM uses the RAM (owner,
  2026-11-10).
- **P1 Owner walkthrough:** round-1 reviews (hub questions and SEC first) plus the F2
  proposals, on one review page.
- **P2 Fix pack "ot1":** the bugs from F3 plus the security fixes the owner accepts,
  test-first, review, `cargo xtask ci` + `cargo xtask e2e`, tag.
- **P3 UX pack "ot2":** the model, visual and stats changes the owner picked from F2.
- **P4 then W0..W7** as before (HANDOFF section 0).

## Second owner test, 2026-11-11 (after ot2d)

"Rest seems to be working." New findings, folded into ot2f:

| ID | Owner's finding | Type | Where |
|---|---|---|---|
| OT-18 | A co-op video game (LEGO Harry Potter) has no win or lose, but the play reads "Lost together"; the form only knows won/lost (`CoopOutcome`, `log_sheet.rs` `coop_won` defaults to true) | gap | `domain/src/session.rs` `CoopOutcome`, play form, History row |
| OT-19 | "Grid is not aligning": the four time tiles use `md:grid-cols-4` on the viewport, the content column beside the side nav is narrower, tiles 3 and 4 run off to the right at about 770 px content width; a stray line left of "All-time stats" | bug | `web/src/time_tiles.rs:246`, Home heading |
| OT-20 | "Shows 0 hours" beside "13 episodes": no TMDB episode runtimes are counted before ot2g; "0 hours" reads as wrong when episodes exist | UX | `time_tiles.rs`, DEC-577 |

Board games: not usable yet (no catalogue source - BGG not built per HUB-7, Wikidata in d2/h1 - and no manual item entry); the play, score and group parts exist.
