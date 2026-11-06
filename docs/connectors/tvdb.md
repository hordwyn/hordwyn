# Connector: TheTVDB (`tvdb`)

Date checked: 2026-11-06

| Item | Value |
|---|---|
| Kinds | show |
| Capabilities | search, details, images |
| API docs | OpenAPI spec https://github.com/thetvdb/v4-api (`docs/swagger.yml`, no licence file; not GPL-family); overview https://thetvdb.com/api-information |
| Terms | https://thetvdb.com/tos (API key and, if applicable, PIN tied to the product described when applying; usage is monitored) and https://thetvdb.com/api-information |
| Licence / pricing | Proprietary. "Our API is available for both commercial projects and to individual developers"; pricing tiers with "significant discounts" for open-source projects. The exact free tier is not stated on the pages read: **unclear terms** (KICKOFF Q11) |
| Attribution | "Unless approved by TheTVDB, attribution with a direct link to TheTVDB.com must be displayed to end users viewing metadata from our API." Sample text: "Metadata provided by TheTVDB. Please consider adding missing information or subscribing." We show that text with a link on item pages that use TVDB data and on the Data sources page |
| Auth | `POST https://api4.thetvdb.com/v4/login` with `{"apikey": ..., "pin": ...}` returns `data.token` (JWT, valid one month); then `Authorization: Bearer <token>`. Config: `APP__CONNECTORS__TVDB__API_KEY` = `<apikey>` or `<apikey>:<pin>` |
| Endpoints | `GET /v4/search?query=<q>&type=series&limit=<n>`, `GET /v4/series/{id}/extended` |
| Rate limits | none published; policy 5/s, burst 5, 4 concurrent |
| Hosts | `api4.thetvdb.com`, `artworks.thetvdb.com` (image URLs returned by the API) |
| Status | behind cargo feature `tvdb`; registered only when a key is configured (unclear terms, Q11) |

## Fixtures

`src/rust/crates/connectors/fixtures/tvdb/*.json` - **synthetic**, shaped after the
`SearchResult` and `SeriesExtendedRecord` schemas of the OpenAPI spec; values written for the
test. No images.
