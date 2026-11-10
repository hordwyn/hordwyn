F4 - Attribution page and next connectors (research, W1 + W5)

Output: `docs/fable/round2/attribution-and-connectors.md`

Goal: everything a workstream agent needs to build the attribution page (W1) and to pick the
order of the next connectors (W5), without further research.

Inputs: `git show wip/fable-reviews:docs/fable/open-data-sources.md`, `docs/connectors/*.md`,
`docs/research/{icons,board-game-sources,anime-manga-sources}.md`, `config/brand/ICONS.adoc`,
DEC-164, DEC-190, DEC-360, R-F3 (HANDOFF 4.1), `deny.toml` / `cargo deny` config.

Part A - attribution page:
1. Every required notice per data provider we ship (TMDB, TheTVDB, IGDB, Trakt line DEC-360,
   BGG when enabled, Wikidata/Commons if W3 comes): exact wording required, logo requirement
   and placement rule, link targets. Cite the provider's terms URL and the date you read it.
2. Fonts and icons: licences and the required notice text (Atkinson Hyperlegible Next OFL 1.1,
   Lucide ISC, MDI Apache-2.0, any service logos).
3. Crate and npm licences: how to generate the list at build time from `cargo metadata` /
   cargo-deny without a GPL tool (name the tool, its licence and version on docs.rs/crates.io).
4. Proposed page structure (sections, what is shown only when a connector is enabled), within
   the density budget.

Part B - connectors:
1. For Open Library, MusicBrainz, Wikidata, TVmaze, VNDB, Metron: licence of the data, terms
   URL, auth, rate limit, required User-Agent/attribution, whether caching/retention limits
   apply (like TMDB 6 months). Update facts from open-data-sources.md where they changed;
   cite everything with the date checked.
2. Recommended order with reasons (user value per kind, effort, terms risk) and which fill
   gaps the owner hit (OT-17 needs runtimes and page counts: which source gives book pages and
   episode runtimes reliably).
3. For the first two: the draft content of `docs/connectors/<id>.md` (format of the existing
   files).
