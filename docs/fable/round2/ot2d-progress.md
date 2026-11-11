# Package ot2d - progress log

Brief: `docs/handoff/fable-briefs/PKG-implement.md`, scope: `docs/fable/round2/next-packages.md`
section "ot2d" (OT-13 colour, OT-14 page widths; DEC-540, DEC-541), design
`docs/fable/round2/ux-owner-test.md` section 3. Nothing is committed. One block per item:
state, tests and results, files, DECs. A later run continues from the first item that is not
`done`.

How the tests are run here (the orchestrator runs `cargo xtask ci` / `e2e`, not this package):
everything inside `nix develop`, from `src/rust`, with `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0`;
Playwright by hand on own ports (3110 to 3114) and an own database (`ot2d_<n>` on 55432), as in
the ot2b log.

## Items

| # | Item | State | DEC |
|---|---|---|---|
| 1 | brand: contrast check extended to the accent, soft and group pairs (`contrast.rs`), server start-up log | done | 580 |
| 2 | default brand file: `[theme]` / `[theme_dark]` with the violet accent and four group tints | done | 580 |
| 3 | stylesheet: new variables with neutral fallbacks, `@theme inline` mapping | done | 580 |
| 4 | ui: `Tint`, badge / segmented / tabs / progress / nav / tooltip take the accent or a tint | done | 581 |
| 5 | web: tints on the kind badge, history icon, time tiles, charts, group switchers; accent on the next-up row | done | 581 |
| 6 | web: page widths (96 rem grid pages and top bar, 80 rem record pages) | done | 582 |
| 7 | guard test: no palette colour in code outside the brand folder | done | 583 |
| 8 | Playwright: widths at 2560 px, tints and contrast in both themes, axe | done | - |
| 9 | docs: white-labelling page, DEC entries | done | 580-583 |
| 10 | checks: fmt, clippy native + wasm, server page tests, cargo leptos build, whole chromium project, screenshots | done | - |

## Log

### First pass (2026-11-11)

DEC numbers in the table are planned (580 to 583); they are written at the end, check the last
number in docs/DECISIONS.md first.

Red runs seen before the code:
- `cargo test -p brand --test contrast`: compile error (no `contrast_checks`, no
  `foreground_token` / `required` on the finding), then 14 passed with the code and the
  brand file values. The ratio table printed by
  `every_pair_of_the_default_brand_passes_in_light_and_dark -- --nocapture` goes into the DEC.
- `cargo test -p web --features ssr --lib`: compile error (no `page_width`, `PageWidth`,
  `Tile.tint`, `Tile.icon`), then 191 passed.
- `cargo test -p ui --lib`: 17 passed (tint classes).
- `cargo test -p xtask` `colour` / `brand_colours_not_hard_coded`: 3 passed (a guard: it
  passes on the repo, the temp-dir test shows it finds a palette class).

Files so far: `config/brand/default/brand.toml`, `crates/brand/src/{contrast,lib}.rs`,
`crates/brand/tests/contrast.rs`, `apps/server/src/lib.rs` (start-up log),
`src/web/style/input.css`, `crates/ui/src/{tint (new),lib,badge,chip,tabs,progress,nav,tooltip}.rs`,
`crates/web/src/{tint (new),lib,shell,time_tiles,charts,home,library,history,item,palette,
episode_list,notify_channels}.rs`, `xtask/src/whitelabel.rs`, `xtask/tests/whitelabel.rs`.

Choices taken (go into the DECs):
- text links keep the foreground colour and their underline; only `ButtonVariant::Link`
  takes the accent (inline links sit on alert and soft surfaces whose pairs are not checked).
- the tooltip is neutral (`bg-foreground text-background`), it was dark only because the
  old primary was dark.
- selected segment / tab: accent text on the background chip; active navigation item:
  accent text on `primary-soft`; progress bars: accent.
- library cards carry no kind badge today, so nothing is tinted there.


### Final runs (2026-11-11), all items done

DEC-580 (values, check, ratios), 581 (where the colour goes), 582 (widths), 583 (guard test)
are written.

- clippy pedantic: wasm (`-p web -p ui --all-targets --target wasm32-unknown-unknown
  --features web/hydrate,web/gallery`) clean; native (`--workspace --all-targets`, CI feature
  set) found `approx_constant` on the ratio 6.28 in the brand test (allowed with a comment),
  then clean. Not rerun after the last edit (one assertion string in
  `apps/server/tests/it/home_page.rs`).
- `cargo fmt --all --check`: clean (after that edit too).
- `cargo test -p brand`: 5 + 14 + 5 passed. `cargo test -p ui --lib`: 17. `cargo test -p web
  --features ssr`: 191 + 3. `cargo test -p xtask --test whitelabel --test locales`: 2 + 5.
- Server page tests (`--test it -- item_page:: library_page:: home_page:: auth_pages::
  admin_pages:: brand::`, `TEST_DATABASE_URL`, `REQUIRE_SERVICES=1`): first run 22 passed,
  1 failed: `home_page::home_shows_continue_up_next_activity_and_stats` asserted
  `...planned</li>`; the line now sits in a `<span>` after its group marker. Assertion updated
  to the new markup, then 23 passed.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
- Playwright, new tests alone (database `ot2d_1`): `shell.spec.ts` "wide screens: ..." and
  `home.spec.ts` "colour: ..." passed. They were written after the code and passed on their
  first run: not seen red in the browser (the unit tests behind them were).
- Whole Playwright `chromium` project on that build (database `ot2d_2`, 2 workers, ports 3110
  to 3114): 137 passed (setup included), 0 failed, 6.6 minutes; 0 `panicked at` lines in the
  server log of the run. No source file of the app changed after this build.
- Screenshots: 20 PNG in `docs/fable/round2/ot2d-shots/` (home, library, item, stats,
  log-dialog; 1280 and 390 px; light and dark), from a temporary spec run against the same
  build (the spec is removed again). The folder has a `.gitignore` with `*`, so nothing in it
  shows up in git.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the wasm
  browser tests of `ui` (`cargo test -p ui --target wasm32-unknown-unknown`), a load run (no
  SQL or read changed).
- Left in place: `target/site` and `target/debug/server` (the final build). Test executables in
  `target/debug/deps` deleted, the `test_*` and `ot2d_*` databases dropped.

Noticed, not built (out of scope):

- `docs/design/ux/design.md` 2.7 ("The default is neutral on purpose"), 2.9 and line 864
  (80 rem) and HANDOFF still describe the old state; the orchestrator owns those files.
- The admin Branding page shows only the two accent swatches; the tints and the soft surfaces
  are not shown there.
- Library cards and Home rows have no kind badge or icon, so no tint (DEC-581).
- Plain text links are not in the accent (DEC-581, limit 1).
- `tests/brands/sample/brand.toml` (green `primary` in light only) is expected to get
  start-up warnings for the accent as text on the page surfaces (the extended check; not
  looked at in a log).

## Review fixes

Review: `docs/reviews/2026-11-11-ot2d-review.md` (OT2D-1 to 13). Same uncommitted tree, nothing
committed. One block per finding; a later run continues at the first finding without a state.
Browser runs as above (ports 3110 to 3114, databases `ot2dfix_<n>` on 55432).

State when this run started: `target/debug/server` (13:54) was newer than `target/site`
(13:17) and not a cargo-leptos build (the page asked for `/pkg/app_bg.wasm`, 404, nothing
hydrated), so the reviewed tree was built again with `cargo leptos build --bin-features
gallery,all-connectors --lib-features hydrate,gallery` (ok, 8 min) before any Rust file was
touched. The red runs below are against that build.

### Red runs in the browser (reviewed code, new or extended tests)

Database `ot2dfix_0`, `--project=chromium --workers=2`, the seven tests named below:
6 failed, 1 passed (the nudge test, which is the "stays as it is" half of OT2D-5).

- OT2D-1 `sources.spec.ts` "the open palette with its no-results card ..." with the extended
  `expectReflow` (now also every open dialog and every box that scrolls up and down):
  `palette 360 horizontal overflow: ["div (dialog) by 48px"]`. The reviewer's sum is confirmed
  in a browser, and it is 48 px, not 22.
- OT2D-2 `shell.spec.ts` "wide screens: ...": `/lists: unused room right of the content`,
  expected 0, received 480 (Home and Library pass the same check).
- OT2D-4 `shell.spec.ts` "the current tab has a bar above it, ...": no bar.
- OT2D-5 `home.spec.ts` "colour: ...": the "All history" link is `foreground`, not `primary`.
- OT2D-13 `shell.spec.ts` "the chord hint takes no room until a chord waits, ...": the empty
  hint is a 364 px2 box with a 1 px border and 12 px padding on `/`.
- OT2D-13 `home.spec.ts` "stats at 390 px: the month labels under the columns do not touch":
  the second label starts 10.4 px inside the first (twelve months, six labels).

### OT2D-3 colour guard - done (tests green, not yet clippy)

`xtask/src/whitelabel.rs`: `colour_literal(line)` finds, in `crates/ui/src`, `crates/web/src`
and `apps/server/src` (comment lines are skipped): a palette class of any of the 22 Tailwind
families, `black` / `white` on a colour utility, an arbitrary colour value (`bg-[#...]`,
`text-[oklch(...)]`, `bg-[red]`), a hex colour (3, 4, 6, 8 digits; not `href="#add"`, not
`&#160;`), a colour function (`rgb(`, `rgba(`, `hsl(`, `hsla(`, `hwb(`, `lab(`, `lch(`,
`oklab(`, `oklch(`, `color-mix(`), an SVG `fill` / `stroke` / `stop-color` / `flood-color` /
`lighting-color` literal and an inline `style` colour (`style="..."`, `style=format!("...")`,
`style:prop="..."`). Allowed: `currentColor`, `none`, `transparent`, `var(--token)`. The
stylesheet keeps the old rule (the six brand families). `colour_offenders` now returns
`path:line: literal`.
- Red on the tree: `cargo test -p xtask --lib --test whitelabel`: `brand_colours_not_hard_coded`
  failed with `apps/server/src/lib.rs:554: rgb(` (a log message) and
  `crates/ui/src/dialog.rs:175: bg-black` (the scrim). The four unit tests were written with
  the code and passed on their first run (not seen red).
- Fixed: the scrim is the token `--scrim` (`input.css`, `bg-scrim`); the log message reads
  "use a hex value or the rgb or oklch notation". Then 17 + 2 passed.

### OT2D-6, 7, 8 brand crate - done

- OT2D-7: `checked_pairs` has `primary-foreground/75` on `primary` at 4.5:1 (the count of a
  pressed chip, blended in sRGB, 8 bit) and each `group-<id>` on `info-soft` at 3:1 (history
  icon of a shared row). 36 pairs per theme. Default brand: count 4.52 (light) / 4.84 (dark);
  tints on `info-soft` 4.90 to 6.41 (light), 6.19 to 8.75 (dark).
- OT2D-6: test `the_sample_brand_gets_the_warnings_its_light_green_accent_earns` loads
  `src/web/tests/brands/sample` and pins five light findings (4.18, 3.00, 4.37, 4.37, 3.97);
  the sample brand itself is unchanged. The guide's example is now `#016630` / `#ffffff` /
  `#f0fdf4`; test `the_example_of_the_white_labelling_guide_passes` reads the adoc.
- OT2D-8: unit test `the_fallbacks_are_those_of_the_stylesheet` parses `:root` and `.dark` of
  `input.css` and compares every token a checked pair names with `fallback`.
- Red: `cargo test -p brand` first run 16 passed, 2 failed (a wrong expected blend for dark,
  and the guide's first new value `#008236` failing the new count pair at 3.48, so the guide
  got `#016630`); second run 1 failed (my test brand lacked `primary-soft`); then 7 + 18 + 5
  passed.

### OT2D-1, 2, 4, 5, 9, 10, 12, 13 code - written, unit level green, browser runs pending

- OT2D-1: palette scope switcher in a `-mx-6 max-w-[calc(100%+3rem)] overflow-x-auto px-6`
  strip (`palette.rs`); the marker dot of a group option shows from 30 rem up only
  (`max-[30rem]:hidden`, `ui/src/chip.rs`), so the five English scopes fit a 360 px phone
  again (302 px in 312) and Library / History / Stats do not scroll their strip there.
- OT2D-2: `/lists` and `/people` stay grid pages and lose their `max-w-5xl` / `max-w-4xl`;
  the card lists get a fourth (lists) / third (people) column where they have 80 rem or more
  (container query `@7xl`, so the list cards on a person's page stay at three); requests and
  merge suggestions on People keep `max-w-4xl` (lines of text).
- OT2D-4: the current tab of the mobile tab bar has a 2 px bar in `primary` above it
  (`ui/src/nav.rs`, `::before`).
- OT2D-5: 38 text links on the page / on cards / in dialogs take `text-primary` and keep the
  underline; the 8 links inside alerts (nudge, admin attention lists, storage, hosted banner,
  sign-in outage, sources "missing" alert) and the error summary keep the text colour.
  Title links that underline on hover only are not coloured.
- OT2D-9: unit test path `/dev/gallery`; `low_contrast_primary_is_reported_per_theme` pins 6
  findings; the browser tests read the brand colours from the brand file (`brandValue` in
  `helpers.ts`), nothing pinned; comment in `home.spec.ts` corrected. The two mutation runs
  the review suggests were NOT done (each needs a full cargo-leptos build); the width test
  was seen red on OT2D-2 and the colour test on OT2D-5 instead.
- OT2D-10: gallery section "Tints" (six samples: marker, kind badge, soft surface with text
  and icon, bar list; the group switcher with markers; a text link) and the test
  `gallery.spec.ts` "tints: ..." (new with the code; it cannot pass on the old build, not run
  there).
- OT2D-12: "contrast" typo, adoc "above", guide example and pair list, design.md 2.7, 2.9
  and the shared page rule in section 6.
- OT2D-13: chord hint is `sr-only` while empty (stays a live region); the column chart hides
  every second time label where the chart is narrower than 28 rem (container query
  `@max-md`), unit test `a_narrow_chart_names_every_second_label_only`.

Runs after these edits: `cargo test -p ui --lib` 17 passed; `cargo test -p web --features
ssr,gallery` 192 + 3 passed; `cargo test -p brand` 7 + 18 + 5; `cargo test -p xtask --lib
--test whitelabel --test locales` 17 + 2 + 5; `cargo fmt --all --check` clean; clippy pedantic
native (`--workspace --all-targets`, CI features) clean after one fix (`is_multiple_of`), wasm
(`-p web -p ui`, `web/hydrate,web/gallery`) clean; server page tests (`--test it -- item_page::
library_page:: home_page:: auth_pages:: admin_pages:: brand:: shell:: ssr_routes::
setup_page::`) 32 passed.

### Build and targeted browser runs (new code)

- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok
  (run after the server page tests, which overwrite `target/debug/server` with a non-leptos
  binary; that is what broke the state found at the start). The new utilities are in
  `target/site/pkg/app.css` (`bg-scrim`, `@max-md:hidden`, `@7xl:grid-cols-3/4`,
  `max-[30rem]:hidden`, the `before:` bar).
- Database `ot2dfix_1`: the seven tests of the red run plus `gallery.spec.ts` "tints: ..." and
  "gallery has no WCAG A/AA violations ..." and `keyboard.spec.ts` "G chords ...": 11 passed
  (setup included), 0 failed.
- DEC-584 (links, supersedes DEC-581 limit 1), DEC-585 (colour guard, supersedes the scope of
  DEC-583), DEC-586 (the other findings; amends DEC-580 to 36 pairs and DEC-581 on the
  marker dots; OT2D-11 unchanged; deferred items) are written.

### Final runs of the review fixes (2026-11-11) - all findings closed

| Finding | State | DEC |
|---|---|---|
| OT2D-1 palette at 360 px, reflow helper | applied | 586 |
| OT2D-2 Lists and People use the grid width | applied | 586 |
| OT2D-3 colour guard | applied | 585 |
| OT2D-4 current mobile tab | applied | 586 |
| OT2D-5 links in the accent | applied | 584 |
| OT2D-6 sample brand warnings pinned, guide example passes | applied | 586 |
| OT2D-7 pressed chip count (and tint on `info-soft`) in the check | applied | 586 |
| OT2D-8 fallbacks tied to `input.css` | applied | 586 |
| OT2D-9 test notes | applied, except the two mutation runs | 586 |
| OT2D-10 gallery tints, more screenshots | applied | 586 |
| OT2D-11 top bar wider than record pages | no change (owner ruling DEC-541) | 586 |
| OT2D-12 typo, adoc, design.md | applied; Branding page swatches deferred | 586 |
| OT2D-13 chord hint, chart labels | applied; broken cover presentation deferred | 586 |

- Whole Playwright `chromium` project on the final build, first run (database `ot2dfix_2`,
  2 workers): 140 passed, 1 failed: `gallery.spec.ts` "chips, segmented control, tab bar
  action and copy field" looked up `radio "List"` on the whole page, which now also matches
  the new "Listen" option of the Tints section (strict mode). The lookup is scoped to its
  "View" group (test fix, no app change). 0 `panicked at` lines.
- Whole `chromium` project again, same build (database `ot2dfix_4`, 2 workers): 141 passed
  (setup included), 0 failed, 6.8 minutes; 0 `panicked at` lines in the server log. No file of
  the app changed after this build; `cargo fmt --all --check` clean at the end.
- Screenshots: 25 PNG in `docs/fable/round2/ot2d-shots/` from a temporary spec against the
  same build (removed again): the 20 names as before, plus `history-1280-light.png`,
  `stats-group-1280-dark.png` (the Read tab), `palette-390-light.png`, `home-2560-light.png`,
  `item-2560-light.png`. The seed has twelve months on the Stats axis now (a book log in
  December 2025), otherwise the data of the first set.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the wasm
  browser tests of `ui`, the whole `cargo test --workspace` (only: brand, ui lib, web ssr,
  xtask lib / whitelabel / locales, server `it` page modules), the two OT2D-9 mutation runs.
  The web unit tests ran before the one-line clippy fix in `charts.rs` (`is_multiple_of`);
  clippy, the build and the browser runs are after it.
- Left in place: `target/site` and `target/debug/server` are the cargo-leptos build of the
  final code (15:00). Test executables in `target/debug/deps` deleted; databases
  `ot2dfix_0..4` and the `test_*` databases of the server test run dropped. 8.5 GB free.

Noticed, not built:

- Palette at 390 px: the key hints of the active option ("Enter Open, Ctrl+Enter ...") lie
  over a title that wraps (visible in `palette-390-light.png`, "Pixel Harbour (2020)"). Not
  from this package.
- On `/stats` the top bar marks "Home" as the current section (the section map sends `stats`
  to Home); visible in `stats-group-1280-dark.png`. Existing behaviour.
- Admin Branding page: only the two accent swatches (DEC-586, deferred).
- A cover that fails to load shows its alt text under the card's button (DEC-586, deferred).
- An `npx tsc` call of this run pulled the placeholder npm package `tsc` into the npm cache
  (it only prints a notice; nothing in the repo changed). There is no type check for the
  specs in the dev shell.
