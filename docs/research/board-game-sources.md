# Research: board game metadata sources beyond BoardGameGeek

Date checked: 2026-11-06 (every URL below was opened on that date unless a line says
otherwise). Written in our own words; no source code of any project was read or copied.
Licence and terms were checked before anything else for every source. Facts that could not be
verified from this build host are marked `UNVERIFIED`. BGG itself was not contacted
(boardgamegeek.com blocks this host, DEC-107); everything about BGG comes from
`docs/connectors/bgg.md` and DEC-190.

Question from the owner: is there a nicer option than BGG, ideally one that lets this app
replace BGG for them altogether?

Short answer: there is no open database that replaces BGG's coverage. There is one open
source worth building on (Wikidata, CC0) and it covers roughly 5,000 games with usable
fields, against BGG's six figures. The realistic design is an own catalogue that is seeded
from Wikidata where possible, filled by the users where not, and that treats BGG as an
optional, separately stored import aid for the user's own collection and plays. Details and
the trade-off are in sections 5 and 6.

## 1. Comparison table

Coverage numbers for Wikidata are SPARQL counts from 2026-11-06 (section 2.1).
"Keys" = can a self-hosted instance use it with its own per-instance credentials.

| Source | Status | API | Data licence | Images | Coverage of our fields | German | Keys | Verdict |
|---|---|---|---|---|---|---|---|---|
| Wikidata | alive, Wikimedia Foundation | Action API, REST API v1, SPARQL, Linked Data, dumps; no key needed | CC0 (data), attribution asked for but not required | via Commons (P18), per-file licence, 607 of 4,748 board games | players min/max ~2,200 items, year ~1,800, publisher ~1,400, min age ~1,000, play time ~800, mechanics ~200, designer ~400 (three properties), GTIN 533; expansions weak | de label on 1,667 of 4,907 BGG-linked items; de search works | yes (none needed; User-Agent required) | **primary seed** |
| BoardGameGeek XML API2 | alive (owner-supplied texts) | XML, per-instance bearer token, >= 5 s between calls | non-commercial, revocable, no modification, ToS 6.D(iv) no competing/displacing use | not covered by the licence | everything we need, > 100,000 games (figure from a third-party dataset description) | partial (alternate names) | yes, per instance, approval needed | **optional import aid only**, never merged |
| Board Game Atlas | **dead** (DNS for boardgameatlas.com and api.boardgameatlas.com fails; closed Oct 2023 per a client app's blog) | gone | n/a | n/a | n/a | n/a | n/a | reject |
| Ludopedia (BR) | alive | LudoAPI v0.0.5, "ALPHA", OAuth 2.0 authorization code, app registration, base `https://ludopedia.com.br/api/v1` | **not stated** anywhere in the OpenAPI document or the site terms | `thumb` URLs on a Google Cloud bucket, terms not stated | players, time, age, year, designers, artists, mechanics, categories, themes, expansions (`tp_jogo` b/e), community complexity; no BGG id field seen | none (pt-BR) | yes, per instance (app_id/app_key) | not for v1; Q11 flag at most |
| Tric Trac (FR) | alive, but every page now redirects through a login route; /cgu and /mentions-legales answer 404 | none found | not found | not found | unknown | none | n/a | reject |
| Luding (DE/EN) | alive ("powered by H@ll900") | none found | not stated on the site | n/a | unknown; Wikidata links 1,180 items to it (P3528) | yes (German-first) | n/a | link-out / cross-reference id only |
| Spiel des Jahres database | alive (Spiel des Jahres e.V., Kerpen) | none found | not stated (only a copyright footer) | n/a | award winners and nominees only; Wikidata P11025 on 374 items | yes | n/a | link-out / cross-reference id only |
| brettspiele.digital (spielbox's "Spieledatenbank") | alive; Institut fuer Ludologie + Residenzschloss Altenburg archive project | none found | not stated | n/a | archive catalogue, not assessed | yes | n/a | reject for data; watch |
| Board Game Oracle | alive, US only | none (third-party scrapers on Apify only) | not read (Terms page not fetched) | n/a | prices, BGG-derived lists | none | n/a | reject |
| BoardGamePrices (UK/EU) | alive; answers this host with a bot challenge (HTTP 202, empty body) | none found (`UNVERIFIED`) | not read | n/a | prices | unknown | n/a | reject |
| Kaggle / IEEE DataPort BGG dumps | alive | CSV download | uploader declares CC BY 4.0; the data is scraped from BGG (Feb 2021) and the uploader cannot relicense it | none | 20k ranked games, no expansions detail, frozen in 2021 | none | n/a | reject (BGG terms, stale) |
| OpenTabletop (tabletop-commons) | specification only, repos pushed 2026-03 and 2026-07, no stars | none (demo API is a sample server, no data) | Apache-2.0 code, CC-BY-4.0 spec | n/a | schema, not data | n/a | n/a | watch; possible vocabulary for our export |
| Tesera (RU) | alive (HTTP 200) | not assessed | not assessed | n/a | not assessed | none | n/a | out of scope |
| Wikimedia Commons | alive | Action API (`imageinfo`, `extmetadata`), direct file URLs | per file: CC0, CC BY, CC BY-SA, PD; NC/ND never | this is the image source | ~936 files in Category:Board games plus 418 named subcategories; mostly photos of play, components, old games; box art is normally not allowed | n/a | yes (User-Agent) | **image source where P18 exists** |
| Publisher press kits (Kosmos, Pegasus, Asmodee) | exist per publisher | none | press/review use with credit; "clarify rights before further use" (Pegasus boilerplate); no general licence | the images we would want | n/a | yes | n/a | reject as a bulk source |

## 2. Sources in detail

### 2.1 Wikidata

Licence. "Structured data" in the main, Property, Lexeme and EntitySchema namespaces is
released under CC0 (https://www.wikidata.org/wiki/Wikidata:Licensing, page last edited
2026-09-24). Text in other namespaces (talk, help) is CC BY-SA 4.0 and we never copy it.
CC0 needs no attribution; the data access page asks for credit anyway and we give it on
the attribution page and on item pages ("Data from Wikidata, CC0").

Contributions. The Wikimedia Terms of Use (https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use)
say contributors waive sui generis database rights and that "facts you contribute to the
projects may be reused freely without attribution"; imported text must be compatibly
licensed and copyright infringement is forbidden. For Wikidata the project-specific licence
page above sets CC0 for the structured data. Consequence for section 4: anything a user
pushes to Wikidata through us becomes CC0 facts; BGG-derived fields must never be pushed
(BGG forbids modification and non-commercial use only; CC0 would strip that).

Access methods (https://www.wikidata.org/wiki/Wikidata:Data_access):

- Linked Data Interface `https://www.wikidata.org/wiki/Special:EntityData/Q17271.json`
  for a known entity ("Use the Linked Data Interface when you need to obtain individual,
  complete entities that are already known to you").
- Action API `https://www.wikidata.org/w/api.php`: `wbsearchentities` (label search in a
  language; a `language=de` search for "Die Siedler von Catan" returns Q17271 first,
  verified), `wbgetentities` (up to 50 ids per call, verified by the docs page).
- Wikibase REST API `https://www.wikidata.org/w/rest.php/wikibase/v1` (OpenAPI document
  version 1.5 fetched at `.../wikibase/v1/openapi.json`): `GET /entities/items/{id}`,
  `/labels`, `/descriptions`, `/statements`, `GET /search/items?q=&language=&limit=&offset=`,
  `POST /entities/items` (create), `PATCH` on items and statements. Edit bodies carry
  `tags`, `bot` (default false) and `comment`. Covered by the Stable Interface Policy
  (https://www.wikidata.org/wiki/Wikidata:Stable_Interface_Policy: breaking changes announced
  "generally ... four weeks before, but no less than two weeks before").
- SPARQL `https://query.wikidata.org/sparql`
  (https://www.mediawiki.org/wiki/Wikidata_Query_Service/User_Manual): 60 s per query,
  "One client (user agent + IP) is allowed 60 seconds of processing time each 60 seconds",
  30 error queries per minute, 5 parallel queries per IP, HTTP 429 with `Retry-After`.
  Board games stay on the main graph after the 2025-05-09 graph split
  (https://www.wikidata.org/wiki/Wikidata:SPARQL_query_service/WDQS_graph_split).
- Dumps (https://www.wikidata.org/wiki/Wikidata:Database_download): weekly JSON, one
  entity per line, `latest-all.json.gz` / `.bz2` at
  https://dumps.wikimedia.org/wikidatawiki/entities/ (the page quotes 130.53 GiB for the
  2024-01-01 gz). Far too big for a 4 GB host; not used. A filtered subset (only items
  with P31 in the board game tree) is feasible as a project-side, periodically published
  seed file later, but it is not needed for v1: on-demand lookups suffice.

Client rules we must implement (https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy,
https://www.mediawiki.org/wiki/API:Etiquette): a descriptive `User-Agent` with contact
("Set an informative User-Agent string with contact information, or you may be IP-blocked
without notice"; generic agents like `curl` get HTTP 403), requests in series not in
parallel, `maxlag` on non-interactive calls, `Accept-Encoding: gzip`, caching, backoff on
`ratelimited` and 429. The REST API answers HTTP 400 `missing-user-agent` without one.
Brand rule 9: the UA string takes the brand name and the instance URL from config, never a
hard-coded product name.

Properties (labels verified through `wbgetentities` and `wbsearchentities`, type=property):

| Field in DESIGN.md 9a | Wikidata property |
|---|---|
| players min / max | P1872 minimum number of players / P1873 maximum number of players |
| play time | P2047 duration (quantity with unit, e.g. Q7727 minute) |
| min age | P2899 minimum age |
| designers | P287 designed by, P170 creator, P12969 game designer (all three are in use) |
| artists | P110 illustrator |
| publishers | P123 publisher |
| year | P577 publication date |
| mechanics | P4151 game mechanics |
| categories | P136 genre |
| expansion_of | P8646 expansion of; item class Q60474521 board game expansion |
| series / editions | P179 part of the series, P747 has edition / P629 edition of |
| BGG id | P2339 BoardGameGeek ID (formatter `https://boardgamegeek.com/boardgame/$1`); also P6160 publisher, P3505 designer, P8675 family |
| barcode | P3962 Global Trade Item Number |
| image | P18 image (Commons file name) |
| other ids | P3528 Luding game ID, P11025 Spiel des Jahres ID, P14915 Tric Trac game ID, P12970 Tabletopia game ID |

The property set matches the one recommended by Wikidata:WikiProject Board Games
(https://www.wikidata.org/wiki/Wikidata:WikiProject_Board_Games, last edited 2026-06-14),
which also prescribes the expansion model (P8646 plus instance of Q60474521) and links
quality queries such as "board games with conflicting BoardGameGeekID's".

Coverage, measured by SPARQL on 2026-11-06:

| Measure | Count |
|---|---|
| items with a BGG id (P2339) | 4,907 |
| items that are board games (P31/P279* Q131436) | 4,748 |
| of the BGG-linked items: players min / max | 2,247 / 2,227 |
| publication date | 1,784 |
| publisher | 1,394 |
| min age | 1,018 |
| play time | 813 |
| genre | 720 |
| image (P18) | 670 |
| GTIN | 533 |
| creator / designed by / game designer | 245 / 117 / 53 |
| mechanics | 217 |
| illustrator | 41 |
| Luding id / Spiel des Jahres id / Tric Trac id | 1,157 / 368 / 87 |
| German label | 1,667 of 4,907 BGG-linked; 1,231 of 4,748 board games |
| BGG ids used on more than one item (dirty data) | 18 |
| items that are expansions (any P8646) | 7,374 across all game types, but only 114 of them carry a BGG id; only 136 BGG-linked items are instance of "board game expansion" |

Reading: Wikidata is good for the well-known games (Catan Q17271 has de label, 42
sitelinks, players, 75 min, publisher, GTIN, image, BGG id 13, Luding, SdJ and Tric Trac
ids), thin for designers, mechanics and artists, and almost empty for expansions. The
long tail of a hobbyist's shelf will not be there. Weight (complexity) does not exist as a
property at all; it is a BGG community statistic and stays BGG-only.

Risks: data quality (18 duplicate BGG ids), vandalism (an unprotected wiki), unstable
"best" values when a property has several statements (take the preferred rank, else the
first normal-rank statement), WDQS throttling per client, and the usual "not guaranteed to
be stable interfaces" caveat from the data access page.

### 2.2 BoardGameGeek (recorded for the trade-off only)

From `docs/connectors/bgg.md` and DEC-190: per-instance non-commercial token, data must
not be modified, licence revocable at any time, "Powered by BGG" credit, images not
covered. The owner also quotes ToS 6.D(iv): no API use that competes with or displaces
BGG. An app whose stated goal is to replace BGG must therefore not build its catalogue out
of BGG API responses. What stays safe: the user's own submissions (their collection and
plays, ToS 5.D), the user-downloaded collection export that needs no token, and a read-only,
separately stored, purgeable enrichment view (section 6).

### 2.3 Board Game Atlas

Dead. `boardgameatlas.com` and `api.boardgameatlas.com` do not resolve from this host
(2026-11-06). A client app's blog (https://www.sofahq.com/blog/board-games-being-removed-from-sofa,
posted 2023-08-29, updated 2023-09-14) says the company "has sadly gone out of business"
with a cutoff of 2023-10-01. No official notice was found. The OpenTabletop spec cites the
same shutdown as its motivation. Lesson for us: a single commercial API as the only
source is a liability; the catalogue must survive its source.

### 2.4 Ludopedia (Brazil)

Alive. API documentation at https://www.ludopedia.com.br/api/documentacao.html (a ReDoc
page; the underlying spec is https://ludopedia.com.br/api/openapi.yaml, fetched, version
0.0.5, contact api@ludopedia.com.br). The intro says the API "se encontra em estagio ALPHA".
Auth: register an application at https://ludopedia.com.br/aplicativos (app_id, app_key,
owner access_token), OAuth 2.0 authorization code at `https://ludopedia.com.br/oauth` and
`https://ludopedia.com.br/tokenrequest`, bearer token on every call. Endpoints: `/jogos`,
`/jogos/{id_jogo}`, `/series`, `/lancamentos`, `/mecanicas`, `/categorias`, `/colecao`,
`/partidas`, `/usuarios`, `/me`, store endpoints. Game fields: `nm_jogo`, `thumb`,
`tp_jogo` (b base, e expansion), `ano_publicacao`, `ano_nacional`, `qt_jogadores_min/max`,
`vl_tempo_jogo`, `idade_minima`, `mecanicas`, `categorias`, `temas`, `artistas`,
`designers`, `comunidade` (complexity 1-5, best and recommended player counts). Page size
max 200. No rate limit, no data licence and no attribution rule appear in the spec; the
site terms (https://ludopedia.com.br/termo_aceite) are marketplace/user terms and say
nothing about API data reuse. Note: the documentation URL answers HTTP 403 to the generic
fetcher but 200 to curl with a descriptive User-Agent.

Verdict: technically the nicest third-party API found, but Portuguese-only, Brazil-centric,
alpha, and without a stated data or image licence. Under Q11 it could only ship behind a
flag that is off by default, and it brings nothing for a German user. Not for v1.

### 2.5 Tric Trac (France)

Alive as a community site (meta description: "le site de reference sur les jeux de
societe"), but `https://www.trictrac.net/` and every game URL now redirect to a
`login-silent` route (a JavaScript app), `/cgu` and `/mentions-legales` answer 404, and a
web search finds no API; the only integration found is a 2009 Tellico search script. Wikidata
created P14915 (formatter `https://trictrac.net/jeu-de-societe/$1`, 101 items). Reject;
keep the id as an outbound link if Wikidata supplies it.

### 2.6 Luding, Spiel des Jahres, brettspiele.digital (German sources)

- Luding (https://luding.org/): German/English game database, "powered by H@ll900". No
  API, export, licence or imprint link was found on the start page (`UNVERIFIED` whether
  an imprint exists deeper). Wikidata P3528 links 1,180 items (formatter
  `https://luding.org/Skripte/GameData.py/DEgameid/$1`). Use: outbound link only.
- Spiel des Jahres (https://www.spiel-des-jahres.de/, database under `/spiele/`): operator
  Spiel des Jahres e.V., Kerpen; the imprint has no reuse terms, only a copyright footer.
  No API. Wikidata P11025 links 374 items (formatter
  `https://www.spiel-des-jahres.de/spiele/$1/`). Use: outbound link and award facts via
  Wikidata P166.
- brettspiele.digital (linked from spielbox.de as "Spieledatenbank"): a cooperation of the
  Institut fuer Ludologie (Berlin) and the Residenzschloss Altenburg, "ein Portal zur
  webbasierten Datenbank analoger Brettspiele aus mehreren deutschen Sammlungen und
  Archiven". No API or terms found. Reject for data; interesting as a future German
  identifier if Wikidata adds a property.

### 2.7 Price sites

Board Game Oracle (https://www.boardgameoracle.com/): US-only price comparison, shows
"BGG Hotness" and "BGG Top 100" lists (so it is itself a BGG data consumer), no API
offered; its Terms page was not fetched. BoardGamePrices (https://boardgameprices.co.uk/):
answers this host with a challenge page (HTTP 202, empty), no API found; `UNVERIFIED`.
The only "APIs" a search finds are paid third-party scrapers on Apify. Reject: no metadata
value, no terms we could record, and scraping is not something we do.

### 2.8 Open datasets of BGG data

Kaggle "Board Games" by andrewmvd (https://www.kaggle.com/datasets/andrewmvd/board-games,
Kaggle API metadata: licence "Attribution 4.0 International (CC BY 4.0)", last updated
2021-10-05, subtitle "Data on 20k Board Games scraped from BoardGamesGeek", collected
February 2021) and the IEEE DataPort "BoardGameGeek Dataset on Board Games"
(https://ieee-dataport.org/open-access/boardgamegeek-dataset-board-games, CC BY, collected
February 2021). Both are BGG scrapes relabelled CC BY by their uploaders. The uploaders do
not own the data; BGG's licence (non-commercial, no modification, revocable) is the only
one that could apply and it does not allow relicensing. Shipping or importing these would
put BGG data into our catalogue by the back door, which DEC-190 rules out, and the data is
frozen in 2021 with no expansions detail and no images. Reject.

### 2.9 OpenTabletop (tabletop-commons)

https://tabletop-commons.github.io/OpenTabletop/ and the GitHub organisation
`tabletop-commons` (repos OpenTabletop, Apache-2.0, last push 2026-07-20; ott-demo-api,
no licence file, last push 2026-03-18; both 0 stars). A specification (OpenAPI 3.2 document,
CC-BY-4.0) modelled on MusicBrainz: games, typed relationships (expands, reimplements,
contains, requires), taxonomy, people, collections, plays, external identifiers. It
explicitly says it ships "schemas and sample data rather than a running server". No
populated database exists. Watch it; when we publish our own JSON schema (Q10) its
vocabulary for game types and relationship kinds is a reasonable thing to align with, but
only after checking that nothing is copied beyond field names.

## 3. Our own catalogue on open data

Model (no change to the D-9a field list; this is about provenance):

- Every board game item in our catalogue is ours: editable by users, exportable under
  our own schema, free of licence strings. Fields carry a `source` marker per value
  (`wikidata`, `user`, `import:<format>`), the way DEC-190 already requires separate
  storage for BGG values.
- Seeding: when a user searches, the app queries Wikidata (`wbsearchentities` in the
  user's language with English fallback, filtered to the board game class tree, or the
  REST `/search/items`), shows candidates with year, publisher and players, and on pick
  fetches the entity once (`Special:EntityData/Qnnn.json`), maps the properties from the
  table in 2.1, resolves linked items (publisher, designer, mechanics labels in en/de with
  one `wbgetentities` batch of up to 50 ids), stores the result as our item with the QID,
  and caches the raw entity JSON under the storage budget (D-8) so a refresh never needs
  the network.
- Missing games: "Create game" form with the same fields, pre-filled from whatever the
  user typed (title, year, players). Local items have no QID until matched later.
- Refresh: a background job may re-fetch Wikidata-sourced values that the user has not
  overridden (the per-value source marker decides); user values always win.
- Identifiers: QID, BGG id (as an identifier only; comes from Wikidata P2339 or is typed
  by the user; used for outbound links, for import matching and nothing else), GTIN(s),
  Luding/SdJ/Tric Trac ids when Wikidata has them.

Matching and dedup, in order of confidence:

1. Wikidata QID equal -> same item.
2. GTIN/EAN equal (P3962; barcode scan on the phone -> SPARQL
   `?item wdt:P3962 "<gtin>"`, or Action API search with `haswbstatement:P3962=<gtin>`
   (`UNVERIFIED` that this CirrusSearch keyword is enabled on Wikidata; SPARQL is the
   verified path)). GTIN identifies an edition, so two GTINs may map to one game; store
   GTINs as a list on the game, not as the key.
3. BGG id equal -> same item (watch the 18 known Wikidata duplicates; prefer the item
   that is instance of a board game class and has sitelinks).
4. Normalised title plus year plus publisher -> candidate, confirmed by the user.
5. Expansions: an item with P8646 or a user-set `expansion_of` is never merged into its
   base game, even when titles overlap.

Import mapping: BGG collection/plays exports and BG Stats files are matched by BGG id
first, then by title+year; unmatched rows create local items flagged "from import" with
only the user's own values (the user's rating, plays, ownership), never with BGG's
catalogue fields.

## 4. Contributing back to Wikidata

What the rules allow:

- Anyone may edit: the `*` (anonymous) group on Wikidata has `edit`, `createpage` and
  `item-term` rights (`meta=siteinfo&siprop=usergroups`, 2026-11-06); anonymous edits are
  rate-limited to 8 per 60 s per IP (`meta=userinfo&uiprop=ratelimits`) and are recorded
  under the IP address. We never edit anonymously from the server: that would expose the
  instance's IP and would be an unattributed mass editor.
- Notability (https://www.wikidata.org/wiki/Wikidata:Notability): an item is fine if it
  has a sitelink, or "refers to an instance of a clearly identifiable conceptual or
  material entity" that "can be described using serious and publicly available
  references", or fills a structural need. A published board game with a publisher, a GTIN
  and an entry in an external database qualifies as a clearly identifiable product; whether
  a BGG page alone counts as a serious reference is not addressed anywhere I could find
  (the WikiProject page has no notability guidance). Practically, items with external-id
  statements (P2339, P3528, P11025) and a publisher are the norm in the project. Prototypes,
  print-and-play files and fan expansions are the grey zone; the flow must not push those.
- Bot policy (https://www.wikidata.org/wiki/Wikidata:Bot_policy): unattended editing needs
  a request at Wikidata:Requests for permissions/Bot, a test run of 50 to 250 edits, the
  bot flag, maxlag, and "Statement-adding bots must add sources". The policy does not
  define the boundary for tools where a human approves each edit (`UNVERIFIED`; nothing
  on the page). Conservative reading: every edit is made by a logged-in human under their
  own account, one at a time, with a source; no server-side account, no batch pushes.
- OAuth (https://www.mediawiki.org/wiki/OAuth/For_Developers and
  https://www.wikidata.org/wiki/Wikidata:REST_API/Authentication): a consumer is registered
  on meta.wikimedia.org and "others will only be able to use it once it is approved by an
  administrator"; the callback URL must match exactly or by prefix; "The application
  secret must be kept secret"; non-confidential clients must use PKCE (S256). OAuth 2.0 is
  recommended. For a self-hosted app this means: either every operator registers their own
  consumer with their own callback URL and waits for approval, or we skip OAuth in v1.
- QuickStatements (https://www.wikidata.org/wiki/Help:QuickStatements): a URL of the form
  `https://quickstatements.toolforge.org/#/v1=<commands>` opens the tool with commands
  pre-filled (tabs as `|`, newlines as `||`, then URL-encoded); `CREATE` and `LAST` create
  an item and address it; sources are `S248` (stated in) / `S813` (retrieved) pairs; the
  user must be logged in through OAuth and autoconfirmed; "Please ensure you do not create
  duplicate items!". Some characters in the URL form "might fail".
- `Special:NewItem?label=...&description=...&lang=de` pre-fills the label, description
  and language inputs (verified by reading the returned HTML on 2026-11-06).

A careful "Suggest to Wikidata" flow for v1 (no server-side edits):

1. Only for local items without a QID whose values all have source `user` or
   `import` and that are not derived from BGG API data (the per-value source marker
   makes this checkable). BGG-sourced fields are never offered.
2. Before suggesting, re-run the match (title in en/de, GTIN, BGG id) and show any
   candidate; the first job is to avoid a duplicate.
3. Build a QuickStatements v1 batch: `CREATE`, labels in the languages the user filled,
   description "board game (YYYY)" / "Brettspiel (YYYY)", `P31` Q131436 (or Q60474521 plus
   `P8646` for an expansion), `P1872`/`P1873`, `P2047` with unit Q7727, `P2899`, `P577`,
   `P3962`, `P2339` if the user entered it, and a source pair on each statement
   (`S854` reference URL to the publisher's product page, `S813` retrieved date; `S854`
   is P854 "reference URL", label not re-verified in this session). Publisher and designer
   are only included when the user picked an existing Wikidata item in our UI (we search
   for it); free-text names are not pushed.
4. Show the batch as text, explain that the submission becomes CC0 under the user's
   Wikidata account, link to the notability page, and open the QuickStatements URL in the
   user's browser. Fallback link: `Special:NewItem` pre-filled with label, description and
   language for users who prefer the plain form.
5. After the user pastes the new QID (or we poll `wbsearchentities` for the label once a
   day for a week), link the local item to the QID and switch its source markers.

Later (v2): a per-instance OAuth 2.0 consumer (operator-registered, callback = instance
URL) so the app can write through the REST API as the user, with `comment` and a
`tags` value, still one human-approved edit at a time.

## 5. Images under free licences

Wikimedia Commons (https://commons.wikimedia.org/wiki/Commons:Licensing): only free
licences (CC0, CC BY, CC BY-SA, PD); "NC" and "ND" and fair use are refused. Each file has
its own licence tag and "it is the responsibility of reusers" to comply. Reuse guidance
(https://commons.wikimedia.org/wiki/Commons:Reusing_content_outside_Wikimedia): credit the
author named on the file page, name the licence and link to it; hotlinking is possible but
"not recommended". Board game box art: the image casebook
(https://commons.wikimedia.org/wiki/Commons:Image_casebook) says "photographs that are
intended to illustrate the game board and/or the box are not normally acceptable";
derivative works rules (https://commons.wikimedia.org/wiki/Commons:Derivative_works) say a
non-free work "simply cannot be rendered free without the consent of the copyright holder".
So what Commons holds for a modern game is a photo of the game in play or of components,
not the cover. Sample: Catan's P18 is "Nepomuk 280 - Osadnici z Katanu.jpg", CC BY-SA 3.0,
author Matej Batha (a photo of a game in progress); Wingspan's P18 is a dice tower photo.
Category:Board games has 936 files and 51 subcategories, Category:Board games by name has
418 subcategories (`categoryinfo`, 2026-11-06). Coverage: 607 of 4,748 board game items
have P18.

How we use it: when a Wikidata item has P18, fetch `imageinfo` with
`extmetadata` (LicenseShortName, LicenseUrl, Artist, Credit) plus the file URL, download
one sized rendition into our storage (allowed: these licences permit copying and the
storage budget applies), store author, licence, licence URL and the Commons file page URL
next to the asset, and render a credit line on the item page ("Photo: <author>, <licence>,
via Wikimedia Commons" with links). CC BY-SA share-alike concerns derivatives; a resized
copy is still the same work and the credit line satisfies attribution. Files tagged with
non-standard or missing licence metadata are skipped (placeholder instead).

Publisher press kits: Kosmos' blogger guide (PDF at
https://www.kosmos.de/Bilder/Footer/Presse/Blogger/Bloggerleitfaden_KOSMOS_01.pdf, file
dates 2017/2018; its text could not be extracted on this host, the summary comes from a
search snippet, `UNVERIFIED`) lets reviewers use cover images for reviews with Kosmos
credited and title, author and publisher named. Pegasus' press boilerplate says use for
"Eigeninformation und redaktionelle Weiterverarbeitung" is free and asks to clarify
copyright before any further use; Pegasus also states that illustration rights stay with
the illustrators. Asmodee offers per-title press kits with press@asmodee.com as contact;
no general terms found. None of this is a licence for an app that stores and displays
covers for arbitrary users. Reject as a source; an operator may still upload a cover they
have permission for.

User uploads: the design already allows photos on plays (D-9a). A user's own photo of
their box is their upload under their responsibility; we keep such images private to the
instance (not in our export and not in shared link views by default) because a box photo
is a derivative of the cover art.

Default remains the placeholder (thumbhash plus dominant colour, DESIGN.md section 7).
With Commons in place the realistic hit rate for a German hobbyist's shelf is low (hundreds
of popular titles), so the placeholder stays the normal look and must be good.

## 6. The trade-off, stated plainly

Depending on BGG:

- Pro: complete catalogue (six-figure game count, expansions, weight, mechanics, artists,
  alternate names, community player-count polls), stable ids everyone uses, imports match
  trivially.
- Contra: non-commercial only, revocable at any time for any reason, no modification (so
  no merging, no corrections, no own catalogue built on it), credit and logo on every
  instance, per-instance application approval, 5 s between calls, no images, and ToS
  6.D(iv): an app that sets out to displace BGG is exactly the use the terms refuse. A
  catalogue filled from the BGG API would have to be purged when BGG says so, and the
  owner's data would go with it.

An open catalogue (Wikidata seed plus user entries):

- Pro: CC0, no key, no revocation, we may merge, correct, translate and export; German
  labels for the popular third; stable QIDs; cross-reference ids to BGG, Luding, Spiel des
  Jahres; images for a few hundred games with clear credits; contributions flow back
  into a public commons instead of a private company.
- Contra: ~5,000 games with usable fields, expansions nearly absent, no weight, thin
  designer/mechanics data, data quality issues; most of a serious collection must be
  typed in by the user or arrive through a one-time import of the user's own BGG
  collection export; search results will often say "not found, create it".

For the owner's goal ("replace BGG for me"): the app can replace BGG as the place where
the owner's collection, plays, loans and stats live, with no BGG token at all, by
importing the collection export once (no token needed per `docs/connectors/bgg.md`) and
entering new games by hand or from Wikidata. It cannot replace BGG as a reference
database of every game ever published; nothing open can today, and the BGG API cannot be
used to build one. The honest middle: open catalogue by default, BGG as an optional,
off-by-default, separately stored enrichment panel ("what BGG says") for operators who
register their own non-commercial application and accept that it can vanish.

## 7. Recommendation

- Primary: own catalogue, seeded on demand from Wikidata (CC0) with the property map in
  2.1, per-value source markers, user-created items when missing. Credit Wikidata on the
  attribution page and per item.
- Fallback: user entry (and import of the user's own BGG/BG Stats exports matched by BGG
  id and title); no second third-party database. Ludopedia only if the owner ever wants
  pt-BR, behind a Q11 flag; Tric Trac, Luding, Spiel des Jahres, brettspiele.digital as
  outbound links from the ids Wikidata supplies.
- Images: Commons P18 renditions with stored author/licence/URL and a credit line; else
  the placeholder; user-uploaded cover photos private to the instance; no press kits.
- BGG stays as an optional enrichment aid exactly as DEC-190 defines it: off by default,
  per-instance token, read-only values in a separate, purgeable store, shown in their own
  "From BoardGameGeek" panel with the credit, never merged into the catalogue, never
  offered to the "Suggest to Wikidata" flow. Importers of the user's own exports need no
  token and stay on.
- Contributing back: v1 ships the QuickStatements / Special:NewItem link flow for
  user-sourced items only; OAuth writes are a v2 item after the per-instance consumer
  question is settled with the operator docs.

## 8. Draft DECISIONS lines (not applied; numbers to be assigned when merged)

```
DEC-191 | Primary metadata source for board games (owner wants to replace BGG) | Own catalogue seeded on demand from Wikidata (CC0): wbsearchentities / REST search in the user's language, entity fetch via Special:EntityData/Qnnn.json, property map P1872/P1873/P2047/P2899/P577/P123/P287+P170+P12969/P110/P4151/P136/P8646/P2339/P3962/P18; per-value source marker (wikidata, user, import); users create items that are missing; raw entity JSON cached under the storage budget; client sends a descriptive User-Agent from brand config, serial requests, maxlag, gzip, backoff on 429 | BGG XML API as the catalogue (forbidden by its no-modification, non-commercial, revocable terms and ToS 6.D(iv)); Ludopedia (alpha, pt-BR, no stated licence); Kaggle/IEEE BGG dumps (relabelled BGG data) | drop the Wikidata seed and keep user entry only; the per-value source marker makes removal mechanical | docs/research/board-game-sources.md; https://www.wikidata.org/wiki/Wikidata:Licensing; https://www.wikidata.org/wiki/Wikidata:Data_access; https://www.mediawiki.org/wiki/API:Etiquette; SPARQL counts 2026-11-06
DEC-192 | Role of BGG after DEC-191 | Optional enrichment and import aid only: connector off by default, per-instance token, values stored read-only in a separate purgeable store and shown in a "From BoardGameGeek" panel with credit and logo; never merged into catalogue fields, never exported as catalogue data, never pushed to Wikidata; BGG id kept on catalogue items as a plain identifier for links and import matching; importers of the user's own collection/plays exports need no token | merge BGG values into the catalogue; seed the catalogue from BGG | enable the connector per instance; delete the store with the existing admin purge | DEC-190; docs/connectors/bgg.md (owner-supplied terms incl. ToS 6.D(iv)); docs/research/board-game-sources.md section 6
DEC-193 | Board game images | Wikimedia Commons file from Wikidata P18 when present: imageinfo extmetadata (LicenseShortName, LicenseUrl, Artist) stored with the asset, one rendition downloaded under the storage budget, credit line on the item page; files without a standard free licence tag skipped; otherwise the thumbhash/dominant-colour placeholder; user-uploaded cover photos stay private to the instance; publisher press kits and BGG images not used | hotlink Commons (not recommended by Commons); press kits (per-publisher press-only terms) | remove the Commons fetch job; placeholders remain | https://commons.wikimedia.org/wiki/Commons:Licensing; https://commons.wikimedia.org/wiki/Commons:Reusing_content_outside_Wikimedia; https://commons.wikimedia.org/wiki/Commons:Image_casebook (board games section)
DEC-194 | Matching and dedup keys for board games | In order: Wikidata QID; GTIN/EAN (P3962, list per item, edition-level); BGG id (with the known Wikidata duplicate ids handled by preferring the item with a board game class and sitelinks); normalised title + year + publisher as a user-confirmed candidate; expansions (P8646 / expansion_of) never merged into their base | title-only fuzzy merge | change the matcher order in one place | docs/research/board-game-sources.md section 3; Wikidata property P3962 and P2339 (wbgetentities 2026-11-06)
DEC-195 | Contributing missing games back to Wikidata | v1: "Suggest to Wikidata" builds a QuickStatements v1 batch (CREATE, labels, description, P31, P1872/P1873, P2047, P2899, P577, P3962, optional P2339, source pairs) or a pre-filled Special:NewItem link, opened in the user's browser under the user's own account after a duplicate check; only for items whose values are user- or import-sourced, never BGG-derived; CC0 consequence explained; no server-side edits, no shared account, no bot | server-side OAuth 2.0 writes via the REST API (needs an operator-registered consumer with admin approval and PKCE; v2 candidate) | remove the link builder | https://www.wikidata.org/wiki/Help:QuickStatements; https://www.wikidata.org/wiki/Wikidata:Notability; https://www.wikidata.org/wiki/Wikidata:Bot_policy; https://www.mediawiki.org/wiki/OAuth/For_Developers; https://www.wikidata.org/wiki/Wikidata:REST_API/Authentication
DEC-196 | Sources assessed and rejected for board games | Board Game Atlas (dead since Oct 2023, DNS gone), Tric Trac (login-gated app, no API, no terms page), Luding and Spiel des Jahres and brettspiele.digital (no API, no reuse terms; kept as outbound links via Wikidata ids P3528/P11025), Board Game Oracle and BoardGamePrices (price sites, no API), Kaggle/IEEE BGG dumps (BGG data relabelled CC BY by uploaders, frozen 2021), OpenTabletop (specification only, no data; watch for export vocabulary), Tesera (not assessed) | - | re-assess any of them when terms or an API appear | docs/research/board-game-sources.md sections 2.3 to 2.9, all checked 2026-11-06
DEC-197 | Ludopedia | Not built in v1. If ever wanted (pt-BR users): Q11 feature flag off by default, per-instance app_id/app_key, OAuth 2.0 authorization code, data and image licence to be obtained in writing from api@ludopedia.com.br first | build it now | - | https://www.ludopedia.com.br/api/documentacao.html and https://ludopedia.com.br/api/openapi.yaml (LudoAPI 0.0.5, "ALPHA", checked 2026-11-06)
```

## 9. Open points for the owner

- Whether the German cross-reference links (Luding, Spiel des Jahres) are worth a line on
  the item page; they cost nothing once the Wikidata ids are stored.
- Whether to ask Ludopedia for their data licence at all (only relevant for pt-BR users).
- Whether the v2 OAuth write path is wanted; it needs each operator to register a
  Wikidata OAuth consumer and wait for approval.
- GTIN lookup via the search keyword `haswbstatement` is unverified; SPARQL works.
