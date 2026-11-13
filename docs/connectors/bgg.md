# Connector: BoardGameGeek (`bgg`)

Since v1-wikidata (2026-11-13) the BoardGameGeek id of a board game arrives from Wikidata (P2339) as a reference identifier only (`attributes.bgg_id` and an outbound link; not an external id, so it never merges two catalogue items, LB-36); nothing is fetched from BoardGameGeek.

Date checked: 2026-11-06 (texts supplied by the owner; this build host cannot reach the site,
DEC-107). Status: **terms recorded, not implemented yet** (waits for the owner's application
token, R-P2). Off by default when built (rule Q11 style: terms are restrictive and revocable).

| Item | Value |
|---|---|
| Kinds | board_game (THINGTYPEs `boardgame`, `boardgameexpansion`) |
| API docs | https://boardgamegeek.com/wiki/page/BGG_XML_API2 (XML API2, marked BETA by BGG) |
| Usage guide | https://boardgamegeek.com/using_the_xml_api (version date 2025-07-02) |
| Terms | wiki page "XML API Terms of Use" (linked from the usage guide; exact URL to confirm) and the site Terms of Service, https://boardgamegeek.com/terms |
| Licence of data | worldwide, non-exclusive, royalty-free licence to reproduce and display, **strictly non-commercial**, revocable by BGG at any time for any reason; Wikipedia-sourced text is CC BY-NC-SA 3.0 |
| Images | not addressed by the terms; default: never stored in our storage (see below) |
| Auth | registered application per instance, `Authorization: Bearer <token>` (no colon), host `boardgamegeek.com` without `www`; env `KK_BGG_TOKEN` |
| Rate limits | 500/503 when too fast; "a 5-second wait between requests seems to suffice"; `thing` takes at most 20 ids; requests server-side only, cached, kept to a minimum; usage visible per application on https://boardgamegeek.com/applications |
| Collections | `collection` may answer 202 (queued): retry with delay; expansions need a second call (`subtype=boardgameexpansion`) |
| Attribution | "Data from BoardGameGeek" by name on every use; "Powered by BGG" logo linked to BoardGameGeek in public-facing uses, text legible (logo files not received yet) |

## How the connector must behave (derived from the terms)

- **Per-instance token.** The project ships no token; each operator registers a
  non-commercial application ("Each application using your library should have its own
  token"; end users never need one). Commercial operation needs a commercial licence from BGG:
  the admin page and the docs say so.
- **No modification.** "You may not modify the data ... in any way": BGG fields are stored as
  received, shown read-only with the source, never merged with other sources or rewritten or
  truncated in storage. User corrections are stored as the user's own values next to them.
  The user's own BGG ratings and plays are the user's submissions (ToS 5.D) and may be
  imported and mapped (DEC-018).
- **Credit.** Source line on every page that shows BGG data, plus the "Powered by BGG" logo
  linked to BGG (shown on every instance, public or not). Wikipedia-sourced descriptions carry
  their CC BY-NC-SA 3.0 notice.
- **Revocable.** Optional and off by default; an admin action purges all cached BGG data;
  nothing else depends on it.
- **Throttle.** One request at a time per instance, >= 5 s apart, through the job queue;
  batch `thing` lookups up to 20 ids; cache responses.
- **Images.** Covers are not copied into our storage (the licence does not mention images);
  the UI shows the placeholder or, if the owner decides so later, the BGG image URL.
- **No AI use of BGG data.** The app sends no BGG data to any AI system. Owner ruling
  2026-11-06: implementing the client from the documented API spec is not "using data to train
  an AI" (DEC-190).

## Importer without a token

Downloading one's own collection while logged in needs no registration, so a BGG collection
export (CSV/XML) can be imported without the connector.

## Fixtures

None yet. Record from the owner's machine once the token exists (`cargo xtask record bgg`),
small XML responses only, source and date in the fixture header.
