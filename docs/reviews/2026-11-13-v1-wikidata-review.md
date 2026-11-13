# Review: v1-wikidata (board games from Wikidata), 2026-11-13

Reviewed: worktree `kithkeep-lane-books`, branch `lane/v1-wikidata` (cut from main at
5661b34), uncommitted: `git diff HEAD` plus the untracked files (connector, tests, fixtures,
mock, two specs, three docs). Read-only review: nothing was built or run (no cargo, npm,
Playwright); the statements about test results are those of the lane's progress log.

Inputs: v1-plan.md 4b and 3.1.1, docs/connectors/wikidata.md, LB-23..33, the progress log,
DEC-169, DEC-192, DEC-195, DEC-530, DEC-611, DEC-641, DEC-642.

Live checks by the reviewer (2026-11-13, `www.wikidata.org`, `www.mediawiki.org`,
`foundation.wikimedia.org`; about 50 GET requests in series with a generic client name and
no contact, never the owner's): see "Verified" at the end. Answers are kept in the session
scratchpad only, nothing was added to the repository.

Severity: high = wrong for a person in ordinary use; medium = wrong in a realistic case, or
a claim of the package that is not true; low = small, cosmetic or hardening.

## Summary

| Severity | Count | Findings |
|---|---|---|
| high | 1 | WD-1 |
| medium | 4 | WD-2, WD-3, WD-4, WD-5 |
| low | 8 | WD-6 .. WD-13 |

Must fix before merge: WD-1, WD-2 (at least the documentation half), WD-3, WD-5.
May defer: WD-4 (soon), WD-6 .. WD-13.

The connector itself is careful work: endpoints, parameters, batching, error objects,
`maxage`, the `Required` contact gate, the host allow-list, no image and no request to
BoardGameGeek are as the plan and the policy pages ask, and the documentation's quotes of
Wikimedia's policy pages match the live pages.

## Must fix before merge

### WD-1 (high) The class list misses whole families of common games

`src/rust/crates/connectors/src/wikidata.rs:41-73` (`CLASSES`),
`docs/connectors/wikidata.md` "The class list".

Problem: the list was built from the subclass tree of board game plus tabletop and card game
classes. Games that Wikidata files under sibling classes are not found at all. Measured
live: of the items that carry a BoardGameGeek id (`haswbstatement:P2339`, 5251 items) the
filter keeps 4852 (92.4 %); 399 are lost. The P31 of the first 50 lost items:

| Class | Label (wbgetentities, en) | In the sample of 50 | Examples lost |
|---|---|---|---|
| Q299191 | chess variant | 15 | shogi (Q131375), janggi, makruk, Chess960, bughouse chess |
| Q1515156 | dice game | 7 | Yahtzee (Q696920), Zombie Dice, Dudo, Mia, craps |
| Q573573 | abstract strategy game | 6 | TwixT, Arimaa, Connect6, Dots and Boxes, tic-tac-toe |
| Q1368898 | game of skill | 3 | Carrom, Simon, Ice Cool |
| Q1509934 | children's game | 2 | Ice Cool, Galloping Pigs |
| Q14947863 | parlour game | 1 | The Werewolves of Miller's Hollow |
| Q1188693 | (label not fetched) | 1 | go (Q11413) |

(The rest of the sample is what the list leaves out on purpose: collectible card games,
tabletop role-playing games, miniatures, a book.)

Failure scenario: a person types "Yahtzee" or "Kniffel" in the Play scope: 0 hits (both
checked live with the filter as built), although Q696920 exists with players and a BGG id.
The same for go, shogi, TwixT, Werewolves of Miller's Hollow. The person concludes the
source does not know the game and adds it by hand without data.

The doc's own statement "the text of the two queries was not kept" also means the list
cannot be regenerated "with a release" as the code comment asks.

Suggested fix: add at least Q1515156, Q573573, Q299191, Q1509934, Q14947863 (and look at
Q1368898 and at go's class Q1188693); count each with `haswbstatement:P31=<class>`
(`totalhits`) and record the count and date in the doc table; extend the test that pins the
list. Record the regeneration method in the doc: the two searches
`haswbstatement:P2339` and `haswbstatement:P2339 haswbstatement:<filter>` give the recall
in two requests, and `haswbstatement:P2339 -haswbstatement:<filter>` lists what is lost.
The filter with 31 classes is 409 characters and a 200 character text in front of it was
accepted live, so a few more classes fit; re-check the longest case after the change.
Noise stays acceptable: "Poker" already gives 30 card game items by LB-23's choice; dice
game will add craps and the like in the same way.

### WD-2 (medium) The member's language never reaches the connector; an item without an English label cannot be found

`src/rust/crates/catalog-core/src/search.rs:165` and
`src/rust/crates/catalog-core/src/refresh.rs:44` (both pass `language: None`; no caller in
the workspace passes anything else), `src/rust/crates/connectors/src/wikidata.rs:275-291,
366-376, 654, 680`; `docs/connectors/wikidata.md` (User-Agent row, "Endpoints used",
mapping rows title / original title / description), LB-28.

Problem: the connector, its tests and its doc describe "the label in the member's
language, else en, else mul, else any". In the running application the language is always
`None`, so every request says `languages=en|mul`. Consequences:

1. A German member never gets the German label or description; `original_title` is never
   set (the title already is the English label). The plan's mapping row "title: label in
   the user's language" is not met, and the doc says it is.
2. The "else any label" fallback in `label()` is dead: the API returns only the languages
   asked, so an item without an `en` or `mul` label arrives without any label and the
   search hit is dropped (`snapshot(.., false)`). Seen live: Q17142058 (has a BGG id, no
   English label). The research doc says only about a third of the items have German
   labels, so the opposite case (German-only small-press games) is the realistic one for
   this owner: typed in German, matched by the full-text search, then silently dropped.
3. The tests that prove the fallback (`the_label_falls_back_from_the_user_language_to_
   english`, the e2e fixture recorded with `languages=de|en|mul`) pass `Some("de")`, which
   production never does.

Failure scenario: a member with the German interface searches "Die Siedler von Catan": the
row reads "The Settlers of Catan"; a game whose item has only a German label is found by
Wikidata's search and then left out, with no word about it.

Suggested fix: before merge, make the documents true (wikidata.md, LB-28: "English, else
`mul`; the member's language is not passed by the search fan-out or the refresh yet") and
decide the language of a shared catalogue item on purpose (the item is shared between
members, so "the member's language" cannot be the answer for stored titles; the instance
language is the candidate). For the dropped hits, ask a fixed wider set for labels (for
example `<instance language>|en|mul|de`) so that "else any" has something to choose from,
with a test that uses `language: None`. The wiring of a language through `SearchRequest`
is platform work and may be its own package, but the false statement and the silent drop
should not be merged as they are.

### WD-3 (medium) The BGG id and the GTINs are stored as matching keys: two different Wikidata items that share one are merged

`src/rust/crates/connectors/src/wikidata.rs:589-597` (external ids `bgg`, `gtin`),
`src/rust/crates/app/src/catalog.rs:312-319, 385-411` (`existing_item` matches on any
external id; `Fill` adds the new ids to the item found),
`src/rust/crates/store-pg/migrations/0001_core.sql:73-79` (primary key
`(kind, namespace, value)`).

Problem: DEC-195 orders the keys (QID first; GTIN is edition-level; BGG id "with the known
Wikidata duplicate ids handled by preferring the item with a board game class and
sitelinks"; expansions never merged into their base). As built, every `bgg` and `gtin`
value is an equal matching key, first come first served. Nothing in the diff handles the
duplicates DEC-195 names. (Five well-known BGG ids probed live had one item each, so the
case is not common, but DEC-195 records that it exists, and GTINs are per edition by
definition.)

Failure scenario: search "X" returns the base game (Q1) and an expansion or a second
edition (Q2) whose item carries the same P2339 or one of the same P3962 codes. The member
adds Q1; a click on the Q2 row then resolves to the Q1 item (`existing_item` finds it by
`bgg`), Q1 silently gains the external id `wikidata=Q2`, the item page shows two "Wikidata"
links, the refresh reads only the first, and Q2 can never be an item of its own. Plays are
then logged on the wrong game; undoing it needs database work.

Suggested fix (test first, in `crates/app/tests`): an item found through a secondary key
is not the same item when it already holds a different id in the new snapshot's own source
namespace (`wikidata=Q1` vs `wikidata=Q2`); in that case the new item is created and the
shared secondary id stays with the first. Alternatively, until that rule exists, keep
`bgg_id` in the attributes only (build the link from it) and do not store `gtin` ids:
nothing reads the `gtin` namespace yet. Record the choice next to DEC-195.

### WD-5 (medium) The new e2e chain has not run as a whole; `sso` was not run on this branch at all

`src/web/playwright.config.ts:33-57`, progress log (FINAL whole run).

Problem: the projects are now `setup -> chromium -> contact -> sso`. Playwright skips a
project when one it depends on has a failure. Before this change a failure in `chromium`
already hid `sso`; that is unchanged. New: (a) a failure in `contact.spec.ts` now hides
`signin.spec.ts`; (b) the lane's final run was `--project=contact` and stopped at the known
timeout in sharing.spec ("promote, accept, see history, unlink, merge and undo the merge",
LB-22), so `contact` did not run in sequence (only alone, after a setup run) and `sso` did
not run at all on the final build. The CI job runs `npx playwright test` (every project,
`.gitlab-ci.yml:145`), so with that sharing test red, neither `contact` nor `sso` has ever
been seen in CI order with this change.

Failure scenario: contact.spec restores the contact in `finally`, but a state it leaves
behind (or the snoozed nudge, or the changed contact text) breaks a sign-in test, or the
other way round the `chromium` leftovers break contact.spec (it asserts the exact nudge
text and that the Wikidata row is the one that waits); nobody sees it until the first
green `chromium` run on main.

Suggested fix: before merge, one run of `setup + chromium + contact + sso` on the final
build with the sharing test green or excluded by name for that run
(`--grep-invert "promote, accept"`), and the result in the progress log. Keep the chain
(running `contact` beside `sso` would race on instance settings). Fix the sharing timeout
in its own package: while it is red it hides two projects, not one.

## May defer

### WD-4 (medium) A `maxlag` or `ratelimited` answer does not pause the source

`src/rust/crates/connectors/src/wikidata.rs:333-354` (`check_error`),
`src/rust/crates/catalog-core/src/http.rs:392, 506` (the pause is set from `Retry-After`
on 429 and 503 only), `src/rust/crates/catalog-core/src/refresh.rs:46-50`.

Problem: the error arrives as HTTP 200 with `Retry-After: 5`. The connector turns it into
`HttpError::RateLimited { 5 s }` for its caller, but the governed client has already
counted the answer as a success: no pause, no breaker. The refresh job logs "details
failed" and goes on to the next item at once.

Failure scenario: the replicas lag for ten minutes while the weekly refresh works through
300 board games: every request carries `maxlag=5`, every one is refused, the job keeps
sending two a second, every item is marked failed and retried after an hour. That is the
opposite of what Manual:Maxlag_parameter and API:Etiquette ask ("wait", "increase the time
between subsequent requests"); with `ratelimited` it prolongs the limit.

Suggested fix: let the connector hand the wait back to the governed client (a
`pause(connector, duration)` on the HTTP port, called from `check_error`'s two codes), or
let the governed client treat a 200 answer of a source that opts in as rate limited when it
carries `Retry-After`. Test: after a `maxlag` answer the next background call waits.
Deferable because the requests are cheap for the source and the case is rare, but it should
be done before an import of a large collection exists.

### WD-6 (low) No compressed transfer

`src/rust/Cargo.toml:60` (reqwest without `gzip`), `wikidata.rs:269` (`headers()`).
API:Etiquette (quoted in the doc's rate limit row) asks for `Accept-Encoding: gzip`; the
client does not send it. Measured live for a search of 20 hits ("chess"): 237 kB plain,
47 kB with gzip. Fix: enable reqwest's `gzip` feature for the governed client (every
connector gains), or note in the doc that it is not done. Platform change, may defer.

### WD-7 (low) The contact is read twice per request

`src/rust/crates/catalog-core/src/http.rs:571` (the `Required` guard) and `:409`
(`contact_agent`). If the admin clears the contact between the two reads, the request goes
out with the plain application User-Agent: no contact leaks, but it is a request to
Wikimedia without contact information, which the policy forbids. Code of v1-books, but
Wikidata is the first `Required` source. Fix: read the contact once and pass the header
value down; fail closed when it is gone.

### WD-8 (low) boardgames.spec reads the whole request log of the mock

`src/web/tests/boardgames.spec.ts:54-59`. `asked.length > 0` and the per-request checks
read every request since the mock started. With `reuseExistingServer` (local runs) and a
kept database the two answers are in the HTTP cache for a day and a restarted mock has an
empty log: the test fails for a reason that is not a defect. The check `/\(.+@.+\)/` also
does not say which contact. Fix: note the log length before the search and expect it to
grow, or search a second title only this spec uses; compare with `E2E_CONTACT` or
`books@example.org` (books.spec sets that one for a while).

### WD-9 (low) Background and interactive calls never share a cached answer

`wikidata.rs:157-161`. `maxlag=5` is part of the URL and so of the cache key: an item
opened a minute ago is fetched again by the refresh job, and the other way round. Harmless
(two requests), but the doc's "kept a day" is per lane. Either note it, or strip `maxlag`
from the cache key in the governed client when that hook exists.

### WD-10 (low) The Home nudge cannot be answered with "no"

`src/rust/crates/web/src/source_hints.rs:199-214`, `nudge.rs:76-85`. An admin who decides
not to give a contact (the consent the `Required` mode is about) sees "Wikidata is not
asked until you set the operator contact" on Home again every seven days; the only way out
is to switch the source off on Admin > Data sources, which the text does not say. Fix: say
it in the hint of the contact field or in the nudge ("or switch Wikidata off"), en and de.
Not verified by a run that a disabled Wikidata row ends the nudge; the unit test at
`source_hints.rs:309` suggests `disabled` is not counted as failing.

### WD-11 (low) Wording

`locales/de.ftl` (`game-fact-players`, `game-players*`): "Personen: 3-4 Personen" says the
word twice; "Spieler*innen" is avoided well, but a label such as "Mitspielende" or the
value without the noun under that label would read better. `locales/en.ftl`: "Age: from 10
years" is fine. No missing key in either file (the lane reports the `locales` test green).

### WD-12 (low) Stale `bgg` names in test data

`src/rust/crates/web/src/setup_state.rs:157`, `source_hints.rs:309`, `palette.rs:1571,
1588` use a source called `bgg` in unit test data. Nothing depends on the old default
(`kinds/src/builtin/play.rs:139` and `kinds/tests/builtins.rs:71` are changed; the `bgg`
import format in `imports.rs:32` is a different thing; the `bgg` namespace of the external
id is the one the importers match on). No action needed; rename when the files are touched.

### WD-13 (low) Tests written after the code

By the lane's own log: `the_registry_asks_wikidata_for_board_games_once_the_contact_is_set`
(connectors) was not seen red; `boardgames.spec.ts`, `contact.spec.ts` and the mock were
written after the implementation and seen only green. The 20 first connector tests, the
injection test, the kinds test and the five web tests were seen red. Acceptable; say so in
the merge note. To see contact.spec red once: run it with the guard at `http.rs:571`
commented out locally (the request count must then differ).

## Merge with main and with v1-manual

Main has three commits after the cut (docs only); no file of this lane is touched by them.

v1-manual (worktree `kithkeep-lane-polish`, uncommitted) touches four files this lane also
changes; the hunks do not overlap today:

| File | This lane | v1-manual | Risk |
|---|---|---|---|
| `web/locales/en.ftl`, `de.ftl` | about lines 301, 1219, 1409, 1430 | about 2049, 2216 | none textual; run the `locales` test after the merge |
| `web/src/item.rs` | `About` 1056, `Attribution` 1186 | `overflow` 592 | none textual |
| `web/src/palette.rs` | import line 23, `SourceFooter` 1103 | 137, 769-1027, tests 1625 | none textual; both add to the palette footer and status texts: look at the open palette once after the merge |
| `docs/modules/ROOT/pages/v1-scope.adoc` | hunks at 36 and 48 | hunks at 8, 56, 59, 71 | six lines apart: may conflict; the page must then say both (board games from Wikidata, and by hand for what it does not have) |

Behaviour after both are merged, whichever order:

- `manual.spec.ts` searches the Play scope and waits for "the last row of an answered
  search". On the v1-manual branch Play has no source; after the merge Play asks the
  Wikidata mock and needs the contact that this lane's `global.setup.ts` sets. It should
  pass (the mock answers 0 hits for an unknown title), but it has not been run that way:
  run manual.spec, boardgames.spec, sources.spec and contact.spec together after the
  second merge.
- LB-33 is open until then: add "the by hand row still works" to boardgames.spec (a game
  the mock does not have) and to contact.spec (without the contact the palette must still
  offer the by-hand row; today it asserts only the no-results card).
- The nudge order (LB-31: contact before "group without a source") was chosen without the
  by-hand path; no change needed, but the nudge text of v1-manual and this one should be
  read side by side once.

## Verified (no finding)

- Rate limits page (mediawiki.org, raw text): "new in 2026 and are subject to
  experimentation and change"; per minute; unidentified 10; "Unauthenticated bot requests
  with a compliant User-Agent header" 200; "3 or fewer" concurrent; 429 or 503 with
  `Retry-After`. As quoted in the doc. The reviewer's own generic client was answered 429
  (a plain text body, not JSON) after about ten requests in a minute, which confirms the
  10 and shows why the contact is not optional. The governed client classifies 429 by
  status, so the non-JSON body is not parsed.
- User-Agent policy (foundation.wikimedia.org, raw text): the quotes in the doc, the
  example `CoolBot/0.0 (https://example.org/coolbot/; coolbot@example.org) ...` and the
  403 sentence are there. `<ident>/<version> (<contact>)` fits the stated generic format.
- API (`action=paraminfo`): `wbgetentities` `ids` limit 50 (500 with higher rights);
  the connector asks at most 20 for a search and truncates linked ids to 50
  (`wikidata.rs:531`). `maxlag`, `maxage`, `smaxage` are main-module parameters. A success
  answer with `maxage=86400&smaxage=86400` came back with
  `cache-control: s-maxage=86400, max-age=86400, public`.
- The class filter: free text plus `haswbstatement:` with 31 values joined by `|` works
  (Catan, Pandemic 17 hits, Gloomhaven 2, Scrabble 5, Codenames 3, Dixit 4, Uno 9, Jenga 1,
  Hive 3, Chess 51, Terraforming Mars 2, 7 Wonders 13, Risk 10, Monopoly 26). Noise is
  small: "Minecraft" 1 hit (a board game of that name), "Dungeons Dragons" 8, "Poker" 30
  (card game class, LB-23's choice). A text of 200 characters before the 409 character
  filter was accepted.
- Search syntax (LB-24): `plain_words` removes `:` and `"`, leading `-` and `!`, the
  operators and `&&`/`||`. Two more candidates were tried live and are harmless: a
  backslash before the space in front of the filter (`obama\ haswbstatement:...`: still 0
  hits, the filter holds) and a full-width colon (`haswbstatement<U+FF1A>P31=Q5`: read as
  text, 1 hit). `*`, `?` and `~` only change how words match inside the filter. Ids are
  checked (`Q` plus 1 to 12 digits) before they go into a request, the language tag is
  letters only, so `|` cannot be injected into `ids` or `languages`.
- Mapping against live data: P1872/P1873 arrive with unit `1`; P2899 with Q24564698 and
  with Q577 (both seen live, both accepted); P2047 in Q7727; P2339 as a string of digits.
  Nothing is fetched from boardgamegeek.com: the id becomes an attribute, an external id
  and a plain link (`item_words.rs:488-506`, digits only, 12 at most); the e2e spec checks
  that the browser asks no Wikimedia host. P18 is not read; the fixtures hold file names
  only, no image, no personal address, no User-Agent of the recording.
- Contact: `ContactUse::Required` (`wikidata.rs:148`); the registry reports
  `needs_contact` and `for_kind` leaves the source out; the governed client refuses the
  request itself without a contact (`http.rs:570-576`); TMDB and the others never get the
  header (`contact_agent`). Tests and e2e use `games@example.org`, `e2e@example.org`,
  `books@example.org`; `KK_OPERATOR_CONTACT` is forced empty for the e2e server; the mock
  records the User-Agent and the specs read it.
- Pace: 2 per second, burst 2, one at a time; `shared_pace` is on for every keyless source
  (`http.rs:96, 396`), so all processes share one turn. A search is two requests, a details
  call two: within the burst.
- Attribution: text, URL and licence line as the doc says; the outbound links are plain
  `<a>` elements with visible names and an underline (colour is not the only cue); the
  footer row names the kinds in words. en and de are complete for the new keys.
- Functions over 100 lines: none added or grown. `SearchPalette` (palette.rs:903, 101
  lines) and `ItemPage` (item.rs:73, 116 lines) were over before and are not touched.
- Cited decisions exist: DEC-169, DEC-192, DEC-195, DEC-530, DEC-611, DEC-641, DEC-642.
