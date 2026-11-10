F5 - Fix pack ot1a: owner-test bugs that need no owner decision (code changes)

This brief is NOT read-only. Follow `docs/handoff/agent-briefs/rules.md` (test-driven, DEC
entries, no commit) instead of the read-only rules in common.md; keep the citation, ASCII and
no-GPL rules from common.md. The owner approved Fable agents making changes (2026-11-10).

Build limits: the VM is shared with another agent. Use `CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0`,
one cargo or Playwright command at a time, targeted tests only. Check `free -g` before a build;
if "available" is below 6 GB, wait and check again. Never `cargo xtask ci` / `e2e` (the
orchestrator runs them). Services: `cd src/rust && cargo xtask services up` if they are down.

Input: `docs/fable/round2/bug-diagnosis.md` (causes, file:line, the test to write first) and
`docs/fable/round2/ux-owner-test.md` (design references). Do the items in this order, each
test-first (failing test seen red, then the fix, then green):

1. OT-8 History tab label spacing (`ui/src/tabs.rs`, `web/src/item.rs:745`).
2. OT-16 duration field visible in the first row of the log sheets as in the mockup
   (`web/src/log_sheet.rs`), within OD-01.
3. OT-12 card action sizes on People (`web/src/people.rs` vs `ui/src/button.rs`).
4. OT-14 shell max width 80 rem as design.md says (`web/src/shell.rs:358`).
5. Palette extra defects from the diagnosis: no second substring filter of server hits in
   `ui/src/command.rs` (typo hits must show), `search_id` race in `web/src/palette.rs`
   (early `search.results` event must not be dropped).
6. OT-1 palette keyboard: first the Playwright test from the diagnosis that presses
   ArrowDown in the real palette and asserts the active option (visible, scrolled into view,
   kept across streamed batches); fix every candidate the test proves; give the active row a
   visible state that meets WCAG 1.4.11 (3:1) using existing tokens.
7. OT-15 minimum: "Mark up to here" / "Mark season" / "Mark show" with nothing unwatched shows
   a polite message instead of doing nothing (en + de Fluent). Do NOT build rewatch marking
   (waits for the owner's model choice).
8. OT-11 read side: show score / win / outcome and people on History rows and the item History
   tab for plays (DTO fields + rendering), no model change.

Out of scope (owner decisions pending): OT-2 thumbnails (hotlinking), OT-3 ranking, OT-4/5/7/9
model and labels, OT-10 dialogs, OT-13 colours, OT-17 stats.

Record each non-trivial choice as a DEC (next free number, check right before writing).
Final report: per item done / not done, tests added and their result (only what you ran),
DEC numbers, files changed. Do not commit.
