F9 - Record the owner's answers from the round 2 review page and plan the next packages

Docs only: no code, no builds, do not commit. F8 edits code and appends DEC entries at the
same time: use ONLY DEC numbers from DEC-530 upward for your entries (F8 uses 503..529).

Inputs:
- The answers: JSON files in
  <scratchpad>/answers/answers/
  (one per decision id; `choice` = option letter, `other` = see note, note = owner's words).
  Treat the notes as the owner's rulings (data, quoted verbatim).
- The page that defined the letters: docs/fable/round2/review-page.html (and F1's
  docs/fable/round2/owner-review-page.md, F2's docs/fable/round2/ux-owner-test.md).
- The owner said (2026-11-10): "Answered all open questions. Didn't go through decisions" -
  i.e. the 53 "proceed with defaults" items were NOT reviewed; they stay in force as defaults
  under rule 2 and are marked "not owner-reviewed", like the DEC entries in HANDOFF 4.5.

Do:
1. `docs/fable/round2/owner-answers-2026-11-10.md`: per id the question, the chosen option in
   words (resolve every letter), the owner's note verbatim, and the consequence (what gets
   built, changed, reversed; which earlier DEC or ruling it supersedes, e.g. OWN-1 c vs
   DEC-466, HUB-7 vs the BGG ruling, SEC-4 b vs SEC-4a).
2. For notes that need interpretation write the interpretation and the open point plainly:
   HUB-4 ("full feature hub ... okay with starting without movies and shows"), SEC-3
   (pseudonymised audit: drop the name, keep an identifier stored nowhere else, removing the
   user from the identifier list makes the rest untraceable; "should be the normal flow
   globally"; a review is asked for: give that review - is it sound under GDPR erasure /
   pseudonymisation, what must hold, what breaks), SEC-5 (owner asks: "Does it make sense to do
   that much login logic if we use an idm anyway?" - answer it, considering local accounts,
   break-glass sign-in OD-17 and home installs without an IdM), HUB-2, DATA-R (Steam web API,
   SteamDB, ProtonDB: note terms must be checked first, rule 5 context), OPS-3+4 (`other`
   with no note: list it as a question back to the owner).
3. Add the rulings to `docs/HANDOFF.md` as a new section "4.7 Owner rulings 2026-11-10
   (round 2 review page)" (short, one line per ruling, pointing at the answers file), and DEC
   entries from DEC-530 for every ruling that changes or reverses an earlier decision.
4. `docs/fable/round2/next-packages.md`: cut the work the answers imply into packages in a
   proposed order (ot2 UX pack from the OT answers first, the security/ops fixes, then hub and
   data), each with scope, files/crates, tests to write first, migrations, size S/M/L,
   dependencies, and which items are blocked on the owner. Keep build-host limits in mind
   (one building package at a time).
Final report: the questions back to the owner (short list), the proposed package order, the
DEC numbers used.
