# Research: UI icon set and third-party service logos

Date checked: 2026-11-05

Scope: pick (a) one UI icon set and (b) one source for third-party service
logos for the Leptos 0.8 `ui` crate. We never draw or generate icons or logos
ourselves. Everything below was checked on the date above; registry metadata
was read from the npm registry (`registry.npmjs.org/<pkg>/latest`) and the
crates.io API (`crates.io/api/v1/crates/<crate>`). Glyph availability was
probed by HTTP status (200 = exists, 404 = missing) against jsDelivr URLs of
the pinned package version, e.g.
`https://cdn.jsdelivr.net/npm/lucide-static@1.69.0/icons/<name>.svg`.

No candidate checked turned out to be GPL/AGPL/LGPL, so none was rejected on
that ground.

## 1. Licence summary

| Library | Version checked | Licence (SPDX) | Source fetched | Bundling in MIT repo + Docker |
|---|---|---|---|---|
| Lucide (`lucide-static`) | 1.69.0 | ISC (Feather-derived glyphs: MIT, Cole Bemis) | registry.npmjs.org/lucide-static/latest ; lucide.dev/license | Yes, keep notices |
| Material Design Icons, Pictogrammers (`@mdi/svg`) | 7.4.47 | Apache-2.0 (brand/logo icons excluded) | registry.npmjs.org/@mdi/svg/latest ; pictogrammers.com/docs/general/license/ | Yes, keep LICENSE |
| Google Material Symbols (`@material-symbols/svg-400`, community repackage by marella) | 0.48.3 | Apache-2.0 | registry.npmjs.org/@material-symbols/svg-400/latest ; developers.google.com/fonts/docs/material_symbols | Yes, keep LICENSE |
| Tabler Icons (`@tabler/icons`) | 3.50.0 | MIT | registry.npmjs.org/@tabler/icons/latest | Yes, keep notice |
| Heroicons (`heroicons`) | 2.2.0 | MIT | registry.npmjs.org/heroicons/latest | Yes, keep notice |
| Phosphor (`@phosphor-icons/core`) | 2.1.1 | MIT ("Copyright (c) 2020 Phosphor Icons") | registry.npmjs.org/@phosphor-icons/core/latest ; github.com/phosphor-icons/homepage/blob/master/LICENSE | Yes, keep notice |
| Simple Icons (`simple-icons`) | 16.36.0 | CC0-1.0 (artwork); trademarks NOT licensed | registry.npmjs.org/simple-icons/latest ; github.com/simple-icons/simple-icons/blob/develop/DISCLAIMER.md | Yes for the files; trademark use rules apply |
| selfh.st/icons (`github.com/selfhst/icons`) | main branch, 2026-11-05 | CC-BY-4.0 (repo LICENSE heading "Attribution 4.0 International") | github.com/selfhst/icons ; github.com/selfhst/icons/blob/main/LICENSE ; selfh.st/icons-about/ | Yes with attribution; trademark use rules apply |
| Game-icons.net | site, 2026-11-05 | CC-BY-3.0 | game-icons.net/about.html | Yes with per-author attribution |
| The Noun Project | terms page, 2026-11-05 | Free tier CC BY 3.0 via their service; paid royalty-free | thenounproject.com/legal/terms-of-use/ | No (see below) |

Notes per library:

- Lucide: lucide.dev/license states the project is under the ISC License and
  lists icons derived from Feather under "The MIT License (MIT)" with
  "Copyright (c) 2013-present Cole Bemis". Both require the copyright and
  permission notice to be included with copies. No per-use (per-screen)
  attribution.
- MDI: the Pictogrammers licence page lists Apache License 2.0 for
  `@mdi/svg`, `@mdi/js`, `@mdi/font`; MIT for non-icon code. Footnote: icons
  in the "Brand / Logos" category are not covered by this licence, and brand
  icons are "deprecated and being removed" for legal reasons. So MDI must
  never be used as a logo source. Apache-2.0 requires shipping the licence
  text (and any NOTICE file, unverified whether MDI ships one: conservative
  reading is to copy it if present).
- Material Symbols: Google states "Material Symbols are available under the
  Apache License Version 2.0." The npm package used for SVGs is a community
  repackage (marella/material-symbols); upstream is google/material-design-icons.
- Tabler, Heroicons, Phosphor: MIT per registry metadata (Phosphor also via
  its LICENSE heading). Keep the notice in our NOTICE file.
- Simple Icons: CC0-1.0, so no attribution is legally required for the
  artwork. DISCLAIMER.md: users must "read these guidelines and ensure their
  usage of the brand's icon is in accordance with them", "seek the correct
  permissions to use the icons relevant to their project", and Simple Icons
  "cannot be held responsible for any legal activity raised by a brand".
  CC0 covers only the contributors' rights; it cannot license the brand
  owner's trademark (or copyright in the logo design).
- selfh.st/icons: GitHub sidebar shows CC-BY-4.0; LICENSE heading is
  "Creative Commons Attribution 4.0 International Public License". The about
  page says icons "are the property of their respective owners/entities" and
  that the collection is "not affiliated with or represented by selfh.st".
  Sources named: project repositories/websites first, then Wikipedia,
  Devicons, walkxcode/dashboard-icons, Iconduck. Conservative reading: the
  CC BY 4.0 grant covers selfh.st's own contribution (curation, conversion);
  rights in the logo itself stay with the brand owner, same as Simple Icons.
  The licence of each upstream source (e.g. walkxcode/dashboard-icons) was not
  checked: unverified.
- Game-icons.net: about page: icons are "provided under the terms of the
  Creative Commons 3.0 BY license", credit example "Icons made by {author}.
  Available on https://game-icons.net". Needs per-author credit; style
  (filled silhouettes, 512 px) does not match a stroke set.
- The Noun Project: terms forbid "large scale copying of Content ... except as
  expressly authorized" (also for public-domain content) and distribution
  without the owners' written consent; free icons require attribution via the
  service. Bundling a set in a public repo is therefore not allowed without
  written permission. Rejected.

## 2. Attribution and how we meet it

| Licence | Requirement | How we meet it |
|---|---|---|
| ISC / MIT (Lucide, Feather part, Tabler) | Keep copyright + permission notice with copies | `THIRD_PARTY_NOTICES` (or `NOTICE`) at repo root, copied into the Docker image (e.g. `/usr/share/doc/<app>/`), and rendered on the About page |
| Apache-2.0 (MDI, Material Symbols) | Ship licence text, keep NOTICE if any, mark modified files | Same notices file; we do not modify glyph paths, only wrap them |
| CC0-1.0 (Simple Icons) | None | Courtesy credit in notices file anyway (cheap, documents provenance) |
| CC-BY-4.0 (selfh.st) | Credit creator, link licence, indicate changes; "any reasonable manner" | Notices file + About page line: "Service logos from selfh.st/icons (CC BY 4.0), https://github.com/selfhst/icons; logos are trademarks of their respective owners." |
| CC-BY-3.0 (Game-icons) | Per-author credit | Only if used; list each icon + author in notices file and About page |

The About page entry also serves the white-label case: operators may rebrand
the app, but the licence notices page must stay reachable (document this in
the white-label docs).

## 3. Coverage of our needs

Probed glyphs (200 = present) in Lucide 1.69.0:

- Media kinds: film, clapperboard, tv, book, book-open, book-headphones
  (audiobook), podcast, music, disc-3, headphones, gamepad-2 (video games),
  drama (visual novel candidate), book-image (comics/manga candidate),
  library, puzzle, dices (board games).
- Board games: dice-5, dices, trophy, podium, timer, hourglass, users, medal,
  swords. Missing: meeple (404).
- Status: bookmark (planned), circle-dashed / circle-play (in progress),
  circle-check (completed), circle-pause (paused), circle-x (dropped).
- Generic UI: search, menu, x, check, chevron-down, chevron-right, star,
  heart, bell, settings, user, users, calendar, upload, download, link,
  share-2, eye, eye-off, sun, moon, ghost. All present.
- Category page lists a Gaming category of 127 icons; summed category counts
  are about 3,600 (categories overlap, so this is an upper estimate).

Meeple gap:

- Tabler 3.50.0: `outline/meeple.svg` 200, `outline/podium.svg` 200.
- MDI 7.4.47: `meeple` 404; has `podium`, `dice-multiple`, `trophy`,
  `timer-outline`, `account-group`, `chess-pawn`, `pause-circle`,
  `bookmark-check`.
- Phosphor 2.1.1: `meeple` 404 (has `dice-five`).
- Game-icons.net: `delapouite/meeple` exists, but CC BY 3.0 and filled style.

Style consistency: Lucide and Tabler are both 24x24, 2 px stroke, round caps
and joins, outline style; a Tabler glyph sits next to Lucide glyphs with
little visual difference. MDI and Material Symbols are filled/solid-first
24 px designs and would look heavier beside Lucide. Heroicons is a small set
(exact count unverified) and lacks game terms. Phosphor is large and
consistent but has no meeple, so it does not beat Lucide + Tabler.

Note: shadcn-style component libraries use Lucide by default, so Lucide also
matches the look our component crate is modelled on.

## 4. Service logo coverage

Simple Icons (slugs.md, develop branch) and selfh.st (SVG probe on jsDelivr,
`gh/selfhst/icons@main/svg/<slug>.svg`):

| Service | Simple Icons slug | selfh.st slug |
|---|---|---|
| Jellyfin | jellyfin | jellyfin |
| TMDB | themoviedatabase | tmdb |
| TheTVDB | absent | tvdb |
| IGDB | igdb | absent |
| Open Library | absent | absent |
| Hardcover | absent | hardcover |
| AniList | anilist | absent |
| MusicBrainz | musicbrainz | musicbrainz |
| Podcast Index | podcastindex | podcast-index |
| VNDB | absent | absent |
| Metron | absent | absent |
| BoardGameGeek | boardgamegeek | absent |
| Trakt | trakt | trakt |
| Goodreads | goodreads | goodreads |
| Radarr | radarr | radarr |
| Sonarr | sonarr | sonarr |
| ntfy | ntfy | ntfy |
| Gotify | absent | gotify |
| Discord | discord | discord |
| Matrix | matrix | matrix (also element) |
| Apprise | absent | apprise |

Simple Icons: 14/21, monochrome single-path SVG + official hex colour, fits a
stroke UI well. selfh.st: 15/21, full-colour. Combined: 18/21; Open Library,
VNDB and Metron have no source in either and must be shown as text (or the
official press-kit asset after checking that service's own terms, per
service, documented in the notices file).

## 5. Trademarks

- Logos are shown only to identify the service: integration settings,
  "data from X" attribution, source badges on metadata, notification target
  pickers. Never in our app logo, favicon, marketing, or as decoration.
- Do not alter logos beyond scaling and the monochrome/colour variant the
  source provides; no implied endorsement ("Works with Jellyfin", not
  "Jellyfin app").
- Before shipping each logo, read the brand guideline linked in Simple Icons'
  data (where present) and the service's own API terms. TMDB's and other
  metadata providers' API terms often demand specific attribution wording and
  logos: unverified here, to be checked in the per-provider research.
- Notices file states: "All product names, logos and brands are property of
  their respective owners. Use does not imply endorsement."
- White-label operators may hide integrations but must not reuse service
  logos as their branding; say so in the white-label docs.

## 6. Leptos integration options

| Option | Licence | Latest version | Leptos 0.8 | Notes |
|---|---|---|---|---|
| `lucide-leptos` (RustForWeb) | MIT | 3.63.0 (updated 2026-10-29) | Yes, depends on `leptos ^0.8.0` | One component per icon; Rust dead-code elimination drops unused ones. Which Lucide upstream release it tracks: unverified. Lucide only, no Tabler fallback. |
| `leptos_icons` | MIT | 0.7.1 (2026-01-04) | Yes, depends on `leptos >=0.8.3` | Renders `icondata` data structs. |
| `icondata` | MIT | 0.7.0 (2026-01-04) | n/a (data only) | Feature-gated sub-crates incl. `icondata_lu` (Lucide, 0.1.0, 2025-06-07), `icondata_tb` (Tabler), `icondata_mdi`, `icondata_si` (Simple Icons). Icon data lags upstream (Lucide sub-crate last published 2025-06). |
| Own build-time codegen | n/a (our code, MIT) | n/a | Yes | Pin SVG packages, generate Rust from an allowlist. |
| SVG sprite (`<use href>`) | n/a | n/a | Yes | One extra static asset; harder to style per-icon, and an external sprite is a separate fetch. Not preferred. |

Size impact: inline SVG path data compiled as `&'static str` is only kept in
the wasm/server binary if referenced. Per-icon components or constants
tree-shake naturally. A single `Icon(name: IconName)` with a `match` over
every icon in a full set would defeat this (all arms reachable), so the enum
must be generated from an allowlist of icons we actually use.

## Recommendation

- UI icon set: **Lucide** (ISC; Feather-derived glyphs MIT). Covers every
  media kind, status and generic UI glyph we need, plus dice, trophy, podium,
  timer and players. Notice-only, no per-use attribution.
- Fallback for missing glyphs: **Tabler Icons** (MIT), same 24 px / 2 px
  stroke style; supplies `meeple`. This replaces the MDI default because
  MDI is filled-style and also lacks a meeple; MDI (Apache-2.0) remains a
  second fallback if Tabler lacks something. Game-icons.net and The Noun
  Project are not used.
- Service logos: **Simple Icons** (CC0-1.0, monochrome) as the primary
  source; **selfh.st/icons** (CC-BY-4.0, attribution in notices file and
  About page) only for the gaps TheTVDB, Hardcover, Gotify, Apprise. Open
  Library, VNDB, Metron: show the service name as text and link to the
  official site/press kit. Logos only to identify services, never as our
  branding.
- Integration plan for the `ui` crate (no runtime fetch, tree-shaken):
  1. Vendor pinned SVGs into `crates/ui/icons/vendor/{lucide,tabler,simple-icons,selfhst}/`
     together with each upstream LICENSE file and a `VERSIONS` file
     (package, version, source URL, date). Update via a `just icons-update`
     script that downloads a given release tarball; no build-time network.
  2. `crates/ui/icons/icons.toml`: allowlist mapping our semantic names to
     sources, e.g. `kind-movie = "lucide:clapperboard"`,
     `status-paused = "lucide:circle-pause"`, `meeple = "tabler:meeple"`,
     `logo-jellyfin = "simple-icons:jellyfin"`, `logo-gotify = "selfhst:gotify"`.
  3. `crates/ui/build.rs` reads the allowlist, parses each SVG (viewBox,
     inner elements, stroke vs fill mode) and writes `icons_gen.rs`: a
     `#[non_exhaustive] enum IconName` with only allowlisted variants, plus
     `const fn` lookups returning `&'static str` inner markup and attributes.
     Build fails on an unknown name or a missing LICENSE file.
  4. `Icon` component: `#[component] fn Icon(name: IconName, #[prop(optional)] class: ..., #[prop(optional)] label: Option<..>)`
     renders an inline `<svg>` with `inner_html`, `currentColor`, size via
     Tailwind classes, `aria-hidden="true"` unless `label` is set (then
     `role="img"` + `<title>`). Works identically in SSR and hydrate.
  5. `ServiceLogo` component wraps `Icon` for `logo-*` names, always paired
     with the service name text (accessibility + trademark clarity), and
     falls back to text-only when no logo is allowlisted.
  6. `THIRD_PARTY_NOTICES` generated by the same build step from the vendored
     LICENSE files, copied into the Docker image and rendered on the About
     page, including the trademark disclaimer and the selfh.st CC BY 4.0
     credit.
  Alternative if codegen is deferred: `lucide-leptos` 3.63.0 (MIT, Leptos
  0.8) for UI glyphs, with the same allowlist idea applied manually; avoid
  `icondata`/`leptos_icons` as primary because their Lucide data lags
  upstream by over a year.
