# Connector: TMDB (`tmdb`)

Date checked: 2026-11-05

| Item | Value |
|---|---|
| Kinds | movie, show |
| Capabilities | search, details, images, episodes (WP4-7) |
| API docs | https://developer.themoviedb.org/reference/intro/getting-started (API v3) |
| Terms | https://www.themoviedb.org/api-terms-of-use ("last updated on: October 20, 2023") |
| FAQ (pricing) | https://developer.themoviedb.org/docs/faq |
| Licence | Proprietary API terms. Free for non-commercial use with attribution; commercial use needs a licence from TMDB (FAQ: "Our API is free to use for non-commercial purposes as long as you attribute TMDB as the source of the data and/or images"). |
| Auth | Bearer token (API Read Access Token, a JWT) in the `Authorization: Bearer <token>` header (https://developer.themoviedb.org/docs/authentication-application). Env: `SOURCE_TMDB_API_KEY`. |
| Rate limits | The old limit (40 requests per 10 s) was disabled on 2019-12-16; upper limits "sit somewhere in the 40 requests per second range" and a 429 must be respected (https://developer.themoviedb.org/docs/rate-limiting). We use 20 req/s, burst 10, 8 concurrent, and honour `Retry-After`. |
| Hosts | `api.themoviedb.org` (API), `image.tmdb.org` (images) |
| Status | enabled behind cargo feature `tmdb` |

## Attribution (terms, paragraph 3)

- "You must use the TMDB logo to identify Your use of TMDB, the TMDB APIs, or TMDB Content."
- The TMDB logo must be less prominent than our own (brand) marks and must not imply endorsement.
- Required notice, shown on the "Data sources" page and with TMDB data on detail pages:
  `This product uses TMDB and the TMDB APIs but is not endorsed, certified, or otherwise approved by TMDB.`
- The FAQ adds that the attribution must appear in an "About" or "Credits" type section.
- Logo: from TMDB's own logo page only (https://www.themoviedb.org/about/logos-attribution); not
  vendored yet. Until then the attribution is text plus link (see docs/research/icons.md).

## Other conditions (terms, checked 2026-11-06)

- No use of TMDB content for training or validating machine-learning or AI systems.
- On termination all TMDB content, including cached content, must be purged.
- Commercial use (fees, ads, revenue) needs a separate written agreement with TMDB; an operator
  who rebrands and sells this application must obtain one.

## Caching

Terms: do not "Cache, for longer than 6 months, any information obtained through or from TMDB".
Our HTTP cache TTL for TMDB is 7 days. An item refreshes when someone opens it, but an item
nobody opens is never refreshed, so a retention job enforces the limit (WP4-8, DEC-453):

- `catalog_items.provider_data_at` (migration `0028_catalog_retention.sql`) records when the
  connector data of an item was obtained: on insert, and again on every successful refresh.
- The cron job `catalog.retention` runs hourly on the `background_refresh` lane. It looks at up
  to 500 items with a `tmdb` external id whose data is older than `jobs.catalog_max_age_days`
  (`APP__JOBS__CATALOG_MAX_AGE_DAYS`, default 150, allowed 1 to 170). When TMDB is usable
  (key present, source not switched off) it queues a normal `catalog.refresh` (deduplicated per
  item; every call goes through the governed client and its rate limit) and a new download of
  the stored cover. When no refresh is possible, or the data is older than 175 days anyway
  (the refreshes kept failing), it purges every field TMDB supplied: title, year, description,
  original title, attributes (genres, runtimes, seasons), cover URL and the stored cover image,
  provenance, per-episode dates and fetch times; the item becomes a stub with a neutral
  placeholder title ("Title unavailable - source data expired"). Only the external id stays,
  with the person's own data (status, history, ratings, notes, tags, lists). This is the strict
  reading of 1.C ("any information"), an orchestrator decision the owner may reverse (UX4-9,
  DEC-466, HANDOFF section 6); DEC-453 had kept title and year.
- A purged item is fetched again automatically once TMDB is usable (at most once a day per
  item, `catalog_items.provider_refetch_after`, migration `0030_catalog_expiry.sql`).
- An item that no library entry, list, session, progress event or review refers to (a search
  result, a removed item without history) is deleted at the limit instead of being refreshed
  forever (UX4-9).
- The same job deletes stored provider responses (HTTP cache entries and their bodies) that
  went stale more than `jobs.catalog_max_age_days` ago.
- The connector list is `catalog_core::retention::CACHE_LIMITED`; no other connector doc in
  `docs/connectors/` states a caching limit (checked 2026-11-08: BGG asks for caching and a
  purge on revocation, IGDB allows caching, Trakt is an importer only).

## Endpoints used

| Op | Request |
|---|---|
| search movie | `GET /3/search/movie?query=<q>&include_adult=false&language=en-US&page=1` |
| search show | `GET /3/search/tv?query=<q>&include_adult=false&language=en-US&page=1` |
| movie details | `GET /3/movie/{id}?language=en-US` |
| show details | `GET /3/tv/{id}?language=en-US` (also `seasons[].air_date`, WP4-7) |
| season episodes (WP4-7) | `GET /3/tv/{series_id}/season/{season_number}?language=en-US`, see below |
| images | `https://image.tmdb.org/t/p/{size}{file_path}` (https://developer.themoviedb.org/docs/image-basics); we fetch `w780` posters and `w1280` backdrops and derive our own variants |

## Fixtures

`src/rust/crates/connectors/fixtures/tmdb/*.json` - **synthetic**: the response bodies are the
example responses published in TMDB's API reference pages (search-movie, movie-details,
search-tv, tv-series-details), fetched 2026-11-05. No API key was available, so nothing was
recorded live. They are small JSON documents of TMDB data, used for tests under the same
non-commercial terms. No images are committed. `cargo xtask record tmdb` re-records them when
`SOURCE_TMDB_API_KEY` is set.

## Season episodes (WP4-7, verified 2026-11-08)

Source: "TV Seasons > Details", https://developer.themoviedb.org/reference/tv-season-details
(read 2026-11-08; the page's embedded OpenAPI 3.1.0 definition, page `updatedAt`
2025-10-01T20:30:35Z). `GET /3/tv/{series_id}/season/{season_number}` on
`https://api.themoviedb.org`, operationId `tv-season-details`, bearer token as for the other
calls. Path parameters `series_id` and `season_number` (both required, int32); query
`language` (default `en-US`) and `append_to_response` (not used). The 200 response has the
top-level fields `_id`, `air_date`, `episodes`, `name`, `networks`, `overview`, `id`,
`poster_path`, `season_number`, `vote_average`; each element of `episodes` has `air_date`
(string), `episode_number` (integer), `episode_type`, `id`, `name`, `overview`,
`production_code`, `runtime` (integer), `season_number`, `show_id`, `still_path`,
`vote_average`, `vote_count`, `crew`, `guest_stars`. The schema marks nothing nullable; the
example has `null` values, so every field we read is optional in our parser.

What we read and keep: `episodes[].episode_number`, `episodes[].air_date`,
`episodes[].runtime` (table `catalog_episodes`, no names, overviews, stills or people), plus
`seasons[].air_date` of `GET /3/tv/{id}` (the season list, `SeasonInfo.air_date`). One call per
season, only in the `catalog.episodes` job on the `background_refresh` lane (queued by the
detail refresh), through the governed HTTP client and its cache (op `season`); a season whose
`episode_count` and `air_date` did not change and whose episodes all aired before today is
not asked for again (DEC in docs/DECISIONS.md, WP4-7).

Terms on storing it (https://www.themoviedb.org/api-terms-of-use, "last updated on: October
20, 2023", read 2026-11-08): section 1.C, "You must not: ... Cache, for longer than 6 months,
any information obtained through or from TMDB or the TMDB APIs."; section 1.D, on termination
"you must promptly delete or otherwise purge all TMDB Content, including any cached content".
The FAQ (https://developer.themoviedb.org/docs/faq, `updatedAt` 2025-10-07, read 2026-11-08)
says nothing about caching. Episode dates are TMDB content like the show details we already
keep, under the same rule: they are re-fetched with the item (see "Caching") and removed with
it (`ON DELETE CASCADE`), and purged by the retention job above.
The terms raise no doubt specific to these fields, so the fetch is
on, not behind an off flag.

Fixture: `fixtures/tmdb/tv-1399-season-1.json` - **synthetic**, the example response of the
reference page above (season 1 of series 1399, ten episodes with their `air_date`,
`episode_number`, `episode_type`, `runtime` and `season_number`; names, overviews, stills, crew
and guest stars left out to keep it small). No key was set on 2026-11-08, so nothing was
recorded live; `cargo xtask record tmdb` re-records it from its `request.url` when
`SOURCE_TMDB_API_KEY` is set. The e2e mock (`src/web/tests/tmdb-mock.mjs`) also serves one
show of its own (id 990001), written for the tests, not TMDB data.
