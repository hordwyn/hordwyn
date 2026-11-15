# v2 notes (owner, 2026-11-14)

Owner's list for after v1.0.0, in the owner's words, with first remarks. Not planned yet.

1. **Hub** - the community catalogue role (hub.hordwyn.com), planned since round 2
   (docs/fable/round2/next-packages.md h1-h7, docs/fable/hub-design.md).
2. **Normal tabletop game tracking** - poker; card games and their variants (Durak,
   Scheisskopf, etc.); Uno?; etc.
   - Remark: Wikidata's card game classes are already in the v1 board game search (DEC-792..811),
     so some traditional card games are found; house rules and regional variants are not. Likely
     needs a "game with variants" model (one game, named variants such as Texas hold'em / Omaha,
     scoring per variant) and games entered once and shared via the hub.
3. **Book series collections in the hub; shareable and votable lists with an order**
   (reading / play / watch order).
   - Example: "want to read" Harry Potter adds the whole series in order, but the person chooses
     which parts they actually want (Fantastic Beasts and Cursed Child belong to the full
     Harry Potter reading, most people only want the original seven).
   - Remark: series membership and order exist as open data (Wikidata "part of the series" with
     a series ordinal; Open Library series where present); the hub would hold curated, versioned
     order lists (core / extended / chronological / publication order) that members vote on;
     adding a series offers a checklist of its parts with the core set preselected.
   - Owner, 2026-11-16: lists are requested for importing items in one go, across kinds, e.g.
     "all Marvel (MCU) films and shows" in release or chronological order. Same mechanism: a
     hub list, imported as a checklist into the library (Planned) and optionally as a list.
     Note: hub-design.md covers the shared catalogue only (items, review, takedown), not lists;
     lists would be a second hub object type, and the design keeps movies and shows out of the
     hub in v1 (licensing), so a Marvel list would hold references (TMDB ids), not data.

## Addition (owner, 2026-11-14): dynamic scoring for game tracking

"Maybe we should research how to plug a dynamic scoring system into the game parts. So when we
get public data (e.g. Wikidata) we can enhance it with rules and scoring metadata to help
players."

Research questions for the v2 planning:
- A declarative scoring model attached to a game or variant (hub data, not code): points per
  round or per hand, a target to reach ("first to 500" in Uno), highest or lowest wins, a loser
  instead of a winner (Durak: the last player holding cards loses; Scheisskopf), teams,
  elimination order, ties, cumulative vs per-round, bonus/penalty items.
- How expressive it must be: a fixed set of building blocks (target, direction, rounds, teams,
  loser rule) vs small formulas; if formulas, a safe evaluator (no general code; DESIGN.md's
  wasmtime plugin idea only if declarative is not enough).
- What the play logger does with it: pre-fill the right inputs (round sheet vs final scores vs
  "who lost"), compute totals and the result, stats per variant (win rate, average score).
- Data: Wikidata only names the game; rules and scoring are our own (hub-curated, CC0 like the
  rest of the hub; rule texts written fresh, never copied from rulebooks); variants and house
  rules as forks of a base definition, votable in the hub like the order lists.
- Prior art to look at (licence first, no GPL code): open scorekeeping apps and their data
  formats, BG Stats' export format (owner sample pending) for what players record.

Owner example (2026-11-14): a cooperative hex-tile game where the group builds one landscape,
fulfils quest tasks and scores categories such as the longest river, keeps the score together,
records how long each play took and which campaign items it unlocks (owner confirmed: Dorfromantik: The Board Game). Requirements this adds:
- **Score sheets with categories** a person can define for a game themselves ("add the game's
  scoring card"), with a computed total; later shareable through the hub.
- **Cooperative plays**: one shared score, no winner; a personal best per group.
- **Campaign progress**: a running total across plays and the next unlock thresholds
  ("which things to free up"), with the unlock state per group.
- Duration per play exists already (v1 play log).

## Addition (owner, 2026-11-14): pluggable, configurable game stats

The owner confirmed the example is Dorfromantik. Wish: configure in the UI which values a game's
score sheet tracks (e.g. "longest railway" in Dorfromantik), and get them as game statistics
across plays - "your longest railway ever", best, average, totals - searchable like any other
stat. The data types must be pluggable into each other, in the same spirit as the existing
adapter/connector architecture.

Design direction to research (fits what exists):
- **Field definitions as data**, like `crates/kinds` already describes media kinds with JSON
  schemas: a score sheet = a list of typed fields (integer points, length/count, yes/no, choice,
  per-player vs shared), with a declared aggregation per field (max, min, sum, average, latest,
  count of true) and an optional total formula from the scoring model above.
- **Storage**: the play keeps its field values as validated JSON (Postgres JSONB, checked
  against the sheet's schema version); statistics computed by generic queries over those
  fields (or a small per-field stats cache like the existing stats cache), not per-game code.
- **Search and stats UI**: fields marked "statistic" appear as stats on the game page and the
  Stats Play tab ("Longest railway: 18, on 2026-11-13 with ..."); Meilisearch facets only where
  it helps to find plays.
- **Sharing**: sheets versioned and shared through the hub (CC0), forked for house rules,
  votable like order lists; a play records which sheet version it used, so old plays keep their
  meaning when a sheet changes.
- **Pluggable**: a sheet can reuse field types defined once (a "longest connected line" field
  type for river/railway, a "set collection" type, a "tasks fulfilled" counter), so new games
  are configuration, not code.

## Owner priority for v2 (2026-11-14) - THE biggest v2 research item

The biggest v2 research item is the score-sheet system above and how it fits into the hub.
The two most important elements:
1. **A UI that makes building a score card for a game easy** (e.g. Dorfromantik): define the
   fields, then at the table type in cards / quests / railway length and see the final score
   computed and shown. The UI helps build the sheet; the sheet does the calculation.
2. **Players get sheets from the hub** so they do not have to build them themselves: the hub
   distributes sheets (versioned, shared, forkable, votable).
Financing idea for the hub: Open Collective or similar (research options and their terms).
Order: research (design of the sheet model, the builder UI, hub distribution, financing) as the
first v2 package after v1.0.0.

## Addition (owner, 2026-11-15): book editions (note only, for the v2 talk)

Found on prod (OT-26 in docs/handoff/owner-test-2026-11-15.md): a Collector's Edition added
by ISBN shows the work with the median page count of all editions, "(estimate)". Today one item
is one work and the edition is dropped. The owner's wish, "it is quite important to have the
edition right":

- **ISBN search = that edition.** Someone who types the ISBN of their copy gets exactly that
  edition (name, format, publisher, date, pages), no estimate.
- **Title search lets you choose the edition.** The work first with the regular hardcover,
  the other editions below (paperback, special or collector's editions, re-releases), with a
  filter field for the edition or collection name.
- **Numbered pages first.** The page total starts from the edition's numbered (readable)
  pages, e.g. Open Library `pagination` "433 pages : map, ..." over `number_of_pages` 448;
  a hand-set total still wins, as today.
- **Where it fits (to discuss):** an edition belongs to the person's entry, not to the shared
  item (two people can own different editions of one work; stats and lists stay per work).
  Possibly the same edition model for other kinds later (board game editions, director's cuts,
  game remasters). Not decided whether this is 1.x or v2.

Owner addition (2026-11-15), the edition can change after planning: someone searches a book,
takes the regular edition and marks it Want to read, because they want to read the book, not
a particular print. Later they buy a used special edition at a book fair. They must be able to
say "read it, in the special edition" on the same entry: the plan, the reading and the stats
stay one book. So the edition is optional and changeable on the entry (Want to read may have
no edition or a provisional one; picking or changing it later keeps the entry's history).
Open for the talk: one edition per entry or per reading (a reread of the paperback after the
special edition), and whether changing the edition updates the page total when it was not set
by hand.

## Addition (owner, 2026-11-15): views per episode (note only, for the v2 talk)

Each episode row should show how often that episode was watched, as a small tag on the row
("2x"), beside "Watched" (OT-29). The effective history has the counts already; open for the
talk: count every view or only finished passes and runs, and how a rewatch run shows it.

## Addition (owner, 2026-11-16): recommendations and upcoming releases (note only, for the v2 talk)

- **Recommendation engine:** suggest what to watch, read or play next from the person's own
  library, ratings and history (and possibly what the household or friends logged). Open for
  the talk: local only (no profile leaves the instance) or with source data (TMDB / IGDB
  "similar" and "recommended" lists), and how it is explained ("because you rated X").
- **Upcoming releases for tracked items:** a calendar / list of what is coming for items in
  the library: next episodes and new seasons of shows (the episode job already stores air
  dates, DEC-448), sequels, new books of followed authors or series, game release dates,
  podcast episodes; optionally a notification when one is out (the notify crate exists).

## Addition (owner, 2026-11-16): language per view (note only, for the v2 talk)

Feature request: log in which language something was watched, read or listened to, e.g.
Pokemon episodes watched in German and again in English, a book read in the original and in
translation. Open: the language on the view (progress event) or on the session; audio vs
subtitles; whether a view in another language counts as a rewatch; stats per language; a
default language per entry (the last one used).

## Addition (owner, 2026-11-16): shared sessions with linked people (for the v2 talk)

How it works today (code read 2026-11-16, OT-33 in docs/handoff/owner-test-2026-11-15.md): a
session logged "with" a person entry that is linked to an account is shown read-only to that
account (if the owner shares sessions) and announced to them; it does not enter their library
or history by itself. They can "Copy to my log" per session. Visibility is computed when read,
so a link accepted later shows every earlier session with that entry and with entries merged
into it. Owner question: should a linked person get the item as a fully logged item of their
own; candidates: automatic copy (opt-in per linked person), "copy all" for a late link, and
copies that carry the episode or page.
- Owner, 2026-11-16: **proper sharing is important.** One person logs the play-through and it
  must not become a chore for the others to copy it over: an automatic copy into the linked
  person's own log, **configurable** (per person who shares, and the receiver can opt out or
  confirm), with the episode or page carried along.
