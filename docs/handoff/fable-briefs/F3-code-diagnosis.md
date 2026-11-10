F3 - Root-cause diagnosis of the owner's bugs (read-only)

Output: `docs/fable/round2/bug-diagnosis.md`

Goal: for OT-1, 2, 3, 8, 11, 15, 16 in `docs/handoff/owner-test-2026-11-10.md`, find the
cause in the code and write the fix plan so a workstream agent can go test-first without
searching. You cannot run anything: reason from the code and the existing tests, and say
where you are unsure.

Per finding:
1. Cause with `file:line` (and the commit/DEC that introduced it if visible in `git log -S`
   or DECISIONS).
2. The failing test to write first: which file (Rust unit, server integration under
   `apps/server/tests/`, ui wasm test, or Playwright spec under `src/web/tests/`), its name,
   and its assertions.
3. The fix, files to touch, risk to other behaviour (keyboard map DEC-422..430, palette
   DEC-426, search DEC-493, bulk dates DEC-446..451).
4. Size S/M/L and whether it depends on an F2 model decision (OT-15 and OT-11 probably do:
   then describe the fix for both outcomes briefly).

Hints (verify, do not trust):
- OT-1: the palette input keeps focus; check whether the result list implements the combobox
  pattern (`aria-activedescendant`, ArrowUp/Down moving the active option, Tab behaviour) in
  `src/rust/crates/ui/` (Command) and `src/rust/crates/web/src/palette*`; WAI-ARIA APG
  combobox pattern is the reference (cite the URL).
- OT-8: `src/rust/crates/web/src/item.rs:745`, a `" "` literal between the label and the count
  span; check how Leptos 0.8 `view!` treats a whitespace-only string literal.
- OT-3: `search` crate ranking (words first, typos later, DEC-493), and whether provider
  popularity or exact-prefix match is used at all; how local hits and provider hits merge.
- OT-11: where a play with score/win is stored (sessions/plays tables), and why the History
  feed and Stats do not show it (feed source filter, stats measure for games, group mapping).
- OT-15: `BulkDates` / "Mark to here" in the episode list and the `App` call it makes for
  already watched episodes.
- OT-2: does the search hit DTO carry a cover url; how the item page gets its cover.
- OT-16: play sheet fields vs `sessions` columns (duration).
