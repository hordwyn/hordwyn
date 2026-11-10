F6 - Build and publish the owner review page (round 2)

Not read-only for artifacts: you publish one claude.ai artifact. Do not edit repo files except
the one source file named below. No builds. Keep the ASCII / no-attribution rules of common.md
for the page text (the page is the owner's, private).

Inputs: `docs/fable/round2/owner-review-page.md` (51 decisions, F1) and
`docs/fable/round2/ux-owner-test.md` section 5 decision table (F2, OT items). Earlier review
pages for tone and pattern: HANDOFF section 9 (read them with the Artifact tool, action read).

Do:
1. Load the `artifact-design` and `artifact-capabilities` skills first (Skill tool) and follow
   them. The page must store the owner's answers durably so the orchestrator can read them back
   with the ArtifactData tool: use the shared database capability, collection `answers`, one
   document per decision id (fields: id, choice, note, updated).
2. Structure: sections in F1's order, with the F2 UX decisions (OT-*) as their own section
   right after Security. Per decision: id, question, default (preselected and marked), options
   as radio buttons, cost, what it blocks, a free-text note, and a link-free one-line reason.
   A "proceed with defaults" section with one checkbox per item ("object") instead of radios.
   A progress counter (answered / total) and a filter "unanswered only".
3. Keep it usable on a phone (16 px gutter, no horizontal scroll), light and dark.
4. Source file: `docs/fable/round2/review-page.html` (write it there, publish from there).
5. Publish with icon "checklist". Final report: the artifact URL, the collection name and the
   document shape, number of decisions on the page, anything left out.
