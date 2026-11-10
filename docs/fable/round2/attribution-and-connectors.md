# Attribution page and next connectors (F4, research for W1 and W5)

Status: COMPLETE (2026-11-10). Items that could not be verified are listed at the end.

Date: 2026-11-10. Every URL below was opened on that date unless a line says otherwise.
Web pages were read through a fetch tool that returns extracts, so a quoted sentence is the
extract's wording of the page; before a required notice is hard-coded, compare it once with
the page in a browser (the notices already in the code match what was read today). No source
code of a GPL-family project was read (R-P1: public API docs, terms and blog posts only).
Nothing was built or run. Code claims carry `file:line` relative to the repo root.

Top findings (details below):

1. The TMDB terms require the logo, not only the text: section 3 "You must use the TMDB logo
   to identify Your use of TMDB". Today no logo is vendored (`config/brand/ICONS.adoc:40-45`),
   so W1 must vendor one official SVG (R-F3 allows it) and show it on the page.
2. There is no page yet that fits "About or Credits": the list lives at the bottom of
   Settings > Data sources (`src/rust/crates/web/src/settings.rs:471-487`), behind sign-in,
   and it does not contain the Trakt line (`web/locales/en.ftl:1731` is only on the import
   card) nor fonts, icons or crates.
3. Licence list: use `cargo-about` 0.9.3 (MIT OR Apache-2.0) at build time; `cargo deny list`
   gives identifiers only, no licence texts. Serve the result from the server, never compile
   the texts into the wasm bundle. Nothing from npm ships in the image.
4. The outgoing User-Agent has no contact unless the brand sets `support_url`
   (`src/rust/crates/brand/src/lib.rs:133-137`). Open Library, MusicBrainz and Wikimedia
   all tie their limits to a contact in the User-Agent (Wikimedia since 2026: 10 requests
   per minute without one, 200 with one). The next connectors need a contact setting.
5. Connector order: Open Library first (books have no source; it is the only source of page
   counts for OT-17), TVmaze second (key-less, no retention limit, per-episode runtimes),
   then MusicBrainz, VNDB, Wikidata generalised (inside W3), Metron last. Metron's licence is
   now known (CC BY-SA 4.0, covers excluded) and it requires one Metron account per user.

## Part A - attribution page

### A1. Required notices per data provider

What exists today: each connector carries an `Attribution { text, url, licence }`
(`src/rust/crates/catalog-core/src/model.rs:44-48`); `ops::sources` returns them for the
registered connectors (`src/rust/crates/api/src/ops.rs:262-275`); the item page prints them
in its footer (`src/rust/crates/web/src/item.rs:892-903`) and Settings > Data sources prints
the list (`src/rust/crates/web/src/settings.rs:471-487`). The struct has no logo, no
separate terms link and no "required or courtesy" flag. Side defect: the admin sources DTO
fills `terms_url` from `attribution.url`, which is the provider's home page, not its terms
(`src/rust/crates/api/src/admin/sources.rs:449`, e.g. `connectors/src/tmdb.rs:68`).

#### TMDB (shipped, cargo feature `tmdb`)

Sources: terms https://www.themoviedb.org/api-terms-of-use ("This Agreement was last updated
on: October 20, 2023"); FAQ https://developer.themoviedb.org/docs/faq (page metadata
`updatedAt` 2025-10-07); logos https://www.themoviedb.org/about/logos-attribution. All read
2026-11-10.

| Item | Requirement |
|---|---|
| Notice text | Terms section 3: "You must place the following notice prominently in or on Your Application": "This [website, program, service, application, product] uses TMDB and the TMDB APIs but is not endorsed, certified, or otherwise approved by TMDB." The code has the "This product ..." form (`connectors/src/tmdb.rs:67`): correct, keep it. The FAQ prints a shorter variant ("This product uses the TMDB API but is not endorsed or certified by TMDB."); the terms are the contract, so the longer text wins |
| Logo | Required. Terms section 3: "You must use the TMDB logo to identify Your use of TMDB, the TMDB APIs, or TMDB Content." FAQ: only an approved logo; colour and aspect ratio must not be changed, no flipping or rotating |
| Prominence | Terms section 3: the TMDB logo "must be less prominent than the logos or marks" of our own application and must not imply endorsement |
| Where | FAQ: the attribution must be within the application's "About" or "Credits" type section. Logos page: "every application that uses our data or images is required to properly attribute TMDB as the source." |
| Link target | https://www.themoviedb.org (FAQ: link back to the website); name it "TMDB" or "The Movie Database" only |
| Other duties to state on the page | none; the six-month cache rule (terms 1.C) and the purge on termination (1.D) are already handled by the retention job (docs/connectors/tmdb.md "Caching") |

Logo files (official page, 2026-11-10; paths are relative to https://www.themoviedb.org):

- Primary full: `/assets/v4/logos/v2/blue_square_2-d537fb228cf3ded904ef09b136fe3fec72548ebc1fea3fbbd1ad9e36364db38b.svg`
- Primary short: `/assets/v4/logos/v2/blue_square_1-5bdc75aaebeb75dc7ae79426ddd9be3b2be1e342510f8202baf6bffa71d7f5c4.svg`
- Primary long: `/assets/v4/logos/v2/blue_long_2-9665a76b1ae401a510ec1e0ca40ddcb3b0cfe45f1d51b77a308fea0845885648.svg`
- Alt long: `/assets/v4/logos/v2/blue_long_1-8ba2ac31f354005783fab473602c34c3f4fd207150182061e425d366e4f34596.svg`
- Alt short: `/assets/v4/logos/v2/blue_short-8e7b30f73a4020692ccca9c88bafe5dcb6f8a62a4c6bc55cd9ba82bb2cd95f6c.svg`

Recommendation for W1:

- Vendor "Alt short" (the wordmark-plus-bar form, smallest footprint) unmodified, e.g. as
  `src/rust/crates/ui/logos/vendor/tmdb/blue_short.svg`, and record URL, date and SHA-256 in
  `config/brand/ICONS.adoc` under "Service logos" (today "None vendored yet",
  `config/brand/ICONS.adoc:40-45`). Never into `config/brand/default/` logo slots (KICKOFF
  section 5). Not an inline SVG component: serve the file as it is (`<img>`), so it stays
  byte-identical.
- Show it on the attribution page only, in the TMDB row, with the notice and the link.
  Height at most the cap height of the page's h2 (about 20 CSS px) so it stays smaller than
  the product name in the shell; `alt="TMDB"`; the link around it gets the accessible name
  "TMDB (themoviedb.org)". The default brand has no drawn mark, only the name as text
  (`config/brand/ICONS.adoc:83-86`), so do not put the TMDB logo in the header, on the
  sign-in page or on item pages: there it would be the most prominent mark on screen.
- Dark theme: the file must not be recoloured. Logotypes are exempt from the WCAG contrast
  minimum (1.4.3, 1.4.11), but check by eye that the blue-green mark is legible on the dark
  surface; if not, put it on a small light plate (padding, no change to the file).
  Not verified: which backgrounds TMDB considers acceptable (the logos page states no rule).
- Item pages keep the text footer (`web/src/item.rs:892-903`); the terms ask for the logo
  to "identify" the use, which the credits page does.

#### TheTVDB (shipped, cargo feature `tvdb`, registered only with a key)

Source: https://thetvdb.com/api-information (2026-11-10).

| Item | Requirement |
|---|---|
| Notice | "Unless approved by TheTVDB, attribution with a direct link to TheTVDB.com must be displayed to end users viewing metadata from our API." Sample text given by TheTVDB: "Metadata provided by TheTVDB. Please consider adding missing information or subscribing." (the code has exactly this, `connectors/src/tvdb.rs:86`) |
| Where | "to end users viewing metadata": on every item page that shows TheTVDB data (done, item footer) and on the attribution page. The "about or readme" allowance is only for command-line products and libraries |
| Logo | not required (the page shows logos but states no duty). Keep text only |
| Link target | https://thetvdb.com/ (`connectors/src/tvdb.rs:87`); a direct link, so no redirector and no `nofollow` |

Unchanged from docs/connectors/tvdb.md. Still unclear: pricing (no free tier named; contact
address only), so the Q11 flag stays.

#### IGDB (shipped, cargo feature `igdb`)

Could not be re-read today: https://api-docs.igdb.com/ and https://www.igdb.com/api both
answered HTTP 403. The record of 2026-11-06 stands (docs/connectors/igdb.md): the business
FAQ expects "fair attribution, i.e. attribution that is visible to your users and located in
a static location (e.g. not in a change log)". No logo duty is recorded.

- Notice (ours, no wording is prescribed): "Video game data from IGDB.com."
  (`connectors/src/igdb.rs:88`), link https://www.igdb.com/.
- Where: attribution page (the "static location") plus the item footer.
- Logo: none. Owner task if wanted: read the FAQ in a browser and confirm no logo rule.

#### Podcast Index (shipped, cargo feature `podcastindex`)

No attribution duty (docs/connectors/podcastindex.md, statement recorded 2026-11-06; the
site answered 403 to the build host on 2026-11-09, not retried today). Courtesy line as in
the code: "Podcast data from the Podcast Index." (`connectors/src/podcastindex.rs:89`), link
https://podcastindex.org/.

#### Trakt (importer, DEC-360)

Source: API Use Policy, read 2026-11-07 (docs/connectors/trakt.md:61-70); the raw file in
the MIT repo was not found at the guessed path today, so it was not re-read.

| Item | Requirement |
|---|---|
| Notice | "[App name] uses Trakt but is not endorsed or certified by Trakt" in the app's "About, Settings, or equivalent information page"; "For a website, use an equivalent About or credits page." |
| Per-item credit | not needed ("You do not need a separate credit beside every item of Trakt data.") |
| Logo | optional; text only (DEC-297) |
| Link | https://trakt.tv |
| When shown | whenever the `trakt-import` feature is compiled in (docs/connectors/trakt.md:336-338), not only after an import |

The string exists as `imports-trakt-credit` with `{ $product }` for the brand name
(`src/rust/crates/web/locales/en.ftl:1731`). Trakt is not a connector in the registry, so
`ops::sources` does not return it: the page needs a second list "importers" (a static
entry behind `cfg(feature = "trakt-import")`). Keep the line on the import card as well.

#### BoardGameGeek (only when enabled; not built; round-1 hub design proposes not to build it)

Source: the owner's copies in `owner-inbox/bgg/` (git-ignored; usage guide version
2025-07-02). https://boardgamegeek.com/using_the_xml_api answered HTTP 403 today.

| Item | Requirement |
|---|---|
| Credit | "In all uses of the BGG XML API, you shall credit BoardGameGeek by name as the source of the data." (`owner-inbox/bgg/xml-api-terms-of-use.txt:15`) |
| Logo | "We require that you include the "Powered by BGG" logo (linked back to BoardGameGeek) in public-facing uses" (same line); "Please display the logo at a size such that the text is easily legible." (line 18). Files: not received; HANDOFF 4.6 names https://boardgamegeek.com/image/7779581 |
| Where | DEC-190/DEC-193: credit and logo on every instance, on each page or panel that shows BGG data ("From BoardGameGeek") and on the attribution page |
| Link target | https://boardgamegeek.com/ (for an item: its BGG page) |
| Extra notice | text whose source is Wikipedia is CC BY-NC-SA 3.0, link http://creativecommons.org/licenses/by-nc-sa/3.0/ (`xml-api-terms-of-use.txt:25`) |
| Caution | the site ToS forbids using BGG logos without approval outside what the API terms grant (`owner-inbox/bgg/terms-of-service-extract.txt:26`): use only the "Powered by BGG" files, unmodified |

Page row only when the connector is enabled and has a token. Suggested text: "Board game
data from BoardGameGeek." plus the logo as a link.

#### Wikidata and Wikimedia Commons (if W3 comes)

Sources: https://www.wikidata.org/wiki/Wikidata:Licensing ,
https://www.wikidata.org/wiki/Wikidata:Data_access ,
https://commons.wikimedia.org/wiki/Commons:Reusing_content_outside_Wikimedia (2026-11-10).

| Item | Requirement |
|---|---|
| Wikidata licence | structured data in the main, Property, Lexeme and EntitySchema namespaces "is released into the public domain under Creative Commons Zero" |
| Wikidata credit | not required ("with no requirement for attribution under CC-0"); requested: "we would greatly appreciate it if, in your project, you mention Wikidata as the origin of your data". Wordings offered by the page: "Powered by Wikidata", "Data from Wikidata", "Source: Wikidata" |
| Wikidata extra request | "offer your users a way to report issues in the data": a link from the item page to the item on wikidata.org does it |
| Wikidata logo | optional "Powered by Wikidata" files on Commons; do not vendor one without reading the file page's own licence and the Wikimedia trademark rules (not read) |
| Commons licence | "Each media file has its licensing specified on its file description page." |
| Commons credit | per file: credit the creator ("it is the content creator who must be credited, not the uploader"), follow the creator's stated credit line if any, and "If the license requires you to link to or provide a copy of the license, do this." Public domain: attribution "is recommended to give correct provenance" |
| Commons hotlinking | "possible, but is not recommended": download one rendition (DEC-194) |

Page rows: "Data from Wikidata (CC0)" with a link to https://www.wikidata.org/ ; "Images
from Wikimedia Commons; each image is credited on its item page" with a link to
https://commons.wikimedia.org/ . The per-file credit ("Photo: <author>, <licence>, via
Wikimedia Commons", licence name linked to its URL, file page linked) belongs next to the
image on the item page (DEC-194), not on the attribution page: the page cannot list every
file within the density budget.

#### Notices the next connectors will add (from Part B)

| Source | Line for the page | Duty |
|---|---|---|
| Open Library | "Book data and covers from Open Library." link https://openlibrary.org/ | courtesy ("A courtesy link back to Open Library is appreciated") |
| TVmaze | "Show and episode data from TVmaze, licensed CC BY-SA 4.0." links: https://www.tvmaze.com/ and https://creativecommons.org/licenses/by-sa/4.0/ ; on the item page link the show's own TVmaze URL | required (CC BY-SA: credit, licence link, say if changed) |
| MusicBrainz | "Music data from MusicBrainz (core data, CC0)." link https://musicbrainz.org/ ; covers: "Cover art from the Cover Art Archive; images belong to their copyright owners." | courtesy for CC0 |
| VNDB | "Contains information from VNDB, which is made available here under the Open Database License (ODbL)." with "VNDB" linked to https://vndb.org/ and the licence name linked to https://opendatacommons.org/licenses/odbl/1-0/ | required when shown publicly (ODbL 4.3, example wording of the licence itself, read 2026-11-10) |
| Metron | "Comic data from Metron, licensed CC BY-SA 4.0." links https://metron.cloud/ and the licence | required |

### A2. Fonts and icons

What ships (`config/brand/ICONS.adoc`, `src/rust/crates/ui/icons/vendor/VERSIONS`):

| Asset | Licence | Notice the licence requires | Text to show |
|---|---|---|---|
| Atkinson Hyperlegible Next 2.001, variable upright and italic woff2 (`config/brand/default/fonts/`) | SIL Open Font License 1.1, no Reserved Font Name (`config/brand/ICONS.adoc:58-61`) | the copyright notice and the licence must travel with every copy; the OFL lets them be "stand-alone text files, human-readable headers or in the appropriate machine-readable metadata fields" that the user can easily view (OFL 1.1 condition 2; the text is `config/brand/default/fonts/OFL.txt`) | "Atkinson Hyperlegible Next. Copyright 2020-2024 The Atkinson Hyperlegible Next Project Authors (https://github.com/googlefonts/atkinson-hyperlegible-next). Licensed under the SIL Open Font License, Version 1.1." plus a disclosure with the full `OFL.txt` (or a link to `/brand/fonts/OFL.txt`, which the app already serves from its own origin per `config/brand/ICONS.adoc:49-52`) |
| Lucide 1.69.0 (`lucide-static`), the icons named in `crates/ui/icons/icons.toml` | ISC; the Feather-derived glyphs are MIT (`src/rust/crates/ui/icons/vendor/lucide/LICENSE`) | ISC: "the above copyright notice and this permission notice appear in all copies"; MIT: the copyright and permission notice "shall be included in all copies or substantial portions" | "Lucide icons. Copyright (c) 2026 Lucide Icons and Contributors. ISC License. Some icons derive from Feather, Copyright (c) 2013-present Cole Bemis, MIT License." plus a disclosure with the vendored `LICENSE` file, complete |
| Tabler Icons 3.50.0, only `meeple` | MIT (`src/rust/crates/ui/icons/vendor/tabler/LICENSE`) | as MIT above | "Tabler Icons (meeple). Copyright (c) 2020-2026 Pawel Kuna. MIT License." (the file writes the first name with the Polish letter; render the name from the file, do not retype it) plus the full file in a disclosure |
| Material Design Icons (`@mdi/svg`) | Apache-2.0 | **not shipped**: DEC-043 kept MDI as a rejected alternative and the vendor directory holds only `lucide/` and `tabler/`. If it is ever added: give recipients a copy of the Apache License 2.0, keep the NOTICE file if the package has one (not verified whether it does, `docs/research/icons.md:43-45`), say so if a file was changed; brand icons of MDI are not covered and must never be used (`docs/research/icons.md:39-43`). Line: "Material Design Icons, Copyright Pictogrammers, Apache License 2.0" plus the licence text | - |
| OT-17 `piggy-bank` | Lucide, same tarball 1.69.0, no new notice | not verified that 1.69.0 contains `piggy-bank` (the owner-test file says it is a Lucide icon) | - |

The licence texts are in the repo and in the image already
(`src/docker/Dockerfile:39-40`: brand directory and `/usr/share/doc/app/icons`); the page
must show them from those files (read at start or `include_str!` on the server side), never
from retyped strings, so a version bump cannot leave a stale notice. The texts stay in
English with `lang="en"`.

Service logos:

- None today. After W1: the TMDB logo (A1). Later, only when enabled: "Powered by BGG".
- If logos from Simple Icons (CC0-1.0) or selfh.st/icons (CC BY 4.0) are ever vendored
  (DEC-043), the CC BY line recorded in `docs/research/icons.md` section 2 applies:
  "Service logos from selfh.st/icons (CC BY 4.0), https://github.com/selfhst/icons; logos
  are trademarks of their respective owners."
- One trademark line for the whole page, shown only when at least one third-party logo is
  rendered: "Names and logos of other services are trademarks of their owners and are shown
  only to identify the service." (KICKOFF section 5: logos only to identify the service.)

White-label: an operator may replace the font and the brand, but the notices page must stay
reachable (`docs/research/icons.md` section 2, last paragraph). An operator who swaps the
font must also swap the font's notice: make the font row data-driven from the brand
directory (name, copyright line, licence file path in the brand config), not a constant.

### A3. Crate and npm licence list at build time

What must be covered:

- Rust crates linked into the server binary and into the wasm bundle sent to every browser
  (MIT, BSD, ISC, Apache-2.0, MPL-2.0, Zlib, Unicode-3.0, BSL-1.0, CDLA-Permissive-2.0, as
  allowed in `src/rust/deny.toml:23-57`). MIT, BSD and ISC require the notice with every
  copy, Apache-2.0 a copy of the licence, so a list of names is not enough: the texts are
  needed, and they must be reachable from the UI because the wasm is distributed to
  visitors.
- npm: **nothing ships**. `src/web/package.json` has only `devDependencies`
  (`@axe-core/playwright` 4.14.0, `@playwright/test` 1.64.1) for the e2e tests; the image
  copies only `src/web/style` and `src/web/public` (`src/docker/Dockerfile:14-15`) and
  `src/web/public` holds only `.gitkeep`. No npm licence list is needed. If a runtime npm
  package is ever added, read `license` from its `package.json` in an xtask step; no extra
  tool.
- Build tools whose output ships: Tailwind CSS standalone CLI 4.3.3
  (`src/sh/ci-tools.sh:8,24`): the generated stylesheet contains Tailwind's base styles
  (MIT). Add one fixed row "Tailwind CSS, MIT, https://tailwindcss.com" (licence not
  re-read today; MIT per the project's repository, from memory). wasm-bindgen's JavaScript
  glue comes from the `wasm-bindgen` crate, which is in `Cargo.lock` and so in the crate
  list. cargo-leptos is a build tool only.
- The Rust standard library is linked statically and is not in `cargo metadata`: add one
  fixed row "Rust standard library, MIT OR Apache-2.0" (from memory, not re-read today).
- The container image is Debian (`src/docker/Dockerfile:31`); its packages keep their own
  notices under `/usr/share/doc/*/copyright` inside the image. One sentence on the page is
  enough; nothing to generate.

Tools (both by Embark Studios, both permissive):

| Tool | Version | Licence | Verified | Gives |
|---|---|---|---|---|
| `cargo-deny` | 0.20.2 (already pinned, `src/sh/ci-tools.sh:5`; newest on crates.io, published 2026-07-09) | MIT OR Apache-2.0 | https://crates.io/api/v1/crates/cargo-deny (2026-11-10) | `cargo deny list -f json -l crate`: licence identifiers per crate, **no texts** (https://embarkstudios.github.io/cargo-deny/cli/list.html) |
| `cargo-about` | 0.9.3, published 2026-10-26, `rust_version` 1.88.0 (the toolchain is 1.99.1, `src/rust/rust-toolchain.toml`) | MIT OR Apache-2.0 | https://crates.io/api/v1/crates/cargo-about ; release assets incl. `cargo-about-0.9.3-x86_64-unknown-linux-musl.tar.gz` with `.sha256` files, https://api.github.com/repos/EmbarkStudios/cargo-about/releases/latest (2026-11-10) | the full licence texts found in each crate, grouped per licence, with the crates that use each text |

Recommendation: `cargo-about`. Reasons: it emits the texts (with each crate's own copyright
lines where the crate ships a licence file), it resolves `A OR B` expressions by a priority
list like cargo-deny does, it has a prebuilt musl binary that fits the existing
`ci-tools.sh` download pattern (`src/sh/ci-tools.sh:15-16`), and it is not GPL.

How (facts from https://embarkstudios.github.io/cargo-about/cli/generate/index.html ,
`.../cli/generate/config.html` and `.../cli/generate/output.html`, 2026-11-10):

1. `src/sh/ci-tools.sh`: add a tool `about` with `ABOUT_VERSION=0.9.3` (same shape as the
   `deny` case). Optionally add `pkgs.cargo-about` next to `pkgs.cargo-deny` in `flake.nix:25`
   (not verified that the pinned nixpkgs has 0.9.x).
2. `src/rust/about.toml` (default location is `<manifest_root>/about.toml`):
   - `accepted = [...]`: the same list as `deny.toml:23-41`, in the order we prefer when a
     crate offers a choice (MIT first, then Apache-2.0, ...). "Priority list of all the
     accepted licenses for a project."
   - per-crate `accepted` for the three exceptions of `deny.toml:44-57`
     (`webpki-root-certs` and `webpki-roots`: CDLA-Permissive-2.0; `libfuzzer-sys`: NCSA).
   - `ignore-dev-dependencies = true` (tests are not distributed). Leave build dependencies
     in: over-attribution is harmless and proc-macro output lands in the binary.
   - `targets = ["x86_64-unknown-linux-gnu", "wasm32-unknown-unknown"]` (plus aarch64 if an
     arm image is built) so Windows-only and macOS-only crates drop out.
   - `private = { ignore = true }` for the workspace's own crates (as `deny.toml:59-60`).
3. New step `cargo xtask licences` (next to the `deny` step at
   `src/rust/xtask/src/main.rs:93`):
   `cargo about generate --workspace --all-features --locked --fail --format json -o <out>/third-party-licences.json`.
   `--fail` "Exits with a non-zero exit code if any crate's license cannot be reasonably
   determined", which makes it a CI gate. Network: without `--offline` the tool "may query
   external sources for license information"; with `--offline` (or `--frozen`) it does not,
   but then clarifications that read licence files from a crate's repository are not
   applied and it falls back to the default licence text, "losing eg. copyright" lines. Use
   `--locked` without `--offline` in CI and in the image build (both have network for
   crates anyway); if the owner prefers no third-party lookups, use `--frozen` and accept
   generic texts for crates that ship no licence file.
4. JSON shape: top-level `overview` (list of `{count, name, id}`: one row per licence) and
   `licenses` (list of `{name, id, text, source_path, used_by}`, `used_by` = list of
   `{crate, path}` where `crate` is the cargo package metadata: name, version, repository,
   authors). `text` is "The full license text". The xtask step should slim this to what the
   page needs (licence id, name, text, and per crate: name, version, repository) and append
   the fixed rows (Rust standard library, Tailwind CSS).
5. Where the file lives: generate it in the image build after the workspace is copied
   (`src/docker/Dockerfile:12`) and copy it to `/usr/share/doc/app/third-party-licences.json`
   beside the icon licences (`src/docker/Dockerfile:40`). The server reads it at start from
   a configured directory and serves (a) the attribution page section server-side and (b)
   a plain-text download. Do **not** `include_str!` it into the `web` crate: that crate is
   also compiled to wasm and several hundred kB of licence text would be sent to every
   browser. In development (`cargo leptos watch`) the file is absent: the section then says
   "The licence list is generated when the release image is built" and the page still works.
   Alternative: commit the generated file and let CI fail on drift (`cargo xtask licences
   --check`); simpler at run time, but it adds a large generated diff to every dependency
   bump. Recommended: generate, do not commit.
6. Tests (test-first): a unit test for the slimming step with a small fixture JSON; an e2e
   check that the page lists at least `leptos` and `axum` when the file is present (name
   the crates after reading `Cargo.lock`, not from this note); the axe pass on the page.

Not verified: whether cargo-about collects Apache `NOTICE` files (its output page does not
mention them). Apache-2.0 section 4(d) asks for NOTICE contents to be passed on. Follow-up
for the workstream agent: list the crates in the cargo registry cache that ship a `NOTICE`
file (a `find` over the vendored sources of the crates in `Cargo.lock`) and append those
texts in the xtask step. Also not verified: an `about.toml` key that switches off the
external lookups without `--offline`.

### A4. Proposed page structure

Route and placement:

- New page `/about` ("About and licences"), a normal content page, **reachable without
  signing in** and linked from: the sign-in page footer, the account menu, the You hub, and
  the bottom of Settings > Data sources (replace the embedded list at
  `web/src/settings.rs:471-487` by a link; keep the anchor `/settings/sources#attribution`
  alive as a link target because the admin page points to it,
  `web/src/admin/sources.rs:75`). Reason: TMDB and Trakt ask for an "About" or "Credits"
  section, the software notices must stay reachable for everyone who receives the wasm
  (also on a white-label instance), and a signed-out visitor of a shared list sees provider
  data too.
- Signed-out visitors see the same page. It lists only names of enabled sources, no keys,
  no status. If the owner does not want the enabled sources public, show sections 2 to 5
  to everyone and section 1 only to signed-in users (default: show all).

Sections (h1 plus five h2; every default view within OD-01: at most 7 rows and 5 inputs):

| # | Section (h2) | Rows by default | Shown when |
|---|---|---|---|
| 1 | Data sources | one row per source: name, the provider's notice (`lang="en"`), licence line, link; the logo where one is required. At most 7 rows; more go behind "Show all sources (N)" | a row only for a connector that is registered (`registry.infos()`, `api/src/ops.rs:262-275`) or an importer that is compiled in. TMDB: feature `tmdb` and a key. TheTVDB: key configured. IGDB, Podcast Index: registered. Trakt: feature `trakt-import`. BGG: enabled with a token. Wikidata and Commons: when W3 ships. If no source is enabled: one sentence "No external data source is set up." |
| 2 | Fonts and icons | 3 rows (font, Lucide, Tabler), each with a "Licence text" disclosure | always; the font row comes from the active brand |
| 3 | Logos and trademarks | 1 or 2 rows (the trademark line; the CC BY credit if such logos ship) | only when at least one third-party logo is rendered |
| 4 | Open-source software | one row per licence from `overview` (name, number of components), the first 6 by count plus "Show all licences (N)"; each row is a disclosure with the licence text and the components (name, version, link). One search field filters components (1 input). A link "Download all notices (text file)" | always; the fallback sentence when the generated file is absent |
| 5 | This application | product name from the brand, version, licence of the software (MIT, `LICENSE` at the repo root, workspace `license = "MIT"` in `src/rust/Cargo.toml:10`), source link if the brand config has one | always |

Row layout for section 1 (one list item, no table, so it reflows at 320 CSS px):
name (bold) / logo if required (linked, `alt` = service name) / notice text / licence line
and link. Required notices are plain text, never inside a collapsed disclosure: TMDB asks
for the notice "prominently".

Data model changes (small):

- `Attribution` (`catalog-core/src/model.rs:44-48`) and `AttributionDto`
  (`contracts/src/lib.rs:264-270`): add `terms_url: String`, `required: bool` and
  `logo: Option<Logo>` with `{ asset, alt, href }` where `asset` names a vendored file
  served by the app (never a provider URL, so no third-party request from the page).
  Fix `admin/sources.rs:449` to use the new `terms_url`.
- A static list of importer credits next to the connector list (Trakt today).
- Later (round-1 WP-OD2): a `licence` tag per field would let this page be generated from
  tags; not needed for W1.

Accessibility (WCAG 2.2 AA) and i18n:

- One h1, h2 per section, lists as `ul`; disclosures are native `details/summary` or the
  existing ui disclosure with `aria-expanded`; OD-02 (remember disclosure state) does not
  need to apply to licence texts.
- Licence and provider texts are English: wrap in `lang="en"` as the current list does
  (`web/src/settings.rs:480-481`). Licence texts in `pre` with wrapping so there is no
  horizontal scroll at 320 px (1.4.10); long crate lists must not trap keyboard focus.
- Logo links: target at least 24 by 24 CSS px (2.5.8), visible focus, accessible name with
  the service name; no information by logo alone (the name is always printed as text).
- Links to providers: `rel="noopener"` only; TheTVDB wants a direct link, so no `nofollow`
  and no redirect page.
- Every label in Fluent en and de (parity test). New keys, draft:
  `about-title` = About and licences / Info und Lizenzen;
  `about-sources-title` = Data sources / Datenquellen (reuse `sources-title`,
  `web/locales/en.ftl:314`); `about-sources-lead` (reuse `sources-lead`, `en.ftl:315`);
  `about-sources-none` = No external data source is set up. / Keine externe Datenquelle
  eingerichtet.; `about-sources-all` = Show all sources ({ $count }) / Alle Quellen
  anzeigen ({ $count }); `about-fonts-title` = Fonts and icons / Schriften und Symbole;
  `about-licence-text` = Licence text / Lizenztext; `about-marks-title` = Logos and
  trademarks / Logos und Marken; `about-marks-line` (the trademark sentence of A2);
  `about-software-title` = Open-source software / Open-Source-Software;
  `about-software-count` = { $count } components / { $count } Komponenten (plural forms);
  `about-software-filter` = Filter components / Komponenten filtern;
  `about-software-download` = Download all notices (text file) / Alle Hinweise
  herunterladen (Textdatei); `about-software-missing` = The licence list is generated when
  the release image is built. / Die Lizenzliste wird beim Bau des Release-Images erzeugt.;
  `about-app-title` = This application / Diese Anwendung; `about-app-licence` =
  { $product } { $version } is licensed under the MIT License. / { $product } { $version }
  steht unter der MIT-Lizenz. Provider notices themselves are not translated.
- White-label test (rule 9): the page takes the product name from the brand; the Trakt
  line already does (`{ $product }`).

Acceptance checks for W1 (suggested): the page is reachable signed-out; with the TMDB mock
enabled the TMDB row shows the exact notice and an `img` whose bytes equal the vendored
file; with no key there is no TMDB row; the Trakt line is present when the feature is on;
font, Lucide and Tabler texts equal the vendored files; axe has no violations; en/de
parity holds.

## Part B - connectors

### B1. Facts per source (Open Library, MusicBrainz, Wikidata, TVmaze, VNDB, Metron)

Base: round-1 `open-data-sources.md` (2026-11-09, branch `wip/fable-reviews`). Each source
was re-read today; "Changed" says what differs from round 1.

One finding applies to four of the six: **the contact in the User-Agent**. The app sends
`<ident>/<version> (+<support_url>)` only when the brand sets `support_url`, else
`<ident>/<version>` with no contact (`src/rust/crates/brand/src/lib.rs:133-137`; built at
`src/rust/apps/server/src/lib.rs:623`). Open Library wants an e-mail or phone number,
MusicBrainz a contact URL or e-mail, Wikimedia a contact (URL or e-mail). Proposal for W5: an
instance setting "operator contact for data sources" (e-mail or URL, asked in the wizard,
editable in the admin UI) that goes into the User-Agent of these connectors; without it
Open Library runs at the anonymous rate, and MusicBrainz and Wikidata stay off with the
reason shown on the admin sources page. This is the operator's contact, not a user's, and
it is sent to the provider with every request: say so next to the field.

#### Open Library

| Item | Value (read 2026-11-10) |
|---|---|
| Data licence | none stated. https://openlibrary.org/developers/licensing : the Internet Archive "does not assert any new copyright or other proprietary rights" over the database; there "may be existing rights issues on some contributions and in some jurisdictions". Treat as `unspecified-open` |
| Terms / policy | https://openlibrary.org/developers/api : "These APIs are **not** intended to serve as a bulk data backend or high-traffic commercial infrastructure."; do not "Make hundreds of single-book requests (use search.json for batch results)"; "Please **do not** use our APIs to bulk download metadata"; "Violations may result in aggressive rate limiting or blocking."; priority to "Open-source and mission-aligned projects", "Book tools & services that facilitate lookup for humans" |
| Auth | none |
| Rate limit | "Default (non-identified requests)": 1 request per second; "Identified requests (with `User-Agent` and `email`)": 3 requests per second |
| User-Agent | "a HEADER that specifies a `User-Agent` string with (a) the name of your application and (b) your contact email or phone number", example `User-Agent: MyLibraryApp (contact@example.org)` |
| Attribution | not required; covers page: "A courtesy link back to Open Library is appreciated." |
| Caching / retention | no limit; caching is what the policy asks for. No purge rule |
| Covers | https://openlibrary.org/dev/docs/api/covers : `https://covers.openlibrary.org/b/$key/$value-$size.jpg`, keys ISBN, OCLC, LCCN, OLID, ID, sizes S, M, L; `?default=false` gives 404 instead of a blank image; lookups by keys "other than CoverID and OLID are rate-limited": "Currently only 100 requests/IP are allowed for every 5 minutes" (403 on excess); "Please, do not crawl our cover API."; "intended for displaying covers on public facing websites and not for bulk download". Licence of the image files: not stated |
| Page counts (OT-17) | search results carry `number_of_pages_median` per work: verified live today, `GET https://openlibrary.org/search.json?q=harry+potter+philosopher&fields=key,title,author_name,first_publish_year,number_of_pages_median,cover_i,edition_count&limit=2` returned 302 for "Harry Potter and the Philosopher's Stone" (`/works/OL82563W`, 400 editions). The field is not in the documented example; the docs say "The schema is not guaranteed to be stable, but most common fields ... should be safe to depend on." Editions carry `number_of_pages`: `GET https://openlibrary.org/isbn/9780140328721.json` returned 96 pages (edition `OL7353617M`, work `OL45804W`) |
| Changed since round 1 | nothing in the terms. New: page-count fields verified; the "identified" tier needs an e-mail or phone in the User-Agent, a URL alone may not count (see the finding above) |

#### MusicBrainz

| Item | Value (read 2026-11-10) |
|---|---|
| Data licence | https://musicbrainz.org/doc/About/Data_License : "The core data of the database is licensed under the CC0, which is effectively placing the data into the Public Domain."; the rest is CC BY-NC-SA 3.0. https://musicbrainz.org/doc/MusicBrainz_Database : core = Areas, Artists, Events, Genres, Instruments, Labels, Mediums, Places, Recordings, Release Groups, Releases, Series, Works, Relationships and URLs, CD Stubs; supplementary = "user submitted annotations, tags (including genre associations) and ratings", derived statistics, search indexes, edit history, non-personal user data |
| Terms | https://musicbrainz.org/doc/MusicBrainz_API and https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting |
| Auth | none for reading public data ("Data submission, as well as requests that involve user information, require authentication.") |
| Rate limit | "All users of the API must ensure that each of their client applications never make more than ONE call per second."; per source IP about one request per second on average, otherwise all requests of that IP are declined with HTTP 503 until the rate drops; global 300 per second |
| User-Agent | required: `Application name/<version> ( contact-url )` or `Application name/<version> ( contact-email )`, e.g. `MyAwesomeTagger/1.2.0 ( me@example.com )`; "there needs to be enough information in the User-Agent string for us to contact the maintainers". Blank or generic agent strings (the page lists blank, "Java", "Python-urllib" and others) are limited to 50 requests per second on average for everyone sending that string, the rest get HTTP 503 |
| Attribution | none for CC0 core; credit and share-alike for supplementary data, which we never store |
| Caching / retention | no limit |
| Format | root `https://musicbrainz.org/ws/2/`; JSON by `fmt=json` or `Accept: application/json`; lookup `/<ENTITY>/<MBID>?inc=`, browse `/<ENTITY>?<LINKED>=<MBID>&limit=&offset=`, search `/<ENTITY>?query=&limit=&offset=` |
| Runtimes (OT-17) | track and recording lengths in milliseconds: not confirmed from the pages read today (the API overview page does not list entity fields) - verify when recording fixtures |
| Changed since round 1 | nothing |

Cover Art Archive (covers) was not re-read today; round 1 stands ("All images are
copyrighted by their respective copyright owners.", no rate limit rule, 2026-11-09).

#### Wikidata (and Wikimedia APIs in general)

| Item | Value (read 2026-11-10) |
|---|---|
| Data licence | CC0 for structured data (https://www.wikidata.org/wiki/Wikidata:Licensing) |
| Terms | https://www.wikidata.org/wiki/Wikidata:Data_access ; https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy (page last edited 2026-03-27); https://www.mediawiki.org/wiki/API:Etiquette ; https://www.mediawiki.org/wiki/Wikimedia_APIs/Rate_limits |
| Auth | none needed |
| Rate limit | **new in 2026**, enforced per minute across "all sites and platforms, including requests to the Action API and REST APIs": unidentified clients (nothing but an IP) 10 requests per minute; unauthenticated client with a compliant User-Agent 200 per minute; authenticated 200 or 2000 per minute by account standing; at most 3 concurrent requests; HTTP 429 (or 503) usually with `Retry-After`, else wait at least five seconds or back off exponentially. The page calls the limits "subject to experimentation and change" |
| User-Agent | `<client name>/<version> (<contact information>) <library/framework name>/<version>`, contact = e-mail, website or wiki user; example `CoolBot/0.0 (https://example.org/coolbot/; coolbot@example.org) generic-library/0.0`; a missing or generic agent can "fail with an HTTP 403 error" |
| Etiquette | requests "in series rather than in parallel"; `maxlag` for non-interactive work; batch ids with `|` (up to 50 entities per Action API request); `Accept-Encoding: gzip`; error code `ratelimited` |
| Attribution | not required; requested ("Data from Wikidata"), plus "offer your users a way to report issues in the data" |
| Caching / retention | no limit |
| Page counts and runtimes | number of pages P1104 on edition items, duration P2047 on films (labels verified in round 1); coverage is sparse and uneven, not measured |
| Changed since round 1 | the numeric rate limits (round 1 had "unpublished for anonymous use"). With a contact User-Agent 200 per minute is ample; without one 10 per minute makes an interactive search unusable. DEC-192's client rules (User-Agent, serial, maxlag) still hold |

#### TVmaze

| Item | Value (read 2026-11-10) |
|---|---|
| Data licence | https://www.tvmaze.com/api : the API is licensed CC BY-SA; the data may be used "for any purpose" as long as "TVmaze is properly credited as source" and the use complies with the ShareAlike provision (round 1 quoted the full sentence and the link to CC BY-SA 4.0 on 2026-11-09) |
| Auth | none for the public API (a separate user API exists for Premium members; not needed) |
| Rate limit | at least 20 calls every 10 seconds per IP address; HTTP 429 on excess, back off and retry; do not leave more than one idle connection open (the IP may be blocked) |
| User-Agent | not required, strongly recommended (so they can identify and contact the application) |
| Attribution | a link back to TVmaze from within the application, for example the URLs the API returns; under CC BY-SA 4.0 also a link to the licence and a note if the data was changed (https://creativecommons.org/licenses/by-sa/4.0/deed.en) |
| Caching / retention | no limit on data. API responses are edge-cached for 60 minutes, so an update can take an hour to show. Images: "You are free to directly link ("hotlink") to our image CDN."; caching recommended; a given image URL never changes content, so images can be cached indefinitely. Rights in the artwork itself: not stated |
| Episode runtimes (OT-17) | verified live today: `GET https://api.tvmaze.com/shows/1/episodes` returned 39 episodes, each with `season`, `number`, `type`, `airdate`, `airtime`, `airstamp` and `runtime` (60, none null); `GET https://api.tvmaze.com/shows/1` has `runtime` and `averageRuntime` (both 60), `externals` (`tvrage`, `thetvdb`, `imdb`), `image.medium` and `image.original` on `static.tvmaze.com`, `updated` (Unix time) |
| Id bridge | `/lookup/shows?thetvdb=:id`, `?imdb=:id`, `?tvrage=:id` (301 on a match, 404 otherwise). There is **no TMDB lookup**: matching an existing TMDB show needs its IMDb or TheTVDB id |
| Changed since round 1 | nothing in the terms; runtime fields now verified |

#### VNDB

| Item | Value (read 2026-11-10) |
|---|---|
| Data licence | https://vndb.org/d17 section 4: "All information on VNDB is made available under the Open Database License." and "Any rights in individual contents of the database are licensed under the Database Contents License."; anime data from AniDB is CC BY-NC-SA 4.0; images and descriptions "may be subject to separate license conditions" |
| Terms | https://api.vndb.org/kana (API v2 "Kana", newest change-log entry 2026-01-10): "This service is free for non-commercial use."; best effort, no stability guarantee; "The data obtained through this API is subject to our Data License" |
| Auth | none for reads ("The majority of the API endpoints below are usable without any form of authentication."); a token only for a user's own list |
| Rate limit | "The server will allow up to 200 requests per 5 minutes and up to 1 second of execution time per minute."; "Requests taking longer than 3 seconds will be aborted." |
| User-Agent | no requirement stated |
| Attribution | ODbL 4.3 notice when shown publicly (wording in A1) |
| Caching / retention | no limit (share-alike applies to a conveyed database, not to display) |
| Hours (OT-17) | `POST /vn` fields `length_minutes` (average of user-submitted play times, may be null), `length_votes`, and `length` (1 to 5, fallback); cover flags `image.sexual` and `image.violence` (0 to 2) must be honoured |
| Changed since round 1 | nothing |

#### Metron

| Item | Value (read 2026-11-10) |
|---|---|
| Data licence | **now known**: CC BY-SA 4.0. Blog post "Using Metron in Commercial Software", 2026-10-04, https://metron-project.github.io/blog/commercial-api-use : "The site's content is licensed under the Creative Commons Attribution-ShareAlike 4.0 International License"; the OpenAPI document at https://metron.cloud/api/schema/ (title "Metron Comicbook Database", version 1.0.0) names the same licence. Round 1 had "not found" |
| Covers | "Metron doesn't hold a license for cover images, and they are not covered by the site's CC BY-SA license."; "Using Metron only to identify comics, and not using its images, avoids the question entirely." |
| Commercial use | allowed ("It doesn't limit what you can do with the data."), subject to the licence |
| Auth | an account is required. Token auth since 2026-07-26 (https://metron-project.github.io/blog/token-authentication): `Authorization: Bearer <your-token>`, tokens created on the profile page, shown once, no expiry by default; Basic auth still works and will be removed later ("a hard cutover"), "new integrations should start with tokens". The schema lists `basicAuth`, `cookieAuth` and `knoxApiToken` (HTTP Bearer) |
| Accounts | multi-user software must have "each user to have their own Metron account"; "Don't ship your app with a single built-in account that all of your users share."; "Rate limits apply per account."; "Please don't create extra accounts to get around the limits." |
| Rate limit | 20 requests per minute and 5,000 per day per account; supporters 7,500 to 25,000 per day (https://metron-project.github.io/blog/supporter-rate-limits); headers `X-RateLimit-Burst-*` and `X-RateLimit-Sustained-*` (`Limit`, `Remaining`, `Reset`); HTTP 429 with `Retry-After` in seconds; a 304 still counts (https://metron-project.github.io/blog/api-best-practices , 2026-04-18, correction 2026-08-21) |
| User-Agent | nothing stated |
| Attribution | "credit Metron, link to the source and the license, and say if you've changed the data"; "You don't need to credit every individual record." |
| Caching / retention | no limit; asked for ("store what you've already looked up instead of requesting it again"); `modified_gt` and `If-Modified-Since` for refreshes |
| Page counts | whether an issue carries a page count: not verified (the schema needs a sign-in to show its components; https://metron.cloud/docs/ answered a bot challenge in round 1 and was not retried) |
| Changed since round 1 | licence known (CC BY-SA 4.0), covers explicitly unlicensed, one account per end user, Bearer tokens. The service software stays GPL-3.0: only blog and API description were read (R-P1) |

Consequence for the design: Metron cannot be an instance-level connector with one key. It
fits the existing per-user credential path (Settings > Data sources shows sources that
"need their own account", `src/rust/crates/web/src/settings.rs:451-452`): each member enters
their own Metron token, searches run with the searching member's token, covers are not
fetched.

Retention summary: none of the six has a TMDB-style time limit, so
`catalog_core::retention::CACHE_LIMITED` stays `["tmdb"]`
(`src/rust/crates/catalog-core/src/retention.rs:26`).

### B2. Recommended order

Where the app stands: connectors exist for movie and show (TMDB, TheTVDB), video_game
(IGDB) and podcast (Podcast Index) (`src/rust/crates/connectors/Cargo.toml:26-33`). The
kind schemas for book, audiobook, comic, manga, music, visual_novel, board_game and anime
exist (`src/rust/crates/kinds/schemas/`) but have no source. The book and comic schemas
already have `page_count` (`kinds/schemas/book.json:24`, `kinds/schemas/comic.json:25`),
the movie and show schemas `runtime_minutes` and `episode_runtime_minutes`
(`kinds/schemas/movie.json:25`, `kinds/schemas/show.json:44`).

What OT-17 needs and who supplies it reliably:

| Stat | Field | Reliable source | Note |
|---|---|---|---|
| Hours of movies | runtime per film | TMDB (already stored) | Wikidata P2047 as a CC0 fallback, sparse |
| Hours of shows | runtime per episode | **TVmaze** `runtime` on every episode (verified); TMDB and TheTVDB episode `runtime` are already fetched and kept (docs/connectors/tmdb.md "Season episodes", docs/connectors/tvdb.md) | the gap today is not a source but the stats query: per-episode runtimes are not read by the stats (DEC-450 item 1). OT-17 can start on TMDB data; TVmaze makes it work without a key and without the six-month rule |
| Pages read | page count per book | **Open Library**: `number_of_pages_median` per work from search, `number_of_pages` per edition by ISBN (both verified) | the only one of the six with usable page counts; Wikidata P1104 is sparse; DNB (round 1) has pages for German editions but needs MARC parsing. Keep the field user-editable: the median is an estimate across editions |
| Hours played | - | none for video games (IGDB time-to-beat not checked); VNDB `length_minutes` for visual novels | hours played must come from the user's own log (OT-16) |
| Hours listened | track lengths | MusicBrainz (field not verified today) | later |

Order:

| # | Connector | User value | Effort | Terms risk | Why here |
|---|---|---|---|---|---|
| 1 | `openlibrary` | high: books have no source at all; unlocks "pages read" (OT-17) and ISBN matching for the Goodreads import the owner will deliver | medium: search, work, edition by ISBN, covers by cover id; no auth | medium-low: no licence text and a "not a backend" policy, but our use (human lookups, cached, identified) is the use they prioritise; needs the contact User-Agent | fills the largest hole and the one OT-17 stat that has no source |
| 2 | `tvmaze` | high: shows work without any key, per-episode runtimes and air dates, no retention purge | low-medium: the episode pipeline exists (`catalog_episodes`, WP4-7); new are the mapping, the id bridge through IMDb or TheTVDB ids and the CC BY-SA credit | low: clear licence; share-alike only matters for exports and the hub | makes a fresh instance useful before the operator has a TMDB key; gives OT-17 its hours of shows on open data |
| 3 | `musicbrainz` (+ Cover Art Archive) | medium: music kind gets a source; audiobook and audio-drama release groups as a bonus | medium-high: release group / release / recording model, 1 request per second needs a queue, field allowlist for CC0 core only | low (CC0), strict rate rule | after the two that serve OT-17; mandatory contact User-Agent |
| 4 | `vndb` | low-medium (niche kind), but complete for it, with play-time minutes | low: one POST endpoint, no key | medium: API "free for non-commercial use", so a notice for commercial operators; image flags | cheap; do it when a small package is wanted |
| 5 | `wikidata` generalised | indirect: CC0 identity under every kind, removes the "Title unavailable" stub after a TMDB purge (DEC-466) | high: property map per kind, merge priority, licence tags | low; needs the contact User-Agent (10 per minute without) | build the client inside W3 (board games) first; generalise afterwards, where it becomes cheap. Round 1 put it first (WP-OD1) for TMDB independence; by user value per kind and OT-17 it comes after the kinds that have nothing |
| 6 | `metron` | medium for comic readers | medium: per-user tokens, 20 per minute, no covers | medium: share-alike, one account per user, Basic auth going away | last; needs the per-user credential path and a decision that comics without covers are acceptable |

Open Library and TVmaze are independent of each other and of W3, so two agents can build
them in parallel. Both need the operator-contact setting first (small shared package).

### B3. Draft connector docs for the first two

Drafts in the format of the existing files. Field names marked "confirm" were not seen in a
response today and must be checked when the fixtures are recorded (no key is needed, so
`cargo xtask record <id>` can record real responses).

Draft `docs/connectors/openlibrary.md` (replaces the "not implemented: rule 6" stub, which
R-P1 / DEC-169 superseded):

```markdown
# Connector: Open Library (`openlibrary`)

Date checked: 2026-11-10. Status: **terms recorded, not implemented yet** (W5).

The service software is AGPL-3.0 (github.com/internetarchive/openlibrary). Under owner
ruling R-P1 (DEC-169) only the public API documentation and policy pages were read; no
source code, schema file or UI text of the project was read or copied. The search docs link
to a Solr schema file in that repository for the full field list: do not open it.

| Item | Value |
|---|---|
| Kinds | book (audiobook editions by ISBN later) |
| Capabilities | search, details, images |
| API docs | https://openlibrary.org/developers/api ; search https://openlibrary.org/dev/docs/api/search ; works, editions, ISBN https://openlibrary.org/dev/docs/api/books ; covers https://openlibrary.org/dev/docs/api/covers |
| Terms | usage policy on https://openlibrary.org/developers/api ; licensing https://openlibrary.org/developers/licensing |
| Licence | No licence text. The Internet Archive "does not assert any new copyright or other proprietary rights" over the database and notes that there "may be existing rights issues on some contributions and in some jurisdictions". Licence tag `unspecified-open`: display freely, never contribute these fields to a CC0 hub |
| Attribution | Not required. "A courtesy link back to Open Library is appreciated." We show "Book data and covers from Open Library." with a link to https://openlibrary.org/ on item pages that use it and on the attribution page |
| Auth | none |
| User-Agent | required for the higher limit: application name plus contact e-mail or phone, e.g. `MyLibraryApp (contact@example.org)`. Ours: brand ident, version and the operator contact of the instance |
| Rate limits | 1 request/s without identification, 3 requests/s with User-Agent and e-mail. Policy: 2/s, burst 2, 2 concurrent when the operator contact is set, else 1/s, burst 1, 1 concurrent. Covers by ISBN, OCLC or LCCN: 100 requests per IP per 5 minutes (403 on excess); covers by cover id or OLID are not limited that way |
| Usage rules | not "a bulk data backend or high-traffic commercial infrastructure"; no bulk download of metadata (monthly dumps exist); not hundreds of single-book requests (use `search.json`); do not crawl the covers API. An operator who runs a high-traffic commercial service must not rely on this connector: say so on the admin page |
| Caching | no limit; cache responses (TTL 7 days) and store covers under the storage budget. No retention rule |
| Hosts | `openlibrary.org`, `covers.openlibrary.org` (confirm when recording whether cover requests redirect to another host, and add it) |
| Status | planned behind cargo feature `openlibrary`; on by default once built (no key, clear usage policy) |

## Endpoints used

| Op | Request |
|---|---|
| search | `GET https://openlibrary.org/search.json?q=<q>&fields=key,title,author_name,first_publish_year,cover_i,edition_count,number_of_pages_median&lang=<xx>&limit=20` (`lang` = two-letter code, boosts matching editions) |
| work details | `GET https://openlibrary.org/works/{OLID}.json` |
| edition by ISBN | `GET https://openlibrary.org/isbn/{isbn}.json` (redirects to the edition, `/books/{OLID}.json`) |
| cover | `https://covers.openlibrary.org/b/id/{cover_i}-L.jpg?default=false` (404 when there is none); never the ISBN form in a loop |

## Fields used

- search `docs[]`: `key` (`/works/OL...W`), `title`, `author_name[]`, `first_publish_year`,
  `cover_i`, `edition_count`, `number_of_pages_median` (seen in a live response on
  2026-11-10; not in the documented example; "The schema is not guaranteed to be stable").
- edition (`/isbn/...json`): title, publisher, publish date, language, number of pages,
  ISBN-10 and ISBN-13, cover ids, work key, author keys (values seen 2026-11-10 for ISBN
  9780140328721: 96 pages, edition `OL7353617M`, work `OL45804W`). Exact JSON key names:
  confirm when recording (`number_of_pages`, `isbn_13`, `covers`, `works` appear in the
  documented legacy examples).
- work: description, subjects, cover ids, author keys - confirm names when recording.

Mapping: `number_of_pages_median` -> `page_count` of a work-level book item, marked as an
estimate; an edition's page count replaces it when the user picked an edition by ISBN. The
user can always overwrite the page count.

## Fixtures

`src/rust/crates/connectors/fixtures/openlibrary/*.json`: to be recorded with
`cargo xtask record openlibrary` (no key needed); small JSON responses only, trimmed to the
fields used, source URL and date in the fixture header; no images. Until recorded, synthetic
fixtures built from the documented examples, marked `synthetic`.
```

Draft `docs/connectors/tvmaze.md` (new file):

```markdown
# Connector: TVmaze (`tvmaze`)

Date checked: 2026-11-10. Status: **terms recorded, not implemented yet** (W5).

| Item | Value |
|---|---|
| Kinds | show |
| Capabilities | search, details, images, episodes |
| API docs | https://www.tvmaze.com/api |
| Terms | licensing section of https://www.tvmaze.com/api |
| Licence | CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/): the data may be used "for any purpose" as long as "TVmaze is properly credited as source" and the use complies with the ShareAlike provision. Licence tag `CC-BY-SA-4.0` on every TVmaze field: shown on the instance with credit; an export that contains TVmaze data carries the credit and the licence; never proposed to a CC0 hub |
| Attribution | Required. A link back to TVmaze satisfies it, for example the URLs the API returns. We show "Show and episode data from TVmaze, licensed CC BY-SA 4.0." on the attribution page (links to https://www.tvmaze.com/ and to the licence) and "Source: TVmaze" linked to the show's `url` on item pages that use it |
| Auth | none (public API). The Premium user API is not used |
| User-Agent | not required, strongly recommended; ours: brand ident, version and the operator contact |
| Rate limits | at least 20 calls per 10 seconds per IP address; 429 on excess. Policy: 1 request/s, burst 5, 2 concurrent; back off on 429; keep at most one idle connection (TVmaze may block an IP that leaves more open) |
| Caching | no limit on data. Responses are edge-cached for 60 minutes on their side; our TTL 24 hours. Images may be hotlinked; caching is recommended and an image URL never changes content, so a stored cover does not need refreshing. Rights in the artwork are not stated: store with provenance, remove on request, as for other providers. No retention rule (not in `CACHE_LIMITED`) |
| Hosts | `api.tvmaze.com`, `static.tvmaze.com` (image URLs returned by the API on 2026-11-10) |
| Status | planned behind cargo feature `tvmaze`; on by default once built (no key, clear licence) |

## Endpoints used

| Op | Request |
|---|---|
| search | `GET https://api.tvmaze.com/search/shows?q=<q>` |
| details | `GET https://api.tvmaze.com/shows/{id}` |
| seasons | `GET https://api.tvmaze.com/shows/{id}/seasons` |
| episodes | `GET https://api.tvmaze.com/shows/{id}/episodes` (specials only with `specials=1`; not requested) |
| id bridge | `GET https://api.tvmaze.com/lookup/shows?imdb=<tt id>` or `?thetvdb=<id>` (301 to the show on a match, 404 otherwise). No lookup by TMDB id exists |
| changes | `GET https://api.tvmaze.com/updates/shows?since=day` (optional, to refresh only shows that changed) |

## Fields used

- show: `id`, `url`, `name`, `type`, `language`, `genres`, `status`, `runtime`,
  `averageRuntime`, `premiered`, `ended`, `network`, `webChannel`, `externals` (`imdb`,
  `thetvdb`), `image.original`, `summary` (HTML: strip tags), `updated`.
- episode: `id`, `season`, `number`, `type`, `airdate` (`YYYY-MM-DD`), `airstamp`,
  `runtime` (minutes). Kept in `catalog_episodes` like the TMDB and TheTVDB episodes: number,
  air date, runtime; no names, summaries or stills.

Seen in live responses on 2026-11-10 (`/shows/1`, `/shows/1/episodes`: 39 episodes, every
one with `runtime` 60).

Mapping: `averageRuntime` (else `runtime`) -> `episode_runtime_minutes`; per-episode
`runtime` -> the episode row, which the stats use for hours watched (OT-17, DEC-450).

## Fixtures

`src/rust/crates/connectors/fixtures/tvmaze/*.json`: to be recorded with
`cargo xtask record tvmaze` (no key needed); small JSON, trimmed, no images. Fixture data
is TVmaze data under CC BY-SA 4.0: the fixture header and this file name TVmaze as the
source with the licence link.
```

## Could not verify

- IGDB attribution rules: https://api-docs.igdb.com/ and https://www.igdb.com/api answered
  HTTP 403 today. The 2026-11-06 record in docs/connectors/igdb.md is the only basis; a logo
  duty was not found then.
- BGG: https://boardgamegeek.com/using_the_xml_api answered HTTP 403. Wording taken from the
  owner's copies in `owner-inbox/bgg/`; the "Powered by BGG" files and their link target
  were not captured there.
- Trakt policy: not re-read today (the raw file path guessed in the MIT repo gave 404);
  wording from docs/connectors/trakt.md (read 2026-11-07).
- Podcast Index statement: not re-read (403 to the build host on 2026-11-09).
- TMDB: no rule found on logo background or minimum size; the logos page lists files and
  colours only.
- Quotes were returned by a page-extract tool; the long TVmaze licence sentence and the
  Open Library licensing page were paraphrased by it, so round 1's verbatim quotes of
  2026-11-09 remain the reference for those two.
- cargo-about: handling of Apache `NOTICE` files; a config key to disable external
  lookups; whether the pinned nixpkgs carries 0.9.x. cargo-about was not run (no builds).
- Tailwind CSS and Rust standard library licences: stated from memory, not re-read today.
- Lucide 1.69.0 containing `piggy-bank`: not checked (the tarball was not downloaded).
- Open Library: JSON key names of work and edition records beyond those listed; whether a
  URL-only contact counts as "identified"; whether cover requests redirect to another host.
- MusicBrainz: length fields of tracks and recordings; Cover Art Archive not re-read.
- Metron: issue page-count field; API docs site (bot challenge in round 1).
- Wikidata: coverage of P1104 and P2047 (not measured); the licence of the "Powered by
  Wikidata" image files.
- TheTVDB: pricing and free tier are still not stated on the page.
