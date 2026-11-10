F7 - Doc-only follow-ups from round 2 (no builds)

Rules: common.md (citations, ASCII, no GPL, no attribution) except that you MAY edit the docs
named here; no code changes, no cargo/npm, do not commit. Other agents (F5 code fixes, F6
review page) work at the same time: touch only the files below.

1. Connector docs: turn the drafts for Open Library and TVmaze in
   `docs/fable/round2/attribution-and-connectors.md` into `docs/connectors/openlibrary.md` and
   `docs/connectors/tvmaze.md`, in the format of the existing files there (check whether an
   openlibrary file already exists and merge instead of overwriting). Re-verify each terms /
   licence / rate-limit statement on the live page with the date checked; mark anything you
   cannot open as unverified.
2. HANDOFF fixes: `docs/HANDOFF.md` section 0 says the architecture review has 10 work
   packages (it lists 12: check `git show wip/fable-reviews:docs/fable/architecture-review.md`).
   Only change that number; do not touch other HANDOFF text.
3. Add `piggy-bank` from Lucide to the vendored icon set ONLY if the repo's vendoring process
   is a pure file copy documented in `config/brand/ICONS.adoc` (same Lucide version as the
   others, source and licence recorded); otherwise describe the steps in your report and skip.
   No hand-written SVG paths (rule 11).

Final report: files changed, what was verified live and when, what was skipped and why.
