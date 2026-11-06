# Connector: IGDB (`igdb`)

Date checked: 2026-11-06

| Item | Value |
|---|---|
| Kinds | video_game |
| Capabilities | search, details, images |
| API docs | https://api-docs.igdb.com/ (API v4) |
| Terms | Twitch Developer Services Agreement (linked from the API docs: "free for non-commercial usage under the terms of the Twitch Developer Service Agreement"); commercial use via partnership (partner@igdb.com). The docs' business FAQ also says commercial projects are allowed |
| Licence | Proprietary API terms (IGDB is owned by Twitch/Amazon). Licence of the IGDB software: not open source (no public repository) |
| Attribution | Business FAQ: "We expect fair attribution, i.e. attribution that is visible to your users and located in a static location (e.g. not in a change log)". We show "Data from IGDB.com" with a link on item pages that use IGDB data and on the Data sources page |
| Caching | FAQ: storing/caching locally is allowed and preferred |
| Auth | Twitch OAuth2 client credentials: `POST https://id.twitch.tv/oauth2/token?client_id=...&client_secret=...&grant_type=client_credentials` returns `access_token` and `expires_in`; every request sends `Client-ID: <id>` and `Authorization: Bearer <token>`. Config: `APP__CONNECTORS__IGDB__API_KEY` = `<client_id>:<client_secret>` (or `KK_IGDB_API_KEY`) |
| Requests | `POST https://api.igdb.com/v4/<endpoint>` with an APICalypse body, e.g. `search "Halo"; fields name,first_release_date,summary,cover.image_id,genres.name,platforms.name,involved_companies.company.name,involved_companies.developer,involved_companies.publisher; limit 20;` |
| Rate limits | 4 requests per second, at most 8 open requests; over the limit the API answers 429. Policy: 4/s, burst 4, 8 concurrent; the token request counts too |
| Hosts | `api.igdb.com`, `id.twitch.tv`, `images.igdb.com` |
| Images | `https://images.igdb.com/igdb/image/upload/t_{size}/{image_id}.jpg`; we fetch `t_cover_big_2x` (2 x 264 x 374) and derive our own variants |
| Status | enabled behind cargo feature `igdb` |

## Fields used

`games`: `id`, `name`, `first_release_date` (Unix seconds), `summary`, `cover.image_id`,
`genres.name`, `platforms.name`, `involved_companies` (`company.name`, `developer`,
`publisher`), `url`.

## Fixtures

`src/rust/crates/connectors/fixtures/igdb/*.json` - **synthetic**: built by hand from the
field reference and request examples in the IGDB API docs (no client credentials were
available). Small JSON only, no images. `cargo xtask record igdb` re-records them when
`KK_IGDB_API_KEY` is set.
