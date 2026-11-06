# Assessment of the earlier autonomous run (API credits, `<projects dir>/testingapi`)

Date: 2026-11-06. Assessor: the session that continued the build (it did not produce the
assessed work). Scope: commits `2f8cbcc..334f682` on `origin/main` (tags m00, m01, m02, m03,
m06), produced in about 90 minutes on 2026-11-05.

## Verdict

A strong foundation. For the milestones it covered (M00 bootstrap, M01 domain/app core, M02
persistence and jobs, M03 catalog core + TMDB, M06 UI library), the work is idiomatic,
documented, well tested and carefully sourced. It was kept as-is and built on; nothing had to
be rewritten. The main practical weakness was that GitLab CI was never fully green, and a
handful of latent bugs that only surfaced once later milestones used the code.

Rating: good (about B+). Quality per line is high; completeness was 5 of 18 milestones.

## What was verified

| Check | Result |
|---|---|
| `cargo xtask ci` on this host from a clean checkout | green after one fix (white-label scan, see below): fmt, clippy pedantic (native + wasm), cargo-deny, sqlx offline cache, 109 tests against a real Postgres, wasm tests |
| GitLab pipeline of the last commit | lint, test, wasm-test, e2e green; docker-image failed; leptos-build hung while uploading its cache; antora never ran |
| Icon research (`docs/research/icons.md`), redone independently | same conclusions (Lucide + Tabler meeple, Simple Icons + selfh.st, Noun Project rejected); all 56 Lucide SVGs and the Tabler meeple are byte-identical to the pinned npm releases; service-logo coverage tables identical |
| TMDB terms (`docs/connectors/tmdb.md`), redone independently | correct; missing only the no-ML-training clause, the purge-on-termination clause and the commercial-use note for white-label operators (added) |
| Decisions log | 49 entries, specific, mostly with sources and versions |
| Architecture test | real: transitive check over `cargo metadata`, unit-tested on hand-built graphs, also forbids direct HTTP clients in connector crates |

## Defects found (all fixed in this session)

1. **Docker image build failed**: the Dockerfile did not copy `src/web/style`, so Tailwind
   aborted `cargo leptos build`. No `.dockerignore`, so the CI target cache would have been
   copied into the build context.
2. **CI cache design**: one cache key for every Rust job, caching the whole target dir; the
   `leptos-build` job stalled uploading about 25,000 files.
3. **Flaky test** `jobs::shutdown_waits_for_running_jobs`: a fixed 50 ms sleep before
   asserting the job was claimed; failed on a loaded GitLab runner.
4. **White-label scan false positive**: it scanned git-ignored local service state
   (`src/rust/.dev`), whose Postgres files contain the absolute checkout path; any checkout
   whose path contains the codename failed `cargo xtask ci`.
5. **`App::upsert_catalog_item` replaced attributes of an existing item**: harmless while only
   refreshes called it, but search results would have wiped refreshed data (fixed with
   `ensure_catalog_item`).
6. **Tracing misuse in the job worker**: `span.enter()` guard held across `.await`.
7. **Test databases were never dropped**: every test run left databases behind (900 MB here).

## Smaller observations

- DEC-001 records Docker as available; on this host it is not (the earlier run may have run
  elsewhere). Not a defect of the code.
- DEC-024 claims test-first development; with one commit per milestone this cannot be
  verified from the history.
- The OIDC, assets, auth, API, search and web crates were stubs; the run stopped after M06.

## What this session added on top

M04 (assets, storage budget), M05 (auth, REST API, SSE, server roles), M08 (search), M07 (web
vertical slice), plus the fixes above. See `docs/REPORT.adoc` and `docs/DECISIONS.md`
(DEC-050 onwards).
