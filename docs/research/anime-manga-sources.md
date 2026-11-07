# Research: anime and manga metadata sources (owner question R-P3)

Date checked: 2026-11-07 (every URL below was opened between 2026-11-06 23:00 UTC and
2026-11-07 01:00 UTC unless a line says otherwise). Written in our own words; no source code of
any GPL-family project was read or copied. Licence and terms were checked before anything else
for every source (the repository licence through the GitHub API, then the terms page). Facts
that could not be verified from this build host are marked `UNVERIFIED`. Pages that could not
be opened are listed in section 7 with the error seen.

Question from the owner (HANDOFF 4.4, DEC-173): AniList stays out (DEC-108). Which sources
can serve anime and manga, and how does the legacy importer match the `anilist` and
`myanimelist` items of the owner's export?

Short answer: use the official MyAnimeList API v2 as the anime and manga catalogue source
(per-instance client id, native ids for the legacy `myanimelist` items), Kitsu as the
key-less fallback and as the id-mapping service (it resolves MAL ids and AniList ids to one
record; verified for all three anime in the fixture), Wikidata (CC0) as the second mapping
path, and keep TMDB for anime that are tracked as shows. Manga: MAL manga endpoints first,
Kitsu manga second, MangaUpdates for publisher and status detail with the required credit,
and the existing book connectors (ISBN) for physical volumes. MangaDex and Jikan are not used
(section 2.10 and 2.3). aniSearch is the one source with German titles and German release
data and is worth an optional, flagged enrichment connector. Details in sections 5 and 6.

## 1. Comparison table

"Keys" = usable from a self-hosted instance with its own per-instance credentials.
"Legacy ids" = can it take the MAL or AniList id from the owner's export directly.
Wikidata counts are CirrusSearch `haswbstatement` totals from 2026-11-07 (section 2.8).

| Source | Status | API / auth | Rate limits | Data and image terms | Attribution | Coverage | German | Keys | Legacy ids | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| MyAnimeList API v2 (official) | alive; reference page titled "MyAnimeList API (beta ver.)"; site ToS last updated 2026-10-16 | REST `api.myanimelist.net/v2`; OAuth2 for user data, `X-MAL-CLIENT-ID` header for public data; client registered per instance | none documented (`UNVERIFIED`; community practice about 1 req/s) | API License and Developer Agreement is shown only at client registration (login); its text is not public (`/apiconfig/references/agreement` is 404). Site ToS: personal, non-commercial licence; no automated access beyond human rates; no collating content "for use elsewhere". Images on `cdn.myanimelist.net`, terms unknown | none stated in the public pages (`UNVERIFIED`); we show "Data from MyAnimeList" with a link anyway | anime series and manga series with counts; no episode list endpoint; search, ranking, season, related works | no (`alternative_titles` = synonyms, en, ja) | yes, client id per instance | MAL ids directly; AniList ids need a mapping | **primary** (behind flag, registered when a client id is set) |
| Kitsu | alive (kitsu.app; server repo pushed 2026-06-20) | JSON:API `kitsu.app/api/edge`, read without auth (verified live) | none found in readable docs, no headers (`UNVERIFIED`) | software Apache-2.0; the terms page `kitsu.app/terms` renders only with JavaScript, not readable here (`UNVERIFIED`); posters on `media.kitsu.app` | none found (`UNVERIFIED`); we credit anyway | anime with episodes (titles, airdate, length), manga with volume and chapter counts, chapter records mostly empty; `mappings` to myanimelist/anime, anilist/anime, anidb, thetvdb/series, thetvdb/season, trakt, aozora, hulu; manga: myanimelist/manga, anilist/manga | no (titles en, en_jp, en_us, ja_jp) | yes (none needed) | **both**: `GET /mappings?filter[externalSite]=anilist/anime&filter[externalId]=ID` (verified for 163292, 53516, 11757) | **fallback and id mapper** (flag off until the owner reads the terms, Q11) |
| Jikan (unofficial MAL API) | alive; jikan-rest pushed 2026-11-03; `api.jikan.moe` not reachable from this host (connection failed) | REST v4, no auth | 3 req/s, 60 req/min, unlimited daily; 24 h cache | code MIT; data is scraped from MAL: "It scrapes the website to satisfy the need for API functionality that MyAnimeList.net lacks"; README: "You are responsible for the usage of this API. Please be respectful towards MyAnimeList's Terms Of Service" | none | same as MAL plus episodes, pictures, external links | no (title types are MAL's) | self-hosting = scraping MAL ourselves | MAL ids | **reject** (MAL ToS forbids automated collection; the official API exists) |
| AniDB | alive but every official page answers a Cloudflare challenge (HTTP 403) to this host and to the fetch tool | HTTP XML API and UDP API with registered client name and version; daily title dump | wiki (via search snippets): "You should not request more than one page every two seconds", heavy local caching required, bans for repeated requests | `UNVERIFIED` from here | `UNVERIFIED` | anime only, episodes, multilingual titles (reported) | reported, `UNVERIFIED` | client registered per instance | AniDB ids via Kitsu mappings and Wikidata P5646 | deferred: owner verifies the wiki in a browser; not v1 |
| Shikimori | alive (shikimori.io; shikimori.one redirects); code repo OSL-3.0, archived 2025-02 | REST v1/v2 and GraphQL; OAuth2 app; reads work without token (verified) | "API access is limited by 5rps and 90rpm"; User-Agent must be the OAuth2 app name; "Please do not parse the main site" | user agreement (Russian, last updated 29.10.2012) says nothing about the API or data reuse | none stated | MAL-derived catalogue; anime id equals MAL id (verified for 11757: `myanimelist_id` 11757) | no (russian, english, japanese) | yes | MAL ids directly | reject (Russian-first data, terms silent) |
| Annict | alive (annict.com; code AGPL-3.0, only docs and terms read, R-P1) | REST v1 and GraphQL, `api.annict.com`; access token required even for works | 600 req/min with token (token+IP), 60 req/min without; `RateLimit-*` headers, 429 with `Retry-After` | API terms enacted 2026-10-25, effective 2026-11-25: free, "commercial or non-commercial, usable without individual permission", works data may be stored without limit, no bulk redistribution, images must not be redistributed or altered | optional ("whether to show the source is up to you") | Japanese anime only with `mal_anime_id` and `syobocal_tid`; no manga; images are social-media images, not posters | no (title, title_kana, title_en) | yes, token per instance | MAL ids via `mal_anime_id` | not for v1; the best terms of the lot; watch |
| TMDB (existing connector) | alive | existing | existing | existing (docs/connectors/tmdb.md) | existing | keyword "anime" lists 4,472 TV series (website, 2026-11-07); `find` accepts `tvdb_id` and `wikidata_id`, not MAL ids | yes (`language=de-DE`) | yes | via Wikidata P4983 (4,022 MAL-linked items) or Kitsu thetvdb mapping plus `find` | keep for anime tracked as shows and movies |
| TheTVDB (existing connector) | alive | existing; `GET /search/remoteid/{remoteId}` and `GET /sources/types` exist (OpenAPI), the source list needs a key (`UNVERIFIED` whether MAL or AniDB are among them) | existing | existing (unclear terms, Q11) | existing | genre "Anime" page paginates to page 88 at 50 per page (about 4,400 series) | partial | yes | via Kitsu `thetvdb/series` mapping or Wikidata P4835 (3,443) | keep as is |
| Wikidata | alive | Action API, Linked Data, SPARQL (WDQS was rate-limited to 1 req/min on 2026-11-06 during an outage) | as in docs/research/board-game-sources.md | CC0 | not required; we credit | 7,277 items with MAL anime id (P4086), 8,210 MAL manga (P4087), 7,172 AniList anime (P8729), 9,106 AniList manga (P8731), 6,446 AniDB (P5646), 5,954 Kitsu anime (P11495), 603 Kitsu manga (P11494), 4,063 MangaUpdates (P11149), 6,008 aniSearch (P12477), 6,542 ANN anime (P1985), 5,319 ANN manga (P1984) | de label on 2,480 of 7,277 MAL-linked anime and 1,596 of 8,210 manga | yes | 7,140 items carry both MAL and AniList anime ids; only 32 AniList-only; `haswbstatement:P8729=163292` resolves the fixture item | **second mapping path** |
| MangaDex | alive; `api.mangadex.org` answers this host, `mangadex.org` pages answer a challenge | REST, reads without auth; `User-Agent` required, no `Via` header | about 5 req/s per IP, 429 then IP ban | Acceptable Usage Policy: "You MUST credit MangaDex", "You CANNOT run ads or paid services on your website and/or apps", "must not be profiting from our API"; images "MUST" be proxied; site terms page not readable (`UNVERIFIED`) | required | manga with `altTitles` per language and `links` (mal, al, kt, mu, bw, amz, ...); covers; it also hosts chapters (scanlations) | yes (`de` seen in altTitles) | yes (none needed) | `links.mal`, `links.al` | **not in v1** (hosts unlicensed scans; AUP bars paid services; owner decision) |
| MangaUpdates (Baka-Updates) | alive | REST v1 `api.mangaupdates.com/v1`, OpenAPI at `/openapi.yaml`; "Most API functions are public and do not require an account" | none numeric: "reasonable spacing between requests", "employ caching mechanisms" | AUP: "You will credit MangaUpdates when using data provided by this API"; no data licence; commercial use not addressed; images on `cdn.mangaupdates.com` | required | manga series: title, associated titles, type (Manga, Manhwa, Manhua, ...), year, status text, genres, categories, authors, publishers (Original, English), latest_chapter, anime start and end, licensed; no MAL or AniList id fields | no German seen (Berserk: ru, he, fa, hi, th, ja, zh, ko) | yes | none; link via Wikidata P11149, aniSearch `baka`, MangaDex `links.mu` | enrichment for manga (publishers, status), credit shown |
| aniSearch (DE) | alive; Database API "v2026-10-06", marked new, "details ... may still change" | REST `api.anisearch.com/v1/anime/{id}`, `/manga/{id}`, `titles`, `associated`, `ratings`, `slugs`; single ids free without token; token by email | 30-request allowance refilled 1/s per IP; id endpoint: wait 10 s without token, 5 s with; exports once per 24 h without token, 1 h with; mandatory meaningful `User-Agent` | "We generally support projects that benefit aniSearch free of charge. Fees may apply to other projects. Scope and terms are agreed individually."; covers carry a rights-holder copyright line; a "no redistribution to third parties" clause was seen only in a search summary (`UNVERIFIED`) | "Please credit aniSearch as your data source"; a synopsis with source anisearch must link to its entry | anime and manga with `external_ids` (myanimelist, anidb; manga: mal, baka), releases per language (en, de: title, publishers, synopsis, dubbed), episodes, characters, staff, covers | **yes** (`lang=de`, `releases.de`) | token per instance (email) | MAL ids in `external_ids` and the `associated` export | optional German enrichment behind a flag |
| Anime News Network Encyclopedia API | alive | XML `cdn.animenewsnetwork.com/encyclopedia/api.xml?anime=ID`, `?manga=ID`, `?title=~name` (primary name only); batching by id lists | 1 req/s per IP (excess delayed; `nodelay.api.xml` gives 503); suggested cache one week | "We provide this API free of charge but you must list Anime News Network as the source of the data" and "include a link to the relevant Encyclopedia entry ... on any page that displays anime/manga/person details" | required with link per page | anime and manga, alternative titles with `lang` attributes (live sample: EN, JA, IT, FR, PT, ES, DE 79 of 1,100), thumbnails | some (alternative titles `lang="DE"`) | yes (none needed) | none; Wikidata P1985/P1984 | candidate for later; XML; not v1 |
| Simkl | alive | `api.simkl.com`, client_id from simkl.com/settings/developer; search by MAL and AniDB ids | 10 GET/s per client_id | API rules: free for non-commercial and for commercial under $150/month revenue, licence above; each item must link to its Simkl page; "tracker apps using the catalog must also offer Simkl sync"; "not a general metadata CDN" | required per item | ids map (simkl, mal, anilist, anidb, kitsu, tmdb, tvdb) | unknown | yes | yes | reject (sync obligation, per-item links, revenue cap) |
| anime-offline-database (manami-project) | GitHub repo archived 2026-07-04; last release `2026-27` (2026-07-04); no successor found | weekly JSON/JSONL release files, no API | n/a | ODbL 1.0 + DbCL 1.0 (LICENSE file, read 2026-11-07); an older snapshot is described as AGPL in a search summary (`UNVERIFIED`, superseded by the current file) | ODbL notice | 41,537 entries with `sources` URLs on myanimelist, anime-planet, kitsu, anisearch, anilist, anidb, simkl, animecountdown, animenewsnetwork, livechart; pictures point at the source CDNs | no | n/a | yes, offline | offline mapping fallback only, fetched at import time, never vendored; stale since 2026-07 |
| Fribb/anime-lists, Anime-Lists/anime-lists | alive | files | n/a | **no licence file and no licence statement** (all rights reserved by default) | n/a | id mappings incl. tvdb, tmdb, imdb | n/a | n/a | n/a | reject |
| Open Library (existing doc) | alive | search and books APIs, covers API | 1 req/s anonymous, 3 req/s with identifying `User-Agent`; covers 100 req/IP per 5 min for ISBN keys, "do not crawl" | no licence statement on the developer pages (`UNVERIFIED`); software AGPL (R-P1 allows the docs) | courtesy link appreciated | manga editions by ISBN (search found "Berserk Deluxe Volume 1" with 4 ISBNs and a cover) | editions where they exist | yes | n/a | physical manga volumes by ISBN |
| Google Books | alive; anonymous quota exhausted from this host (HTTP 429 "Queries per day") so a key is needed | REST, API key | per project | Books API terms last modified 2012-04-25: no fees to users without agreement; content removal duties; Google APIs ToS apply | branding guidelines not fetched (`UNVERIFIED`) | volumes by ISBN | editions where they exist (`UNVERIFIED`) | yes | n/a | physical manga volumes by ISBN (existing path) |
| Comic Vine | alive | REST, key per site | "200 requests per resource, per hour", velocity detection | "The API is strictly for non-commercial use only"; "Do not edit, manipulate or reproduce on any other medium"; access revocable; Fandom ToS answered 403 here | "please link back to us" | US comics; manga partial (secondary reports only) | no | yes | none | reject |
| LiveChart, Anime-Planet, Notify.moe, Bangumi | alive | LiveChart and Anime-Planet: no official public API found (secondary); Notify.moe: MIT code, API not assessed; Bangumi: Chinese, licence file not found | n/a | n/a | n/a | n/a | n/a | n/a | n/a | out of scope |

## 2. Sources in detail

### 2.1 MyAnimeList official API v2

- Reference: https://myanimelist.net/apiconfig/references/api/v2 (page title "MyAnimeList API
  (beta ver.)"). Endpoints seen in the reference: `/anime` (search), `/anime/{anime_id}`,
  `/anime/ranking`, `/anime/season/{year}/{season}`, `/anime/suggestions`, `/manga`,
  `/manga/{manga_id}`, `/manga/ranking`, `/users/{user_name}`, `/users/{user_name}/animelist`,
  `/users/{user_name}/mangalist`, forum endpoints. There is no episode-list endpoint.
- Auth, quoted: "When user login is not required, the X-MAL-CLIENT-ID request header can be
  used to authenticate the client by setting your API client ID." The security scheme is an
  API key header named `X-MAL-CLIENT-ID`. OAuth2 (`main_auth`, scope `write:users`) is only
  needed for user lists.
- Fields (`fields=` parameter), from the examples: `id,title,main_picture,alternative_titles,
  start_date,end_date,synopsis,mean,rank,popularity,num_list_users,num_scoring_users,nsfw,
  created_at,updated_at,media_type,status,genres,my_list_status,num_episodes,start_season,
  broadcast,source,average_episode_duration,rating,pictures,background,related_anime,
  related_manga,recommendations,studios,statistics`; manga adds `num_volumes,num_chapters,
  authors{first_name,last_name},serialization{name}`. `alternative_titles` has `synonyms`,
  `en` and `ja` only: no German titles.
- Rate limits: not stated anywhere in the reference (`UNVERIFIED`). Secondary sources use
  about 1 request per second; we would use 1 req/s, burst 2, honour 429 and `Retry-After`.
- Terms: the "API License and Developer Agreement" is presented when a client is created at
  https://myanimelist.net/apiconfig (login). The public URL `/apiconfig/references/agreement`
  answers 404 (checked 2026-11-07) and an archive copy could not be fetched. Its content is
  therefore `UNVERIFIED`; the owner reads and accepts it when registering the client. A
  third-party guide reports a usage selector with "Hobbyist / Non-commercial" during
  registration (`UNVERIFIED`), which matters for a white-label operator that charges money.
- Site terms (https://myanimelist.net/membership/terms_of_use, "LAST UPDATED: October 16,
  2026"): the Service licence is "solely for your personal, noncommercial use"; "You also agree
  not to use or launch any automated system, including without limitation, 'offline readers,'
  'spiders,' 'robots,' etc., that accesses the Service for any malicious purpose or in a
  manner that sends more request messages to the Company servers than a human can reasonably
  produce in the same period of time by using a conventional on-line web browser, and you
  agree not to collate or aggregate any of the content available through the Service for use
  elsewhere." The same terms say MAL "may, from time to time, deploy technology such as
  application programming interfaces (or 'APIs') for our use or for use by third party
  application developers".
- Images: `main_picture.medium/large` on `cdn.myanimelist.net`; no image terms found
  (`UNVERIFIED`). We would fetch, derive our own variants and never commit them (rule 8).
- Coverage: the largest anime and manga catalogue in this comparison (Wikidata links 7,277
  anime and 8,210 manga items to it; MAL's own totals were not read).
- Risks: "beta" label, undocumented limits, non-public agreement, no episode data, images
  terms unknown, non-commercial purpose at registration. Mitigation: KICKOFF Q11 (feature flag
  on only when `KK_MAL_CLIENT_ID` is set, open question recorded), 7-day cache, attribution
  text with link on item pages and the Data sources page.

### 2.2 Kitsu

- Licence of the software: `hummingbird-me/kitsu-server` and `kitsu-web` are Apache-2.0
  (GitHub API, 2026-11-07); the `api-docs` repository has no licence field and was last pushed
  2024-08-17 (its README says the docs are being converted from API Blueprint to OpenAPI 3).
  The Apiary page and `hummingbird-me.github.io/api-docs` render only with JavaScript.
- Terms: `https://kitsu.app/terms` and `kitsu.io/terms` return a JavaScript shell ("Please
  enable JavaScript to get the best Kitsu experience"); the terms template in the Apache-2.0
  web repository is a 58-byte stub. The terms are therefore `UNVERIFIED` and the connector
  falls under KICKOFF Q11 until the owner reads them in a browser.
- API (verified live, no auth, `Accept: application/vnd.api+json`):
  `GET https://kitsu.app/api/edge/anime?filter[text]=...&include=mappings` returns `titles`
  (`en`, `en_jp`, `en_us`, `ja_jp`), `canonicalTitle`, `episodeCount`, `subtype`, `status`,
  `startDate`, `posterImage` on `media.kitsu.app`. `GET /anime/{id}/episodes` returns episode
  number, titles, airdate, length (25 for Sword Art Online). `GET /manga/{id}/chapters` exists
  but the Berserk sample has 5,000 placeholder chapters with null titles: not usable for
  chapter metadata; `volumeCount` was 0 and `chapterCount` null for Berserk, so counts are
  unreliable too.
- Mappings: `GET /mappings?filter[externalSite]=<site>&filter[externalId]=<id>&include=item`
  resolved `myanimelist/anime 53516`, `myanimelist/anime 11757` and `anilist/anime 163292`
  each to exactly one Kitsu anime. Sites seen on popular items: `myanimelist/anime`,
  `anilist/anime`, `anidb`, `thetvdb/series`, `thetvdb/season`, `thetvdb`, `trakt`, `aozora`,
  `hulu`; manga: `myanimelist/manga`, `anilist/manga`. The AniList anime id for SAO equals
  the MAL id (11757) on Kitsu and on Wikidata; that is a coincidence of early AniList ids, not
  a rule (163292 maps to a different MAL id).
- Rate limits: none in the readable material and no rate-limit headers in responses
  (`UNVERIFIED`). We would use 2 req/s, burst 4, and back off on 429.
- Risks: unknown terms, small team, weak manga chapter data, no German titles.

### 2.3 Jikan (unofficial MAL API)

- Licence: MIT (`jikan-me/jikan`, `jikan-me/jikan-rest`, GitHub API 2026-11-07). OpenAPI
  document read from `storage/api-docs/api-docs.json` in the MIT repository (version 4.0.0):
  "Jikan is an Unofficial MyAnimeList API. It scrapes the website to satisfy the need for a
  complete API - which MyAnimeList lacks." Rate limits: "Daily Unlimited, Per Minute 60
  requests, Per Second 3 requests. Note: It's still possible to get rate limited from
  MyAnimeList.net instead." Caching: "All requests are cached for 24 hours." Read-only.
  README: Docker self-hosting (`jikanme/jikan-rest`), "Tests may fail due to rate limit from
  MyAnimeList (HTTP 429)", and the disclaimer "You are responsible for the usage of this API.
  Please be respectful towards MyAnimeList's Terms Of Service".
- Compatibility with MAL's terms: MAL's site terms forbid automated systems beyond human
  request rates and forbid collating or aggregating content for use elsewhere (section 2.1).
  Jikan does exactly that on our behalf; a self-hosted Jikan would make this application the
  scraper. The public instance adds a third-party dependency with no terms of its own that
  could be shut down by MAL at any time, and `api.jikan.moe` did not even answer this host.
  Since the official API covers search and details with a free client id, there is no reason
  to take that risk. Verdict: reject; the only thing Jikan adds (episode lists, pictures,
  external links) can come from Kitsu and TMDB.

### 2.4 AniDB

- Every official page (https://wiki.anidb.net/API, `HTTP_API_Definition`,
  `UDP_API_Definition`, the MediaWiki `api.php`, `anidb.net/policy/*`, the title dump
  `anidb.net/api/anime-titles.xml.gz`) answered a Cloudflare challenge (HTTP 403, "Just a
  moment...") both to this host and to the fetch tool. Everything below is `UNVERIFIED` and
  comes from search-engine snippets of the official wiki and from forum posts.
- Reported rules: HTTP XML API with registered `client` (lower-case registered identifier,
  "this is not your project name"), `clientver` and `protover`; "all users of this API should
  employ heavy local caching"; "You should not request more than one page every two seconds";
  the API "should not be used to 'download' AniDB", bans are automatic and reported to decay
  after about 24 hours; a daily `anime-titles` dump exists. Data licence and attribution rules
  were not found. AniDB is reported to carry multilingual titles including German, which is
  why it matters for us, but this could not be checked.
- Verdict: deferred. The owner opens the wiki in a browser and records the rules and the data
  licence; only then can a connector be written (XML, strict pacing, per-instance registered
  client). Until then AniDB ids are only stored as cross-references (from Kitsu `anidb`
  mappings and Wikidata P5646: 6,280 of the 7,277 MAL-linked items also carry an AniDB id).

### 2.5 Shikimori

- Code: `shikimori/shikimori` is OSL-3.0 (not GPL-family) and archived (2025-02). Site alive
  at shikimori.io (shikimori.one redirects there with 301).
- API docs (https://shikimori.io/api/doc): three versions, GraphQL preferred; "Please do not
  parse the main site. Fetch all necessary data via API."; "API works with HTTPS protocol
  only"; "OAuth2 is used for authentication"; "API access is limited by 5rps and 90rpm";
  "Add your Oauth2 Application name to User-Agent requests header. Don't mimic a browser."
  New posters are only in GraphQL.
- Live check: `GET https://shikimori.io/api/animes/11757` without a token returned name,
  russian, english, japanese, synonyms, episodes 25, kind tv, `myanimelist_id` 11757 (equal
  to the Shikimori id).
- Terms: https://shikimori.io/terms, Russian user agreement, "Last updated: 29.10.2012", no
  clause about the API or about reusing catalogue data; it forbids using information obtained
  through the site for commercial purposes where that violates other users' rights.
- Verdict: reject. The catalogue is MAL-derived, the audience and titles are Russian, there
  are no German titles and the terms are silent on data reuse.

### 2.6 Annict

- Code AGPL-3.0; per R-P1 only the public API docs and terms were read, nothing else. The
  developer site moved: developers.annict.com now says the docs live in the Annict help
  (https://wikino.app/s/annict).
- API terms (https://annict.com/terms/api, enacted 2026-10-25, effective 2026-11-25): the API
  is free of charge; commercial or non-commercial use needs no individual permission; works
  data may be stored with no limit on period or count and need not be deleted after stopping
  API use; no supplying the data as a bulk dataset; images must not be redistributed or
  altered; attribution is optional ("whether to show Annict as the data source is up to you");
  one year's notice before an API version is discontinued; commercial AI/ML use needs
  permission.
- Rate limits (help page "Annict rate limits"): 600 requests per minute per access token and
  source IP, 60 per minute per IP without a token, `RateLimit-Limit/Remaining/Reset` headers,
  429 with `Retry-After`.
- Data (REST v1 works page): `title`, `title_kana`, `title_en`, `media`, `season_name`,
  `episodes_count`, `official_site_url`, `wikipedia_url`, `syobocal_tid`, `mal_anime_id`,
  social-media images; filters `filter_title`, `filter_ids`, `filter_season`. Japanese anime
  only, no manga, no posters, no German.
- Verdict: the cleanest terms in this comparison, but the data does not fit a German-speaking
  user's library. Worth revisiting for Japanese broadcast data; not v1.

### 2.7 TMDB and TheTVDB (existing connectors)

- TMDB: the keyword page https://www.themoviedb.org/keyword/210024-anime/tv shows "4.472
  Serien" (2026-11-07). The `find` endpoint
  (https://developer.themoviedb.org/reference/find-by-id) accepts `external_source` values
  `imdb_id`, `facebook_id`, `instagram_id`, `tvdb_id`, `tiktok_id`, `twitter_id`,
  `wikidata_id`, `youtube_id`: no MAL or AniList ids, but a Wikidata id or a TVDB id from
  Kitsu's `thetvdb/series` mapping gets us from a MAL item to a TMDB show. Episode data,
  German titles and images are already handled.
- TheTVDB: the genre page https://thetvdb.com/genres/anime paginates to page 88 at 50 rows
  (about 4,400 series). The OpenAPI document (`docs/swagger.yml`, 2026-11-07) has
  `GET /search/remoteid/{remoteId}` and `GET /sources/types`; which remote sources exist needs
  an authenticated call (`UNVERIFIED`).
- Mapping fit: anime items in MAL are per season or cour (the fixture's "Sword Art Online" MAL
  11757 is season 1 with 25 episodes; Wikidata holds it as Q132174429 "Sword Art Online,
  season 1"), while TMDB and TVDB hold one series with seasons. A MAL-shaped anime kind cannot
  be replaced by TMDB without an episode-offset mapping, which the unlicensed Anime-Lists file
  provides and we cannot use. So TMDB stays the source for anime the user tracks as a show, and
  the anime kind keeps MAL-shaped identities.

### 2.8 Wikidata

- Licence CC0 for structured data (docs/research/board-game-sources.md, section 2.1).
- Properties (found with `wbsearchentities`, type property, 2026-11-07): P4086 MyAnimeList
  anime ID, P4087 MyAnimeList manga ID, P8729 AniList anime ID, P8731 AniList manga ID, P5646
  AniDB anime ID, P8888 AniDB episode ID, P11495 Kitsu anime ID, P11494 Kitsu manga ID, P11149
  MangaUpdates manga ID, P12477 aniSearch anime ID, P1985 Anime News Network anime ID, P1984
  Anime News Network manga ID. No property exists for MangaDex, Shikimori, Annict,
  Anime-Planet, LiveChart, Simkl, Bangumi, Notify.moe or Comic Vine (search returned none).
- Counts (CirrusSearch `haswbstatement`, which is not subject to the WDQS limit): see the
  table. Combined: 7,140 items with both P4086 and P8729 (32 AniList-only), 8,005 manga with
  both P4087 and P8731 (1,101 AniList-only), 6,280 MAL-anime items with AniDB ids, 5,837 with
  Kitsu ids, 5,943 with aniSearch ids, 4,022 with TMDB TV ids (P4983), 3,443 with TVDB ids
  (P4835), 3,636 MAL-manga items with MangaUpdates ids. German labels: 2,480 of 7,277 anime,
  1,596 of 8,210 manga.
- Fixture ids: `haswbstatement:P4086=53516` -> Q126720032; `P4086=11757` -> Q132174429 (labels
  en "Sword Art Online, season 1", no de label; P8729 11757, P5646 8692, P11495 6589, P1985
  13858, P12477 7335, no TMDB or TVDB id); `P8729=163292` -> Q117354421. One search request per
  id, no SPARQL needed.
- Caveat: Wikidata models some anime at season level and some at series level, so an id hit
  must be checked against episode counts before it is trusted.
- WDQS note: on 2026-11-06 the query service answered "Aggressively rate-limiting to 1 req /
  min - this rule was created during active wdqs outage". The importer should use the search
  API and `Special:EntityData` only.

### 2.9 MangaUpdates (Baka-Updates Manga)

- API: https://api.mangaupdates.com/ (OpenAPI 3 at https://api.mangaupdates.com/openapi.yaml,
  354 KB, read 2026-11-07). "Most API functions are public and do not require an account."
  "MangaUpdates makes no warranties about service availability, correctness of the data, or
  anything else." Acceptable Use Policy: "You will credit MangaUpdates when using data
  provided by this API. You will use reasonable spacing between requests so as not to
  overwhelm the MangaUpdates servers, and employ caching mechanisms when accessing data." and
  no deceiving users, illegal actions, spam or damaging the database; "We reserve the right to
  change this policy at any time." No data licence, nothing on commercial use.
- Live: `POST /v1/series/search {"search":"Berserk","perpage":1}` returned 61 hits; `GET
  /v1/series/51239621230` returned title, associated titles (10, none German), description,
  image on `cdn.mangaupdates.com`, type "Manga", year 1989, status text "43 Volumes (Ongoing)"
  with Markdown, genres, 264 categories, authors, publishers (Dark Horse "English", Hakusensha
  "Original"), `latest_chapter` 386, `anime` start/end text, `licensed` true. No MAL or AniList
  ids in the record.
- Rule 7: MangaUpdates also catalogues scanlation release lists (`/releases/*` endpoints).
  Those endpoints must never be called; only `/series/search` and `/series/{id}` are in scope.
- Verdict: enrichment for manga (publisher, licensing status, type) with the credit line.

### 2.10 MangaDex

- API docs: https://api.mangadex.org/docs/ (landing page read via the fetch tool; the site
  pages behind `mangadex.org` answer a challenge to this host). Acceptable Usage Policy,
  quoted: "We make our API entirely public and free to use exclusively for projects that
  satisfy the following:" "You MUST credit MangaDex", "You MUST credit scanlation groups (and
  honor their content removal requests) if you allow reading chapters", "You CANNOT run ads or
  paid services on your website and/or apps", "You may accept donations however.", "The point
  is that you must not be profiting from our API." Limitations page: "A global limit of
  approximately 5 requests per second per IP address", `User-Agent` required and not spoofed,
  no `Via` header, 429 then a temporary IP ban (403), images: clients "MUST proxy the requests
  your users make to our services".
- Live: `GET https://api.mangadex.org/manga?title=Berserk&limit=1` works without auth and
  returns `title`, `altTitles` with language keys (de, en, ja, ko, ru, zh, ...), `links`
  (`al` AniList, `mal`, `kt` Kitsu, `mu` MangaUpdates, `ap`, `bw`, `amz`, `cdj`, `ebj`, `raw`,
  `engtl`), `status`, `year`, `lastVolume`, `lastChapter`, `availableTranslatedLanguages`.
- Rule 7 assessment: using only `/manga`, `/cover` and never `/chapter`, `/at-home` or
  `/feed` would not search for, index or download releases, so the letter of rule 7 is
  satisfiable. But the site's core content is fan-translated scans of licensed manga, the AUP
  binds us to credit MangaDex on every page that shows its data (which advertises a scanlation
  host inside a library app), and "You CANNOT run ads or paid services" conflicts with the
  white-label operator who may charge for hosting. Verdict: not in v1, no connector; recorded as
  an owner question. Its `links` object is the richest manga id map, but Kitsu's manga mappings
  and Wikidata cover the MAL and AniList pairs we need.

### 2.11 aniSearch

- https://api.anisearch.de/ (German) and https://api.anisearch.com/ (English): "Database API"
  (Id, Titles, Associated, Ratings, Slugs, Dubs endpoints) and "User API" (OAuth 2.0). "We
  generally support projects that benefit aniSearch free of charge. Fees may apply to other
  projects. Scope and terms are agreed individually. Send us a brief description of your
  project at api@anisearch.com." A search summary of the same site reported "Redistributing
  any data obtained via the API to third parties is strictly prohibited"; that sentence was not
  on the pages fetched (`UNVERIFIED`).
- Database API guide (v2026-10-06): "The Database API is new. Based on feedback, details of
  Version 1 (such as fields, values and limits) may still change." Mandatory meaningful
  `User-Agent` (generic agents get 403 or 423 and a temporary IP block). General limits: per
  IP "an allowance of up to 30 requests, refilled at 1 request per second"; without a token a
  shared pool of 20 refilled at 10/s; 429 with `Retry-After`. Server-side cache of title data
  3 to 7 days.
- Anime guide: "Single IDs and exports are available without a token. For a token, email
  api@anisearch.com with your website or app and how you plan to integrate aniSearch." Waits:
  id endpoint 10 s without token, 5 s with; `titles`, `dubs`, `associated`, `ratings`,
  `slugs` exports once per 24 h without token, 1 h with. Attribution: "Please credit aniSearch
  as your data source. When reusing a synopsis with source anisearch, include a link to the
  corresponding aniSearch entry directly alongside it." Every response repeats this in an
  `attribution` and `license.synopsis` field. Record fields: `external_ids` (myanimelist,
  anidb), `title`, `title_native`, status, year, season, type, episodes, duration, aired
  dates, studios, genres and tags, `cover.medium/large` on `cdn.anisearch.com` with a
  `copyright` line, synonyms, related, `releases` per language with `de` and `en` title,
  publishers, synopsis and dubbing status; `?lang=de` for German output. Manga guide: same
  shape with volumes, chapters, publishers; `associated` maps `mal` and `baka`
  (MangaUpdates).
- Verdict: the only source in this list with German titles, German publishers and German
  release dates for both anime and manga, plus MAL and AniDB ids. Terms are individual and the
  API is new, so: optional enrichment connector behind a flag, token requested by the operator,
  attribution line and synopsis link exactly as required, no synopsis reuse without the link.
  Covers carry a rights-holder copyright and should be treated like other provider images
  (fetched and cached, never committed).

### 2.12 Anime News Network Encyclopedia API

- https://www.animenewsnetwork.com/encyclopedia/api.php: XML feed; details via
  `https://cdn.animenewsnetwork.com/encyclopedia/api.xml?anime=ID`, `?manga=ID`, `?title=ID`,
  batching with repeated parameters or slash-separated ids, `?title=~name` matches the primary
  name only; reports via `reports.xml?id=...&type=anime&nlist=50`. Terms of Service, quoted:
  "We provide this API free of charge but you must list Anime News Network as the source of
  the data" and "include a link to the relevant Encyclopedia entry (like ' full details at
  Anime News Network ') on any page that displays anime/manga/person details". Usage Limits:
  "The API is rate-limited to 1 request per second per IP address; requests over this threshold
  will be delayed." The page suggests caching details "for a week".
- Live: `?title=~Sword Art Online` returned several `anime` elements with `info` children
  carrying `lang` attributes (sample counts EN 300, JA 193, IT 148, FR 147, PT 129, ES 88, DE
  79) and `Picture` thumbnails on `cdn.animenewsnetwork.com`.
- Verdict: a workable second source of German alternative titles with simple terms, but XML,
  primary-name search only and a per-page link requirement. Candidate after aniSearch.

### 2.13 Simkl

- API rules (https://api.simkl.org/api-rules.md): free for non-commercial apps and for
  commercial apps "that generate less than $150 / month in revenue", licence above that; each
  displayed item must "link back to the Simkl page for that specific item"; "tracker apps using
  the catalog must also offer Simkl sync"; "Simkl is not a general metadata CDN (use TVDB or
  TMDB for raw metadata)"; 10 GET/s per client_id. Verdict: reject.

### 2.14 Offline mapping files

- anime-offline-database: LICENSE file "made available under the Open Database License 1.0"
  with contents under the Database Contents License 1.0 (read 2026-11-07). README: 41,537
  entries (week 27 of 2026), weekly releases, fields `sources`, `title`, `type`, `episodes`,
  `status`, `animeSeason`, `picture`, `thumbnail`, `synonyms`, `relatedAnime`, `tags`,
  `duration`, `studios`, `producers`, `score`; per-site files (`myanimelist-minified.json`,
  `anilist-minified.json`, `kitsu-minified.json`, `anidb-minified.json`,
  `animenewsnetwork-minified.json`) in the release `2026-27` of 2026-07-04. The GitHub
  repository "was archived by the owner on Jul 4, 2026" with no stated reason and no successor
  found. ODbL is share-alike for databases that are publicly conveyed; using the file at import
  time to translate ids, without vendoring it or republishing a derived database, keeps us
  clear of that. cargo-deny's allow-list does not include ODbL, which is another reason not to
  vendor it.
- Fribb/anime-lists and Anime-Lists/anime-lists: no LICENSE file (404 on the default branch)
  and no licence sentence in either README. Without a licence nothing may be copied: reject,
  even though they are the only TVDB/TMDB episode-offset mappings around.

### 2.15 Volumes as books

- Open Library (https://openlibrary.org/developers/api): "1 request per second" anonymous,
  "3 requests per second" with `User-Agent: MyLibraryApp (contact@example.org)`; covers API
  `https://covers.openlibrary.org/b/$key/$value-$size.jpg`, "Currently only 100 requests/IP are
  allowed for every 5 minutes" for ISBN and other non-OLID keys, "Please, do not crawl our
  cover API", a courtesy link back is appreciated. No data licence sentence on the developer
  or dumps pages (`UNVERIFIED`). Live: "Berserk Deluxe Volume 1" (OL20066940W, 2019, 4 ISBNs,
  cover id) found; "Sword Art Online" light novels found. Manga volumes are ordinary editions
  here; series grouping does not exist.
- Google Books: terms https://developers.google.com/books/terms ("Last modified: April 25,
  2012"): "You may not charge users any fee for the use of your application" without a
  separate agreement, content removal on request, the Google APIs Terms of Service apply.
  The anonymous quota was exhausted from this host (429 "Queries per day"), so an API key is
  required for any real use; German edition coverage is `UNVERIFIED`.
- Consequence: physical manga volumes are books. When a user adds a volume by ISBN, the
  existing book path (Google Books with key, Open Library once built under R-P1) supplies it
  and the item links to the manga series entry.

### 2.16 Comic Vine

- https://comicvine.gamespot.com/api/: "You must log in to get a Comic Vine API Key", "sign up
  for a unique api key for each site individually", "We officially support 200 requests per
  resource, per hour", "The API is strictly for non-commercial use only", "On any page you use
  our data, please link back to us", "Do not edit, manipulate or reproduce on any other
  medium", "We can refuse access at anytime". The linked Fandom terms answered 403 to this
  host. Manga coverage is reported partial by third parties. Verdict: reject for manga.

## 3. German titles

| Source | German title data |
|---|---|
| aniSearch | yes: `releases.de.title`, publishers, synopsis, dubbed; `lang=de` |
| TMDB | yes for shows and movies (`language=de-DE`), existing |
| Wikidata | de labels on about a third of MAL-linked anime and a fifth of manga |
| ANN | alternative titles with `lang="DE"` on a minority of entries |
| MangaDex | `altTitles.de` (not used) |
| AniDB | reported, `UNVERIFIED` |
| MAL, Kitsu, Jikan, Shikimori, Annict, MangaUpdates | no |

## 4. Matching the legacy export

The fixture (`src/rust/crates/testkit/fixtures/import/legacy/export.json`) has anime items
with `source` = `myanimelist` (identifier = MAL anime id, e.g. 53516, 11757) and `anilist`
(identifier = AniList anime id, e.g. 163292); `source_id` carries the romaji title. The full
export also has manga (README), expected as `myanimelist`/`anilist` with manga ids.

Resolution chain per item (stop at the first hit, store every id learned in the item's
external-id set so later sources and re-imports match without a lookup):

1. `myanimelist`: the identifier is the MAL id. With the MAL connector enabled, fetch
   `/anime/{id}` (or `/manga/{id}`) and create the item. Without it, resolve through Kitsu
   `mappings` (`myanimelist/anime` or `myanimelist/manga`) to the Kitsu record.
2. `anilist`: Kitsu `GET /mappings?filter[externalSite]=anilist/anime&filter[externalId]=ID&
   include=item` (manga: `anilist/manga`); the item's other mappings yield the MAL id (verified
   for 163292 -> Kitsu 47272; the MAL id comes from the sibling `myanimelist/anime` mapping of
   that item, fetched with `GET /anime/{kitsuId}/mappings`).
3. Wikidata: `action=query&list=search&srsearch=haswbstatement:P8729=ID` (manga P8731), then
   `Special:EntityData/Q....json` for P4086/P4087 (MAL), P5646, P11495, P4983, P4835, P12477.
   Check `episodeCount` against the export's highest `anime_episode_number` when the item may
   be a season-level entity.
4. anime-offline-database release file (ODbL, downloaded to a temporary directory at import
   time if the operator opts in, never vendored): find the entry whose `sources` contains
   `https://anilist.co/anime/ID` and take the `myanimelist.net/anime/` and `kitsu.app/anime/`
   urls. Anime only; stale after 2026-07.
5. Title search by `source_id` on the primary connector, presented to the user for
   confirmation (the importer never auto-links on a title match alone).
6. Unresolved items are imported as "unmatched" with their raw source and identifier kept, so
   a later source (AniDB, aniSearch) can still link them.

## 5. Recommendation

Anime:

- Primary: MyAnimeList API v2, cargo feature `mal`, registered only when
  `KK_MAL_CLIENT_ID` is configured (same pattern as TVDB, Q11 because the developer agreement
  is not public), `X-MAL-CLIENT-ID` for public data, 1 req/s, 7-day cache, attribution "Data
  from MyAnimeList" with a link on item pages and the Data sources page. Anime items keep
  MAL-shaped identities (per season or cour) because that is how the owner's history is
  recorded (episode numbers per entry).
- Fallback and id mapper: Kitsu, cargo feature `kitsu`, no key, 2 req/s, flag off by default
  until the owner has read `kitsu.app/terms` in a browser (Q11 open question); used by the
  importer for MAL and AniList id resolution and by the catalogue when MAL is not configured;
  episodes from `/anime/{id}/episodes`.
- Shows: TMDB stays the source for anime the user tracks as a show or movie; link anime items
  to TMDB through Wikidata (P4983) or Kitsu's TVDB mapping plus TMDB `find`.
- German enrichment (phase 2): aniSearch behind a flag with an operator-requested token;
  attribution and synopsis link as required. ANN as the second candidate.
- Not used: Jikan (MAL terms), Shikimori, Annict (data fit), Simkl (terms), AniDB (unverified
  rules; deferred to the owner), Fribb and Anime-Lists (no licence).

Manga:

- Primary: MyAnimeList API v2 `/manga` (same client id; `num_volumes`, `num_chapters`,
  `authors`, `serialization`).
- Fallback and id mapper: Kitsu manga with `myanimelist/manga` and `anilist/manga` mappings
  (counts unreliable; use for identity and titles).
- Enrichment: MangaUpdates `/series/search` and `/series/{id}` for publishers, licensing status
  and type, with "Data from MangaUpdates" credit; never the `/releases/*` endpoints (rule 7).
  Linked through Wikidata P11149, aniSearch `baka`, or title match with confirmation.
- Volumes: physical volumes are books by ISBN through the existing book connectors; a volume
  item links to its manga series.
- Not used: MangaDex (section 2.10, owner question), Comic Vine (non-commercial only, weak
  manga coverage).

## 6. Draft DECISIONS lines (not applied; DEC-203 is the last entry today)

```
DEC-204 | R-P3: anime catalogue source after AniList (DEC-108) | MyAnimeList API v2 behind cargo feature `mal`, registered only when `KK_MAL_CLIENT_ID` is set (its API License and Developer Agreement is shown only at client registration and is not public: Q11 open question for the owner to read and accept it); `X-MAL-CLIENT-ID` for public data, 1 req/s burst 2, 7-day cache, attribution with link on item and Data sources pages; anime items keep MAL-shaped (per season/cour) identities | Kitsu as primary (terms unreadable); TMDB only (season mapping impossible without the unlicensed Anime-Lists file); AniDB (official docs unreachable) | disable the feature, the external-id set keeps MAL ids for a later switch | docs/research/anime-manga-sources.md 2.1, 5
DEC-205 | Key-less fallback and MAL/AniList id mapping service | Kitsu (`kitsu.app/api/edge`, JSON:API, Apache-2.0 platform) behind cargo feature `kitsu`, off by default until the owner has read kitsu.app/terms (JavaScript-only page, unverifiable from the build host: Q11); 2 req/s; `mappings` resolve myanimelist/anime, anilist/anime, anidb, thetvdb/series (verified for the three fixture ids) | Wikidata only (7,140 of 7,277 MAL anime items carry AniList ids but season-level items need checks); anime-offline-database only (archived 2026-07) | turn the flag off; mappings are stored as external ids, nothing else depends on Kitsu | docs/research/anime-manga-sources.md 2.2, 4
DEC-206 | How the legacy importer matches `anilist` and `myanimelist` items | Chain: MAL id direct (MAL connector) -> Kitsu mappings -> Wikidata search `haswbstatement:P8729=ID` / `P8731=ID` then Special:EntityData -> optional anime-offline-database release file downloaded at import time (ODbL, never vendored) -> title search on the primary connector with user confirmation -> import as unmatched with raw ids kept; every id learned is stored in the item's external-id set | auto-link on title match (rejected: false positives); require MAL for all (rejected: AniList-only items would fail) | each step is a separate resolver; remove or reorder | docs/research/anime-manga-sources.md 4
DEC-207 | Manga sources | MAL `/manga` primary (same client id), Kitsu manga fallback and id mapper (`myanimelist/manga`, `anilist/manga`; counts unreliable), MangaUpdates `/series/search` and `/series/{id}` for publishers, status and type with the required "credit MangaUpdates" line and never its `/releases/*` endpoints (rule 7); physical volumes are books by ISBN through the book connectors and link to the series | MangaDex (DEC-208); Comic Vine (strictly non-commercial, weak manga coverage); aniSearch manga (phase 2 enrichment) | disable per feature; MangaUpdates ids live in the external-id set | docs/research/anime-manga-sources.md 2.9, 2.15, 5
DEC-208 | MangaDex as a manga metadata source | Not used in v1 and no connector written: the API itself is public and free, metadata-only use (`/manga`, `/cover`) would satisfy the letter of rule 7, but the site hosts fan-translated scans, its Acceptable Usage Policy requires crediting MangaDex on every page that shows its data and forbids "ads or paid services", which a white-label operator may run; recorded as an owner question | metadata-only connector behind an off-by-default flag (possible later if the owner accepts the policy) | owner can approve a flagged, metadata-only connector | docs/research/anime-manga-sources.md 2.10
DEC-209 | Jikan (unofficial MAL scraper API) | Rejected, public instance and self-hosted alike: MAL's terms (2026-10-16) forbid automated access beyond human rates and collating content for use elsewhere, Jikan "scrapes the website", and the official API covers our reads with a free client id; `api.jikan.moe` did not answer the build host | use the public instance (third-party dependency on scraped data) | n/a | docs/research/anime-manga-sources.md 2.3
DEC-210 | German titles and German releases for anime and manga | aniSearch Database API (v2026-10-06, still changing) as an optional enrichment connector in a later phase, behind a flag, token requested by the operator from api@anisearch.com, mandatory custom User-Agent, 1 request per 10 s (5 s with token) on id endpoints, attribution "Data from aniSearch" and the synopsis link rule honoured; ANN Encyclopedia API as the second candidate (1 req/s, source and link per page) | Wikidata de labels only (about a third of anime); AniDB (unverified) | remove the flag; aniSearch and ANN ids stay as cross-references | docs/research/anime-manga-sources.md 2.11, 2.12, 3
DEC-211 | Offline id-mapping files | anime-offline-database (ODbL 1.0 + DbCL 1.0; GitHub repo archived 2026-07-04, last release 2026-27) may be fetched by the importer at run time as an optional resolver, never vendored or redistributed (ODbL share-alike, not in the cargo-deny allow-list); Fribb/anime-lists and Anime-Lists/anime-lists are rejected: no licence file or statement | vendor the file (rejected) | drop the resolver | docs/research/anime-manga-sources.md 2.14
DEC-212 | AniDB | Deferred: every official page (wiki, policy, title dump) answers a Cloudflare challenge to the build host and to the fetch tool, so its rules, licence and attribution are unverified; AniDB ids are stored as cross-references (Kitsu `anidb` mapping, Wikidata P5646) until the owner reads wiki.anidb.net/HTTP_API_Definition and wiki.anidb.net/API in a browser and records the terms | write a client from secondary sources (rejected, rule 1) | owner ruling with the verified rules | docs/research/anime-manga-sources.md 2.4
```

## 7. Pages that could not be read from this host (2026-11-07)

| URL | Result |
|---|---|
| https://myanimelist.net/apiconfig/references/agreement | HTTP 404 (agreement only shown at client registration) |
| https://web.archive.org/... of the same | fetch tool cannot reach web.archive.org |
| https://wiki.anidb.net/API, /HTTP_API_Definition, /UDP_API_Definition, wiki api.php, https://anidb.net/policy/*, http://anidb.net/api/anime-titles.xml.gz | HTTP 403 Cloudflare challenge (host and fetch tool) |
| https://kitsu.app/terms, https://kitsu.io/terms | JavaScript shell, no text |
| https://kitsu.docs.apiary.io/, https://hummingbird-me.github.io/api-docs | JavaScript-rendered, no text |
| https://api.jikan.moe/v4/... | connection failed from the host; fetch tool returned nothing |
| https://mangadex.org/compliance, https://api.mangadex.org/docs/2-limitations/ (host) | challenge page; the limitations page was read through the fetch tool |
| https://www.fandom.com/terms-of-service-pp1 (Comic Vine terms) | HTTP 403 / 402 |
| https://www.googleapis.com/books/v1/volumes (anonymous) | HTTP 429 daily quota of the anonymous project |
| https://query.wikidata.org/sparql | HTTP 429 "Aggressively rate-limiting to 1 req / min" (outage rule); counts taken via the search API instead |
| https://developers.annict.com/docs/... | HTTP 404; docs moved to https://wikino.app/s/annict |

## 8. Sources

All opened 2026-11-06/07 unless noted.

- MyAnimeList API reference: https://myanimelist.net/apiconfig/references/api/v2
- MyAnimeList Terms of Use: https://myanimelist.net/membership/terms_of_use (last updated 2026-10-16)
- Kitsu live API: https://kitsu.app/api/edge/ (anime, manga, mappings, episodes, chapters); repos https://github.com/hummingbird-me/kitsu-server, https://github.com/hummingbird-me/kitsu-web (Apache-2.0), https://github.com/hummingbird-me/api-docs
- Jikan: https://github.com/jikan-me/jikan-rest (MIT; README.MD and storage/api-docs/api-docs.json), https://github.com/jikan-me/jikan (MIT)
- AniDB (not readable): https://wiki.anidb.net/API, https://wiki.anidb.net/HTTP_API_Definition; secondary: search snippets and https://anidb.net/forum/thread/85761 (not opened)
- Shikimori: https://shikimori.io/api/doc, https://shikimori.io/terms, https://shikimori.io/api/animes/11757, https://github.com/shikimori/shikimori (OSL-3.0, archived)
- Annict: https://annict.com/terms/api (enacted 2026-10-25), https://wikino.app/s/annict/pages/419 (rate limits), https://wikino.app/s/annict/pages/364 (REST works), https://github.com/annict/annict (AGPL-3.0, licence only)
- TMDB: https://developer.themoviedb.org/reference/find-by-id, https://www.themoviedb.org/keyword/210024-anime/tv
- TheTVDB: https://raw.githubusercontent.com/thetvdb/v4-api/main/docs/swagger.yml, https://thetvdb.com/genres/anime
- Wikidata: https://www.wikidata.org/w/api.php (wbsearchentities, list=search with haswbstatement), https://www.wikidata.org/wiki/Special:EntityData/Q132174429.json
- MangaDex: https://api.mangadex.org/docs/, https://api.mangadex.org/docs/2-limitations/, https://api.mangadex.org/manga?title=Berserk&limit=1
- MangaUpdates: https://api.mangaupdates.com/, https://api.mangaupdates.com/openapi.yaml, live /v1/series/search and /v1/series/51239621230
- aniSearch: https://api.anisearch.de/, https://api.anisearch.de/docs/api_database.html, https://api.anisearch.de/docs/api_database_anime.html, https://api.anisearch.de/docs/api_database_manga.html
- Anime News Network: https://www.animenewsnetwork.com/encyclopedia/api.php, https://cdn.animenewsnetwork.com/encyclopedia/api.xml?title=~Sword%20Art%20Online
- Simkl: https://api.simkl.org/, https://api.simkl.org/api-rules.md, https://api.simkl.org/guides/anime
- anime-offline-database: https://github.com/manami-project/anime-offline-database (archived), LICENSE and README on the master branch, releases/latest (2026-27)
- Fribb/anime-lists and Anime-Lists/anime-lists: README.md of each; LICENSE 404
- Open Library: https://openlibrary.org/developers/api, https://openlibrary.org/dev/docs/api/covers, https://openlibrary.org/developers/dumps, https://openlibrary.org/search.json
- Google Books: https://developers.google.com/books/terms
- Comic Vine: https://comicvine.gamespot.com/api/
- Legacy fixture: src/rust/crates/testkit/fixtures/import/legacy/export.json and README.md
