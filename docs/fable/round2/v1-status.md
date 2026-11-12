# v1 status (2026-11-12, main at tag ot1b)

Status: COMPLETE. Read-only review: no code was changed, nothing was built or run for this
file. Claims are cited by `file:line` (working tree of `main`, a714069) or by DEC number;
what was not checked is listed in section 7. Paths: `crates/` = `src/rust/crates/`,
`apps/` = `src/rust/apps/`.

## 0. Summary

For a home instance the product is close to v1, and what is missing is operations, not
features. Movies, shows and video games work end to end (find, add, track, rewatch, plays,
import, stats); podcasts can be found and added but were never owner-tested and have no
episode list from their source. The other eight kinds (anime, manga, books, audiobooks,
music, comics, visual novels, board games) have no catalogue source and no manual entry:
they can only arrive through an import file. Nothing found in the owner tests is still open
as a blocker; about twenty small defects are known and listed. None of the twelve security
and twelve operations findings of the round-1 reviews has been built yet. For a single home
instance on the LAN behind your own reverse proxy three of them block a v1, all operations:
the image has no `cli` (so the reindex that the ot1b upgrade asks for cannot be run from the
image), the three first-run settings are missing from `.env.example`, and there is no
backup-and-restore runbook with commands. For an instance reachable from the internet seven
block (those three plus security headers, the body limit on server functions, sign-in
throttling and the compose defaults), and two more as soon as someone outside the household
gets an account. Recommendation: call v1 "home instance, four kinds with a source", ship it
after three small packages (v1-ops, sec0, v1-polish) and one owner acceptance run; everything
else, including public images, the rest of the security pack, runtimes per episode, books
and board games, is after v1.

## 1. What works per media kind

The registry holds twelve kinds (`crates/kinds/src/builtin/mod.rs:25-36`); there is no
thirteenth. Four catalogue connectors exist and are in the release build
(`apps/server/src/lib.rs:186-201`, `src/rust/Cargo.toml` `bin-features = ["all-connectors"]`):
TMDB (movie, show; `crates/connectors/src/tmdb.rs:57`), TheTVDB (show; `tvdb.rs:70`, only
with a key, DEC-447), IGDB (video game; `igdb.rs:76`), Podcast Index (podcast;
`podcastindex.rs:77`). The other kinds name connectors that do not exist (`anilist`,
`openlibrary`, `hardcover`, `musicbrainz`, `metron`, `vndb`, `bgg`: `screen.rs:134`,
`read.rs:45,82,120`, `listen.rs:44,114`, `play.rs:90,139`).

There is no manual item entry: the REST API has no route that creates a catalogue item
(`crates/api/src/routes.rs`: `/items/{id}` is read-only, `POST /library` takes an existing
item). The only other ways an item comes into being are an import row
(`crates/importers/src/runner.rs:471-486`, a title-only stub) and the watched webhook
(movies and episodes, `crates/integrations/src/lib.rs:233-258`).

Importers in the build: our own format as JSON Lines or CSV, the legacy Ryot export, Trakt
JSON files (`crates/importers/src/lib.rs:299-307`). Goodreads, BG Stats and BGG are not
built (waiting for sample exports, OWN-3).

| Kind | Find and add (source) | Tracking model | Plays / sessions | Import | Stats |
|---|---|---|---|---|---|
| Movie | yes: TMDB | watched, counts every time (DEC-537) | "Watched with..." people | Ryot, Trakt, generic, webhook | count and hours (runtime, `tmdb.rs:294`) |
| Show | yes: TMDB; TheTVDB with a key | episodes, bulk marks, rewatch runs (DEC-587, 590) | "Watched with..." people | Ryot, Trakt, generic, webhook | episodes; hours only when the source gives a series runtime (`tmdb.rs:326`), else "runtimes not known yet" (DEC-600) |
| Video game | yes: IGDB | sessions | yes: duration, players, scores, winner, co-op with no result, Plays tab, win rate (DEC-595..606) | Ryot (status only, `legacy.rs:17,271`), generic | plays, hours, win rate |
| Podcast | yes: Podcast Index | episodic, but the source gives an episode count only, no episode list (`podcastindex.rs:78,183`) | - | generic only (Ryot `podcast` rows are row errors, `legacy.rs:111-117,322-327`) | episodes; no time |
| Anime | NO source (AniList terms, DEC-108) | episodic, no episode list | - | Ryot with a MAL or AniList id (`legacy.rs:113,127-128`): title-only stub, episodes land in season 1 (`legacy.rs:258-264`); generic | episodes; no time |
| Book | NO source | page field exists (`crates/web/src/log_sheet.rs:1194`), no page total | - | Ryot (status only), generic | pages if logged by hand; nothing from a Ryot import |
| Manga | NO source | page / chapter / volume | - | generic only | pages |
| Comic | NO source (owner: not now, WP-OD9) | pages | - | generic only | pages |
| Audiobook | NO source | position field exists (`log_sheet.rs:1261`), no total length | - | generic only | minutes moved forward |
| Music | NO source | position | - | generic only | minutes moved forward |
| Visual novel | NO source | sessions | play dialog exists | generic only | plays, hours |
| Board game | NO source (no BGG connector, DEC-530; hub or Wikidata later) | sessions | play dialog, scores, groups exist; nothing to attach them to | generic only | plays, hours, win rate |

Notes:

- "generic only" means: a person writes a JSON Lines or CSV file in our format with the
  kind and a title; the row becomes a title-only item without cover, description or
  lengths, because rows of our own format need no provider id
  (`crates/importers/src/lib.rs:309-315`, `runner.rs:441-461`). This works for every kind
  by code reading and is the one way to get a book or a board game into an instance today.
  It is a workaround, not a feature a v1 user would find.
- Anime that TMDB lists can be tracked today as kind "show". The kind "anime" itself cannot
  be searched.
- A Ryot row of kind anime or book creates a stub with its Open Library, Google Books, MAL
  or AniList id and queues a refresh that no connector can answer, so the item stays
  title-only. The owner's Ryot import "worked beautifully" (owner-test-2026-11-10.md); how
  the anime and book rows of that file look in the library was not looked at here.
- Legacy views without a date are logged on the day the item entered Completed (DEC-355):
  stats show a peak on the old tracker's migration day. "Date unknown" on events is W7,
  unscheduled.
- Show hours: the stats multiply episodes by one series-level runtime
  (`crates/store-pg/src/repos/stats_cache.rs:265-270`). Runtimes per episode are stored
  from TMDB and TheTVDB (`tmdb.rs:377`, `tvdb.rs:455`) but not read by the stats (DEC-450
  item 1, package ot2g).
- The owner's own words on board games (2026-11-11): "not usable yet". That holds.

## 2. Known open bugs and gaps

Everything marked "noticed, not built" or deferred in the round-2 progress files, the
package reviews and the DEC entries they point to. None of it is a crash or a data loss in
normal use. Grouped by what a v1 user feels.

### 2.1 High: a v1 user hits it and it matters

| # | What | Source |
|---|---|---|
| G1 | Eight of twelve kinds cannot be added (section 1). The palette shows Read and Listen scopes that find nothing for books, audiobooks and music | section 1; DEC-106..108, DEC-530 |
| G2 | The export carries no plays: players, scores, winner, co-op result and duration are lost in an export and re-import. The export is not a backup for games | DEC-602; ot2f-progress.md:404 |
| G3 | Show hours are missing for every show whose source gives no series runtime; the tile says "runtimes not known yet". The owner's wish "hours of shows" (OT-17) is half met | DEC-600, DEC-450; ot2g not built |
| G4 | After an upgrade the browser can keep the old app bundle (file names are not hashed, no cache header): pages misbehave until a hard reload | OPS-8; `src/rust/Cargo.toml` has no `hash-files` |
| G5 | The upgrade to ot1b asks for one `cli reindex`; the image has no `cli` and there is no button for it | `docs/modules/ROOT/pages/operations.adoc:43`; `src/docker/Dockerfile:37-38` |

### 2.2 Medium: visible, with a way around

| # | What | Source |
|---|---|---|
| G6 | A logged play cannot be edited, only undone and logged again (a play saved as "Lost together" before OT-18 stays so) | ot2f-progress.md "Noticed" |
| G7 | The item page does not refresh when its entry is changed in another tab or on another device | DEC-569; ot2b-progress.md:281 |
| G8 | A library title that is not among the first 20 catalogue hits has no "In your library" row in the palette, and is missing if no source names it | DEC-615 (waits for the W2 load run) |
| G9 | The History year filter does not use the person's time zone (plays around New Year land in the wrong year) | C4; DEC-601; next-packages.md:607 calls it "a real bug" |
| G10 | "Completions" in stats and goals count only events: a show or book set to Completed by hand is not a completion, a finished rewatch run is | DEC-594 (OT2E-3), needs a ruling on the measure |
| G11 | Covers: palette rows show a cover only when the instance has stored it, so most search results show the kind icon; every other page loads unstored covers straight from the provider's host. The two rules disagree | DEC-608; ot1b-progress.md:100-106 |
| G12 | A first log queued offline leaves the add bar enabled: a second click queues a second log | ot2b-progress.md:285 |
| G13 | Offline "Log N again": with a browser clock behind the server the queued episodes are refused on flush | ot2e-progress.md:207 |
| G14 | "Watched with..." offers "Yesterday" while a rewatch run started today, and refuses it on Save | ot2e-progress.md:211 (owner's call) |
| G15 | Goals still count time in minutes while the stats speak in hours | ot2c-progress.md:108 |
| G16 | A book finished without a page count adds no pages, a play without a duration no time; the footnote does not say so | ot2c-progress.md "Noticed" |
| G17 | The win rate is all-time only, no year filter; a play shared by a linked account counts on the People card but not in the own win rate (explained in the text, DEC-604) | ot2f-progress.md "Noticed"; DEC-604 |
| G18 | No "lowest score wins" setting: golf-like games mark the wrong winner unless corrected by hand | DEC-603 |
| G19 | TheTVDB reads only the first page of a season's episodes; a very long season would be cut short (not seen) | DEC-447 |
| G20 | Web push is not built; the channel type exists and fails with "not available" | DEC-141 |
| G21 | Sync covers library entries only; no desktop shell | DEC-156; REPORT.adoc "Known gaps" |

### 2.3 Low: cosmetic or rare

- `/stats` marks "Home" as the current section in the top bar
  (`crates/web/src/shell.rs:43`; ot2d-progress.md:273, again in ot2f).
- Palette at 390 px: the key hints of the active row lie over a title that wraps
  (ot2d-progress.md:270).
- A podcast's episode rows still say "Watched" / "Mark S1 E1 watched" (DEC-564); the toast
  of a video game session reads "Logged: Play" (ot2a-progress.md:122).
- The bulk question says "as a rewatch" also where the answer logs plain extra views (a
  show that is In progress) (ot2e-progress.md "Noticed").
- A cover that fails to load shows its alt text under the card's button (DEC-586).
- On a phone, content scrolled under the fixed bottom bar is partly covered
  (ot2c-progress.md "Noticed").
- "Add a person" and "New list" still use the side sheet; the duration field wraps under
  the date chips at 390 px (ot2b-progress.md:127).
- The Plays table has no place and notes columns (ot2f-progress.md "Noticed").
- The totals row and a chart can differ by a minute at a rounding edge (DEC-578, OT2C-4).
- The admin Branding page shows two of the colour swatches only (DEC-586).
- A palette row does not get its cover when the cover is stored while the palette is open;
  recent items in the idle list have no cover box (ot1b-progress.md:107).
- With more than 32 events of other searches inside one request an early batch of the
  current search can be lost (two tabs of one account searching) (DEC-510).
- The typo phase of the Postgres search has no title tie-break (ot1b-progress.md "Noticed").
- OT-19 "a stray line left of All-time stats": not reproduced; needs the owner's screenshot
  (DEC-599).
- People: no avatar upload, no fuzzy duplicate suggestions, no restore of a removed person;
  the History person filter misses rows logged with an entry merged into that person
  (DEC-353).
- No "Mark owned", no owned flag (DEC-420; comes with board games).
- `docs/design/ux/design.md` and the mockups still describe the pre-ot2 labels and the side
  sheet (DEC-564); a docs debt, not a user defect.

### 2.4 Not measured

- No release load run since ot2c: the host lacks the 15 GB of free disk (DEC-579, DEC-605,
  DEC-615, DEC-616). The play read over all items is linear in the person's plays (74 ms at
  5 000 plays, DEC-605) and runs on every Stats page. For one household this is fine.
- `/people` has an unmeasured query per row (perf-proposals.md section 3, branch
  `wip/fable-reviews`); W2.

## 3. Security and operations for a release

Two release shapes: (a) one home instance on a LAN behind the owner's own reverse proxy,
members are the household; (b) any instance reachable from the internet. "Built" was
checked against the code on 2026-11-12: none of the packages sec0 to sec4, ops1, ops2,
test1, test2 exists yet; the tags after `ux4` are the owner-test packages only.

### 3.1 Security (security-audit.md on `wip/fable-reviews`, section 5)

| Id | Finding | Owner's answer | Built | Blocks (a) | Blocks (b) |
|---|---|---|---|---|---|
| SEC-1 (medium) | No security headers, no CSP, no cache directives on signed-in responses | (a) all headers and a CSP with nonce in the app, plus proxy docs (DEC-536); package sec1 | no (no header name anywhere in `crates/`, `apps/`) | no (the proxy can set them; the proxy page of OPS-2 should show how) | YES |
| SEC-2 (medium) | Server-function requests have no body limit: an unauthenticated client can post gigabytes to `/_fn/sign_in` | default, not owner-reviewed: 2 MiB limit; package sec0 | no (limits only on the REST router, `crates/api/src/routes.rs:125-129`) | no (household only), but it is one layer and one test: do it | YES |
| SEC-3 (medium) | Deleting a user leaves search documents, import files and the name in audit rows | free text: records carry an identifier, never a name; deleting the mapping cuts the link (DEC-533); package sec3 | no | no | no for a family instance; YES as soon as someone outside the household has an account |
| SEC-4 (medium) | Outbound calls to admin-chosen hosts are not confined; the HTTP client follows redirects | (b) private targets allowed in home mode, blocked in hosted mode (DEC-534); package sec2 | no (no redirect policy in `crates/catalog-core/src/http.rs`) | no | no while the owner is the only admin; YES with other admins (hosted mode) |
| SEC-5 (medium) | Sign-in throttle is per name and per process: anyone can lock the admin name, every attempt costs an argon2 run | (b) per-address budget, concurrency cap, progressive delay, counters in Postgres; owner asked whether it is worth it with an identity provider (DEC-535); package sec2 | no | no | YES (unless every sign-in, including the local break-glass path, is closed off at the proxy) |
| SEC-6 (low) | Compose ships default passwords and binds plain HTTP on every interface | (a) required variables, loopback bind, start-up warning (DEC-536); package ops1 | no (`src/docker/compose.yml:14,16,22,41,53`) | no | YES |
| SEC-7 (low) | An import file is parsed into memory as a whole (64 MiB cap per upload) | default: stream or cap; unplaced | no | no | no |
| SEC-8 (low) | Build tools are downloaded without checksums | default; with the Dockerfile work (ops2) | no | no | no (before public images) |
| SEC-9 (low) | Storage error text goes back to webhook senders | default; package sec0 | no (`crates/api/src/hooks.rs:48-50`) | no | no |
| SEC-10 (info) | Sessions slide without a hard cap; no session list, no "sign out everywhere" | (a) both (DEC-536); package sec4 | no (`crates/auth/src/lib.rs:701`) | no | no; first in line after the blockers |
| SEC-11 (info) | SSO hint cookie keeps the login hint for 400 days | default: accepted per addendum-1 4.2 | - | no | no |
| SEC-12 (info) | Requests without `Origin` pass the CSRF guard | default: no fix required | - | no | no |

The audit's "checked and holds" list is long (passwords, sessions, TOTP, OIDC, secrets at
rest, tenant isolation, SSRF guard for notification targets, no dynamic SQL); the findings
above are what is left, not a sample.

### 3.2 Operations (ops-readiness.md on `wip/fable-reviews`, section 9)

| Id | Finding | Owner's answer | Built | Blocks (a) | Blocks (b) |
|---|---|---|---|---|---|
| OPS-1 (high) | The `cli` binary is not in the image: no reindex, no key rotation, no admin reset, no restore step for image users | default: ship it; package ops1 | no (`src/docker/Dockerfile:26-27,37-38`) | YES (the ot1b upgrade note needs `cli reindex`, operations.adoc:43; restore needs `cli reindex --clear`, :62) | YES |
| OPS-2 (high) | `PUBLIC_URL`, `TRUSTED_PROXIES`, `SECURE_COOKIES` are not in `.env.example`; no reverse-proxy page; with the defaults sign-in from another host fails. The owner hit this | default: add the keys, write the page; package ops1 | no (no match for the three names in `config/.env.example`) | YES (shape (a) is exactly "behind a reverse proxy") | YES |
| OPS-3 (medium) | `secrets.key` lives inside the data directory | "something else", empty note: NO RULING (Q1) | no (`Dockerfile:46-47`) | no (the runbook must say: back the key up with the dump) | no |
| OPS-4 (medium) | Nothing tells the admin to back up the generated key | as OPS-3: NO RULING | no | no (same runbook line) | no |
| OPS-5 (medium, forward) | Backfills and non-concurrent indexes run inside start-up | default; a rule for future migrations | - | no | no |
| OPS-6 (medium) | Only rollback is a dump restore; not documented | (a) dump-restore only, documented; log the migrations; dry run (DEC-543) | partly: the rule and two migration notes are in operations.adoc:20-47; the log line and the dry run are not built | no | no |
| OPS-7 (low) | Compose stop grace (10 s) shorter than the job shutdown grace (20 s) | default; package ops1 | no | no | no |
| OPS-8 (medium) | Unhashed `/pkg/app.*` without cache headers: stale app after upgrades | carried by SEC-1 (a) | no | no, but every upgrade shows it (G4); the one-line variant `Cache-Control: no-cache` on `/pkg` belongs in v1 | no |
| OPS-9 (medium) | Backup docs name what, not how; no restore order, no verification | default; package ops1 | partly: "what" is in operations.adoc:49-62; no commands, no restore order, no check | YES (docs only: a tracker's one unrecoverable failure is losing the database or the key) | YES |
| OPS-10 (low) | No `/metrics`, no access log | default; unplaced | no | no | no |
| OPS-11 (medium) | Worker has no health signal; `/healthz` is a constant | default; unplaced | no (`apps/server/src/lib.rs:430`) | no | no |
| OPS-12 (medium) | Two web processes do not share events or throttles | (a) supported shape: one web process plus any number of workers (DEC-552) | docs statement not written | no | no (state the shape in the docs) |
| OPS-I | Publish images | (b) public registry on tags; which registry: Q4 (DEC-543) | no | no | no (a public image is a third shape; its own preconditions: OPS-1, OPS-2, SEC-1, SEC-2, SEC-6, attribution page in the image, baseline squash) |
| OPS-B | Build profile: bundle size against build memory | (a) new Dockerfile structure, measure, then the owner picks (DEC-552) | no | no | no |

### 3.3 Tests and release confidence (test-audit.md on `wip/fable-reviews`)

| Id | Finding | Owner's answer | Built | Blocks (a) | Blocks (b) |
|---|---|---|---|---|---|
| TEST-1 | CI retries a failed browser test once and passes (`src/web/playwright.config.ts:21`); nothing counts how often | (a) a flaky test fails the pipeline (DEC-545); package test1 | no | no: the tag runs are local `cargo xtask e2e` runs without retries (the retry is on only with `CI` set), and the last five report 0 failed and 0 panics (HANDOFF section 0) | no; do it before public images, where the pipeline is the release gate |
| TEST-2 | The browser suite runs in parallel against one server and two shared admin accounts; several specs change instance-wide state (hosting mode, time zone, a source switch) | default, precondition of TEST-1 | no (`playwright.config.ts:20`, one serial project `sso` only) | no (a test-order risk, not a product defect) | no |
| TEST-8 | All imports share one ordered lane; one member's large import delays everyone's | (a) per-user lane (DEC-545); package test2 | no | no (one household) | no (matters with several importing members) |
| TEST-3 | About a dozen tests were never seen red | default; unplaced | partly: the ot packages record mutation runs for their own new tests (DEC-617) | no | no |
| TEST-5 | Nine REST routes have no HTTP-level test | default; unplaced | no | no (the web app reaches the same use-cases) | no |
| TEST-10 | No test runs `--role web` plus `--role worker` as two processes; the `fs` blob store has one path untested | default; unplaced | no | no (compose runs one `--role=all` process) | no |
| - | The image and compose start were never run on the build host (no Docker); only the owner's podman run at home shows it works | owner check OWN-8 "compose healthy within 60 s", promised | owner | part of the v1 acceptance run (section 5) | same |

### 3.4 Count

- **(a) home instance on the LAN: 3 blocking items** - OPS-1, OPS-2, OPS-9. All three are in
  one package (ops1), one is code (a second binary in the image), two are docs and config.
- **(b) reachable from the internet: 7 blocking items** - OPS-1, OPS-2, OPS-9, SEC-1, SEC-2,
  SEC-5, SEC-6. Two more (SEC-3, SEC-4) the moment a person outside the household gets an
  account or an admin role.
- Test findings: 0 blocking for either; TEST-1 and TEST-2 gate public images.

## 4. Open owner questions

None of these blocks the v1 cut of section 5. Each has a default in force.

| # | Question | Default in force |
|---|---|---|
| Q1 | OPS-3+4: "something else" without a note. Move the secrets key out of the data directory, with a banner (a), a wizard step (b), docs only (c), or what else? | Nothing moves. The runbook says where the key is and that it is backed up with the dump (owner-answers 11.6) |
| Q2 | HUB-4: is "everything we do" = every kind, facts and images only, movies and shows last? Bridge or full copy for kinds with an open database? Which kind after board and video games? | That reading (DEC-532); nothing is built before h1 |
| Q3 | SEC-3: when an account is deleted, do other members' people entries keep the display name ("watched with Anna") or show "Deleted member"? | Today's behaviour: the name stays (next-packages.md:367-368) |
| Q4 | OPS-I: which public registry and image name? | No image is published |
| Q5 | HUB-2: confirm "CC BY 4.0 and CC0 only, no revocable licence" | As ticked (DEC-532) |
| Q6 | SEC-5: may the Postgres counters wait until several web processes are supported? | In-memory counters first; the Postgres store is the last, separable step (DEC-535) |
| Q7 | OWN-4: MangaDex metadata, yes or no? | Not used (HANDOFF section 6) |
| Q8 | DATA-R: what do you want from Steam (library and playtime, store metadata, both)? Is a link to ProtonDB enough? | Nothing is built; the terms research (data-r) is docs-only and not started |
| Q9 | HUB-11: engage a lawyer (contributor terms, image terms, DSA scope, hashes of purged TMDB text, title and year after a TMDB purge, movies and shows on a CC0 hub) | Owner action; hub phases h3 to h5 wait |
| Q10 | Hotlinking (DEC-608): may pages load a cover straight from the provider's host before the instance has stored it? | Palette rows: stored covers only. Item page, Library and Home cards: they do load from the provider (G11). One ruling should cover both |
| Q11 | Rewatch stop (DEC-590, orchestrator decision): stopping a rewatch writes a stop marker, the run ends there and later views are plain extra views | As built; a status set by hand without the marker keeps the older behaviour |
| Q12 | Solo plays (DEC-603, orchestrator default): a scored play of one person has no result unless a winner is marked; a "No winner" row can be chosen | As built: solo scores do not count in the win rate |
| Q13 | German wording (DEC-594): "Wiederholung abbrechen" / "abgebrochen" for a stop, "beendet" for a finished pass, "Vorzeitig abgebrochen" in History; these replace the texts approved in ux-owner-test.md 1.3 | As built |
| Q14 | OT-19 stray line beside "All-time stats": a screenshot, please (DEC-599) | Left open |
| Q15 | "Yesterday" chip while a rewatch run started today: hide it? (ot2e-progress.md:211) | Shown, refused on Save |
| Q16 | "Completions" (DEC-594, OT2E-3): should an entry completed by hand count? | Events only |

Standing, not questions: the 53 "proceed with defaults" items of the review page are in
force and not owner-reviewed (owner-answers section 12); the owner's promised deliveries
(sample exports OWN-3, the anime and manga actions OWN-4, the three live checks OWN-8, a
staging instance HUB-13); the funnel review on 2026-12-06 (DEC-552).

## 5. Proposed v1 cut

**v1 = shape (a): one home instance, the four kinds that have a source (movies, shows,
video games, podcasts), Ryot and Trakt import.** Three packages, in this order, then the
owner's acceptance run and the tag.

| # | Package | Content | Size |
|---|---|---|---|
| 1 | v1-ops (ops1 as planned, plus one line) | `cli` in the image (OPS-1); the three first-run keys in `.env.example` and a "Behind a reverse proxy" page with Caddy and nginx snippets, the SSE note and the security headers to set at the proxy (OPS-2, covers SEC-1 for shape (a)); backup, restore and upgrade runbook with commands, restore order and a check, incl. "back up `secrets.key` with the dump" (OPS-9, OPS-3+4 docs part); compose requires its passwords and binds to loopback, stop grace 30 s (SEC-6, OPS-7); "one web process plus workers" stated (OPS-12); `Cache-Control: no-cache` on `/pkg` so an upgrade does not leave a stale app (OPS-8, the cheap variant; hashed names stay with sec1) | M, mostly docs and config |
| 2 | sec0 (as planned) | body limit on server functions (SEC-2), generic error text for webhook senders (SEC-9) | S |
| 3 | v1-polish (new) | the small visible defects in one sweep: `/stats` marks Home (2.3), History year filter in the person's time zone (G9), podcast rows say "Watched" and the "Logged: Play" toast, palette key hints at 390 px, alt text of a broken cover, the bulk question's wording on an In progress show; and a "What v1 covers" docs page that names the kinds without a source and the generic file as the way in (G1) | S-M |
| 4 | Owner acceptance (no build) | upgrade the home instance to the new image, run `cli reindex`, sign in through the proxy, one backup and one restore into an empty database by the runbook, "compose healthy within 60 s" (OWN-8). Then tag `v1` | owner, an evening |

Why this and no more: all three (a) blockers are in package 1; package 2 is a day and
removes the one unauthenticated memory hole; package 3 is what a first-time user would
otherwise report back within the first hour. Nothing here needs a migration, a load run or
an owner answer.

**Explicitly after v1**, in the order of next-packages.md unless noted:

- First after v1, v1.1: **ot2g** (hours per episode, "Up to date" badge; closes G3). It is
  planned behind d3 (TVmaze), but the runtimes per episode are already stored from TMDB and
  TheTVDB (`tmdb.rs:377`, `tvdb.rs:455`), so it can be built on today's data and re-pointed
  later. It needs a migration, a stats rebuild and the load run the host cannot do now,
  which is why it is not in v1. Then **plays in the export** (G2) and editing a play (G6).
- Internet exposure (shape (b)): sec1, sec2 (SEC-5 first), then sec4, sec3. Do not expose an
  instance before sec1 and sec2 are in.
- Public images: test1, W1 (attribution page in the image), base (export round trip, which
  needs G2 fixed first, then the baseline squash), ops2. The squash is not needed for v1:
  the 33 migrations keep working for the home instance.
- The rest of the owner's rulings: prefs, label, test2, arch-q, W2 (with all the moved load
  runs: DEC-605, 615, 616), d1 to d4, arch-b, the hub (h1 brings board games), d5 to d7
  (books, music, visual novels; d5 can move forward if books matter before the hub), W4
  anime and manga, W6 web push, W7 "date unknown", sample-bound importers, Steam.

## 6. What the owner can do now

1. Answer Q1 (one word is enough: a, b or c) and Q10 (hotlinking yes or no); both change
   what package 1 and a later cover fix write.
2. Say whether "v1 = four kinds with a source" matches what was meant by "the core items".
   If books or board games are meant to be in v1, the cut grows by d5 (M) or h1 (L, after
   arch-b, d1, d2) and v1 moves out by weeks.
3. Send the OT-19 screenshot.

## 7. Not verified

- Nothing was built, started or clicked. Section 1 is from reading the connectors, the
  importers, the kinds and the web crate; the generic-file path for kinds without a source,
  the look of Ryot-imported anime and book rows, and how the item page of a podcast lists
  episodes (the source gives a count only) were not exercised.
- "Built: no" in section 3 rests on greps of the working tree on 2026-11-12 and the tag
  list; the round-1 findings' own `file:line` references were re-read only where a current
  line is cited here.
- The statement that local e2e runs use no retries rests on `playwright.config.ts:21` and
  on finding no `CI` variable set in `src/rust/xtask/src`; the GitLab job does set `CI=1`
  (`.gitlab-ci.yml:145`).
- perf-proposals.md, hub-design.md, architecture-review.md and open-data-sources.md were
  read through the owner review page and the answers file, not line by line.
- `docs/DECISIONS.md` was read by entry (DEC-141, 156, 353, 355, 420, 447, 450, 510, 564,
  569, 578, 586, 590, 594, 599, 601 to 605, 608, 615) and by a search for "deferred" and
  "not built"; older deferrals whose packages were built since may be listed in the log and
  not here.
- Sizes are estimates in the scale of next-packages.md (S up to a day, M a few days, L a
  week or more).
