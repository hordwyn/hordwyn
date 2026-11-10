F8 - Apply the ot1a review (code changes, same tree as F5, not committed yet)

Rules: `docs/handoff/agent-briefs/rules.md` (test-driven, DEC entries, do not commit), build
limits as in F5 (`CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0`, one cargo/Playwright at a time,
check `free -g`, wait below 6 GB available, never `cargo xtask ci`/`e2e`).

Input: `docs/reviews/2026-11-10-ot1a-review.md` (OT1A-1..11), the F5 progress log
`docs/fable/round2/fix-pack-ot1a-progress.md`, the uncommitted diff (`git diff`).
Do not touch `config/brand/ICONS.adoc`, `src/rust/crates/ui/icons/*`,
`docs/fable/round2/review-page.html`.

Every finding is applied or explicitly rejected/deferred with a DEC entry (rule 5):
- MUST: OT1A-1 (stale results unfiltered -> wrong item on Enter / Ctrl+Enter), OT1A-2
  (OT-15 note carries over to another item / survives refetch). Failing test first.
- SHOULD (apply unless it turns out large): OT1A-3 batch the play reads with a `get_many`
  port over the existing `sessions_where_ids`, correct DEC-502; OT1A-4 German "Du" form in
  the shared-win string (plus a de test); OT1A-5 repeated status re-announced; OT1A-6
  tighten the late-batch assertion; OT1A-8 scroll the kept active row into view; OT1A-10
  make the item_plays test able to fail on order and the 50 cap.
- Your call (apply or defer with a reason): OT1A-7, OT1A-9, OT1A-11.

Re-run the targeted tests of every file you touch plus the Playwright specs `item`, `shell`,
`people`, `imports`, `slice` (OT1A-7). Append a section "F8 review fixes" to the progress
log after each finding. Final report: per finding applied / rejected / deferred with DEC,
tests run and results (only what you ran), files changed.
