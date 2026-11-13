# Connector: Podcast Index (`podcastindex`)

Date checked: 2026-11-06

| Item | Value |
|---|---|
| Kinds | podcast |
| Capabilities | search, details, images |
| API docs | https://podcastindex-org.github.io/docs-api/ ; source of the docs (MIT): https://github.com/Podcastindex-org/docs-api (`api_src/`) |
| Terms | https://podcastindex.org/ : "The core, categorized index will always be available for free, for any use." API keys at https://api.podcastindex.org |
| Licence | Data: free for any use (statement above). Docs repository: MIT. API server repository (github.com/Podcastindex-org/api): Apache-2.0 |
| Attribution | Not required by the statement above; we show "Podcast data from the Podcast Index" with a link, as a courtesy and for provenance |
| Auth | Headers `X-Auth-Key` (API key), `X-Auth-Date` (current Unix time, 3-minute window), `Authorization` = lowercase hex `sha1(key + secret + date)`, and a `User-Agent` identifying the product (ours comes from the brand). Config: `APP__CONNECTORS__PODCASTINDEX__API_KEY` = `<key>:<secret>` or `SOURCE_PODCASTINDEX_API_KEY` |
| Endpoints | `GET /api/1.0/search/byterm?q=<terms>&max=<n>` (matches title, author, owner), `GET /api/1.0/podcasts/byfeedid?id=<id>` |
| Rate limits | none published; policy 2 requests/s, burst 4, 2 concurrent |
| Hosts | `api.podcastindex.org`. Artwork URLs point at the podcasters' own servers; they are not on the allowlist, so the server does not download them and the browser loads them directly (redirect fallback) |
| Status | enabled behind cargo feature `podcastindex` |

## Fields used

`feeds[]` / `feed`: `id`, `title`, `url` (feed URL), `description`, `author`, `artwork`
(fallback `image`), `language`, `explicit`, `episodeCount`, `itunesId`, `podcastGuid`,
`categories` (map id -> name).

## Fixtures

`src/rust/crates/connectors/fixtures/podcastindex/*.json` - **synthetic**: built from the
example values in the docs' property files (MIT). No images.
