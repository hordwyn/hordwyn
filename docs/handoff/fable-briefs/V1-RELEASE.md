V1-RELEASE - the last package before the owner's acceptance run (normal flow, not Fable)

Spec: docs/fable/round2/v1-plan.md "Addendum 2" (v1-release items 1-6), "Addendum 3" (KK_ prefix),
"Addendum 4" (test databases), plus:
- LB-33 / v1-wikidata review: the "add by hand" checks for board games now that both v1-manual
  and v1-wikidata are on main (manual.spec searches Play and hits the Wikidata mock; a board game
  added by hand gets the same facts form as the others where it makes sense).
- The greenfield rule ends with this package (PKG-implement.md). From here on, no migration may
  change after it is in a tag.

Items, in this order (test-first where behaviour changes):
1. Remove pre-v1 shims: `storage.fs_legacy_root` and the blob move from the old layout
   (unless a fresh install needs it), the "Coming from a build before 0034" section, the one-way
   note for 0032, the reindex note for ot1b, pre-0034 sentences in .env.example, install.adoc,
   README.md, compose.yml.
2. Rename the provider key variables' `KK_` prefix (initials of the former name) to a neutral
   prefix without a brand (e.g. `PROVIDER_`/`SOURCE_` - pick one, record it), in code,
   config/.env.example, CI, docs, xtask record, tests; no alias for the old names.
3. `store_pg::testing` drops the `test_<uuid>` database it created when the test ends (also on
   success; keep it on failure or with `KEEP_TEST_DB=1`).
4. Squash all migrations into `0001_v1_baseline.sql`: first prove equality - build a database
   from the old chain and one from the baseline and compare `pg_dump --schema-only` (normalised)
   in a test that runs once and is then kept as a fixture comparison against a stored schema
   dump; regenerate `.sqlx`; remove the old files.
5. `CHANGELOG.md` (Keep a Changelog, https://keepachangelog.com/en/1.1.0/) with a v1 entry that
   says what v1 covers (docs/modules/ROOT/pages/v1-scope.adoc is the source); a release notes
   template under docs/modules/ROOT/pages/releases/ (highlights, upgrade steps: routine +
   release-specific, known issues, rollback); routine upgrade procedure in operations.adoc plus
   a "Staging" page (second compose project on the same host as the restore rehearsal, a
   production backup restored into it, upgrade staging first, checklist, then production).
6. `cargo xtask upgrade-test <previous-tag>`: builds the previous tag's schema and seed data in a
   local Postgres database (from `git show <tag>:...` migrations or a stored schema fixture of
   the tag), runs the current migrations and a read-back smoke over the main repos; before v1
   there is no previous tag, so it skips with a message; wire it into CI for tags after v1.
7. Release checklist in docs/HANDOFF.md section 1 (changelog, notes, upgrade-test green, staging
   upgrade by the owner, tag, push).
8. The board-game by-hand checks above.
9. The website: set `content/release.toml` `version = "1.0.0"` only if the orchestrator says so
   (the owner tags v1); leave it empty but prepare the confirmed ids list as a comment.

Final runs: fmt, clippy pedantic native and wasm, `cargo test -p xtask` and `-p web` (all
binaries), the whole server `it` binary, `cargo leptos build` with the e2e features, the WHOLE
Playwright chain (all projects) with your own server name and ports.
