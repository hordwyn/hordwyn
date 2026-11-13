# Connector: Wikidata (`wikidata`) - implemented (v1-wikidata), board games only

Date checked: 2026-11-13 (terms, policy pages, rate limits and API behaviour as read and
answered live on that day; see "Verified live"). Earlier record of the facts:
docs/fable/round2/attribution-and-connectors.md (A1, B1), read 2026-11-10.

MediaWiki and Wikibase, the software behind the API, are GPL software. Only the public API
documentation, the policy pages listed below and the answers of the public API were read
(DEC-169); no code, schema or UI text of that software was opened or copied.

| Item | Value |
|---|---|
| Kinds | board_game (DEC-641: board games in v1 come from Wikidata, not BoardGameGeek, DEC-530). Expansions are items of their own (DEC-195) |
| Capabilities | search, details. No images (DEC-642): P18 points to Wikimedia Commons, whose files carry their own licences and authors; it is never read |
| Code | `src/rust/crates/connectors/src/wikidata.rs`, cargo feature `wikidata` (part of `connectors/all` and `server/all-connectors`); no key. `connectors.wikidata.base_url` moves it to another host (the e2e mock, a mirror) |
| API docs | Action API: https://www.wikidata.org/w/api.php (module help: `?action=help&modules=wbgetentities`, `?action=help&modules=query%2Bsearch`); search keywords of Wikidata's search: https://www.mediawiki.org/wiki/Help:Extension:WikibaseCirrusSearch (`haswbstatement`, OR with `|`); maxlag: https://www.mediawiki.org/wiki/Manual:Maxlag_parameter |
| Terms and policy | https://www.wikidata.org/wiki/Wikidata:Data_access (best practices, attribution request); https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy ; https://www.mediawiki.org/wiki/API:Etiquette ; https://www.mediawiki.org/wiki/Wikimedia_APIs/Rate_limits . All re-read 2026-11-13 |
| Licence | https://www.wikidata.org/wiki/Wikidata:Licensing : "All structured data (i.e. the main, Property, Lexeme, and EntitySchema namespaces) is released into the public domain under Creative Commons Zero." Licence tag CC0: the fields may later be contributed to a CC0 hub (DEC-192) |
| Attribution | Not required. Data access page: "We offer the data in Wikidata freely and with no requirement for attribution under CC-0. In return, we would greatly appreciate it if, in your project, you mention Wikidata as the origin of your data." We give the credit as a courtesy: "Data from Wikidata (CC0)." (one of the page's own examples, "Data from Wikidata") with a link to https://www.wikidata.org/ on the item page of every item that came from it and on the attribution page; licence line "CC0 1.0 (structured data); the credit is a courtesy". The page also asks: "Please offer your users a way to report issues in the data": the item page links the item on wikidata.org ("Elsewhere: Wikidata"), where anybody can correct it |
| Auth | none |
| User-Agent | Required by Wikimedia's policy: "Scripts should use an informative User-Agent string with contact information, or they may be blocked without notice"; "The contact information should be given as an email address, a website, or a wiki user"; example `CoolBot/0.0 (https://example.org/coolbot/; coolbot@example.org) generic-library/0.0`; "Do not use generic agents such as "curl", "lwp", "Python-urllib""; the code examples section: a missing, empty or generic agent "will fail with an HTTP 403 error". Ours: `<brand ident>/<version> (<operator contact>)`, with the instance setting `instance.operator_contact` (Admin > Data sources, or `KK_OPERATOR_CONTACT`). The connector declares `ContactUse::Required`: **without a contact it is not asked at all**; the sources page and the search footer show "Operator contact missing" and the admin's Home names it. Every board game search sends the words typed to wikidata.org with the operator's contact in the header; the hint of the contact field says so |
| Rate limits | Rate limits page ("new in 2026 and are subject to experimentation and change"), per minute, "across all sites and platforms, including requests to the Action API": unidentified (IP only) 10; "Unauthenticated bot requests with a compliant User-Agent header" 200; at most "3 or fewer" concurrent requests; 429 (or 503 when a backend is overloaded) "typically also have the Retry-After header set"; without it "wait at least five seconds, or implement exponential back-off". API:Etiquette: "Making your requests in series rather than in parallel [...] should result in a safe request rate"; ids joined with `|`; `Accept-Encoding: gzip`; on the error code `ratelimited` "increase the time between subsequent requests". Policy as built: 2 requests per second, burst 2, one at a time (120 a minute at most, under the 200); `Retry-After` on 429 and 503 is honoured by the governed client, and so is a `maxlag`/`ratelimited` error in an HTTP 200 answer (see maxlag); every request asks for `Accept-Encoding: gzip` (the governed client decodes the answer before it is parsed or kept; a 20-hit search is about 47 kB instead of 237 kB, LB-38); every process of the instance takes its turn from one pace in the database (`source_pace`, every connector without a key, v1-books LB-19) |
| maxlag | API:Etiquette: "If your task is not interactive, i.e. a user is not waiting for the result, you should use the maxlag parameter." Sent as `maxlag=5` on every call of the background lanes (refresh, import, prefetch), never on an interactive one. A lagged answer is HTTP 200 with `{"error": {"code": "maxlag", ...}}` and `Retry-After: 5` (Manual:Maxlag_parameter; recorded live): the connector reports it as rate limited and asks the governed client to pause the source for the answer's `Retry-After`, else for the `lag` the answer names (rounded up), else five seconds; `ratelimited` the same way. The next request to the source then waits for the pause, or fails at once without being sent when the pause is longer than the 10 s the client waits (LB-37) |
| Caching | Every request carries the API's own `maxage=86400&smaxage=86400`: a successful answer then says `cache-control: s-maxage=86400, max-age=86400, public` and the governed client keeps it a day; an error answer says `private, must-revalidate, max-age=0` and is never kept (both recorded). Connector default 24 hours. Item values are refreshed after seven days (the refresh job). `maxlag=5` is part of the URL and so of the cache key: an answer fetched for a person and one fetched by the refresh job are kept apart, so one item may be asked twice a day (LB-42) |
| Hosts | `www.wikidata.org` only. Not used: the SPARQL query service, dumps, Wikimedia Commons, the REST and GraphQL APIs, any write |

## Endpoints used

| Op | Request |
|---|---|
| search | `GET https://www.wikidata.org/w/api.php?action=query&list=search&srnamespace=0&srlimit=<1..20>&srprop=&srsearch=<words> haswbstatement:P31=Q131436\|P31=Q60474521\|...&format=json&formatversion=2&maxage=86400&smaxage=86400[&maxlag=5]`: the full-text search (labels and aliases of every language) limited to items that are an instance (P31) of one of the classes below. `srprop=` (empty) leaves out snippets and sizes; the answer is the list of item ids in rank order. No hit: no further request |
| hits | `GET https://www.wikidata.org/w/api.php?action=wbgetentities&ids=Q1\|Q2\|...&props=labels\|descriptions\|claims\|sitelinks&languages=<lang>\|en\|mul\|<more>&format=json&maxage=86400&smaxage=86400[&maxlag=5]`: all hits of a search in one request (up to 20 of the 50 ids the module allows) |
| details | the same `wbgetentities` for the one id, then `wbgetentities&ids=...&props=labels&languages=<lang>\|en\|mul\|<more>` for the labels of its genres and mechanics (up to 50 ids; none asked when it has none) |

The typed words are sent as plain words: whitespace is collapsed, at most 200 characters,
and the search syntax of the source is taken out (`:` and `"` become spaces, a leading `-`
or `!` is dropped, `OR`, `AND`, `NOT` are lowercased, `&&` and `||` dropped), so that what a
person types cannot widen or replace the class filter (LB-24). A query of nothing but such
characters asks nothing. An id is put into a request only when it is `Q` and 1 to 12 digits
without a leading zero; anything else is "not found" without a request. `<lang>` is the
primary subtag (`de` of `de-DE`) of the language the request is made in: for a search the
member's chosen language, else the instance language (the brand's `default_locale`); for a
refresh the instance language, because the catalogue item is shared by every member and the
refresh is what keeps it (LB-35). `mul` is Wikidata's code for labels valid in every
language. `<more>` is a fixed wider set, `de|fr|es|it|nl|pl|pt|sv|da|nb|fi|cs|hu|ru|uk|ja|zh|ko`
(each language once), so that an item without an `en` or `mul` label still has a label to
show: the API answers only the languages asked.

The fallback the plan kept ready (`wbsearchentities`, a label prefix search per language,
filtered on P31 afterwards) is **not built**: the class-filtered full-text search works as
written, with free text and several classes joined by `|` (LB-23), and it finds the short
titles the fallback would miss ("Root", "Azul", "Carcassonne" on 2026-11-13).

## The class list (2026-11-13)

`CLASSES` in the connector, 39 classes. A constant, because resolving the class tree per
search would need SPARQL; regenerate it with a release. The first 31 were produced at
development time, once, from the query service (the only use of SPARQL, by hand, not by the
application): the subclass tree of board game (`?c wdt:P279* wd:Q131436`) with the number
of items that are an instance of each class (`?i wdt:P31 ?c`), then the same for the
classes board games are also filed under (tabletop game, card game and their subclasses);
two classes were counted with the search itself (`haswbstatement:P31=<class>`,
`totalhits`). The text of the two queries was not kept; the counts are those of the
answers. That tree missed whole families of games filed under sibling classes (review
WD-1): the last eight rows were added after measuring what the filter loses.

**Regeneration and coverage (the method, LB-34).** Items that carry a BoardGameGeek id are
the yardstick of "a game someone plays at a table". With the Action API alone, serial
requests and a generic User-Agent (unidentified clients get 10 a minute: 7 s apart):

1. `list=search&srsearch=haswbstatement:P2339&srlimit=1` -> `searchinfo.totalhits` (all);
2. `srsearch=haswbstatement:P2339 haswbstatement:<filter>` -> kept;
3. `srsearch=haswbstatement:P2339 -haswbstatement:<filter>&srlimit=500` -> the items lost;
   `wbgetentities&props=labels|claims&languages=en` in batches of 50 gives their P31, and
   counting those classes shows what to add (then count each with
   `haswbstatement:P31=<class>` and look at a sample of its items for noise).

`<filter>` is the list joined as in a search, `P31=Q131436|P31=Q60474521|...`. Measured
2026-11-13: 5251 items with a BGG id; the 31 classes kept 4852 (92.4 %); the 39 keep 5155
(**98.2 %**). Of the 96 still lost: collectible card game 30, game of skill 11, "game"
(Q11410) 7, no P31 at all 9, tabletop role-playing game and supplements 6, miniature games
6, and single strays (three-player chess, janggi variant, sequential game, ...). The filter
is 513 characters; a search with 200 typed characters in front of it was accepted (the
longest the connector sends).

| Class | Label | Items (2026-11-13) |
|---|---|---|
| Q131436 | board game | 4404 |
| Q60474521 | board game expansion | 147 |
| Q3244175 | tabletop game | 383 |
| Q142714 | card game | 1665 |
| Q3177859 | dedicated deck card game | 52 |
| Q15804899 | deck-building game | 15 |
| Q1191150 | trick-taking game | 66 |
| Q1150710 | strategy game | 36 (search total) |
| Q1272194 | tile-based game | 56 (search total) |
| Q4931403 | board wargame | 209 |
| Q1783817 | cooperative board game | 57 |
| Q985834 | shogi variant | 42 |
| Q267989 | mancala | 35 |
| Q18703581 | game on cell board | 25 |
| Q5188437 | cross and circle game | 23 |
| Q2796140 | variants of draughts | 14 |
| Q1275314 | go variant | 8 |
| Q21608615 | economic simulation board game | 7 |
| Q788553 | German-style board game | 6 |
| Q1182245 | deduction game | 5 |
| Q10924689 | four-in-a-row | 5 |
| Q10499647 | quiz board game | 4 |
| Q17144589 | train game | 4 |
| Q1099522 | race game | 3 |
| Q1548611 | Game of the Goose | 3 |
| Q1637382 | Hounds and Jackals | 3 |
| Q31977010 | Gonu | 3 |
| Q4927217 | block wargame | 2 |
| Q48997832 | legacy game | 1 |
| Q66152389 | historical board game | 1 |
| Q104714331 | bluffing game | 1 |
| Q1515156 | dice game (Yahtzee, Zombie Dice, Dudo) | 160 (search total) |
| Q573573 | abstract strategy game (TwixT, Arimaa, Connect6) | 83 (search total) |
| Q299191 | chess variant (shogi, janggi, Chess960) | 163 (search total) |
| Q1509934 | children's game | 350 (search total) |
| Q14947863 | parlour game (Werewolves of Miller's Hollow) | 128 (search total) |
| Q839864 | party game (Codenames: Pictures, Just One, Time's Up!) | 92 (search total) |
| Q5161688 | connection game (Y, Gonnect, Onyx) | 13 (search total) |
| Q133889197 | xiangqi variant | 10 (search total) |

Children's game and party game bring some games that are not played at a table (musical
chairs, karaoke, wink murder); they show only when the typed words match, as with card game
(LB-23). Left out on purpose after the measurement: game of skill (Q1368898, 108: Carrom and
KLASK, but mostly darts, laser tag, knife throwing, egg-and-spoon race) and mind sport
(Q1188693, the class of go: chess, bridge, e-sports).

Left out on purpose: classes whose items are not games one plays at a table, or are not
single games: board game franchise (Q28114062, 59), castle (Q12669521, 12, misfiled items),
fantasy sport (Q5434359, Q2625781), chess playing software (Q133935013), board video game
(Q19272838), chess video game (Q71679871), computer shogi (Q5157583); tabletop role-playing
game (Q1643932, 1720), collectible card game (Q734698, 134), Magic: The Gathering set
(Q22815098, 151), miniature wargaming and miniatures games (Q532716, Q62570118, Q5146062),
paper-and-pencil game (Q2347716), solitaire (Q2056928), Skat variant (Q11298749), tarot card
game (Q510406), ace-ten game (Q56281803); game (Q11410, 1309: too broad); and the classes
with one or two items that are themselves one game or a stray (shogi Q131375, go Q11413,
Verquere Q2642425, fantasy wargame Q5434360, m,n,k-game Q6711932, Q6983400). Labels as given by
`wbgetentities` in English on 2026-11-13.

## Mapping

Statements: an "unknown value" or "no value" statement and a deprecated one are skipped;
of the rest, the preferred ones when there are any, else the normal ones ("best rank"). A
single value is the first of the best rank; lists (genres, mechanics, GTINs) take every
statement that is not deprecated.

| Ours | Wikidata |
|---|---|
| external id `wikidata` | the QID: the only catalogue matching key of this source (LB-36) |
| title | label in the request's language (`<lang>` above), else `en`, else `mul`, else the first of the wider set. A search hit without any of these labels is left out; the details of a known item without one show the QID |
| original title | the `en` label when it differs from the title |
| description | the item's description in the request's language, else `en` (a short phrase, CC0) |
| year | P577 (publication date), the earliest of the best rank, year precision or finer (a decade is no year); years before the common era are negative |
| `players_min`, `players_max` | P1872, P1873 (quantities without a unit) |
| `playtime_min_minutes`, `playtime_max_minutes` | P2047 in minutes (unit Q7727) or hours times 60 (Q25235), rounded; the lower and upper bound when the statement has them, else the amount; several statements give the lowest and the highest; any other unit (seconds, days, none) is skipped; zero is no value |
| `min_age` | P2899 in years (unit Q24564698 "years old", or Q577 "year"); a value in months is skipped |
| `mechanics`, `categories` | the labels of the items of P4151 (game mechanics) and P136 (genre), details only; an item without a label in the languages asked is left out, never shown as its id |
| `bgg_id` | P2339 (BoardGameGeek game ID) when it is a number: a reference identifier for the outbound link, nothing is fetched from BoardGameGeek. Not an external id: Wikidata has items that share a BGG id (DEC-195), and an external id would merge them into one catalogue item (LB-36) |
| `gtins` | P3962 (GTIN), up to five codes of 8 to 14 digits: reference identifiers, not matching keys (a GTIN is per edition) |
| popularity | the number of sitelinks (a tie-break between rows, DEC-611) |
| `weight`, `players_best` | none: BoardGameGeek community figures, not in Wikidata |
| image | none (DEC-642); P18 is not read |

Designers, artists and publishers (P287, P170, P12969, P110, P123) are not stored: the kind
models people as catalogue relations, and there is no store for them yet. An expansion is
an item of its own and never merged with its base (DEC-195); the relation (P8646) is not
stored in v1.

Coverage is what the source has: about 4,700 board game items, players on about 2,200,
playing time on about 800, German labels on about a third (docs/research/board-game-
sources.md 2.1). Many searches find nothing, or a row with a title and a year only. Wikidata
is an open wiki: a wrong value arrives as it is, and is corrected on Wikidata (the item page
links there).

## Errors

The API answers errors with HTTP 200 and an `error` object: `maxlag` and `ratelimited` ->
rate limited, and the source is paused (`Retry-After`, else the `lag`, else 5 s; LB-37); `no-such-entity` -> not found; any other code -> a parse error with the
code. An entity that does not exist inside a batch is `{"id": ..., "missing": ""}` and is
left out. An empty map of labels or descriptions may arrive as `[]` and is read as empty.

## Verified live (2026-11-13)

Requests to `www.wikidata.org` from the build host with the generic client name
`api-docs-check/0.1 (generic test client)` (no contact, never the owner's), serial, at most
one per second:

- `list=search` with free text plus `haswbstatement:P31=Q131436|P31=Q60474521|...` (the 31
  classes): Catan 49 hits (first Q17271), "Die Siedler von Catan" 26, Wingspan 1
  (Q65784798), "Zug um Zug" 3, Dominion 8, Carcassonne 18 (first Q17262), Azul 7 (first
  Q44367843), Root 1 (Q101242941), "zzqqxx nope" 0.
- `wbgetentities` for Q17271 (de|en|mul), the three Catan hits, Q65784798 (en|mul), the
  labels of Q788553 and Q142714, Q99999999999 (`no-such-entity` error with HTTP 200), an
  unknown language code (`languages=zz`: a warning, the rest answered), the labels of the
  classes in the table.
- `maxlag=-1` (the documented way to provoke the error): HTTP 200, error code `maxlag`,
  `Retry-After: 5`, `cache-control: private, must-revalidate, max-age=0`.
- With `maxage=86400&smaxage=86400`: `cache-control: s-maxage=86400, max-age=86400, public`.
- The policy pages listed in the table, as wikitext (`?action=raw`).
- Review WD-1 (same client, requests 7 s apart): the coverage measurement of "The class
  list" (about 20 requests), the class counts of the eight added classes, a 200 character
  text before the 39-class filter (accepted, 0 hits), and "Kniffel" with the filter (finds
  Yahtzee, Q696920).

Fixtures: `src/rust/crates/connectors/fixtures/wikidata/` (9 recorded answers, data CC0;
the claims of the three entity answers are cut to the properties the connector reads plus
P18 and P8646, references removed; the others untrimmed). The unit tests that need other
shapes (ranks, units, bounds, missing labels, error objects) build entity JSON by hand in
the recorded shape, in `tests/wikidata.rs`. The e2e mock `src/web/tests/wikidata-mock.mjs`
serves the same fixtures. The request URLs of the fixtures were rewritten on 2026-11-13
(review fixes) for the 39-class filter and the wider `languages` set; their bodies are as
recorded (asked with `de|en|mul` or `en|mul` and the 31 classes): the API answers only the
languages asked, so the recorded labels are a subset of what the new request returns; the
hit lists of the three searches stay as recorded with the 31 classes.
