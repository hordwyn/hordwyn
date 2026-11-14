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
