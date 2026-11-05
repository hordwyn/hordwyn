# Connector: TMDB (`tmdb`)

Date checked: 2026-11-05

| Item | Value |
|---|---|
| Kinds | movie, show |
| Capabilities | search, details, images |
| API docs | https://developer.themoviedb.org/reference/intro/getting-started (API v3) |
| Terms | https://www.themoviedb.org/api-terms-of-use ("last updated on: October 20, 2023") |
| FAQ (pricing) | https://developer.themoviedb.org/docs/faq |
| Licence | Proprietary API terms. Free for non-commercial use with attribution; commercial use needs a licence from TMDB (FAQ: "Our API is free to use for non-commercial purposes as long as you attribute TMDB as the source of the data and/or images"). |
| Auth | Bearer token (API Read Access Token, a JWT) in the `Authorization: Bearer <token>` header (https://developer.themoviedb.org/docs/authentication-application). Env: `KK_TMDB_API_KEY`. |
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

## Caching

Terms: do not "Cache, for longer than 6 months, any information obtained through or from TMDB".
Our HTTP cache TTL for TMDB is 7 days and catalog refreshes run well inside six months, so no
TMDB data is kept unrefreshed longer than allowed. Images are re-fetchable from `image.tmdb.org`.

## Endpoints used

| Op | Request |
|---|---|
| search movie | `GET /3/search/movie?query=<q>&include_adult=false&language=en-US&page=1` |
| search show | `GET /3/search/tv?query=<q>&include_adult=false&language=en-US&page=1` |
| movie details | `GET /3/movie/{id}?language=en-US` |
| show details | `GET /3/tv/{id}?language=en-US` |
| images | `https://image.tmdb.org/t/p/{size}{file_path}` (https://developer.themoviedb.org/docs/image-basics); we fetch `w780` posters and `w1280` backdrops and derive our own variants |

## Fixtures

`src/rust/crates/connectors/fixtures/tmdb/*.json` - **synthetic**: the response bodies are the
example responses published in TMDB's API reference pages (search-movie, movie-details,
search-tv, tv-series-details), fetched 2026-11-05. No API key was available, so nothing was
recorded live. They are small JSON documents of TMDB data, used for tests under the same
non-commercial terms. No images are committed. `cargo xtask record tmdb` re-records them when
`KK_TMDB_API_KEY` is set.
