# Connector: TVmaze (`tvmaze`) - not implemented yet (terms recorded)

Date checked: 2026-11-10 (terms, licence, rate limits and sample responses read on the live
pages that day, see "Verified live").

| Item | Value |
|---|---|
| Kinds | show |
| Capabilities | search, details, images, episodes |
| API docs | https://www.tvmaze.com/api (root URL `https://api.tvmaze.com`) |
| Terms | sections "Licensing", "Rate limiting" and "Caching" of https://www.tvmaze.com/api |
| Licence | CC BY-SA 4.0. "Use of the TVmaze API is licensed by CC BY-SA. This means the data can freely be used for any purpose, as long as TVmaze is properly credited as source and your usage complies with the ShareAlike provision." The words "CC BY-SA" link to http://creativecommons.org/licenses/by-sa/4.0/ . Licence tag `CC-BY-SA-4.0` on every TVmaze field: shown on the instance with credit; an export that contains TVmaze data carries the credit and the licence; never proposed to a CC0 hub |
| Attribution | Required. "You can satisfy the attribution requirement by linking back to TVmaze from within your application or website, for example using the URLs available in the API." The CC BY-SA 4.0 deed adds: "give appropriate credit, provide a link to the license, and indicate if changes were made" (https://creativecommons.org/licenses/by-sa/4.0/deed.en). We show "Show and episode data from TVmaze, licensed CC BY-SA 4.0." on the attribution page (links to https://www.tvmaze.com/ and to the licence) and "Source: TVmaze" linked to the show's `url` on item pages that use it |
| Auth | none (public API). The user-level API for Premium members is not used |
| User-Agent | "While not required, we strongly recommend setting your client's HTTP User Agent to something that'll uniquely describe it." Ours: brand ident, version and the operator contact |
| Rate limits | "API calls are rate limited to allow at least 20 calls every 10 seconds per IP address. If you exceed this rate, you might receive an HTTP 429 error."; a stricter limit may be imposed temporarily; "simply retry the request after a small pause". "Leaving more than 1 connection to our servers idle may result in your IP getting blocked." Policy: 1 request/s, burst 5, 2 concurrent; back off a few seconds on 429; keep at most one idle connection |
| Caching | no limit on data. "All output is cached by our HTTP load balancers for 60 minutes" (the show index and the full schedule for up to 24 hours; neither is used); our TTL 24 hours. Images: "You are free to directly link ("hotlink") to our image CDN."; caching recommended; "Images can safely be cached indefinitely: on our end the content of a specific image URL will never change", so a stored cover does not need refreshing. Rights in the artwork are not stated: store with provenance, remove on request, as for other providers. No retention rule (not in `CACHE_LIMITED`) |
| Hosts | `api.tvmaze.com`, `static.tvmaze.com` (image URLs returned by the API on 2026-11-10) |
| Status | planned behind cargo feature `tvmaze` (W5); on by default once built (no key, clear licence) |

## Endpoints used

| Op | Request |
|---|---|
| search | `GET https://api.tvmaze.com/search/shows?q=<q>` (array of `{score, show}`) |
| details | `GET https://api.tvmaze.com/shows/{id}` |
| seasons | `GET https://api.tvmaze.com/shows/{id}/seasons` |
| episodes | `GET https://api.tvmaze.com/shows/{id}/episodes` ("By default, specials are not included"; `specials=1` adds them; not requested) |
| id bridge | `GET https://api.tvmaze.com/lookup/shows?imdb=<tt id>`, `?thetvdb=<id>` or `?tvrage=<id>` (HTTP 301 to the show's URL on a match, 404 otherwise). No lookup by TMDB id is documented: matching an existing TMDB show needs its IMDb or TheTVDB id |
| changes | `GET https://api.tvmaze.com/updates/shows?since=day` (also `week`, `month`; optional, to refresh only shows that changed) |

## Fields used

- show: `id`, `url`, `name`, `type`, `language`, `genres`, `status`, `runtime`,
  `averageRuntime`, `premiered`, `ended`, `network`, `webChannel`, `externals` (`imdb`,
  `thetvdb`, `tvrage`), `image.medium`, `image.original` (`image` is null when there is
  none), `summary` (HTML: strip tags), `updated` (Unix time).
- season: `id`, `number`, `episodeOrder`, `premiereDate`, `endDate`.
- episode: `id`, `season`, `number`, `type`, `airdate` (`YYYY-MM-DD`), `airstamp`,
  `runtime` (minutes). Kept in `catalog_episodes` like the TMDB and TheTVDB episodes: number,
  air date, runtime; no names, summaries or stills.

Mapping: `averageRuntime` (else `runtime`) -> `episode_runtime_minutes`; per-episode
`runtime` -> the episode row, which the stats use for hours watched (OT-17, DEC-450).

## Verified live (2026-11-10)

Page opened and compared with the statements above: https://www.tvmaze.com/api (licensing,
rate limiting, caching, images, lookup, updates); the CC BY-SA 4.0 deed. All quotes above
are the pages' wording.

Sample requests (one each):

- `/shows/1`: "Under the Dome", `runtime` 60, `averageRuntime` 60, `externals`
  `{"tvrage": 25988, "thetvdb": 264492, "imdb": "tt1553656"}`, both image URLs on
  `static.tvmaze.com`, `updated` present.
- `/shows/1/episodes`: 39 episodes, every one with `runtime` 60; keys `id`, `season`,
  `number`, `type`, `airdate`, `airtime`, `airstamp`, `runtime`, `name`, `summary`,
  `image`, `rating`, `url`, `_links`.
- `/shows/1/seasons`: 3 seasons with the season keys listed above.
- `/search/shows?q=girls`: entries of `{score, show}`, `show` with the show keys above.
- `/lookup/shows?thetvdb=81189`: 301 to `https://api.tvmaze.com/shows/169`.

Not verified: rights in the artwork (not stated); the actual limit beyond the documented
"at least 20 calls every 10 seconds" (not probed); the terms of the Premium user API (not
read, not used).

## Fixtures

`src/rust/crates/connectors/fixtures/tvmaze/*.json`: none yet. To be recorded with
`cargo xtask record tvmaze` (no key needed); small JSON, trimmed, no images. Fixture data
is TVmaze data under CC BY-SA 4.0: the fixture header and this file name TVmaze as the
source with the licence link.
