# Connector: AniList (`anilist`) - not implemented

Date checked: 2026-11-06

| Item | Value |
|---|---|
| Kinds | anime, manga |
| API docs | https://docs.anilist.co/guide/graphql/ (GraphQL, `POST https://graphql.anilist.co`) |
| Terms | https://docs.anilist.co/guide/terms-of-use |
| Rate limits | 90 requests per minute (currently degraded to 30/min), 429 with `Retry-After` |
| Status | **not implemented: the terms forbid this use** |

## Why not

The terms of use state: "Use of the AniList API within competing, non-complementary services
of the same nature is prohibited. This includes, but is not limited to, anime and manga list
or tracker services. The restriction applies to all data provided through the API, including
both user data and media data." This application is a media tracker with lists, so it falls
under that clause. The terms also prohibit "using the AniList API as a backup or data
storage service" and "hoarding or mass collection of data".

Anime and manga need another source (owner decision, see DECISIONS DEC-108). TMDB already
covers many anime series as shows.
