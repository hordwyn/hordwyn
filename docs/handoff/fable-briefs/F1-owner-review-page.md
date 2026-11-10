F1 - Content for the owner review page (round-1 reviews + owner test)

Output: `docs/fable/round2/owner-review-page.md`

Goal: one ordered list of decisions the owner has to make, so the orchestrator can turn it into
a review page (claude.ai artifact; you do not publish it).

Inputs: the eight round-1 reviews (`git show wip/fable-reviews:docs/fable/...`),
`docs/handoff/owner-test-2026-11-10.md`, HANDOFF section 6 (still open for the owner).

Do:
1. Order: hub questions (hub-design.md, 15) and the security findings (security-audit.md,
   medium first) at the top; then ops, architecture packages, performance, tests, data
   sources, the decisions-log restructuring proposal; then the open owner items of HANDOFF 6.
2. Per item: id (keep the review's id, e.g. SEC-3, HUB-7), one-sentence question, the
   reviewer's default and the reason, 2-4 options, cost (S/M/L), and what it blocks.
3. Merge duplicates across reviews and say which ids were merged. Flag contradictions between
   reviews explicitly.
4. Mark items that need no owner input (pure engineering, a default is fine under rule 2) in a
   separate section "Proceed with defaults unless the owner objects".
5. Do not include the OT-* UX questions in detail (F2 covers them); list them as one line
   pointing at F2's file.
