# v2 notes (owner, 2026-11-14)

Owner's list for after v1.0.0, in the owner's words, with first remarks. Not planned yet.

1. **Hub** - the community catalogue role (hub.hordwyn.com), planned since round 2
   (docs/fable/round2/next-packages.md h1-h7, hub-design.md on branch wip/fable-reviews).
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
