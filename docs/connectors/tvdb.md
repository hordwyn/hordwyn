# Connector: TheTVDB (`tvdb`)

Date checked: 2026-11-06

| Item | Value |
|---|---|
| Kinds | show |
| Capabilities | search, details, images, episodes (WP4-7) |
| API docs | OpenAPI spec https://github.com/thetvdb/v4-api (`docs/swagger.yml`, no licence file; not GPL-family); overview https://thetvdb.com/api-information |
| Terms | https://thetvdb.com/tos (API key and, if applicable, PIN tied to the product described when applying; usage is monitored) and https://thetvdb.com/api-information |
| Licence / pricing | Proprietary. "Our API is available for both commercial projects and to individual developers"; pricing tiers with "significant discounts" for open-source projects. The exact free tier is not stated on the pages read: **unclear terms** (KICKOFF Q11) |
| Attribution | "Unless approved by TheTVDB, attribution with a direct link to TheTVDB.com must be displayed to end users viewing metadata from our API." Sample text: "Metadata provided by TheTVDB. Please consider adding missing information or subscribing." We show that text with a link on item pages that use TVDB data and on the Data sources page |
| Auth | `POST https://api4.thetvdb.com/v4/login` with `{"apikey": ..., "pin": ...}` returns `data.token` (JWT, valid one month); then `Authorization: Bearer <token>`. Config: `APP__CONNECTORS__TVDB__API_KEY` = `<apikey>` or `<apikey>:<pin>` |
| Endpoints | `GET /v4/search?query=<q>&type=series&limit=<n>`, `GET /v4/series/{id}/extended`, `GET /v4/series/{id}/episodes/official?page=0&season=<n>` (WP4-7), `GET /v4/series/{id}/episodes/official?page=<n>` (season counts, issue 3) |
| Rate limits | none published; policy 5/s, burst 5, 4 concurrent |
| Hosts | `api4.thetvdb.com`, `artworks.thetvdb.com` (image URLs returned by the API) |
| Status | behind cargo feature `tvdb`; registered only when a key is configured (unclear terms, Q11) |

## Fixtures

`src/rust/crates/connectors/fixtures/tvdb/*.json` - **synthetic**, shaped after the
`SearchResult` and `SeriesExtendedRecord` schemas of the OpenAPI spec; values written for the
test. No images.

## Season episodes (WP4-7, verified 2026-11-08)

Source: the official OpenAPI spec, https://raw.githubusercontent.com/thetvdb/v4-api/main/docs/swagger.yml
(`info.version` 4.7.10, read 2026-11-08). `GET /series/{id}/episodes/{season-type}`
(operationId `getSeriesEpisodes`): path `id` (number) and `season-type` (string; the spec gives
`default`, `official`, `dvd`, `absolute`, `alternate`, `regional` as examples, no enum); query
`page` (integer, required, default 0), `season` (integer), `episodeNumber` (integer, needs
`season`), `airDate` (`yyyy-mm-dd`). The 200 response is `{data: {series: SeriesBaseRecord,
episodes: [EpisodeBaseRecord]}, status}`; `EpisodeBaseRecord` has, among others, `number`
(integer), `seasonNumber` (integer), `aired` (string, no format stated), `runtime` (integer,
nullable), `name`, `overview`, `isMovie` (int64).

What we call and keep: `GET /v4/series/{id}/episodes/official?page=0&season=<n>`, aired order
("official", the same season type the `details` season list keeps), one call per season in the
`catalog.episodes` background job; only `number`, `aired` (first ten characters as
`YYYY-MM-DD`) and `runtime` of the episodes whose `seasonNumber` is the asked season. Not
verified: the spec declares no `links` paging object on this endpoint and no page size, so we
read page 0 only, one season at a time; a season longer than one page would be cut short (not
seen; record live when a key exists).

## Season counts (OT-27, issue 3, release 1.0.1)

The extended series record has no episode counts, so until 1.0.1 every season was stored
with `episode_count` 0: the web app drops such seasons, and the episode list of a TheTVDB
show held only the logged episodes. `details` now also reads the whole aired-order list,
`GET /v4/series/{id}/episodes/official?page=<n>` from page 0, and counts per season the
regular episodes (number > 0); the earliest `aired` of a season is its air date, and a season
only that list has is added. It follows `links.next` of the answer while it is set (the spec
declares no `links` on this endpoint, the other paged v4 lists carry it), at most 10 pages,
and reads one page when the answer has none. Not verified live (no key on the build host):
the page size and whether this endpoint sends `links`. Shows stored before 1.0.1 get the
counts with `cli refresh-source tvdb --apply` (release notes of 1.0.1).

Fixtures: `fixtures/tvdb/episodes-81189-s1.json` (one season), `episodes-81189-p0.json` and
`episodes-81189-p1.json` (the whole list in two pages) - **synthetic**, shaped after the spec
above; values written for the test.
