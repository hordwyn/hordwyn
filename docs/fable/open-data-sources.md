STATUS: COMPLETE (2026-11-09)

# Research: the most open metadata sources per media kind

Date: 2026-11-09. Every URL below was opened on that date unless a line says otherwise.
Written in our own words; no source code of any GPL-family project was read (owner ruling
R-P1: public API docs and terms of GPL/AGPL services may be read to write our own client).
Licence and terms were checked before the API shape for every source. Facts that could not be
verified from this build host are marked `UNVERIFIED`; pages that could not be opened are
listed in section 16 with the error seen. Wikidata counts are CirrusSearch
`haswbstatement` totals from `https://www.wikidata.org/w/api.php?action=query&list=search`
taken on 2026-11-09 (one request per count; no SPARQL used). Property labels were verified
with one `wbgetentities` call on the same day.

Question from the owner: per media kind, which source is the most open, so the app depends
less on restrictive providers like TMDB (terms 1.C: no caching beyond six months, commercial
use only with a written agreement; docs/connectors/tmdb.md)? Builds on
`docs/research/board-game-sources.md` (board games, Wikidata client rules, Commons) and
`docs/research/anime-manga-sources.md` (anime, manga); those two are not redone, only
summarised with their verdicts. Results feed `docs/fable/hub-design.md` section 11 (which
kinds the hub must cover).

Legend for the verdict column: **primary** = recommended default source for the kind;
**fallback** = second source or flagged enrichment; **avoid** = not to be built.
"Keys" = usable from a self-hosted instance with per-instance credentials or none.
"SA" = share-alike licence (CC BY-SA, ODbL): fine on an instance with attribution, but such
fields can never be pushed to the CC0 hub.

## Contents

1. Summary table (one line per kind)
2. Films
3. TV shows and episodes with air dates
4. Anime (summary of the existing research)
5. Manga and comics
6. Books
7. Audiobooks
8. Podcasts
9. Music
10. Video games
11. Board games (summary of the existing research)
12. Visual novels
13. Images: open sources of cover art and their licences
14. What this changes in the connector set
15. Prioritised connector work packages
16. Pages that could not be read
17. Sources

## 1. Summary table

| Kind | Recommended primary (licence) | Fallback / enrichment | What the hub must fill |
|---|---|---|---|
| film | Wikidata identity (CC0) + Wikipedia summary (CC BY-SA 4.0, instance only); TMDB demoted to operator-enabled enrichment for posters and descriptions | TMDB (proprietary, 6-month cache), OMDb and IMDb datasets rejected | nothing in v1 (hub-design 11: movies out of scope); later a QID bridge |
| show + episodes | TVmaze (CC BY-SA 4.0, no key) for shows, episodes, air dates; Wikidata for ids | TMDB, TheTVDB (existing, flagged) | nothing; SA data cannot enter the hub |
| anime | MyAnimeList API v2 (proprietary, per-instance client id) - unchanged | Kitsu, Wikidata mapping, Annict watch | identifiers and facts only (H7 later) |
| manga | MAL manga + Kitsu + MangaUpdates (credit) - unchanged; volumes by ISBN | Open Library, DNB (CC0) for volumes | identifiers only |
| comics (western) | Grand Comics Database (CC BY-SA 4.0 per site footer, `UNVERIFIED` from here) behind a Q11 flag; Inducks for Disney comics (own licence, commercial ok with credit) | Metron (licence not found, Q11), Comic Vine rejected | issue-level data is thin everywhere open: candidate hub kind later |
| book | Open Library (no licence text; IA asserts no new rights; mission-aligned to self-hosted FOSS) + DNB SRU (CC0, no key) for ISBN lookups + Wikidata (CC0) for works/authors | Inventaire (CC0), BookBrainz (CC0 core, alpha); Google Books rejected | none (hub-design 11 holds); an ISBN -> QID bridge at most |
| audiobook | no single source: Open Library / DNB editions by ISBN, MusicBrainz release groups of type Audiobook / Audio drama (CC0), LibriVox (public domain) | Wikidata (no Audible property) | **yes**: narrator, duration, abridged flag; the one kind with no open home |
| podcast | Podcast Index (free for any use, key per instance) - unchanged | weekly SQLite dump; Wikidata P11740 (4,946 items) | none |
| music | MusicBrainz core (CC0) + Cover Art Archive (images: rights with owners, "use at your own risk") | Discogs dumps (CC0) only; Discogs API, Last.fm, Deezer, Spotify, TheAudioDB rejected; ListenBrainz as importer | none |
| video game | Wikidata (CC0; 178,897 games, 152,986 with IGDB ids, 129,148 with Steam ids) + hub H7 | IGDB (existing, caching allowed) for covers and summaries; MobyGames, RAWG, TheGamesDB, Giant Bomb rejected or unverified | yes (planned H7): facts and covers the open world lacks |
| board game | Wikidata (CC0) + hub H1 - unchanged | BGG not built (hub-design) | yes (H1) |
| visual novel | VNDB (data ODbL + DbCL; API free for non-commercial use, no key for reads) | Wikidata P3180 (12,075 items) | none |

## 2. Films

### 2.1 Candidates

| Source | Data licence | API terms | Attribution | Caching / storage | Commercial | Rate limit | Key | Images | Coverage and quality | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| Wikidata | CC0 (board-game-sources 2.1) | Wikimedia API etiquette: descriptive User-Agent, serial requests, maxlag; "Authenticated API requests receive higher rate limits" (access policy, numbers on a sub-page not read) | not required; we credit | unlimited | yes | unpublished for anonymous use; WDQS 60 s/min (board-game-sources) | none | P3383 film poster: 4,713 items; P18 image | 351,774 items instance of film (Q11424); 1,210,143 items with an IMDb id (P345); 289,498 with a TMDB movie id (P4947). Fields: title P1476, director P57, cast P161, duration P2047, genre P136, country P495, original language P364, publication date P577. No synopsis (descriptions are one line) | **primary for identity and facts** |
| Wikipedia REST summary | CC BY-SA 4.0 (and GFDL where applicable): "Permission is granted to copy, distribute and/or modify Wikipedia's text" under CC BY-SA 4.0; attribution: "a link back to the article is generally thought to satisfy the attribution requirement"; changes must be released under CC BY-SA 4.0 or later | REST `GET https://en.wikipedia.org/api/rest_v1/page/summary/{title}`; same User-Agent rule | link to the article | unlimited (SA) | yes | not read (access-policy sub-page) | none | each image "has its own license", some non-free | lead paragraph of the article in the language of the wiki (de.wikipedia for German) | **fallback for descriptions on the instance**; never to the hub (SA) |
| TMDB (existing) | proprietary (docs/connectors/tmdb.md) | no caching beyond 6 months, purge on termination, logo + notice, no AI use | logo and notice | 6 months | written agreement needed | ~40 req/s | per instance | posters, backdrops | best coverage and synopses in many languages | **fallback, operator-enabled**; identity should no longer depend on it (section 14) |
| OMDb | "All content licensed under CC BY-NC 4.0"; the Terms of Use (2015-03-12) grant use of contributions "solely for personal, non-commercial purposes", "You may not build a business utilizing the Contributions, whether or not for profit", "you may not create, recreate, distribute or advertise an index" | key by e-mail; "FREE! (1,000 daily limit)"; poster API for patrons only | retain notices | not addressed | no | 1,000/day | per instance | posters only for patrons | IMDb-derived fields (ratings, plot); the data source is not named | **avoid** (NC, personal use, index clause) |
| IMDb non-commercial datasets | "Subsets of IMDb data are available for access to customers for personal and non-commercial use"; "You can hold local copies of this data, and it is subject to our terms and conditions" | daily TSV files (title.basics, title.akas, title.episode, title.ratings, title.crew, title.principals, name.basics) at datasets.imdbws.com | per the linked licensing article (not read) | local copies allowed | no | n/a (bulk) | none | none | complete; multi-GB bulk files, far too large for the host and forbidden for an app that may be run commercially | **avoid** |
| Trakt (existing importer) | proprietary; policy forbids "building a reusable copy of Trakt's catalog" (docs/connectors/trakt.md) | import of the user's own data only | about page line | user data only | yes | n/a | n/a | none | n/a | importer only, as today |
| TheTVDB (movies) | proprietary, pricing by sales contact: "Our API is available for both commercial projects and to individual developers", "we offer significant discounts" for FOSS projects | attribution "with a direct link to TheTVDB.com must be displayed to end users" | required | not addressed | paid | none published | per instance | yes | good | fallback as today (unclear terms, Q11) |

### 2.2 Recommendation

- Identity from Wikidata (CC0): QID, title and year, director, IMDb/TMDB/TheTVDB ids, runtime,
  genre, country, original language. This is what makes the TMDB retention rule survivable:
  when TMDB content expires (DEC-466), the item keeps its CC0 title and year instead of
  becoming "Title unavailable - source data expired". Search by title through
  `wbsearchentities` in the user's language, as for board games.
- Descriptions: Wikipedia summary (CC BY-SA 4.0) with a link credit, provenance
  `wikipedia:<lang>` and licence tag `CC-BY-SA-4.0`, instance only. TMDB overview where the
  operator enabled TMDB.
- Posters: TMDB when enabled (6-month rule as today), else Commons P3383 (public-domain
  posters for old films; 4,713 items), else the placeholder. There is no open source of
  modern film posters (section 13).
- Hub: no film kind in v1 (hub-design 11). The QID bridge above is the "later at most"
  option named there and costs nothing extra once the Wikidata connector exists.

## 3. TV shows and episodes with air dates

### 3.1 Candidates

| Source | Data licence | API terms | Attribution | Caching / storage | Commercial | Rate limit | Key | Images | Coverage and quality | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| TVmaze | "Use of the TVmaze API is licensed by CC BY-SA. This means the data can freely be used for any purpose, as long as TVmaze is properly credited as source and your usage complies with the ShareAlike provision." (link to CC BY-SA 4.0) | public API without key; "In addition to the free public API, there's a user-level API available for all Premium members." | "You can satisfy the attribution requirement by linking back to TVmaze from within your application or website, for example using the URLs available in the API." | images "can be cached indefinitely" (a changed image gets a new URL); hotlinking the CDN is allowed, local caching recommended | "any purpose"; a separate licence / SLA via sales contact | "at least 20 calls every 10 seconds per IP address", 429 on excess; "Leaving more than 1 connection to our servers idle may result in your IP getting blocked"; descriptive User-Agent recommended | none | `image.medium` / `image.original` per show and episode; rights of the artwork itself not stated | `/search/shows?q=`, `/singlesearch/shows?q=`, `/lookup/shows?thetvdb=|imdb=`, `/shows/:id/episodes`, `/shows/:id/episodesbydate?date=`, `/schedule?country=&date=`, `/schedule/web`, `/schedule/full` (future episodes, cached 24 h). Wikidata holds 38,304 TVmaze series ids (P8600), so the catalogue is at least that large | **primary** for shows, seasons, episodes and air dates |
| Wikidata | CC0 | as above | - | unlimited | yes | as above | none | P18 | 90,304 television series (Q5398426), 182,563 episode items (Q21191270); episode-level air dates exist as P577 on episode items but coverage per series is uneven (`UNVERIFIED`, not measured); ids: P8600 TVmaze, P4983 TMDB TV (68,025), P4835 TheTVDB | **id bridge and identity**, not the episode source |
| TMDB (existing) | proprietary, 6-month cache | as section 2 | logo + notice | 6 months | agreement | ~40/s | per instance | posters, stills | episode air dates per season (WP4-7) | fallback, operator-enabled |
| TheTVDB (existing) | proprietary, paid tiers, FOSS discount | attribution link required | required | not addressed | paid | none published | per instance (key + optional PIN) | yes | episodes per season (WP4-7) | fallback, Q11 as today |
| Trakt | proprietary; catalogue copies forbidden | importer only | about line | user data only | yes | n/a | n/a | none | n/a | importer only |
| IMDb datasets | personal, non-commercial | bulk TSV incl. title.episode | - | local copies | no | n/a | none | none | complete | **avoid** |

### 3.2 Recommendation

- TVmaze becomes the default show connector: search, details, season list, episodes with
  `airdate`, `airstamp` and runtime, and the schedule endpoints for "what airs this week".
  No key, no retention limit, attribution satisfied by the TVmaze URL on the item page and
  the Data sources page.
- Licence handling: every TVmaze field carries licence tag `CC-BY-SA-4.0`. On the instance
  that means a credit link; for the hub it means the provenance filter blocks TVmaze fields
  (hub-design 6: only CC0-compatible provenance may be proposed). Our own export of a user's
  library may contain TVmaze facts only with the credit and licence attached (SA).
- Images: TVmaze permits hotlinking and indefinite caching of the files it serves, but says
  nothing about the rights in the artwork; treat like TMDB images (held under the storage
  budget, provenance recorded, removed on request), minus the six-month rule.
- TMDB and TheTVDB stay as operator-enabled fallbacks; the field priority puts Wikidata
  identity first, TVmaze second, TMDB/TheTVDB third.
- Hub: not needed for shows; SA data cannot be contributed anyway.

## 4. Anime

Summary of `docs/research/anime-manga-sources.md` (2026-11-07), nothing re-fetched today:

- Primary: MyAnimeList API v2 (proprietary; developer agreement shown only at client
  registration; per-instance `KK_MAL_CLIENT_ID`; off until configured, DEC-213..221).
- Fallback and id mapper: Kitsu (terms page unreadable from this host, Q11 flag).
- Wikidata (CC0): 7,277 items with MAL anime ids, mapping path to TMDB/TheTVDB; German labels
  on about a third.
- The most open terms of the lot were Annict (free for commercial and non-commercial use,
  works data storable without limit; Japanese anime only, no manga) and anime-offline-database
  (ODbL + DbCL, archived 2026-07). Neither replaces MAL for coverage or for the owner's
  history.
- AniList stays out (DEC-108); Jikan, Shikimori, Simkl, Fribb lists rejected.
- Hub: identifiers and facts only, descriptions off (hub-design 11, H7 later): synopses on
  MAL/Kitsu are often copied and would be laundered.

No change recommended today. One addition: with the Wikidata connector generalised (WP-OD1),
anime items get QIDs and the mapping path becomes the same code as for films and shows.

## 5. Manga and comics

### 5.1 Manga

Summary of the existing research: MAL manga endpoints first, Kitsu manga second, MangaUpdates
(credit required, no data licence) for publishers and status, physical volumes as books by
ISBN. MangaDex not in v1 (owner question). Comic Vine rejected ("strictly for non-commercial
use only", "Do not edit, manipulate or reproduce on any other medium"). Open today for
volumes: Open Library and DNB (section 6), both without a key.

### 5.2 Western comics: candidates

| Source | Data licence | API terms | Attribution | Caching / storage | Commercial | Rate limit | Key | Images | Coverage and quality | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| Grand Comics Database (GCD) | per a search excerpt of the comics.org footer: all portions of the database, except where noted, are licensed under CC BY-SA 4.0 (`UNVERIFIED`: www.comics.org, docs.comics.org and the Library of Congress dataset record all answered 403 to this host) | `GET https://www.comics.org/api/` answered 200 with links to `series/` and `publisher/` (no licence text on it); a GCD wiki page (search excerpt) says endpoints are stable but fields are not, auth is Basic or session | CC BY-SA attribution | unlimited (SA) | yes (SA) | not found | account for the API (`UNVERIFIED`) | cover scans on the site; not covered by the data licence (`UNVERIFIED`) | the reference database for printed comics worldwide; MySQL dump about 2 GB (search excerpt); Wikidata: 1,907 series with GCD series id (P3589), 254 issues (P11308). The site software gcd-django is GPL-3.0 (GitHub API `license.spdx_id`): only its public API and terms may be read (R-P1) | **primary candidate for comics, Q11 flag** until the owner reads comics.org/licensing in a browser |
| Metron | not found: the best-practices post (2026-04-18) and the blog index carry only "Copyright (c) 2026 The Metron Project"; a post "Using Metron in Commercial Software" (2026-10-04) exists but its URL could not be resolved from here (`UNVERIFIED`) | REST API with HTTP Basic auth; token-based auth introduced 2026-07-26; "two independent throttle windows per authenticated user": 20 requests/minute and 5,000/day (reduced from 30 and 10,000 in March 2026); donors get an elevated daily limit; `modified_gt` and `If-Modified-Since` supported (a 304 still counts) | not stated | not addressed | asked often; the answer is in the unread post | 20/min, 5,000/day | account per instance | issue covers served by Metron; rights not stated | curated US comics (publishers, series, issues, creators, arcs); smaller than GCD; the service software is GPL-3.0 (docs/connectors/metron.md), R-P1 allows its API docs | **fallback, Q11 flag**; licence must be read first |
| Inducks (Disney comics) | own licence "Inducks Licence" (search excerpt of inducks.org/bolderbast/xxlicence.html, version 4D): covers all data and the database structure, excludes pictures and programs; public use including commercial use permitted with a link to an Inducks URL and a credit note; Wikipedia calls it a copyleft-style free licence (version 4B cited) | raw data files, no REST API found; the site answered an Anubis challenge to this host (`UNVERIFIED` for the exact wording) | required (link + credit) | unlimited | yes | n/a | none | excluded from the licence | about 170,000 Disney publications (Wikipedia); Wikidata: 6,170 items with an INDUCKS issue id (P4484), also publication/story/creator ids P4480-P4485; relevant for German readers (Lustiges Taschenbuch, Micky Maus) | **fallback for Disney comics** (file import job, not a live API) |
| Comic Vine | non-commercial only, no reproduction, revocable (anime-manga-sources 2.16) | 200 requests per resource per hour | link back | forbidden | no | 200/h | per site | yes | US comics, good | **avoid** |
| Giant Bomb / Fandom | same owner as Comic Vine; page answered 403 (`UNVERIFIED`) | - | - | - | - | - | - | - | - | avoid |
| Marvel Comics API | developer.marvel.com now redirects (301) to www.marvel.com; the API appears retired (`UNVERIFIED`) | - | - | - | - | - | - | - | - | avoid |
| Wikidata | CC0 | - | - | - | - | - | none | P18 | 5,671 items instance of comics (Q1004); thin | id bridge only |
| Open Library, DNB | section 6 | | | | | | | covers (OL) | collected editions by ISBN | volumes as books |

### 5.3 Recommendation

- Comics as a kind are the weakest open spot after audiobooks: the open databases (GCD,
  Inducks) are share-alike or custom-licensed and reachable only with an account or as file
  dumps; Metron's licence is unknown. Build GCD first behind a Q11 flag once the owner has
  read the licence and API pages in a browser (this host is blocked), Metron second under the
  same condition, Inducks as an import job for Disney titles. Collected editions by ISBN go
  through the book path (Open Library, DNB).
- Hub: comics issue data cannot be seeded from CC0 sources (Wikidata is thin, GCD is SA), so
  if the owner wants comics without GCD, comics become a hub kind filled by contributors
  (after H7). Not before.

## 6. Books

### 6.1 Candidates

| Source | Data licence | API terms | Attribution | Caching / storage | Commercial | Rate limit | Key | Images | Coverage and quality | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| Open Library (Internet Archive) | no licence statement on the developer pages; the licensing page says "The Internet Archive does not assert any new copyright or other proprietary rights" over Open Library material and "There may be existing rights issues on some contributions and in some jurisdictions" | the APIs "are not intended to serve as a data backend for third-party services"; priority to "open-source and mission-aligned projects"; do not scrape HTML, do not spread requests over IPs, do not make hundreds of single-book requests, do not use it as a backend for high-traffic commercial services; "Please do not use our APIs to bulk download metadata"; monthly dumps exist instead. The service software is AGPL-3.0; R-P1 allows its API docs | none required; "a courtesy link back to Open Library is appreciated" (covers page) | not restricted; caching is asked for | not a high-traffic commercial backend | 1 request/s anonymous, 3/s with `User-Agent: MyLibraryApp (contact@example.org)` | none | Covers API `https://covers.openlibrary.org/b/{isbn|olid|id|oclc|lccn}/{value}-{S|M|L}.jpg`, `?default=false` for 404; "Currently only 100 requests/IP are allowed for every 5 minutes" for keys other than CoverID and OLID (403 on excess); "intended for displaying covers on public facing websites and not for bulk download"; bulk tar files on archive.org; licence of the cover files not stated | search `https://openlibrary.org/search.json?q=&fields=&lang=&limit=&page=` (docs: `key`, `title`, `author_name`, `first_publish_year`, `cover_i`, `edition_count`, `editions` sub-fields; "The schema is not guaranteed to be stable"), works `/works/{OLID}.json`, editions `/books/{OLID}.json`, `/isbn/{isbn}.json`, legacy `/api/books?bibkeys=ISBN:...&jscmd=data`; Wikidata holds 501,593 Open Library ids (P648) | **primary** for search, works, editions and covers; licence tag `unspecified-open` (IA asserts no rights), so not pushed to the hub until clarified |
| Deutsche Nationalbibliothek (DNB) | CC0 1.0: press release 2015-05-13, from 2015-07-01 "all bibliographic data of the German National Library and the authority data of the Integrated Authority File (GND)" "can be freely re-used under "Creative Commons Zero" (CC0 1.0) terms" | SRU: "The SRU interface can be accessed free of charge and without registration."; `https://services.dnb.de/sru/dnb?version=1.1&operation=searchRetrieve&query=<CQL>&recordSchema=MARC21-xml`; default RDFxml; 10 records default, 100 max per reply, 99,000 per query; a CQL ISBN index exists per third-party tools but was not confirmed on the page read (`UNVERIFIED`) | none (CC0); we credit | unlimited | yes | none stated | none | none (no covers) | every German publication with an ISBN since 1913 plus the German Music Archive (`dnb.dma`); MARC 21 parsing needed; no descriptions | **primary CC0 companion** for ISBN lookups of German-language editions and for hub seeding of books if ever wanted |
| Wikidata | CC0 | as section 2 | - | unlimited | yes | - | none | P18 | 545,199 literary works (Q7725634), 860,677 editions (Q3331189), 350,702 items with ISBN-13 (P212); author P50, pages P1104, language P407; ids P648 Open Library, P7823 BookBrainz work (4,946), P12351 BookBrainz edition | **identity bridge** (QID per work and author) |
| Inventaire | CC0: "Those data will are published under a CC0 license" (Entities data wiki page; footer "Content is available under Creative Commons Zero (Public Domain) unless otherwise noted") | `https://api.inventaire.io` (OpenAPI 3.1.1); `/api/entities?action=by-uris` (`refresh=true`), `?action=resolve`; returns Wikidata and Inventaire entities in one simplified form; dumps at dumps.inventaire.io (JSON, NDJSON, TTL) hold only Inventaire's own entities; ISBNs attach to editions, never works | not required | unlimited | yes | not found | none | cover licence not stated | Wikidata-centred works/editions/authors/series plus user contributions, Open Library and some national libraries as sources; the server code is on Codeberg (licence not checked; only the API and wiki were read) | **fallback**: a convenient Wikidata-plus-ISBN resolver; optional |
| BookBrainz (MetaBrainz) | "The BookBrainz core data is licensed under the Creative Commons CC0 license." (identifiers, names, types, dates, comments for authors, edition groups, editions, publishers, works, relationships); "Other publically available data is licensed under the Creative Commons CC BY-SA license." | "currently in alpha version", live docs at `https://api.test.bookbrainz.org/1/docs/` (JavaScript-only Swagger UI, not readable here); weekly SQL dump `ftp://ftp.musicbrainz.org/pub/musicbrainz/bookbrainz/latest.sql.bz2` | none | unlimited | yes | not found | none | none | small, growing; Wikidata: 4,946 BookBrainz work ids | watch; not v1 |
| Google Books | Books API ToS (2012-04-25): "You may not charge users any fee for the use of your application" without agreement; content-removal duties; branding guidelines: "Every book result displayed in your application must have a prominent link to" the Google Books page, "The 'powered by Google' logo must appear adjacent to these results", "You may not reorder or otherwise alter the results", results must not be intermixed with third-party results | API key; `GET https://www.googleapis.com/books/v1/volumes?q=isbn:...`; `imageLinks` thumbnails; default daily quota not stated on the page read (anonymous quota from this host was exhausted on 2026-11-07) | mandatory logo and links | not addressed | no fees | per project | per instance | thumbnails only | good for ISBN lookups | **avoid** (branding and no-intermixing rules conflict with a multi-source catalogue) |
| Hardcover | secondary review (Bindery project doc, 2026-08-14): "the same licensing as OpenLibrary", 60 requests/minute, personal projects may use any data, professional projects only book/edition/series facts, no hotlinking for professional use; docs.hardcover.app answered 522 twice (`UNVERIFIED`) | GraphQL, account token | not stated | - | restricted for "professional" use | 60/min | per user | hotlinking rules | - | not v1; re-check when reachable |
| Library of Congress | US government data; APIs page read gives no rights or limits (sub-pages not read, `UNVERIFIED`) | loc.gov JSON API | - | - | - | - | none | - | US-centric | not needed |
| ISBNdb, WorldCat | paid APIs (not checked today, `UNVERIFIED`) | - | - | - | - | - | - | - | - | avoid |

### 6.2 Recommendation

- Primary: Open Library for title search, work/edition records and covers. It is the only
  key-less source with covers and a worldwide catalogue, and its policy explicitly prioritises
  open-source and mission-aligned projects; a self-hosted MIT tracker with caching and an
  identifying User-Agent is the intended use. Treat its licence as "no rights asserted by IA,
  per-record rights unknown": display freely, do not contribute Open Library text to the hub.
- CC0 companion: DNB SRU for ISBN lookups. For a German owner most physical books resolve
  there with authoritative title, author, publisher, year, pages and GND ids, under CC0 and
  without a key. MARC 21 XML parsing is the cost.
- Identity: Wikidata QIDs for works and authors (CC0) via P648 / P212 where present.
- Covers: Open Library covers API by OLID or cover id (no per-IP cap), ISBN keys only as a
  fallback (100 per 5 minutes). Licence of the files is unstated; store with provenance and
  honour takedowns, as for the other providers.
- Hub: books stay off the hub (hub-design 11). Nothing to fill.

## 7. Audiobooks

### 7.1 Candidates

| Source | Data licence | API terms | Attribution | Caching | Commercial | Rate limit | Key | Images | Coverage and quality | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| Open Library / DNB | section 6 | audio editions carry ISBNs like any edition | as section 6 | - | - | - | none | OL covers | editions exist where publishers issued an ISBN; narrator and duration fields are not reliable (`UNVERIFIED`, not measured) | **primary by ISBN** |
| MusicBrainz | CC0 core data | release groups with secondary type "Audiobook", "Audio drama" or "Spokenword" (confirmed on the Release Group/Type page); API section 9 | none; we credit | unlimited | yes | 1 request/s | none | Cover Art Archive | CD and digital audiobook releases, especially German Hoerspiele (audio dramas) which MusicBrainz covers well (`UNVERIFIED`, not measured) | **primary for CD/digital releases** |
| LibriVox | "all our recordings are public domain (definitely in the USA, and maybe in your country as well"; "there is no need to credit LibriVox, although of course we much prefer if you do credit us" | `https://librivox.org/api/feed/audiobooks?title=&author=&fields=&extended=1&coverart=1&limit=&offset=&format=json` (limit max 500), `/audiotracks?project_id=`, `/authors`; "no rate limit is currently in force", may add 429 later; add a delay for large pulls | optional | unlimited | yes | none yet | none | `coverart_jpg`, `coverart_thumbnail` (volunteer-made; licence not stated on the page, `UNVERIFIED`) | public-domain audiobooks only | **fallback, optional connector** |
| Audible / Audnexus | no public Audible API; Audnexus is a community proxy over an unpublished Amazon endpoint whose terms restrict automated access (secondary review) | - | - | - | - | - | - | - | - | **avoid** |
| iTunes Search API | "approximately 20 calls per minute"; promotional content "only to promote store content and not for entertainment purposes", must sit next to a store badge, "is not used for independent entertainment value" | `https://itunes.apple.com/search?entity=audiobook`, `/lookup?id=` | store badge | streaming only for previews | - | 20/min | none | artwork is "Promo Content" under the badge rule | large | **avoid** as a data source |
| Wikidata | CC0 | - | - | - | - | - | - | - | no Audible property exists (property search returned none); LibriVox author id P1899 only | weak |

### 7.2 Recommendation

- Model an audiobook as an edition of a book item (format audio) with narrator, duration
  and abridged flag as user-editable fields. Resolve by ISBN through Open Library and DNB;
  resolve CD/digital releases through MusicBrainz (release group type Audiobook / Audio
  drama) with Cover Art Archive covers; LibriVox for public-domain titles.
- This is the one kind where no open source carries the fields that matter (narrator,
  duration): **the hub must fill audiobook editions** (new kind after H7), seeded from
  MusicBrainz audiobook release groups (CC0) and user entries.

## 8. Podcasts

| Source | Data licence | API terms | Attribution | Caching | Commercial | Rate limit | Key | Images | Coverage | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| Podcast Index (existing) | "The core, categorized index will always be available for free, for any use." (recorded 2026-11-06 in docs/connectors/podcastindex.md; podcastindex.org and api.podcastindex.org answered 403 to this host today) | key + secret per instance, SHA-1 auth header (existing connector) | none required; we credit | unlimited | yes | none published | per instance | artwork URLs of the podcasters | 4 million+ feeds; weekly SQLite dump `https://public.podcastindex.org/podcastindex_feeds.db.tgz` ("about 1.8GB", "becomes a 5GB file", James Cridland 2026; no terms stated for the file) | **primary**, unchanged |
| fyyd | fyyd.de/api-doc answered an Anubis challenge; the GitHub docs repo path was 404 (`UNVERIFIED`); older docs required app registration | - | - | - | - | - | - | - | German-centred directory | not v1 |
| iTunes Search | section 7 | `entity=podcast,podcastEpisode` | badge | - | - | 20/min | none | promo-only | large | **avoid** |
| Wikidata | CC0 | - | - | - | - | - | none | - | 9,997 podcasts (Q24634210), 4,946 with a Podcast Index id (P11740), 6,557 Apple Podcasts ids (P5842) | id bridge only |

No change. The hub does not need podcasts (hub-design 11).

## 9. Music

### 9.1 Candidates

| Source | Data licence | API terms | Attribution | Caching | Commercial | Rate limit | Key | Images | Coverage | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| MusicBrainz | core data CC0: areas, artists, events, genres, instruments, labels, mediums, places, recordings, release groups, releases, series, works, relationships and URLs, CD stubs. Supplementary data CC BY-NC-SA 3.0: "User-submitted annotations", "Tags (including genre associations)", "Ratings", derived statistics, search indexes, edit history | `https://musicbrainz.org/ws/2/{entity}/{mbid}?inc=...&fmt=json`, browse `?artist={mbid}&limit=&offset=`, search `?query=`; "All users of the API must ensure that each of their client applications never make more than ONE call per second."; "Each request sent to MusicBrainz needs to include a User-Agent header, with enough information in the User-Agent for us (MusicBrainz) to contact the application maintainers." (`Application name/<version> ( contact-url )`); per-IP average 1 req/s, 503 on excess, global 300/s; "We ask commercial supporters to support us"; "Personal use of our datasets will always be free." | none for CC0; credit for supplementary data | unlimited | yes (support asked) | 1/s | none | via Cover Art Archive | the reference open discography; server software GPL-2.0-or-later (docs/connectors/musicbrainz.md), R-P1 allows its API docs | **primary** - use core fields only; never store tags, ratings, annotations (NC-SA) |
| Cover Art Archive | "All images are copyrighted by their respective copyright owners." (coverartarchive.org); "These covers are being collected for archival purposes, and the collection will be accessible to the public."; "Use the images at your own risk."; stored on Internet Archive servers | `https://coverartarchive.org/release/{mbid}/front-500`, `/release-group/{mbid}/front-250|500|1200`, `/release/{mbid}/` JSON index; "There are currently no rate limiting rules in place." (some endpoints still document a 503 for exceeded limits) | none stated | not restricted | not stated | none stated | none | this is the image source | front covers for a large share of releases | **primary image source**, provenance and takedown handling like other covers |
| Discogs API | CC0 Data: release titles, notes, dates, format, track listings, barcodes and identifiers, credits, versions, URLs, artist and label names; Restricted Data: user data, marketplace data and images. API terms (last updated 2025-05-27): "Data provided by Discogs." must appear "directly next to" the data with a link, no `nofollow`; "You may not cache or store the Content longer than is necessary" and content more than six hours older than Discogs' own may not be displayed; "Commercial use of Our API and the Content is generally permitted" except listed cases; restricted data never commercial; rate limits per application, extra keys forbidden | token per instance; images need authenticated signed URLs | required inline | **6 hours** | mostly | per app | per instance | restricted | huge | **avoid for the catalogue** (the six-hour rule is stricter than TMDB) |
| Discogs data dumps | "made available under the CC0 No Rights Reserved license" (data.discogs.com): monthly XML of releases, artists, labels, masters; no images | bulk only | none | unlimited | yes | n/a | none | none | complete; multi-GB | possible hub seed for releases missing in MusicBrainz; not v1 |
| ListenBrainz | listens published under CC0 (docs, search excerpts; listenbrainz.org/data is JavaScript-only) | `GET /1/user/{name}/listens?min_ts=&max_ts=&count=` (max 1000; "Do not scrape or paginate through the user's entire history using the listens endpoint", use the export API), `POST /1/submit-listens` with `Authorization: Token <user token>`; "never make more than ONE call per second", `X-RateLimit-*` headers | none | unlimited | yes | 1/s | user token | none | the user's own listening history | **importer / scrobble target**, not a catalogue |
| Last.fm | "You are permitted to use the Last.fm Data solely for non-commercial purposes"; storage "a maximum of 100 MB"; "You will not use any audio, audiovisual, images and/or artwork"; credit and links required | key | required | 100 MB | no | enforced | per instance | excluded | - | **avoid**; at most a user-export importer later |
| Deezer | "strictly limited for a non-commercial purpose and in a non-commercial environment" | developer account | trademark rules | not addressed | no | - | per instance | - | - | **avoid** |
| Spotify | Developer Terms v10 (2025-05-15): "Do not store Spotify Content indefinitely", caching limited to "temporary caching of" "metadata and cover art", "do not transfer Spotify Content to third parties", no ML training | app registration | branding guide | temporary only | restricted | - | per instance | temporary only | - | **avoid** |
| TheAudioDB | no licence or terms on the API page; free key "123", 30 req/min free, premium $8/month 100/min; lookups by MusicBrainz ids | key | none stated | not addressed | not stated | 30/min | per instance | artist and album art | fan-art style images | **avoid** (no licence) |
| Wikidata | CC0 | - | - | - | - | - | none | P18 | 310,830 albums (Q482994), 244,768 MusicBrainz release group ids (P436), 166,249 Discogs master ids (P1954); performer P175, label P264 | id bridge |

### 9.2 Recommendation

MusicBrainz core (CC0) as the catalogue, Cover Art Archive for covers, ListenBrainz as the
import source for listening history (user token), Wikidata for QIDs. Build a per-field
allowlist so that no NC-SA supplementary field is ever stored. No hub role (hub-design 11).

## 10. Video games

### 10.1 Candidates

| Source | Data licence | API terms | Attribution | Caching | Commercial | Rate limit | Key | Images | Coverage | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| Wikidata | CC0 | as section 2 | - | unlimited | yes | - | none | P18 (logos, screenshots; box art rarely free) | 178,897 items instance of video game (Q7889); 152,986 with an IGDB id (P5794), 129,148 with a Steam app id (P1733), 33,450 with a MobyGames id (P1933, "former scheme"); platform P400, developer P178, publisher P123, publication date P577, genre P136, series P179 | **primary for identity and facts** |
| IGDB (existing) | proprietary (Twitch); free for non-commercial use, commercial via partnership; "storing/caching locally is allowed and preferred" (docs/connectors/igdb.md) | Twitch client credentials, 4 req/s | "Data from IGDB.com" visible | unlimited | partnership | 4/s | per instance | covers, allowed to cache | about 300k games, summaries, platforms | **fallback for covers and summaries**, flagged as today |
| MobyGames | API page and terms page answered 403 (`UNVERIFIED`). Search excerpts: the FAQ allows reprinting for non-commercial personal use with credit and a direct link; commercial use via e-mail; the MobyPro page lists "2x hourly rate limit for the non-commercial API" as a paid feature | key, apparently tied to MobyPro | credit + link | - | by arrangement | hourly | per instance | - | deep historical coverage | **avoid** (non-commercial, paywalled, unverified) |
| RAWG | rawg.io/apidocs answered 522 and rawg.io/tos_api 403 (`UNVERIFIED`). Search excerpts of the docs: free tier "personal and hobby projects", non-commercial, 20,000 requests/month, required backlinks; business plan 50,000/month; ToS 2021-03-01 | key | backlink required | - | paid | 20k/month | per instance | yes | 500k+ games | **avoid** |
| TheGamesDB | no licence or terms anywhere on thegamesdb.net (only "(c) 2026 TheGamesDB"); key via forum request; monthly allowance per key | key | - | - | - | monthly allowance | per instance | CDN images | emulator-oriented | **avoid** (no terms) |
| Steam Web API | terms "Last updated July 2010": "You are limited to one hundred thousand (100,000) calls to the Steam Web API per day"; users must be told what Steam Data is stored; Valve links must not be `nofollow` | key | Valve name/logo links | notify users | not addressed | 100k/day | per instance | - | serves player and app data, not a catalogue; the storefront `appdetails` endpoint is undocumented | **avoid** |
| Giant Bomb | 403 to this host (`UNVERIFIED`); same owner as Comic Vine | - | - | - | - | - | - | - | - | avoid |

### 10.2 Recommendation

Wikidata (CC0) is strong enough for identity and the main facts of most games the owner
will track, and it links to IGDB and Steam ids for enrichment. IGDB stays as the flagged
enrichment connector for covers and summaries because its terms allow caching (no retention
problem). The hub takes video games as the second kind exactly as hub-design WP-H7 plans,
to hold the facts and the contributor-licensed images the open world lacks.

## 11. Board games

Summary of `docs/research/board-game-sources.md` (2026-11-06) and hub-design 3.6: own
catalogue seeded on demand from Wikidata (CC0; ~5,000 games with usable fields, expansions
weak), Commons images where P18 exists (607 of 4,748), user entry, importers of the user's
own BGG / BG Stats exports, hub kind H1. BGG's terms (non-commercial, revocable, no
modification, ToS 6.D(iv)) rule it out as a catalogue source; hub-design decides not to build
the BGG connector at all. Nothing new found today; the alternatives list (Board Game Atlas
dead, Ludopedia alpha, Luding/Spiel des Jahres link-outs) stands.

## 12. Visual novels

| Source | Data licence | API terms | Attribution | Caching | Commercial | Rate limit | Key | Images | Coverage | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| VNDB | "All information on VNDB is made available under the Open Database License." and "Any rights in individual contents of the database are licensed under the Database Contents License." (d17 section 4); exception: anime data from AniDB is CC BY-NC-SA 4.0; images and descriptions "may be subject to separate license conditions"; the site code is AGPL (docs/connectors/vndb.md; R-P1 allows the API docs) | Kana API `https://api.vndb.org/kana`: "The majority of the API endpoints below are usable without any form of authentication."; "This service is free for non-commercial use."; "The server will allow up to 200 requests per 5 minutes and up to 1 second of execution time per minute."; requests over 3 s aborted; "The data obtained through this API is subject to our Data License."; `POST /vn` with `filters` (`search`, `lang`, `released`, `tag`, `developer`...), `fields` (`title`, `titles`, `image.url`, `image.thumbnail`, `developers`, `extlinks`...), `results` max 100, `page`; `/release`, `/producer`, `/character`, `/staff`, `/tag`; list endpoints need a token | ODbL attribution | unlimited (ODbL, SA) | API: non-commercial; data dumps carry the same usage terms; commercial users "get in touch" | 200 req / 5 min | none for reads | `image.url`, `image.thumbnail`, flagging averages (sexual, violence); dumps: covers about 9 GiB via rsync, "use of individual images is considered "fair use"" | the only database of the kind; daily dump `vndb-db-latest.tar.zst` ("NOT considered a stable API"), tags and traits JSON; Wikidata: 12,075 items with a VNDB id (P3180) | **primary** |
| Wikidata | CC0 | - | - | - | - | - | none | - | 12,075 VNDB-linked items; titles and years | id bridge |

Recommendation: VNDB connector, on by default for reads (clear terms, no key), with a notice
to commercial operators that API use needs VNDB's agreement. Fields carry licence tag
`ODbL-1.0` (SA): instance display with attribution, never to the hub. Image flags
(`sexual`, `violence`) must be honoured before any cover is shown. Exclude the AniDB-derived
anime relations (NC-SA). No hub role.

## 13. Images: open sources of cover art

The hard truth first: covers, posters and box art are copyrighted works of publishers and
studios. No open database can license them; the open databases either exclude images from
their licence (Inducks, Discogs restricted data, VNDB "fair use"), disclaim them (Cover Art
Archive "copyrighted by their respective copyright owners", "Use the images at your own
risk"), or say nothing (Open Library, TVmaze artwork, Metron). Only Wikimedia Commons offers
per-file free licences, and Commons normally refuses box and poster art for modern works
(board-game-sources 5). What we can do is pick sources whose *terms of service* allow us to
fetch and keep renditions, record provenance, show credits and honour takedowns, and keep the
hub to CC0 / CC BY uploads (hub-design 7).

| Source | Licence of the files | Terms for fetching and keeping | Rate limit | Useful for |
|---|---|---|---|---|
| Wikimedia Commons (P18, P3383) | per file: CC0, CC BY, CC BY-SA, PD; NC/ND never (board-game-sources 5) | copy allowed; credit author, licence, link; `imageinfo` + `extmetadata` | Wikimedia etiquette | film posters of old films (4,713 items with P3383), photos of board games, game logos, author portraits |
| Cover Art Archive | "All images are copyrighted by their respective copyright owners." | public archive, "Use the images at your own risk."; "There are currently no rate limiting rules in place."; files on archive.org, filenames never change | none stated | music and audiobook covers |
| Open Library covers | not stated; IA asserts no new rights | "intended for displaying covers on public facing websites and not for bulk download"; courtesy link appreciated; bulk tar dumps for bulk needs | 100 per IP per 5 min for ISBN/OCLC/LCCN keys; none stated for OLID and cover-id keys | book and manga volume covers |
| TVmaze images | not stated (TVmaze's CC BY-SA covers its data) | hotlinking allowed, local caching recommended, "can be cached indefinitely" | 20 per 10 s | show and episode images |
| IGDB images | proprietary | caching "allowed and preferred" | 4/s | game covers |
| TMDB images | proprietary | six-month cache, purge on termination | ~40/s | film and show posters while TMDB is enabled |
| VNDB images | collection ODbL, "use of individual images is considered "fair use"" | API URLs; rsync dumps, bandwidth strictly limited | API limit | visual novel covers, with the sexual/violence flags |
| Discogs images | restricted data, authenticated signed URLs, six-hour rule | not usable | - | avoid |
| iTunes artwork | "Promo Content", badge and promotional-use conditions | not usable for a tracker | 20/min | avoid |
| LibriVox cover art | made by volunteers; licence not stated on the pages read (`UNVERIFIED`) | fields `coverart_jpg`, `coverart_thumbnail` | none yet | public-domain audiobooks |
| Podcast artwork | the podcaster's own | loaded by the browser from the feed host (existing connector) | n/a | podcasts |
| Publisher press kits | per publisher, press use only | reject as a bulk source (board-game-sources 5) | - | - |
| User uploads | the user's responsibility | private to the instance by default (board-game-sources 5) | - | everything else |

Consequences: the thumbhash placeholder remains the normal look for board games, comics,
audiobooks and many books; the per-asset provenance record (source, URL, licence tag, fetch
date) that DEC-193 introduced for Commons becomes the rule for every image source.

## 14. What this changes in the connector set

Add (new connectors, all key-less unless noted):

- `wikidata` generalised from the board-game design to films, shows, anime mapping, books,
  music and games: `wbsearchentities` in the user's language, `Special:EntityData/Q.json`,
  `wbgetentities` up to 50 ids; one property map per kind. CC0 identity layer under every kind.
- `tvmaze` (shows, episodes, air dates, schedule) - becomes the default show source.
- `wikipedia` summaries (CC BY-SA 4.0) as the open description source for films, shows, games
  and books on the instance; credited link; never exported to the hub.
- `openlibrary` (search, works, editions, covers) and `dnb` (SRU by ISBN, CC0) - books,
  manga volumes, comics trade paperbacks, audiobook editions. R-P1 unblocks Open Library.
- `musicbrainz` + `coverartarchive` (music; audiobook release groups). R-P1 unblocks it.
- `vndb` (visual novels). R-P1 unblocks it.
- `listenbrainz` importer (user token; listens as history).
- `librivox` optional (public-domain audiobooks).
- `gcd`, `metron` (comics) and an `inducks` file import: Q11 flags, off by default, only after
  the owner has read the licence pages in a browser (this host is blocked by all three).

Demote:

- `tmdb`: from the identity source of films and shows to an operator-enabled enrichment
  connector for posters, backdrops and overviews. Identity (title, year, ids) comes from
  Wikidata (CC0) and, for shows, TVmaze. Effect on DEC-466: the retention job then purges only
  TMDB-sourced fields and the item keeps a real title - the strict reading of 1.C stays, the
  placeholder title disappears for every item that has a QID or a TVmaze id.
- `tvdb`: stays optional (unclear pricing), behind TVmaze in the field priority.
- `igdb`: stays, but as enrichment behind Wikidata identity; it is the only cover source for
  games and its caching terms are fine.
- `mal` / `kitsu`: unchanged, with Wikidata mapping through the shared connector.

Retire or never build:

- `bgg` (hub-design decision), `anilist` (DEC-108), Google Books, OMDb, IMDb datasets, Comic
  Vine, Giant Bomb, RAWG, MobyGames, TheGamesDB, Steam Web API, Discogs API, Last.fm, Deezer,
  Spotify, TheAudioDB, iTunes Search, Audnexus.

Provenance model: add a `licence` tag per field and per asset (`CC0-1.0`, `CC-BY-SA-4.0`,
`ODbL-1.0`, `unspecified-open`, `proprietary:<connector>`). The hub's provenance filter
(hub-design 6) then becomes a one-line rule (only `CC0-1.0` and `user` / `import:*` may be
proposed), the attribution page is generated from the same tags, and the TMDB retention job
selects by tag instead of by connector id.

Hub kinds (hub-design 11, updated): board_game (H1), video_game (H7), **audiobook editions**
(new; narrator, duration, abridged; seeded from MusicBrainz audiobook release groups), comics
issues (later, only if the owner wants comics without GCD), anime/manga identifiers (H7
later). Films, shows, books, music, podcasts, visual novels: no hub kind; films and shows get
the QID bridge at most.

## 15. Prioritised connector work packages

| # | Package | Why first | Depends on |
|---|---|---|---|
| WP-OD1 | `wikidata` connector generalised (search in the user's language, entity fetch, per-kind property maps for film, show, video_game, book, music; ids to TMDB/TVDB/TVmaze/IGDB/Steam/OL/MB) with licence tag `CC0-1.0`; field priority: Wikidata identity above every proprietary source | removes the TMDB dependency for title and year of every film and show (DEC-466 pain), and is the base for the hub seed of games | board-game Wikidata client (DEC-191 rules: User-Agent, serial, maxlag) |
| WP-OD2 | Per-field and per-asset `licence` tag, attribution page generation from tags, retention job keyed by tag, hub provenance filter keyed by tag | makes every later connector mechanical to classify and keeps SA data out of the hub | - |
| WP-OD3 | `tvmaze` connector: search, details, seasons, episodes with air dates, schedule; CC BY-SA credit link; image handling like TMDB without the six-month rule; TMDB demoted to enrichment for shows | the owner's shows get a key-less, retention-free source with better episode data than TMDB | WP-OD1, WP-OD2 |
| WP-OD4 | `openlibrary` (search, ISBN, works, covers by OLID) + `dnb` SRU ISBN lookup (MARC 21 XML, CC0) | books have no connector at all today; DNB is CC0 and fits the German owner | WP-OD2 |
| WP-OD5 | `musicbrainz` + `coverartarchive` with the core-field allowlist; audiobook release groups; `listenbrainz` importer | music and audiobooks have no connector today; CC0 | WP-OD2 |
| WP-OD6 | `vndb` connector (Kana API, ODbL tag, image flags) | small, clear terms, key-less | WP-OD2 |
| WP-OD7 | `wikipedia` summary connector (CC BY-SA) for descriptions of films, shows, games, books | lets TMDB be switched off entirely on instances that accept Wikipedia text | WP-OD1 |
| WP-OD8 | Hub kinds: `video_game` (WP-H7 as planned) and `audiobook` editions (new kind descriptor, seed job from MusicBrainz audiobook release groups) | the two kinds where the open world lacks the fields users need | hub H1-H3, WP-OD1, WP-OD5 |
| WP-OD9 | Comics: owner reads GCD licence/API and Metron commercial post in a browser; then `gcd` behind Q11, `metron` behind Q11, `inducks` import | blocked from this host; licence first | owner |
| WP-OD10 | `librivox` optional connector; Discogs dump based hub seed for music gaps (if ever) | nice to have | WP-OD5 |

## 16. Pages that could not be read (2026-11-09)

- https://www.discogs.com/developers - HTTP 403. The API Terms of Use were read instead
  through the Zendesk JSON endpoint
  `https://support.discogs.com/api/v2/help_center/en-us/articles/360009334593.json`
  (updated 2025-07-23; body "Last Updated: May 27th, 2025"). The HTML article URL gave 403.
- https://www.comics.org/ , https://docs.comics.org/wiki/Main_Page , https://www.comics.org/download/
  and https://www.loc.gov/item/2018487926 - HTTP 403 (GCD licence `UNVERIFIED`; only
  `https://www.comics.org/api/` answered 200).
- https://www.mobygames.com/info/api/ , https://mobygames.com/info/api ,
  https://www.mobygames.com/info/terms/ , https://Mobygames.com/mobypro/ - HTTP 403.
- https://rawg.io/apidocs and https://api.rawg.io/docs/ - HTTP 522; https://rawg.io/tos_api - 403.
- https://docs.hardcover.app/api/getting-started/ - HTTP 522 (three attempts).
- https://metron.cloud/docs/ - Anubis challenge ("Access Denied"); the blog at
  metron-project.github.io was readable, but the post "Using Metron in Commercial Software"
  (2026-10-04) was not found at the guessed URL (404).
- https://inducks.org/ and https://inducks.org/bolderbast/xxlicence.html - Anubis challenge;
  licence wording from a search excerpt and Wikipedia only.
- https://fyyd.de/api-doc - Anubis challenge; https://github.com/eazyliving/fyyd-api - 404.
- https://podcastindex.org/ and https://api.podcastindex.org/ - HTTP 403 (statement taken
  from docs/connectors/podcastindex.md, recorded 2026-11-06).
- https://api.thegamesdb.net/ and https://api.bookbrainz.org/1/docs/ - JavaScript-only Swagger
  UI, no readable text; https://listenbrainz.org/data/ - JavaScript-only.
- https://archive.org/about/terms.php - only the title was returned.
- https://www.giantbomb.com/api/ - HTTP 403.
- https://developer.marvel.com/terms - 301 to https://www.marvel.com/ (API presumed retired).
- https://www.theaudiodb.com/api_guide.php - 404 (the `free_music_api` page was read instead).
- https://www.dnb.de/EN/Professionell/Metadatendienste/Metadaten/Titeldaten/titeldaten_node.html
  - 404 (the 2015 press release and the SRU page were read instead).
- https://www.wikidata.org SPARQL endpoint was not used (counts via CirrusSearch only).

## 17. Sources

All opened 2026-11-09 unless noted. Wikidata counts: `https://www.wikidata.org/w/api.php?action=query&list=search&srsearch=haswbstatement:<P or P=Q>&srnamespace=0&srlimit=1&format=json`; labels: `action=wbgetentities&props=labels`; property lookups: `action=wbsearchentities&type=property`.

- TVmaze API: https://www.tvmaze.com/api (licensing section, rate limits, endpoints, images)
- OMDb: https://www.omdbapi.com/ ; https://www.omdbapi.com/legal.htm (Terms of Use 2015-03-12);
  https://www.omdbapi.com/apikey.aspx ("FREE! (1,000 daily limit)")
- IMDb datasets: https://data.imdb.com/non-commercial-datasets/ (redirect target of
  developer.imdb.com/non-commercial-datasets/)
- TheTVDB: https://thetvdb.com/api-information
- TMDB: docs/connectors/tmdb.md (terms read 2026-11-05 and 2026-11-08)
- Trakt: docs/connectors/trakt.md (policy read 2026-11-07)
- Wikipedia copyrights: https://en.wikipedia.org/wiki/Wikipedia:Copyrights ; REST API:
  https://www.mediawiki.org/wiki/Wikimedia_REST_API ; access policy:
  https://www.mediawiki.org/wiki/Wikimedia_APIs/Access_policy
- Wikidata licensing and client rules: docs/research/board-game-sources.md 2.1 (2026-11-06)
- Open Library: https://openlibrary.org/developers/api ; https://openlibrary.org/dev/docs/api/covers ;
  https://openlibrary.org/dev/docs/api/search ; https://openlibrary.org/dev/docs/api/books ;
  https://openlibrary.org/developers/licensing
- DNB: https://www.dnb.de/EN/Ueber-uns/Presse/ArchivPM2015/metadatenCC0.html (2015-05-13);
  https://www.dnb.de/EN/Professionell/Metadatendienste/Datenbezug/SRU/sru_node.html
- Inventaire: https://api.inventaire.io/ ; https://wiki.inventaire.io/wiki/Entities_data
- BookBrainz: https://bookbrainz.org/about ; https://bookbrainz.org/licensing ; https://bookbrainz.org/develop
- Google Books: https://developers.google.com/books/terms (2012-04-25);
  https://developers.google.com/books/branding ; https://developers.google.com/books/docs/v1/using
- Hardcover (secondary): https://raw.githubusercontent.com/vavallee/bindery/HEAD/docs/third-party-data.md
  (review dated 2026-08-14)
- LibriVox: https://librivox.org/api/info ; https://librivox.org/pages/public-domain/
- iTunes Search API: https://developer.apple.com/library/archive/documentation/AudioVideo/Conceptual/iTuneSearchAPI/index.html ;
  https://performance-partners.apple.com/search-api (redirect target of Searching.html)
- Podcast Index: docs/connectors/podcastindex.md (2026-11-06);
  https://james.cridland.net/blog/2026/podcast-index-sql-data-mining/ ;
  https://en.wikipedia.org/wiki/Podcast_Index
- MusicBrainz: https://musicbrainz.org/doc/About/Data_License ; https://musicbrainz.org/doc/MusicBrainz_Database ;
  https://musicbrainz.org/doc/MusicBrainz_API ; https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting ;
  https://musicbrainz.org/doc/Release_Group/Type ; https://metabrainz.org/datasets
- Cover Art Archive: https://coverartarchive.org/ ; https://musicbrainz.org/doc/Cover_Art_Archive ;
  https://musicbrainz.org/doc/Cover_Art_Archive/API
- ListenBrainz: https://listenbrainz.readthedocs.io/en/latest/users/api/index.html ;
  https://listenbrainz.readthedocs.io/en/latest/users/api/core.html ; licence via search excerpts
  of https://listenbrainz.readthedocs.io/en/latest/users/listenbrainz-dumps.html
- Discogs: Zendesk JSON of article 360009334593 (API Terms of Use); https://data.discogs.com/
- Last.fm: https://www.last.fm/api/tos ; Deezer: https://developers.deezer.com/termsofuse ;
  Spotify: https://developer.spotify.com/terms (v10, 2025-05-15); TheAudioDB:
  https://www.theaudiodb.com/free_music_api
- VNDB: https://api.vndb.org/kana ; https://vndb.org/d17 ; https://vndb.org/d14
- IGDB: docs/connectors/igdb.md (2026-11-06/07)
- Steam Web API terms: https://steamcommunity.com/dev/apiterms (July 2010)
- MobyGames, RAWG, TheGamesDB (search excerpts only, see section 16); https://thegamesdb.net/
- Metron: https://metron-project.github.io/blog/api-best-practices (2026-04-18, correction
  2026-08-21); https://metron-project.github.io/blog/tags/api
- GCD: https://www.comics.org/api/ ; https://api.github.com/repos/GrandComicsDatabase/gcd-django
  (licence GPL-3.0, not read further); licence wording from search excerpts (section 16)
- Inducks: https://en.wikipedia.org/wiki/Inducks ; licence wording from a search excerpt of
  https://inducks.org/bolderbast/xxlicence.html
- Anime and manga: docs/research/anime-manga-sources.md (2026-11-07)
- Board games and Commons: docs/research/board-game-sources.md (2026-11-06)
- Hub: docs/fable/hub-design.md sections 0, 1, 3.6, 11, WP-H7
