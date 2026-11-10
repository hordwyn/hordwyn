F2 - UX pass on the owner's test findings

Output: `docs/fable/round2/ux-owner-test.md`

Goal: concrete, owner-decidable proposals for OT-4, 5, 6, 7, 9, 10, 12, 13, 14, 15 (model
side), 16 and 17 from `docs/handoff/owner-test-2026-11-10.md`.

Read: `docs/design/ux/design.md`, `addendum-1.md`, `docs/design/ux/mockups/index.html`,
`docs/design/DESIGN.md`, the status/rewatch/session DECs in `docs/DECISIONS.md` (grep
`rewatch`, `Completed`, `Planned`, `status`, `session`, `runtime`, `DEC-450`), and the
current code: item page `src/rust/crates/web/src/item.rs`, log sheets (grep `log_form`,
`Sheet`), `ui` Sheet/Dialog, People cards, Stats and Home charts, brand tokens in
`config/brand/default/`.

Sections:
1. **Library and status model (OT-4, 5, 7, 9, 15).** Describe today's model exactly (with
   file:line and DEC refs): what "Add to library" does, status values, how a rewatch is
   stored, what "Mark to here" does on watched episodes. Then propose ONE coherent model with
   at most one alternative: e.g. status = where you are with the item now, history = every
   time you did it; "Add to library" vs "Want to watch" naming; how a stopped rewatch shows;
   how the item page header reads after 1 watch and after 3. Include exact en copy for buttons,
   pill and confirmation toasts, and the German keys' meaning. State what changes in the
   domain/events (migration or not) and what stays.
2. **Log sheets (OT-6, 10, 16).** "Watched with" / "Read with" / "Played with" per kind verb;
   centered dialog vs side sheet (WCAG, mobile: what to do below 640 px); field layout within
   OD-01; play sheet with duration (OT-16) and where score/win then show up (OT-11 link).
3. **Visual pass (OT-12, 13, 14).** Colour: propose an accent and per-group tints using the
   brand token system (no hard-coded colours outside the brand config, rule 9), contrast
   numbers for each pair (WCAG 1.4.3 / 1.4.11), light and dark. Wide screens: max width per
   page type. Button sizing rule for card actions.
4. **Stats (OT-17).** Time first: hours of movies, hours of shows, pages read, hours played,
   counts secondary; the hero tile with Lucide `piggy-bank` and total hours (check the icon
   exists in the vendored Lucide version and cite the file). What data is missing for each
   number (per-episode runtime DEC-450, movie runtime, game playtime from OT-16, book pages)
   and the cheapest way to get it from the stats cache (WP4-1, migration 0023). Text
   equivalents for every chart.
5. **Decision table:** per OT id the proposed default, alternatives, cost S/M/L, and the
   packages it implies (server / web / migration / i18n).
