# Connector: Trakt (`trakt`) - import of the user's own data

Date checked: 2026-11-07. Status: **terms recorded, importer built** (phase 3, WP3-7, DEC-360). Scope: the
user downloads or exports their own Trakt data (history, ratings, watchlist) and uploads the
JSON files; the importer parses the documented `/sync/history`, `/sync/ratings` and
`/sync/watchlist` shapes. No OAuth pull, no API calls, no API app for now (DEC-297).

| Item | Value |
|---|---|
| Kinds | movie, show (episodes and seasons as progress on a show) |
| Capabilities | import only (upload of JSON files); no search, details or images |
| API docs | https://developer.trakt.tv/ (JavaScript app; `https://docs.trakt.tv` redirects there). Source of the guides and the OpenAPI contract: https://github.com/trakt/trakt-api (MIT, `LICENSE` fetched 2026-11-07). Legacy docs: https://trakt.docs.apiary.io/ (deprecated 2026-06-11, still served; raw blueprint `https://jsapi.apiary.io/apis/trakt.apib`) |
| API terms | "API Use Policy", `projects/developer/src/lib/guides/api-use-policy.md` in the repo above (`updatedAt: 2026-10-20`), published at https://developer.trakt.tv/docs/api-use-policy |
| App terms | "Create an App" guide (`create-an-app.md`, `updatedAt: 2026-10-01`), https://developer.trakt.tv/docs/create-an-app |
| Site terms | https://app.trakt.tv/terms ("Last Updated - October 27, 2026"; `https://trakt.tv/terms` 301s there) |
| Branding | https://app.trakt.tv/branding ("Branding Requirements") |
| Licence of data | proprietary; use governed by the API Use Policy (see below). The user's own history, ratings and lists are explicitly carved out of the catalog restrictions |
| Images | none imported; the importer never stores Trakt images (hotlinking is forbidden by the Images guide and the policy ties Trakt artwork to a running integration) |
| Auth | none for the file importer. Live API use would need a registered app (`trakt-api-key` = client id, `trakt-api-version: 2`, OAuth bearer token for `/sync/*`) |
| Rate limits | not applicable to the file importer. API: `AUTHED_API_GET_LIMIT` 1000 calls per 5 minutes, `AUTHED_API_POST_LIMIT` 1 call per second, 429 with `Retry-After` (apiary "Rate Limiting") |
| Attribution | "[App name] uses Trakt but is not endorsed or certified by Trakt" on the About/credits page (policy); logo optional |
| Status | planned: importer behind cargo feature `trakt-import`, **on by default** (recommendation below, DEC-297) |

## 1. Terms: verdict

**Allowed with conditions.** Importing a user's own exported Trakt data into a self-hosted
tracker, at the user's request, is a use the API Use Policy names as permitted. The
conditions are the attribution line, no redistribution of Trakt catalog data, no images, and
the branding rules if the logo is ever shown. Details, quoted from the policy (fetched
2026-11-07 from the MIT repo; same text as the published page):

- Scope: "This policy covers developers' and integrators' use of the Trakt API, including
  the commercial uses described above. The general Terms of Use cover use of Trakt's
  user-facing services."
- Permitted uses include "A personal or self-hosted tool that syncs a user's media server
  activity with their own Trakt account." and "A one-off personal script that helps a user
  export or migrate their own watch history, ratings, or lists." The examples table lists
  "A personal script migrates a user's own watch history to their account on another
  platform." as "Yes, at the user's request."
- Imports: "Helping a user import, export, or migrate their own Trakt history, ratings, or
  lists at their request is permitted. For one-time imports, we recommend supporting the
  user's Trakt data export (https://app.trakt.tv/settings/data)." and "Two-way sync is
  encouraged where appropriate, but is not a condition of permission for a one-time import."
- Commercial use: "Standard API access is free for commercial and noncommercial use.
  Commercial status, revenue, and user numbers alone do not require Premium, and Standard
  access has no per-user fee or revenue share." Also: "Uses permitted by this policy do not
  require individual approval, written confirmation, or a separate commercial agreement
  before launch."
- Storing the user's data: "Caching does not permit building a reusable copy of Trakt's
  catalog or continuing to use Trakt-sourced catalog metadata and artwork after your
  integration ends. This restriction concerns Trakt-supplied catalog content, not users' own
  watch history, ratings, or lists, or content independently obtained from another source."
  So the user's history, ratings and watchlist entries may be kept for good; the
  `title`/`year`/`ids` snapshots embedded in them are used for matching only (section 3).
- Not permitted: "Scraping, bulk harvesting, or mirroring Trakt data to build a reusable copy
  of its catalog, community activity, ratings, or lists.", "Selling Trakt API access or
  datasets obtained through it.", "Redistributing Trakt data as a feed or dataset that other
  services use to populate their own catalogs ...". Also: "Do not use Trakt data in
  applications or websites that promote copyright infringement or piracy." (rule 7 covers
  this).
- Attribution: "Include "[App name] uses Trakt but is not endorsed or certified by Trakt" in
  your app's About, Settings, or equivalent information page and in its app-store
  description, where applicable. For a website, use an equivalent About or credits page."
  and "You do not need a separate credit beside every item of Trakt data."
- Logo: "Using the Trakt logo is optional. If you use it, follow the Trakt Branding Guide:
  use the approved assets without modifying them, keep the required clear space, and do not
  incorporate the Trakt logo into your own identity or imply Trakt sponsorship or
  endorsement." The branding page adds "Do not alter, modify, rotate, or recolor the logo in
  any way." and "Use only the approved logo versions provided here; do not recreate or modify
  them." We show the name as text (DEC-297); a logo would come only from that page.
- Availability: "The API is provided as-is, with no warranty. Rate limits, features,
  endpoints, and access may change at any time, without a guaranteed notice or transition
  period."

**Does processing a user's own exported file need an API app?** No. The policy ties app
registration to API access ("To use the Trakt API, you'll first need to create a new API
app", Create an App guide); the file importer makes no API call. The policy's own
recommendation for one-time imports is the user's data export. Registration would also be
pointless: "We will automatically delete API app registrations after 30 consecutive days with
no usage." Registration needs "a verified GitHub account connected through the developer
portal". If a live pull is added later, each operator registers their own app (no shared
client id in the repo, rule 8) and accepts the Create an App terms, which include "Use only
documented API methods and please don't scrape the website for information."

**Site Terms of Use** (https://app.trakt.tv/terms, 2026-10-27): they govern the user's
account, not our software. Relevant passages: the licence grant is "for your personal,
non-commercial use"; "Contents" (including "information, data, text ... User Submissions")
may only be displayed, downloaded or printed "for the purpose of using the Services as an
internal or personal business resource"; "You do not have permission to copy, reproduce,
make derivative works from, distribute, republish, download ... any of the Contents without
the prior written permission of Trakt"; by submitting data the user grants Trakt "all right
and title to such User Submission". Read on their own these clauses are more restrictive
than the API policy; the API policy states explicitly that it is the document covering
developers and that user-owned history, ratings and lists are not catalog content, and Trakt
itself offers the data export at `settings/data`. We rely on the API policy and the export
feature; the site terms remain a residual uncertainty (recorded in DEC-297).

**Unclear / unverified:**

- The exact contents and file format of Trakt's own "Raw Export" ZIP. The page
  `https://app.trakt.tv/settings/data` needs a login and could not be read; no official
  support article describing the files was found. A third-party guide (achriom.com, "How to
  Export Your Trakt Data (2026)", dated 2026-08-27, fetched 2026-11-07) says: Settings ->
  Data -> "Raw Export" -> "Export now", a `trakt-backup-[date].zip` sent by email, no VIP
  needed, covering watched movies and episodes with dates, ratings, personal lists and the
  watchlist, not comments or follows. It does not name the files or their format. Older
  third-party sources (2017) say the CSV export was VIP-only. **Action for the owner:** run
  the export on your account and drop the ZIP into `owner-inbox/trakt/`; the importer's
  fixture for that format is written from it (same rule as Q4: from the file, not from
  anyone's description).
- Whether `/sync/watchlist` items of type `season` and `episode` still exist in the current
  API: the legacy blueprint documents them; the current contract types the response only as
  `rank`, `id`, `listed_at`, `type` plus passthrough. The importer accepts all four types.

## 2. Data shapes

Sources: legacy blueprint `https://jsapi.apiary.io/apis/trakt.apib` (sections "Standard
Media Objects", "Dates", "Get History", "Get Ratings", "Get Watchlist", fetched 2026-11-07)
and the current ts-rest/zod contract in https://github.com/trakt/trakt-api (MIT):
`projects/api/src/contracts/sync/index.ts` (`syncHistoryItemResponseSchema`,
`syncRatingItemResponseSchema`, `syncListItemResponseSchema`),
`projects/api/src/contracts/_internal/response/{movieIdsResponseSchema,showIdsResponseSchema,episodeIdsResponseSchema,listMetadataResponseSchema}.ts`,
and the guides `standard-media-objects.md` and `dates.md`, all fetched 2026-11-07. Field
names below are quoted exactly as documented.

Common rules:

- Dates: "All dates will be GMT and returned in the ISO 8601 format like
  `2014-09-01T09:10:11.000Z`." (dates.md)
- Each response is a JSON array of items. Every item has a `type` and, under the key named
  by that type, a standard media object; episode and season items also carry a `show`
  object.
- `ids` per object (current contract; `nullish` = may be null or absent):
  - movie: `trakt` (int), `slug` (string), `imdb` (string, nullish), `tmdb` (int, nullish)
  - show: `trakt`, `slug`, `tvdb` (int, nullish), `imdb` (nullish), `tmdb` (nullish)
  - season: `trakt`, `tvdb`, `tmdb` (the blueprint examples show seasons without `trakt`
    in some sync responses; treat every season id as optional)
  - episode: `trakt`, `tvdb` (nullish), `imdb` (nullish), `tmdb` (nullish)
- Standard media objects (standard-media-objects.md): movie `title`, `year`, `ids`; show
  `title`, `year`, `ids`; season `number`, `ids`; episode `season`, `number`, `title`, `ids`.
- Matching order used by Trakt itself for sync input, useful as our order too: "IDs will be
  matched in this order `trakt`, `imdb`, `tmdb`, `tvdb`, and `slug`. If nothing is found, it
  will match on the `title` and `year`." (blueprint, Group Sync)

### 2.1 `GET /sync/history[/{type}[/{id}]]`

Blueprint: "Returns movies and episodes that a user has watched, sorted by most recent. ...
The `id` _(64-bit integer)_ in each history item uniquely identifies the event ... The
`action` will be set to `scrobble`, `checkin`, or `watch`." Current contract
(`syncHistoryItemResponseSchema`): `id` int, `watched_at` datetime, `action` string, `type`
string, passthrough for the rest. `type` is `movie` or `episode` (shows and seasons expand
to episodes).

```json
[
  {
    "id": 1982346,
    "watched_at": "2014-03-31T09:28:53.000Z",
    "action": "scrobble",
    "type": "movie",
    "movie": {
      "title": "The Dark Knight",
      "year": 2008,
      "ids": { "trakt": 4, "slug": "the-dark-knight-2008", "imdb": "tt0468569", "tmdb": 155 }
    }
  },
  {
    "id": 1982347,
    "watched_at": "2014-02-27T09:28:53.000Z",
    "action": "checkin",
    "type": "episode",
    "episode": {
      "season": 2,
      "number": 1,
      "title": "Pawnee Zoo",
      "ids": { "trakt": 251, "tvdb": 797571, "imdb": null, "tmdb": 397629 }
    },
    "show": {
      "title": "Parks and Recreation",
      "year": 2009,
      "ids": { "trakt": 4, "slug": "parks-and-recreation", "tvdb": 84912, "imdb": "tt1266020", "tmdb": 8592 }
    }
  }
]
```

Fields the importer needs: `id` (source event id), `watched_at`, `type`, `movie.ids` or
`show.ids` + `episode.season` + `episode.number`; `title`/`year` as a fallback match and for
the unmatched list. `action` is kept in the row notes only if the owner wants it (default:
dropped).

### 2.2 `GET /sync/ratings[/{type}[/{rating}]]`

Blueprint: "Get a user's ratings filtered by `type`. You can optionally filter for a specific
`rating` between 1 and 10." `type` values `movies`, `shows`, `seasons`, `episodes`, `all`.
Current contract (`syncRatingItemResponseSchema`): `rated_at` datetime, `rating` int, `type`
string, passthrough. Item `type` is `movie`, `show`, `season` or `episode`.

```json
[
  {
    "rated_at": "2014-09-01T09:10:11.000Z",
    "rating": 10,
    "type": "movie",
    "movie": {
      "title": "TRON: Legacy",
      "year": 2010,
      "ids": { "trakt": 1, "slug": "tron-legacy-2010", "imdb": "tt1104001", "tmdb": 20526 }
    }
  },
  {
    "rated_at": "2014-09-01T09:10:11.000Z",
    "rating": 8,
    "type": "season",
    "season": { "number": 0, "ids": { "tvdb": 439371, "tmdb": 3577 } },
    "show": {
      "title": "Breaking Bad",
      "year": 2008,
      "ids": { "trakt": 1, "slug": "breaking-bad", "tvdb": 81189, "imdb": "tt0903747", "tmdb": 1396 }
    }
  },
  {
    "rated_at": "2014-09-01T09:10:11.000Z",
    "rating": 10,
    "type": "episode",
    "episode": {
      "season": 4,
      "number": 1,
      "title": "Box Cutter",
      "ids": { "trakt": 49, "tvdb": 2639411, "imdb": "tt1683084", "tmdb": 62118 }
    },
    "show": { "title": "Breaking Bad", "year": 2008, "ids": { "trakt": 1, "slug": "breaking-bad", "tvdb": 81189, "imdb": "tt0903747", "tmdb": 1396 } }
  }
]
```

Fields the importer needs: `rating` (1..10), `rated_at`, `type`, the `movie`/`show` ids
(episode and season ratings: `show.ids` plus `episode.season`/`episode.number` or
`season.number`).

### 2.3 `GET /sync/watchlist[/{type}[/{sort_by}[/{sort_how}]]]`

Blueprint: "Returns all items in a user's watchlist filtered by type." `type` values `all`,
`movies`, `shows`, `seasons`, `episodes`. "Each watchlist item contains a `notes` field with
text entered by the user." "When an item is watched, it will be automatically removed from
the watchlist." Current contract: `listMetadataResponseSchema` = `rank` int, `id` int,
`listed_at` datetime, `notes` string nullish; `syncListItemResponseSchema` adds `type` and
passthrough.

```json
[
  {
    "rank": 1,
    "id": 101,
    "listed_at": "2014-09-01T09:10:11.000Z",
    "notes": "Need to catch up before TRON 3 is out.",
    "type": "movie",
    "movie": {
      "title": "TRON: Legacy",
      "year": 2010,
      "ids": { "trakt": 1, "slug": "tron-legacy-2010", "imdb": "tt1104001", "tmdb": 20526 }
    }
  },
  {
    "rank": 2,
    "id": 102,
    "listed_at": "2014-09-01T09:10:11.000Z",
    "notes": null,
    "type": "show",
    "show": {
      "title": "The Walking Dead",
      "year": 2010,
      "ids": { "trakt": 2, "slug": "the-walking-dead", "tvdb": 153021, "imdb": "tt1520211", "tmdb": 1402 }
    }
  }
]
```

Fields the importer needs: `type`, `listed_at`, `notes`, `rank`, the `movie`/`show` ids
(season and episode items carry `show` too).

### 2.4 Related shapes (not imported in v1)

- `GET /sync/watched/{type}` (blueprint): per movie/show `plays`, `last_watched_at`,
  `last_updated_at`; shows add `reset_at` and `seasons[].episodes[]` with `number`, `plays`,
  `last_watched_at`. Aggregated; `/sync/history` carries the same information per event, so
  the importer does not need this file. Accepting it is a possible later addition.
- Trakt's own importer format (`POST /sync/history` body): arrays `movies`, `shows`,
  `seasons`, `episodes` of standard media objects with an optional `watched_at`
  ("Set to `released` ... Set to `unknown` to mark the item as watched without a specific
  date."). Not accepted by us in v1; mentioned because community backup tools emit it.
- Trakt's "Raw Export" ZIP: format unverified (section 1); fixture from the owner's file.

## 3. Importer mapping

Target: `ImportRow` of `crates/importers` (`source_id`, `kind`, `title`, `year`,
`external_ids`, `status`, `rating` 0..=100, `notes`, `history[]` with `date` and tagged
`progress`), one row per movie or show, keyed by source + source id (DESIGN.md 16).

| Trakt | Our model | Notes |
|---|---|---|
| history item, `type: movie` | row `kind: movie`; `history[]` entry `date = watched_at`, `progress = Finished` | one entry per event (`id`), rewatches stay separate events |
| history item, `type: episode` | row `kind: show` (keyed by the show); `history[]` entry `date = watched_at`, `progress = Episode { season: episode.season, episode: episode.number }` | the show is the library entry; episodes are progress (DESIGN progress model `Episodic`) |
| `movie.ids` / `show.ids` | `external_ids`: `trakt`, `imdb`, `tmdb`, `tvdb`, `slug` as strings | match order trakt-independent: `tmdb` first (our catalog is TMDB-backed), then `imdb`, `tvdb`; `trakt` and `slug` are stored for idempotency and the unmatched list only |
| `title`, `year` | row `title`, `year` | fallback match and display in the preview/unmatched list; never stored as catalog data (policy: Trakt snapshots are "for matching and display") |
| rating item `rating` 1..10 | `rating = rating * 10` (0..=100, DEC-018, lossless) | `rated_at` is not stored (no field; recorded as "lost") |
| rating item `type: show` / `movie` | rating on the row | - |
| rating item `type: season` / `episode` | **lost** (reported in the preview) | no per-episode or per-season rating in the model |
| watchlist item | row `status = planned` when the row has no history, `notes = notes` | `rank`, `listed_at` lost (no field); DESIGN "planned" status |
| row with history, no watchlist entry | `status = completed` for movies; shows: `in_progress` (Trakt removes watched items from the watchlist; completion of a show is decided by the app from episode progress, not by the file) | the preview shows the chosen status per kind |
| `action` (`scrobble`/`checkin`/`watch`) | lost | - |
| history `id` | part of `source_id` for idempotency (`trakt:history:<id>` folded into the row's event list) | re-import creates no duplicates (DESIGN 16) |
| items without `tmdb`/`imdb`/`tvdb` | unmatched list, manual match through the palette | phase 3 plan |
| playback progress, collection, lists other than the watchlist, favorites, comments, follows | not imported, listed as "not imported" in the preview | out of scope |

Upload contract: one or more JSON files, each a top-level array; the file's kind is detected
from the item shape (`watched_at` -> history, `rated_at` + `rating` -> ratings, `listed_at`
-> watchlist), so the user can upload the three files in any order or one combined array.
Streaming parse, dry-run preview with counts per kind and the lost fields above (R-35).

### Recommendation: on by default

Ship the Trakt file importer **enabled by default** (cargo feature `trakt-import` in the
default set; no admin toggle needed because it makes no network calls and holds no key).
Reasons: the API Use Policy names exactly this use as permitted without approval, for
commercial and non-commercial operators alike; the importer touches no Trakt server, stores
no catalog content or images, and keeps only data the policy carves out as the user's own.
The only obligation is the credit line on the attribution page, which costs nothing and is
added together with the feature. This differs from BGG (restrictive, revocable, token per
instance -> off by default) and from the Q11 rule for unclear terms: the terms here are
clear for the file path. A later OAuth pull would be a separate feature, off by default,
with a per-operator app registration.

## 4. Attribution

- Credits/"Data sources" page: `<brand name> uses Trakt but is not endorsed or certified by
  Trakt`, with a link to https://trakt.tv. Shown whenever the importer feature is compiled
  in (the text does not depend on an import having happened).
- Where it is shown now (WP3-7, DEC-360): there is no About/credits page yet (the attribution
  page is open, HANDOFF 5 item 5), so the line and the link are on the Trakt card of
  Settings > Imports and export (the policy allows "Settings"). When the attribution page is
  built, the line must be added there too.
- No per-item credit required; the imported rows show "Imported from Trakt" as the source
  in the history detail, as every importer does.
- Logo: not shown (text only). If the owner wants it, the files come unmodified from
  https://app.trakt.tv/branding with the clear-space rule ("equal to the height of the "T"
  in Trakt") and are recorded in `config/brand/ICONS.adoc`.

## 5. Fixtures

`src/rust/crates/importers/fixtures/trakt/{history,ratings,watchlist}.json` - **synthetic**:
built from the example responses in the Trakt API blueprint (`/sync/history`,
`/sync/ratings/*`, `/sync/watchlist/*`, fetched 2026-11-07), trimmed to a few items each,
with the show/episode variants included. Small JSON documents of Trakt example data, used
for tests under the API Use Policy. A fixture of the "Raw Export" ZIP format follows once the
owner drops a real export into `owner-inbox/trakt/` (anonymised subset, as for the legacy
export).

## 6. Pages read (2026-11-07)

| Page | Result |
|---|---|
| https://app.trakt.tv/terms | read (HTML, "Last Updated - October 27, 2026") |
| https://app.trakt.tv/branding | read |
| https://trakt.docs.apiary.io/ and `https://jsapi.apiary.io/apis/trakt.apib` | read (blueprint, marked deprecated) |
| https://developer.trakt.tv/ (= docs.trakt.tv) | HTML shell only (JavaScript app, `noindex`); `/llms.txt` 404. Content taken from the MIT repo that generates it |
| https://github.com/trakt/trakt-api (LICENSE, README, guides, contracts) | read |
| https://github.com/trakt/trakt-api/discussions/808 | read (docs moved 2026-06-11) |
| https://app.trakt.tv/settings/data | **not read** (login required) |
| https://www.achriom.com/blog/how-to-export-your-trakt-data/ | read (third party, 2026-08-27) |
