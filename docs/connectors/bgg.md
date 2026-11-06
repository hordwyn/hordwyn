# Connector: BoardGameGeek (`bgg`) - blocked

Date checked: 2026-11-06

| Item | Value |
|---|---|
| Kinds | board_game |
| API | XML API2 at `https://boardgamegeek.com/xmlapi2/...` |
| Auth | registered application + bearer token (D-5.3); without a token the API answers `401` (observed 2026-11-06) |
| Status | **blocked: documentation and terms not reachable** |

## Why blocked

From this build host boardgamegeek.com answers with a Cloudflare block (curl) or `403`
(web fetcher) for the API wiki page, the "using the XML API" page and the XML API terms of
use. Reading an archived copy was refused by the session's permission policy. Rule 1 forbids
writing the connector from memory, so it waits for the documentation (DEC-107).

Owner action: provide the text of the BGG XML API2 wiki page and the XML API terms of use, or
run this milestone where the site is reachable; apply for an application token (Q5,
`KK_BGG_TOKEN`).
