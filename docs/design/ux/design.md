# Brand and UX design

Status: proposal v1, 2026-11-06; owner rulings applied (owner review 2026-11-06, DEC-171, DEC-172; see section 10). Companion documents: `research.md` (evidence, cited) and
`mockups/index.html` (static screens). Written for the owner and for the developer who
implements it in the Leptos/Tailwind codebase (`src/rust/crates/ui`, `src/rust/crates/web`).

Conventions: the product is white-label, so this document calls it "the product" or
"[Brand]". The default name (Hordwyn) stays in `config/brand/default/brand.toml` only. Every claim that
rests on outside evidence points to a numbered finding in `research.md` as `[R-nn]`; everything
else is design judgement and is marked as such where it matters. ASCII throughout, except the
German microcopy examples.

Contents

1. What we are designing and why
2. Brand: positioning, personality, voice, microcopy, naming, logo, colour, type, spacing,
   radius, elevation, motion, icons
3. Information architecture
4. Deployment modes and the credential model
5. Key flows, step by step
6. Page-by-page specifications
7. Component inventory (what exists in `ui`, what to add or change)
8. Accessibility requirements
9. Implementation plan (phased, mapped to the owner's complaints)
10. Decisions for the owner

---

## 1. What we are designing and why

The build works end to end (sign-up, search, add, log, companions, lists, sharing,
notifications, integrations), but it was built milestone by milestone, so every page is a
form on a white page and the navigation is a flat row of seven links. The owner's test run
produced six complaints; all of them are symptoms of three missing things:

1. **No model of "where am I and what is the next step".** The home page is a marketing
   stub, the library is one grid for twelve kinds, and the header does not know whether you
   are signed in.
2. **No model of "why is nothing happening".** Search without sources is silent;
   failing channels, senders and sources show at best a date.
3. **No form discipline.** Constraints live in the server; the client shows "check the
   fields".

The design below fixes those three things first and uses them everywhere. The guiding
principle is the one the owner already likes in the palette: **type, see, act, done**, with
the app doing the bookkeeping. Everything that cannot be done in that rhythm (setup, keys,
channels) is moved into a settings area with the same discipline, so the daily surface stays
small.

What the research adds (details and citations in `research.md`):

- People keep a tracker when logging is fast enough to do in passing; the moment it takes
  ten steps or a phone detour, the habit dies [R-05, R-06, R-12, R-13]. The design makes
  the primary action of every item one click away and prefills everything.
- The two things people fight with in every tracker are the status model (exclusive status
  vs. many shelves/tags) and the lack of a private note [R-16, R-18, R-19]. Status, tags,
  lists and notes are therefore four distinct, visible concepts.
- Navigation studies of the biggest book tracker show a convoluted IA, a landing on an empty
  library and redundant sections as the main failures [R-24, R-25]. The design uses five top
  level sections, a "Now" home, and empty states that always carry the next action.
- There are no public heatmaps or analytics of any tracker app [R-54]; the design therefore
  recommends instrumenting the logging funnel ourselves (section 9, phase 4).

---

## 2. Brand

### 2.1 Positioning

A private, calm place to keep what you watch, read, listen to and play, together with the
people you do it with. It is not a social network, not a recommendation engine and not a
media server. It runs where you decide, under your name.

One-line positioning for operators (used in README and the brand guide, not in the UI):
"A self-hosted, white-label media tracker for households, clubs and small communities."

What it is **not**, because these decide many details: not gamified (no streaks, no
"behind schedule" [R-04]), not loud (no confetti, no mascots), not opinionated about taste
(ratings optional, "no rating" is a first-class state [R-02]).

### 2.2 Personality

Three words, used as a test for every screen and sentence:

- **Quiet.** The interface recedes; covers and titles are the colour. Neutral greys, one
  accent at most, no decoration.
- **Dependable.** Things you logged stay logged, undo is everywhere, and the app says what
  it is doing when something takes time or fails.
- **Companionable.** People are first-class: "with Anna", "Anna won", "shared with you".
  Social features are explicit and consent-based, never automatic.

### 2.3 Voice and tone

- Second person, present tense, plain words. "Watched S2 E5" not "Episode marked as
  consumed".
- Sentences, not labels, wherever a human would say a sentence: history rows, activity feed,
  empty states, errors.
- Say what happened, what it means, what to do next. In that order. Errors never blame and
  never joke [R-38].
- Numbers are facts, not scores. "38 movies this year, 6 more than last year at this time";
  never "You are 4 books behind".
- Admin-only text is marked as such in its own sentence ("As an admin you can...").
- German: "du", never "Sie"; this is a household product. Brand operators may override any
  string, including switching to "Sie", through `[strings.de]`.

Words we use and avoid (English / German):

| Use | Avoid |
|---|---|
| Watched, read, listened, played / gesehen, gelesen, gehoert, gespielt | consumed, completed (as a verb) |
| Log, logged / eintragen, eingetragen | submit, record (noun ok) |
| People / Personen (UI), companion only in docs and API | contacts, friends |
| Planned, In progress, Completed, Paused, Dropped / Geplant, Begonnen, Abgeschlossen, Pausiert, Abgebrochen | backlog, consumed, finished, Done (status; owner ruling DEC-171) |
| Data source / Datenquelle | provider, connector (code only) |
| Sender / Sender (integrations) | integration token, hook |
| Something went wrong + what to do | Oops, error 500, invalid |

### 2.4 Microcopy for key moments (English and German)

The German text may use umlauts (locale files are exempt from the ASCII rule); it avoids the
sharp s to stay safe in any file.

**Empty library (new user)**

- EN title: "Nothing here yet" / body: "Add the first thing you are watching, reading or
  playing. Press Ctrl+K, type a title and press Enter." / buttons: "Find something",
  "Import a file".
- DE: "Noch nichts hier" / "Trag das Erste ein, das du gerade schaust, liest oder spielst.
  Drück Strg+K, tipp einen Titel und drück Enter." / "Etwas finden", "Datei importieren".

**No data sources (admin, palette footer and home nudge)**

- EN: "Search is not set up yet. This instance has no data sources, so nothing can be found
  online. You can still add items by hand." / "Set up data sources".
- DE: "Die Suche ist noch nicht eingerichtet. Diese Instanz hat keine Datenquellen, darum
  kann online nichts gefunden werden. Du kannst Einträge trotzdem von Hand anlegen." /
  "Datenquellen einrichten".

**No data sources (member)**

- EN: "Search is not set up yet. Ask your admin to add data sources. You can still add items
  by hand." / "Ask admin" (sends an admin notification).
- DE: "Die Suche ist noch nicht eingerichtet. Bitte deinen Admin, Datenquellen
  hinzuzufügen. Du kannst Einträge trotzdem von Hand anlegen." / "Admin fragen".

**One kind has no source**

- EN: "Board games were not searched: no source is set up." (footer line, admin link)
- DE: "Brettspiele wurden nicht durchsucht: keine Quelle eingerichtet."

**Search without results**

- EN: "Nothing matches 'harbuor masters'. Searched your library, TMDB and Open Library." /
  "Try 'harbour masters'", "Add 'harbuor masters' by hand".
- DE: "Nichts passt zu 'harbuor masters'. Durchsucht: deine Bibliothek, TMDB und Open
  Library." / "'harbour masters' versuchen", "'harbuor masters' von Hand anlegen".

**Search still running**

- EN: "Asking IGDB..." (live region, polite) / DE: "Frage IGDB..."

**Logging success (status toast, 10 s, with Undo)**

- EN: "Logged: Northern Lights Station S2 E5." / "Logged: Harbour Masters, Anna won." /
  "Saved page 212 of 410." / Undo.
- DE: "Eingetragen: Northern Lights Station S2 E5." / "Eingetragen: Harbour Masters, Anna
  hat gewonnen." / "Seite 212 von 410 gespeichert." / Rückgängig.

**Errors**

- Save failed, data kept (inline in the sheet): EN "Could not save. Your entry is still here;
  try again." / DE "Konnte nicht speichern. Dein Eintrag ist noch da; versuch es noch einmal."
- Source failing (palette footer): EN "TMDB is not answering (retrying in 4 min)." / DE
  "TMDB antwortet nicht (neuer Versuch in 4 Min.)."
- Offline (banner, role=status): EN "You are offline. Changes are kept and sent when you are
  back." / DE "Du bist offline. Änderungen werden gespeichert und später gesendet."
- Session expired: EN "Your session ended. Sign in again; nothing was lost." / DE "Deine
  Sitzung ist abgelaufen. Melde dich neu an; es ist nichts verloren."
- Whole service down (full page): EN "Sorry, something is wrong on this server. Your data is
  safe. Try again in a few minutes." / DE "Auf diesem Server stimmt gerade etwas nicht. Deine
  Daten sind sicher. Versuch es in ein paar Minuten noch einmal."

**Forms**

- Username hint (before input): EN "Used in links and for sign-in. 3 to 32 characters." /
  DE "Für Links und zum Anmelden. 3 bis 32 Zeichen."
- Username live line: EN "Will be saved as robin (capitals become lowercase)" / DE "Wird als
  robin gespeichert (Grossbuchstaben werden klein)".
- Username with a space: EN "'anna k' has a space. Try 'anna-k' or 'anna_k'." / DE "'anna k'
  enthält ein Leerzeichen. Versuch 'anna-k' oder 'anna_k'."
- Username taken: EN "'anna' is already taken. Free: anna2, anna.k." / DE "'anna' ist schon
  vergeben. Frei: anna2, anna.k."
- Password hint: EN "At least 10 characters. No other rules; a long phrase is best." / DE
  "Mindestens 10 Zeichen. Keine weiteren Regeln; ein langer Satz ist am besten."
- Error summary heading: EN "There are 2 problems" / DE "Es gibt 2 Probleme".

### 2.5 Naming guidance for the white-label default

- The default `name` in `config/brand/default/brand.toml` is "Hordwyn" (since 2026-11-12;
  before that a codename); nothing in the UI depends on its meaning. The docs and the
  mockups use
  "[Brand]".
- Rules for an operator's name (brand guide): one or two words, readable at 14 px in the
  wordmark slot, works as a cookie prefix (`short_name` reduced to `[a-z0-9]`), does not
  contain a third party's name (no "Plex tracker", no "TMDB diary"; the data sources' terms
  forbid implying endorsement).
- Nouns inside the UI are product-neutral and stay stable across brands: Library, History,
  Lists, People, Notifications, Integrations, Data sources, Settings. Operators may override
  any of them via `[strings.<locale>]`, but the keys stay.
- Page titles are `"{page} - {name}"`; the home page is just `{name}`.

### 2.6 Logo direction

No drawn or generated mark (KICKOFF rule 11). The default logo is:

- **Wordmark:** the brand name set in the UI font at weight 650, letter-spacing -0.01em.
- **Mark:** a 28 px rounded square (radius 0.45rem) filled with `--primary`, containing the
  Lucide `library` icon in `--primary-foreground` at 16 px. The square is a CSS element,
  not an asset; it is the same shape as poster corners, so the mark reads as "a shelf".
- **Favicon / PWA icon:** the same Lucide `library` glyph on a `--primary` square, exported
  by the brand build helper from the vendored SVG (no new artwork). Operators replace it with
  their own file in `[logos]`.
- Dark mode flips `--primary`, so the mark stays black-on-white in light and white-on-black
  in dark without a second file.
- Alternative glyphs from the vendored set if an operator wants a different default feel:
  `bookmark` (saving), `sparkles` (anime-leaning instances), `meeple` (Tabler, board game
  clubs). They are all already licensed and listed in `config/brand/ICONS.adoc`.

### 2.7 Colour system (design tokens)

The existing CSS custom properties in `src/web/style/input.css` stay the contract; this
section fixes their values and adds four semantic tokens. Values are Tailwind v4 palette
names with their sRGB hex (computed from the oklch definitions in `tailwindcss@4.1.0/theme.css`,
https://unpkg.com/tailwindcss@4.1.0/theme.css). Contrast ratios are WCAG 2.x relative-luminance
ratios computed from those hex values; AA means >= 4.5:1 for text, >= 3:1 for large text and
UI boundaries (WCAG 1.4.3 and 1.4.11, see research.md R-41).

**Light theme**

| Token | Value | Hex | Pair | Ratio | AA |
|---|---|---|---|---|---|
| `--background` | white | #ffffff | | | |
| `--foreground` | zinc-950 | #09090b | on background | 19.89 | pass |
| `--card`, `--popover` | white | #ffffff | | | |
| `--primary` | zinc-900 | #18181b | | | |
| `--primary-foreground` | zinc-50 | #fafafa | on primary | 16.98 | pass |
| `--secondary`, `--muted`, `--accent` | zinc-100 | #f4f4f5 | | | |
| `--secondary-foreground` | zinc-900 | #18181b | on secondary | 16.11 | pass |
| `--muted-foreground` | zinc-600 | #52525c | on background / on muted | 7.73 / 7.02 | pass |
| `--destructive` | red-700 | #c10007 | white on it / it on background | 6.42 / 6.42 | pass |
| `--border` | zinc-200 | #e4e4e7 | on background | 1.27 | decorative only |
| `--input` | **zinc-500** (was zinc-300) | #71717b | on background | 4.83 | pass (boundary >= 3) |
| `--ring` | zinc-500 | #71717b | on background | 4.83 | pass |
| `--success` (new) | emerald-700 | #007a55 | on background | 5.37 | pass |
| `--warning` (new) | amber-700 | #bb4d00 | on background | 5.05 | pass |
| `--info` (new) | sky-700 | #0069a8 | on background | 5.85 | pass |
| `--success-soft` etc. (new) | emerald-50-like tints | see mockup CSS | semantic text on soft | >= 4.5 by construction (text colour unchanged, background near white) | pass |

Note on `--input`: the current zinc-300 (#d4d4d8) gives 1.48:1 on white, which fails
WCAG 1.4.11 for a control whose only visual identification is its border. zinc-500 passes. Decided (owner review 2026-11-06, DEC-171): field borders are zinc-500.
If the owner finds 1 px zinc-500 too heavy, the alternative that still passes is a 1 px
zinc-400 border (2.63, fails alone) **plus** a `--muted` filled background, so the field is
identified by its fill; the mockups use the zinc-500 border.

**Dark theme**

| Token | Value | Hex | Pair | Ratio | AA |
|---|---|---|---|---|---|
| `--background` | zinc-950 | #09090b | | | |
| `--foreground` | zinc-50 | #fafafa | on background | 19.05 | pass |
| `--card`, `--popover` | zinc-900 | #18181b | | | |
| `--primary` | zinc-50 | #fafafa | | | |
| `--primary-foreground` | zinc-900 | #18181b | on primary | 16.98 | pass |
| `--secondary`, `--muted`, `--accent` | zinc-800 | #27272a | | | |
| `--secondary-foreground` | zinc-50 | #fafafa | on secondary | 14.26 | pass |
| `--muted-foreground` | zinc-400 | #9f9fa9 | on background / on card / on muted | 7.56 / 6.74 / 5.66 | pass |
| `--destructive` (fill) | red-700 | #c10007 | white on it | 6.42 | pass |
| `--destructive-text` (new) | red-400 | #ff6467 | on background / on card | 6.88 / 6.13 | pass |
| `--border` | zinc-800 | #27272a | on background | 1.34 | decorative only |
| `--input` | **zinc-500** (was zinc-700) | #71717b | on background / on card | 4.12 / 3.67 | pass (boundary) |
| `--ring` | zinc-400 | #9f9fa9 | on background | 7.56 | pass |
| `--success` | emerald-400 | #00d492 | on background / on card | 10.29 / 9.17 | pass |
| `--warning` | amber-400 | #ffb900 | on background / on card | 11.58 / 10.33 | pass |
| `--info` | sky-400 | #00bcff | on background / on card | 9.11 / 8.12 | pass |

Rules:

- Semantic colours carry meaning only together with an icon or a word (status dots always
  have a text label next to them). Colour alone never encodes state (WCAG 1.4.1).
- `--primary` is the brand's one accent. The stylesheet stays neutral (the tables above are
  its fallbacks); since the owner test (OT-13, DEC-540, DEC-580) the default brand file
  `config/brand/default/brand.toml` sets a violet accent: light `primary = violet-700
  (#7008e7)`, `primary-foreground = white` (7.30), `primary-soft = violet-50 (#f5f3ff)`;
  dark `primary = violet-400 (#a684ff)`, `primary-foreground = zinc-950` (6.98),
  `primary-soft = violet-950 (#2f0d68)`. The accent colours filled buttons, pressed chips,
  progress and chart bars, the text of the selected segment, tab and navigation item, and
  text links on the page and on cards (underlined; links on alerts and soft surfaces keep
  the text colour, DEC-581, DEC-584). `primary-soft` is the surface of the current
  navigation item, the "next up" row and the hero tile. The current tab of the mobile tab
  bar also has a 2 px bar in the accent above it, so it is not told apart by hue and weight
  alone.
- One tint per activity group, each with a soft surface (DEC-540, DEC-580), light / dark:
  watch fuchsia-700 #a800b7 / fuchsia-400 #ed6aff on #fdf4ff / #4b004f; read orange-700
  #ca3500 / orange-400 #ff8904 on #fff7ed / #441306; listen teal-700 #00786f / teal-400
  #00d5be on #f0fdfa / #022f2e; play blue-700 #1447e6 / blue-400 #51a2ff on #eff6ff /
  #162456. A tint colours the kind icon, a marker dot (from 30 rem up) and the chart bars
  of its group; the soft surface sits under normal text (time tiles, kind badge). Without
  brand values the tokens fall back to grey (`group-*` = `muted-foreground`, `group-*-soft`
  = `muted`, `primary-soft` = `secondary`). A tint never carries the meaning alone: the
  group's icon or name is always next to it (WCAG 1.4.1).
- The brand crate checks the pairs the pages rely on at start-up and logs a warning per
  pair below 4.5:1 for text or 3:1 for bars and markers (it does not stop the server); the
  list of pairs and the ratios of the default brand are in DEC-580 and DEC-586, the guide
  for operators in `docs/modules/ROOT/pages/white-labelling.adoc` ("Colours").
- No colour is written down in component code: classes name tokens, and a repository test
  fails on a palette class, hex value, colour function or SVG / inline style colour in
  `crates/ui`, `crates/web` and `apps/server` (DEC-583, DEC-585).
- Covers, the accent and the group tints are the only saturated surfaces. Poster
  placeholders use the item's dominant colour (already stored, DEC-094) so empty grids are
  not grey.
- Dark is not an inverted light theme: cards are one step lighter than the page
  (zinc-900 on zinc-950), shadows are replaced by borders, and elevation is expressed by
  surface lightness.

### 2.8 Typography

Default: the default brand ships **Atkinson Hyperlegible Next** (SIL OFL 1.1; files from
https://github.com/googlefonts/atkinson-hyperlegible-next, not the Braille Institute
download, which has its own EULA), **self-hosted and never loaded from a CDN**. The licence
is recorded on the open-source attribution page (see 2.11). Decided (owner review
2026-11-06, DEC-171). The system font stack stays as the fallback at the end of the
`font-family` list, so text renders before the font loads and if a file is missing. The
stack is Tailwind's `--font-sans`:
`ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"` (Tailwind v4 theme.css, URL above). The mockups use the
same with explicit fallbacks (`-apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial`).
The resulting list is `"Atkinson Hyperlegible Next", ui-sans-serif, system-ui, sans-serif, ...`.

An operator who wants a different web font may use any of these, verified SIL OFL 1.1 and self-hostable
(research.md R-53):

- Inter, https://github.com/rsms/inter/blob/master/LICENSE.txt (OFL 1.1)
- Source Sans 3, https://github.com/adobe-fonts/source-sans/blob/release/LICENSE.md (OFL 1.1,
  reserved font name "Source")
- IBM Plex Sans, https://github.com/IBM/plex/blob/master/LICENSE.txt (OFL 1.1, reserved
  name "Plex")
- Geist, https://github.com/vercel/geist-font (OFL 1.1)
- Atkinson Hyperlegible Next from https://github.com/googlefonts/atkinson-hyperlegible-next
  (OFL 1.1 there; the default brand font, see above)

The brand config gets `[fonts] sans = "Atkinson Hyperlegible Next"` (default brand) with files inside the brand
directory; the shell emits `@font-face` with `font-display: swap`. Never load from a CDN.

Scale (rem, line-height), tuned for dense library pages:

| Role | Size | Weight | Notes |
|---|---|---|---|
| Display (home greeting) | 1.5 / 1.2 | 650 | letter-spacing -0.02em |
| H1 | 1.5 / 1.2 | 650 | one per page |
| H2 | 1.05 / 1.3 | 600 | section heads |
| H3 | 0.95 / 1.3 | 600 | card titles |
| Body | 0.9-1.0 / 1.5 | 400 | 1.0 in forms and long text, 0.9 in dense lists |
| Small | 0.8 / 1.4 | 400-500 | meta lines, hints |
| Caption | 0.72 / 1.4 | 600 | badges, uppercase section labels with 0.06em tracking |
| Mono | 0.8 | 400 | tokens, URLs, `ui-monospace` stack |

Body text never goes below 0.8 rem (12.8 px); in dark mode nothing below 0.85 rem for
paragraphs (NN/g dark mode guidance, R-42). Titles clamp to two lines in cards.

### 2.9 Spacing, radius, elevation, motion

- **Spacing:** 4 px base (Tailwind scale). Page gutter 1 rem (16 px) at every width. Content
  max-width (owner test OT-14, DEC-541, DEC-582): 96 rem (1536 px) for grid pages (Home,
  Library, Lists, People, Stats; their grids get more columns, the content uses the whole
  column) and for the top bar on every page; 80 rem (1280 px) for record pages (an item,
  History, one list, one person, You, settings, admin, signed-out pages); 36-52 rem for
  forms and wizards inside either. Card padding 1 rem; dense lists 0.65 rem vertical.
  Section gap 1.75 rem.
- **Radius:** `--radius: 0.5rem` stays. Buttons, inputs, chips (pill), cards (`radius + 4px`),
  dialogs (`radius + 6px`), posters (`radius`), avatars (full).
- **Elevation:** three levels. 1: cards (1 px border + `0 1px 2px` 6% shadow). 2: popovers,
  dropdowns, quick-action menus (`0 4px 12px` 10%). 3: dialogs and sheets (`0 16px 40px`
  18%). In dark mode shadows go to 50-60% black and borders carry the edge. Sticky top bar
  uses a translucent background with blur, no shadow.
- **Motion:** 150 ms ease-out for hover/colour, 200 ms for sheets and dialogs (fade +
  8 px translate), 250 ms for list reorder. No bounce, no scale on hover (posters lift by
  shadow only). Skeletons pulse at 1.5 s; under `prefers-reduced-motion: reduce` every
  transform becomes a fade and the pulse stops (WCAG 2.3.3 technique, R-41).
- **Live updates:** when `entity.updated` refetches a list, new rows fade in; nothing jumps.

### 2.10 Iconography

- Lucide (ISC) for everything, Tabler (MIT) only for glyphs Lucide lacks (`meeple`), as
  decided in DEC-043 and listed in `config/brand/ICONS.adoc`. No other sets; no inline
  hand-drawn paths.
- Sizes: 16 px in text and chips, 20 px in buttons and nav, 28 px in card heads, 40 px in
  empty states. Stroke width 2 everywhere (the vendored default).
- Every icon-only control has `aria-label`; decorative icons are `aria-hidden`.
- Kind icons are fixed (`icons.toml` already maps them) and appear in: kind chips, badges on
  item pages, history rows, palette groups. They never replace the kind's name in navigation.
- Additional icons to vendor (all Lucide, same licence): `tag`, `tags`, `filter`,
  `arrow-up-down` (sort), `lock`, `copy`, `clock`, `repeat` (rewatch), `check-check`
  (mark all), `inbox`, `history`, `shield` (admin), `key-round`, `plug` (integrations),
  `database` (sources), `wifi-off` (offline), `circle-alert`, `chevrons-up-down`, `hash`.

### 2.11 Required attribution and open-source attribution page

- **TMDB logo:** the attribution logo the TMDB terms require is committed unmodified from
  TMDB's official source, with the terms noted next to the file (R-F3, DEC-170; supersedes
  the "operator supplies it" idea). It is shown in the attribution footer and on the data
  sources page wherever TMDB data appears.
- **Open-source attribution page** (linked from the footer and Settings): the Rust crates
  and their licences, the font (Atkinson Hyperlegible Next, OFL 1.1 text), the icon
  libraries, and the data sources with the attribution each one requires. Generated from
  the dependency metadata where possible so it cannot drift.

---

## 3. Information architecture

### 3.1 The five concepts a user must learn

Everything in the product is one of these; the UI never mixes them in one control
[R-16, R-18, R-19, R-21, R-22]:

| Concept | What it is | Cardinality | Where it is edited |
|---|---|---|---|
| **Kind** | What something is (movie, book, board game...). Fixed by the kinds crate, grouped into four activity groups. | exactly one per item, never changes | set when added |
| **Status** | Where you are with it: Planned, In progress, Completed, Paused, Dropped, plus kind-specific ones (Endless for games; ownership flags for board games are separate, see below). | exactly one per library entry | status menu, derived from logging |
| **Progress and history** | Append-only events: watched S2E5, page 212, a play with players. Statuses are derived from them where possible. | many per entry | log sheet, item page, webhooks, imports |
| **Tags** | Your own flat labels ("cosy", "with Anna", "lent out"). | many per entry | item page, bulk edit |
| **Lists** | Ordered, named, shareable collections. | many per entry | Lists page, palette |

Plus two per-entry fields that research says people miss most: a **private note** on every
entry regardless of status [R-16], and **rating** that can be absent [R-02].

Board game ownership (owned, previously owned, wishlist with priority, want to play, for
trade, preordered) is a separate set of flags on the entry, not a status [R-21]. "Owned"
appears as a chip on the item page and as a filter.

"Categories" in the owner's complaint means two different things, and the design gives each
its own control: the **activity group** (Watch / Read / Listen / Play, a fixed grouping of
kinds) and **tags** (free-form, user-owned). There are no "shelves" as a separate concept;
a saved filter ("view") can be added in phase 4 if tags and lists turn out not to cover it.

Kind groups (from the kinds crate's own module split `screen`, `read`, `listen`, `play`):

| Group | Kinds | Progress models |
|---|---|---|
| Watch | movie, show, anime | Single, Episodic |
| Read | book, manga, comic | Paged |
| Listen | audiobook, podcast, music | Timed, Episodic |
| Play | video game, visual novel, board game | Sessions |

A runtime kind (WASM, later) declares its group in the descriptor; unknown group falls into
"Other", which only appears when non-empty.

### 3.2 Navigation model

**Desktop (>= 760 px): a top bar with five sections, search, add, notifications, account.**

```
[mark Brand]  Home  Library  History  Lists  People   [ Search or add...  Ctrl K ]  [+]  [bell]  [avatar]
```

- Five sections is the upper bound that navigation research supports for a bar [R-26, R-29].
  Settings, admin, data sources, integrations, imports and sign-out live in the avatar menu
  and in the settings area; they are not daily surfaces.
- The search pill is a real control in the bar on every page (NN/g: visible search box,
  R-31) and opens the palette; "+" opens the same palette in add mode (focus in the input,
  scope "All").
- The bar knows the session server-side: the server renders either the signed-in bar or the
  "Sign in / Create account" pair, from `ClientConfig` (add `signed_in: bool`,
  `display_name`, `is_admin`, `unread: u32`). No Suspense gap, no flash, and no "Create
  account" after sign-in (owner complaint 1). After sign-in the server function returns and
  the client does a full navigation, so the next render is correct by construction (the code
  already does `location.assign`).
- Current section is marked with `aria-current="page"` and a filled background.
- The account menu (`DropdownMenu`): display name and username, Settings, Admin (if admin),
  Theme (light/dark/system), Sign out.

**Mobile (< 760 px): bottom tab bar with five destinations; top bar reduced to mark,
search and bell.**

```
Home   Library   [+]   History   You
```

- "You" is a hub page: People, Lists, Notifications, Settings, Sign out. The hub pattern
  keeps the bar at five (decided, owner review 2026-11-06, DEC-171) and avoids the "More" overflow that hides content [R-29].
- "+" opens the palette full screen in add mode.
- Tab bar is 72 px tall including the home indicator inset; targets are at least 48 px
  [R-40]. `scroll-padding-bottom` equals its height so focused elements are never hidden
  behind it (WCAG 2.4.11, R-41).
- No hamburger menu anywhere [R-29].

**Keyboard map (global):** `Ctrl/Cmd+K` palette, `N` new (palette in add mode), `G then H/L/Y/T/P`
go to Home/Library/History/Lists/People, `?` shortcut help, `Esc` closes any layer and returns
focus. All shortcuts are single keys only when focus is not in a text field.

### 3.3 Section map

```
Home (Now)                     Library                         History
  continue (in progress)         group: All/Watch/Read/Listen/Play   stream by day
  up next (planned)              kind chips with counts          filters: group, person, year
  recent activity                status tabs                     shared with me (filter)
  all-time stats               filters: tags, facets, rating,  stats panel (year, compare)
  admin nudge (sources)            owned                         all stats page
                                 sort, view (grid/list)
                                 bulk select (list view)
                                 item page
                                   action bar (per progress model)
                                   rating, tags, lists, note
                                   tabs: episodes|chapters|plays|tracks, about, history, notes, related
                                   watched with, attribution

Lists                          People                          Settings (you)
  mine / shared with me /        cards, requests, invite,         account (profile, password, TOTP, OIDC)
  on this instance               link, merge, groups              notifications (inbox, channels, rules, log)
  list page (order, share)       person page (history with X)     integrations (senders)
  public shared page                                              data sources (per-user credentials)
                                                                  imports and export
                                                                  privacy (share sessions, DNF visibility)
Admin (instance)
  overview (health: db, search, storage, jobs)
  data sources (keys, status, licence mode)
  users and sign-up (mode, invites, roles)
  notifications defaults (instance rules, locks)
  storage report
  branding (read-only view of the brand config)
```

### 3.4 Statuses, derived states and the "next step"

- Core statuses stay as in `domain::kind::Status::CORE`. The UI shows them as Planned,
  In progress, Completed, Paused, Dropped (en) / Geplant, Begonnen, Abgeschlossen,
  Pausiert, Abgebrochen (de). Decided (owner review 2026-11-06, DEC-171); the key
  `status-completed` stays.
- Logging drives status: the first progress event moves Planned -> In progress; the finishing
  event moves -> Completed; Dropped records progress-at-drop and is private by default [R-20].
  A user can always set a status by hand, and the app never blocks a user state on provider
  metadata (for example a show that is still airing can be Completed) [R-23].
- "Up to date" (every aired episode watched, show still running) is a derived badge on
  In progress entries, not a status [R-28].
- Every entry has a computed **next step** used by cards, the home page and `Ctrl+Enter`
  in the palette:

| Progress model | Next step | Label |
|---|---|---|
| Single | finished | "Watched" (or "Watched again" when Completed) |
| Episodic | next unwatched episode | "Watched S2 E5" |
| Paged | update page/chapter | "Update page" (+10 / +25 / finished) |
| Timed | update position | "Update position" |
| Sessions | new session | "Log a play" / "Log a session" |

- Rewatch/reread: when an item is Completed and the user logs again, the sheet pre-ticks
  "rewatch", history keeps every play, counts show distinct titles and total plays
  separately [R-10].
- "Mark this and all previous", "Mark season", "Mark show" are visible menu items on the
  episode list, with a per-show "do not ask again" preference [R-07].
- Dates: default "now"; "Yesterday" and "Pick a date" are one tap; "date unknown" is a
  legal value that still counts in totals [R-08, R-09].

### 3.5 Filters, sort, views

- Filters are chips and dropdowns above the grid; active ones become removable badges; the
  full state is in the URL (`/library?group=watch&kind=show&status=in_progress&tag=cosy`).
- Default sort is "Last activity" (diverse, shows breadth) [R-30]; options: Title, Added,
  Year, Your rating, Progress, Next release, Plays (board games), Priority (user field).
- Grid for browsing, list/table for comparing and bulk editing [R-30, R-15]. The list view
  has checkboxes and a bulk bar: set status, add tag, add to list, mark owned, remove.
- Facets come from the kind descriptor's `facets()` (genres, platforms, players...), so a new
  kind gets filters without UI changes.

### 3.6 History, companions, sessions, sharing

Routes are `/history` and `/people`. The product is greenfield, so there are no redirects
from `/sessions` or `/companions` (DEC-171).


- History is one stream for all kinds, grouped by day, written as sentences, with the group
  switcher and a person filter [R-28]. Shared sessions from linked users are rows in the same
  stream with "Copy to my log".
- People (companions) are cards; the self-companion is implicit. Link request, invite
  (promote), unlink, merge are actions on the card. A person page is History filtered to that
  person plus stats (together, plays, win rate, most played game).
- Sharing a list is a visibility choice (private / everyone here / anyone with the link) with
  its consequence stated under each option. Making it private again is the same dialog.
- The public shared list page has the brand header, no ratings or notes, and a "Sign in to
  add these to your library" call to action.

### 3.7 Settings and admin

- Settings is one area with a left side nav (desktop) / stacked list (mobile), split into
  "You" and "Instance (admin)". Admin items are hidden for members.
- Every settings page uses the same anatomy: H1, one-sentence lead, cards or a table, inline
  forms, inline status. No page is a bare list of inputs.
- Admin overview shows health of the parts users cannot see: database, search index
  (Meilisearch or Postgres fallback), storage budget, job backlog, data sources, mail. Each
  row: status dot + word + last check + one action.

---

## 4. Deployment modes and the credential model

### 4.1 Two modes, one switch

The owner needs the same binary to serve (a) paying users on his server and (b) a family at
home. The research says the credential model cannot be one global policy, because the
providers differ [R-43 to R-49]:

| Source | Who the credential belongs to | Shared budget? | Commercial use |
|---|---|---|---|
| TMDB | application (instance) | limits are per IP, so per-user keys would not help | needs a written agreement; attribution and logo required; cache <= 6 months |
| IGDB (Twitch) | application (client id + secret, server-side only) | 4 req/s per client id | non-commercial only without a partnership |
| Podcast Index | application; key and secret must stay server-side | unpublished | allowed, but no public proxying of their API |
| TheTVDB v4 | project key, plus a **per-user subscriber PIN** in the user-supported model | per key | negotiated licence or per-user subscriptions |
| Open Library | none; identify with User-Agent and contact email | per IP (1 req/s unidentified, 3 identified) | n/a |
| MusicBrainz | none; User-Agent; 1 req/s per IP | per IP | non-commercial free, commercial plans exist |
| Hardcover | **per user** personal token, must not be pasted into third-party apps; OAuth intended | per token | user data only on the user's behalf |
| BoardGameGeek | application bearer token (policy page not reachable from the build host, DEC-107) | unknown | unknown |

So the model is: **instance credentials by default, per-user credentials only where the
provider requires them, and never both for the same source.** A per-user credential is a
different thing from an "override": it is the provider's own identity model (TheTVDB PIN,
Hardcover account), so it is shown to members only for those sources.

`config` gets one new setting and the admin UI edits it:

```toml
[instance]
hosting = "home"   # default; or "hosted". The wizard asks (DEC-171)
```

It changes defaults, not features:

| | home | hosted |
|---|---|---|
| sign-up default | invite (first account is admin, as now) | invite; "closed" suggested after onboarding |
| per-user credential sections | hidden unless a configured source needs them | shown for TheTVDB PIN and Hardcover |
| licence reminders | off | on: the admin records per source whether an agreement is on file; the data-sources page shows a warning until recorded |
| attribution footer | as required by terms | as required by terms, plus the TMDB logo (committed, 2.11) |
| admin "ask admin" notifications | on | on |
| rate-limit protection | shared instance budget; queue priorities already in place | same, plus per-user fair-share on `interactive` lane (phase 4) |

### 4.2 Where keys live

1. **Environment** (`APP__CONNECTORS__<ID>__API_KEY`): keeps working, wins over the database,
   and the UI shows the key as "set by environment" (read-only). This is the headless and
   compose path.
2. **Database, encrypted** (new `connector_credentials` table: connector id, scope
   `instance` or `user`, user id nullable, ciphertext, created by, created at, last tested
   at, last result): set through the admin UI or the wizard. Encrypted with a key from the
   environment (`APP__SECRETS__KEY`), following the pattern already used for channel secrets
   (DEC-140: stored, never returned, kept when blank).
3. **Per-user** rows use the same table with `scope = user`. Members edit them under
   Settings > Data sources; they are never shown to admins.

The governed client reads credentials per request from a small cache; changing a key in the
UI takes effect without restart. Connectors registered "only when a key is configured"
(DEC-110) become registered always, with a `Credential` state: `missing`, `env`, `db`,
`invalid` (last 401/403). Search skips `missing` and `invalid` connectors and reports them in
the palette footer instead of silently returning nothing (owner complaint 3).

### 4.3 First-run experience per mode

- **Home, compose:** `docker compose up`, open the URL, the setup wizard appears (no users
  exist). Step 1 admin account; step 2 "How you host" (two cards: Home / Hosted, with the
  defaults listed); step 3 data sources (cards per compiled-in source, Test per card, all
  skippable); step 4 done (checklist: add your first item, invite your household, connect
  your media server). Keys given in the environment show as already set in step 3.
- **Home, headless:** all keys in `.env`; the wizard still asks for the admin account and
  hosting mode (two screens), then shows step 3 as a read-only summary.
- **Hosted:** same wizard, Hosted selected; step 3 adds the licence checkbox per source
  ("Agreement on file") and a reminder that TMDB/IGDB/MusicBrainz free tiers exclude paid
  services. After the wizard the admin lands on Admin > Users to set sign-up to closed or
  invite and to create invites.
- The wizard is reachable only while no admin exists or `setup_completed_at` is null, like
  Gitea's install lock and Rocket.Chat's run-once wizard [R-50]; afterwards everything is
  under Admin.

### 4.4 How the UI explains missing or failing sources

Status per source is computed from the governed client and the credential state and shown
in three places with the same vocabulary:

| State | Admin sees (Data sources, palette footer) | Member sees |
|---|---|---|
| Working | green dot "Working", calls in last 24 h | nothing special |
| Not set up | grey dot "Not set up" + "Set up" | "Board games: no source. Ask admin" (footer); home page has no banner for members |
| Needs your account | n/a | "Connect your Hardcover account" / "Add your TheTVDB PIN" |
| Key rejected | red "Key rejected (401)" + link to the card | "temporarily unavailable" (no detail) |
| Rate limited | amber "Rate limited (429 x212 today)" | "slow right now" |
| Circuit open | red "Not answering, retry in 4 min" | "not answering, retrying" |
| Disabled by config | grey, no action | hidden |

Rules: a search never returns an empty list without a reason line; the admin nudge on Home
appears only while a whole kind group has no working source and can be snoozed for 7 days;
members get an "Ask admin" button that emits the existing admin notification path (new event
`admin.source_requested`, audience admin, dedupe per source per day).

---

## 5. Key flows

Format per flow: entry points; steps (fewest-clicks path first); states (empty, loading,
error, offline). "Sheet" means a dialog on desktop and a bottom sheet on mobile, same
component. Mockups: `mockups/<name>.html`.

### 5.1 First run: admin setup wizard

Entry: any URL while no user exists (server redirects to `/setup`); `/setup` returns 404 once
`setup_completed_at` is set. Mockup: `sources.html` (top half).

1. **Admin account.** Username, display name (optional), password, email (optional). Inline
   validation as in 5.2. Creates the admin and signs in. (1 screen, 3 fields.)
2. **How you host.** Two selectable cards: "Home: free, for family and friends" / "Hosted:
   you run this for other people". Each lists its defaults. Continue. (1 click.)
3. **Data sources.** One card per compiled-in source: covers which kinds, one line of terms,
   credential fields, Test, "how to get a key" in three numbered lines. Keys from the
   environment are shown as set. Everything is skippable; nothing blocks Continue. Hosted
   mode adds "Agreement on file" per source. (0 to n fields.)
4. **Done.** Checklist (GOV.UK task-list style, R-34): Add your first item (opens palette),
   Invite your household (opens People > Add), Connect your media server (Integrations),
   Set notification defaults (Admin). Each row is a link; completed rows turn green on
   later visits. "Go to Home".

States: a Test shows a spinner in the button and reports time plus one sample result
inline; errors are specific (401 "key rejected", network "could not reach api.example",
429). Leaving the wizard keeps completed steps (resumable, NN/g wizard guidance, R-34). If the
database is unreachable the server already retries (DEC-167); the browser sees the generic
"problem with the service" page.

Fewest clicks (home, keys in env): 3 fields + Continue, Home card + Continue, Continue,
Go to Home = 5 clicks.

### 5.2 Sign-up and sign-in with inline validation

Entry: top bar "Create account" (signed-out), `/invite/<token>`, "Sign in" link. Mockup:
`signup.html`.

Sign-up:

1. Hints under each field state the rule before typing (username 3-32, a-z 0-9 _ . -;
   password at least 10 characters, no other rules).
2. Username: while typing, a live line shows the normalised value ("Will be saved as
   robin") so capitals never fail; on blur (or 500 ms pause) the client checks the pattern
   and asks the server for availability; a taken name suggests three free variants.
3. Password: Show toggle, paste allowed, live character count.
4. Submit with errors: error summary at the top ("There are 2 problems", links to fields),
   focus moves to it, inline messages repeat the same text (GOV.UK pattern, R-37). Fields keep
   their values. Server error codes map 1:1 to field messages (`invalid_username`,
   `invalid_display_name`, `weak_password`, `conflict`); the generic "check the fields" is
   removed.
5. Success: full navigation to Home (new user empty state).

Sign-in: username + password; TOTP appears as a second step with its own heading and a
6-digit field with `inputmode=numeric`; wrong credentials give one message under the form
without saying which part was wrong; lock-out shows the remaining minutes
(`too_many_attempts`, DEC-155). Closed sign-up renders an explanation page, not a rejected
submit.

Offline: the submit button is disabled with "You are offline" under it.

### 5.3 Add something

Entry: `Ctrl+K`, the search pill, "+" in the bar, the mobile "+" tab, "Find something" in
empty states, "Add" on the library page, "Add by hand" everywhere search can fail. Mockup:
`palette.html`.

Palette path (fewest clicks): `Ctrl+K`, type, `Enter` on a library hit opens it;
`Ctrl+Enter` on a provider hit adds it as Planned and keeps the palette open (toast
"Added to Planned", Undo). Mouse path: hover row, "Add". "Add as..." menu: In progress,
Completed (with date), to list X, with tag.

In-page path: Library > "Add" opens the same palette scoped to the current group. Item page
of a not-yet-added catalog item: "Add to library" button with the same menu.

By hand: the last palette row is always "Add '<query>' by hand": a short sheet with title,
kind, year; the item is a local catalog stub that can be matched to a source later ("Fetch
details" on the item page once a source exists).

States: fewer than 2 characters shows recent items and suggested commands; while connectors
run, the footer says "Asking IGDB..." (polite live region) and results stream in grouped by
source and kind; no results shows the no-results card with spelling suggestion, "add by
hand" and which sources were skipped; no sources at all shows the setup card (admin) or
"Ask admin" (member); offline shows library results only and a line "Offline: showing your
library".

### 5.4 Log progress

Entry: next-step button on home cards, library cards (hover quick action), the item page
action bar, `Ctrl+Enter` in the palette, webhook (automatic), import. Mockups: `item.html`,
`log.html`, `mobile.html`.

- **Episode (Episodic):** one click on "Watched S2 E5" logs now, alone; the button advances
  to S2 E6 in place (auto-advance, R-06, R-09); toast with Undo. "With..." opens the sheet
  (date chips, people toggles, rating, note, rewatch). Episode list: check circles per
  episode; row menu "Mark this and all previous", "Mark season", "Mark show"; first use of
  a bulk action asks "Also mark previous episodes?" with "Never ask for this show" [R-07].
  Backfill uses "On release dates" or "Spread over a date range" (R-09).
- **Pages (Paged):** inline page field with +10 / +25 / "Finished"; chapter or percent as
  alternative units (R-14); Enter saves; history row shows "pages 180 to 212".
- **Position (Timed):** `h:mm:ss` field with +15 min / Finished.
- **Play session with companions (Sessions, board game):** "Log a play" opens the sheet
  prefilled with now, last duration, last location and the last group that played this
  game (R-12). Add players by typing (creates a companion inline). Scores optional; winner
  derived from the highest score, overridable; co-op switch. "More" discloses expansions,
  scenario, notes, photo. Share switch defaults to the privacy setting. `Ctrl+Enter` saves.
  Saving with nothing changed logs "played now with the last group" (1 click + 1 click).
- **Video game session:** date, minutes, note, companions (co-op); "Completed" and
  "Endless" are status buttons next to it (R-13).
- **Watched with (Single):** "Watched" logs now; "Watched with..." opens the lighter sheet.

States: saving shows the button in a pending state (spinner, label kept); failure keeps the
sheet open with an inline error and the data; success closes and toasts (role=status, 10 s,
Undo); offline queues the command (client op id exists already) and shows "Saved offline,
will sync" in the toast. A refetch from `entity.updated` never resets an open sheet (the
code already keeps action state outside the view).

### 5.5 Rate and review

Entry: item page row, the log sheet, the palette "Add as... Completed with rating". Rating
control: 5 stars with half steps on pointer (stored 0-100, DEC-018), keyboard arrows move in
half steps, `0` clears; "no rating" is shown as hollow stars with "Not rated" text. Review:
the Notes tab has a private note field (always) and a "Publish as review" toggle that makes
it visible on shared lists and to linked people, with a spoiler checkbox. Per-episode notes
exist in the episode row menu (R-10/A6).

### 5.6 Find something in a big library

Entry: Library page, palette with "in my library" scope (`/` focuses the library filter
box). Path: group switcher (1 click) > kind chip (1) > status tab (1) > type in the filter
box (matches title, original title, tags, note). Sort menu and facet dropdowns refine.
List view + bulk bar for cleanup. All state in the URL; back button works. 50k entries are
paged (DEC-150) with "Show more" and a count; the filter box queries the server, not the
page.

### 5.7 Set up a data source

Entry: wizard step 3, Admin > Data sources > row "Set up", home nudge, palette footer link.
Mockup: `sources.html`. Steps: open card, read the three "how to get a key" lines (link to
the provider's own developer page), paste the key, Test (runs a real search for a fixed
sample and shows the result and time), Save. A failed Test keeps the value and explains.
Per-user sources: Settings > Data sources > "Connect" (Hardcover: OAuth when available,
else token field with the provider's own warning) / "Add PIN" (TheTVDB).

### 5.8 Set up a notification channel

Entry: Settings > Notifications > Channels > "Add a channel"; the inbox empty state links
here. Mockup: `notifications.html`. Steps: pick the type (chips), fill the per-type fields
(constraints as hints, inline errors on blur, secrets as password fields "stored encrypted,
never shown again"), "Add and send a test". The test result appears inline on the new card
(green "Test delivered" or red with the channel's error). Then the Rules tab: switches per
event row; the first switch turned on for a new channel asks whether to apply the instance
defaults (one click). Signed webhook secrets are shown once with Copy (as now).

### 5.9 Connect a media server via the webhook

Entry: Settings > Integrations > "Connect a sender"; home checklist. Mockup:
`integrations.html`. Steps: 1 name it; 2 token and URL shown once with Copy, help beside it
with the body shape and the three plugin steps in generic words (no third-party code or docs
are used, DEC-149); 3 "Waiting for the first call..." listens on SSE and turns green on the
first accepted post, or shows the 401/422 reason with the offending field; "Send a test call
from here" posts a sample so the user can see the whole path work without the media server.
Sender cards afterwards show health and the last recorded item; refused calls get a
persistent inline explanation, never a toast (R-38).

### 5.10 Share a list

Entry: list card menu, list page "Share", item page "Add to list" > list > share. Mockup:
`lists.html`. The dialog is three radio options with consequences; choosing "Anyone with the
link" creates the link in place with Copy; re-sharing replaces the link (as now) and says so;
"Private" revokes. The public page shows title, owner display name, posters, and a sign-in
call to action; never ratings or notes.

### 5.11 Invite a companion

Entry: People > person card "Invite" (no account) / card menu "Link to account" (has an
account); the log sheet's "+ new person" creates the companion first. Mockup:
`companions.html`. Invite: a dialog states the consequence (they see the plays you logged
with them, nothing in your log changes), shows the single-use link (14 days, DEC-022) with
Copy. Link: a field for their username, "Send link request"; the other user sees a request
card at the top of their People page with Accept / Decline. Both outcomes notify through the
existing social events. Merge: suggested for near-duplicate names; one dialog, keep/fold,
undoable through the merge record (DEC-017).

---

## 6. Page-by-page specifications

Shared rules for every page: one H1; `<Title>` is `"{page} - {name}"`; main content max
96 rem on grid pages and 80 rem on record pages (2.9) with 1 rem gutters; loading uses skeletons shaped like the final layout (never a
page-wide spinner, R-39); errors from server functions render inline with a Retry button;
`unauthorized` redirects to sign-in with `?next=`; every list refetches on `entity.updated`
without resetting scroll or open sheets. Mobile rules per page are listed where they differ
from "stack vertically".

### 6.1 Home (`/`) - mockup `home.html`, `mobile.html`

- Signed out: brand headline, one sentence, "Sign in" and "Create account" (or "This
  instance is invite-only" when closed). No feature marketing.
- Signed in: greeting with display name and a one-line summary; admin nudge (only admins,
  only while a kind group has no working source, snoozable); **Continue** (up to 6 in-progress
  entries, most recent activity first, each with its next-step button and a secondary link);
  **Up next** (planned, 6 posters, "All planned"); **Recent activity** (10 rows: own
  events, shares, link requests, import results); **Stats** (all-time totals per kind group as native SVG charts, each with a text
  equivalent such as a table or sentence; link to the Stats page).
- Stats page (`/stats`, phase 1): everything over time, with a tab per group (Watch, Read,
  Listen, Play) and an overall tab; per year, per month, per person. Charts are native SVG
  with text equivalents (decided, owner review 2026-11-06, DEC-171).
- Empty (no entries): the empty-state block with "Find something" (palette) and "Import a
  file"; sections do not render as empty boxes (Carbon: one primary action per screen).
- Keyboard: `J/K` move between continue cards, `Enter` runs the next step.
- Mobile: sections stack; Up next becomes a horizontal rail with scroll snapping; the nudge
  is one line.

### 6.2 Library (`/library`) - `library.html`

- Header: H1 with total count, Import, Add. Row 1: group segmented control (All, Watch,
  Read, Listen, Play; remembered per user), view toggle, sort. Row 2: kind chips with counts
  (hidden when zero unless All). Row 3: status tabs with counts; filter buttons (Tags, kind
  facets, Rating, Owned for board games); active filters as removable badges; a filter text
  box (`/`).
- Grid: `PosterCard` 2:3 (1:1 for music and board games), status pill with the next step,
  progress bar on the edge, hover/focus quick menu (next step, status, tag, list, remove).
  Minimum card width 8.5 rem; 2 columns at 360 px, up to 8 at 1280 px.
- List view: `DataGrid` with Title, Kind, Progress, Rating, Tags, Last activity, next-step
  button; checkboxes and a bulk bar (status, tag, list, owned, remove) with a count.
- States: empty library (same block as Home); empty filter result names the active filters
  and offers "Clear filters" and the nearest non-empty status; loading shows 12 poster
  skeletons; error inline with Retry.
- Keyboard: arrows move focus in the grid (roving tabindex), `Enter` opens, `W` runs the next
  step on the focused card, `T` tag, `L` list.
- Mobile: group control scrolls horizontally under the title; filters open a bottom sheet;
  3 posters per row; bulk edit only in list view.

### 6.3 Item page (`/items/:id`) - `item.html`

- Layout: cover column (14 rem) with an "entry facts" card (status, added, started,
  finished, rewatches, owned flags; each editable in place); main column with kind badges,
  title, original title, meta line; **action bar card** (next step, primary button, "With...",
  overflow menu: set status, rewatch, remove, report wrong match); rating/tags/lists row;
  tabs (kind-specific first tab, About, History, Notes, Related); companions row ("Watched
  with" aggregated); attribution footer.
- Action bar variants per progress model (section 3.4).
- About tab: description, people (cast, authors, designers) as chips linking to a filtered
  library search, attributes from the kind schema rendered as a definition list (generic
  for any kind), external ids as links to the provider pages.
- Episodes tab: season chips with x/y counts, "Mark season watched", vertical episode list
  (R-28) with check circles, next-up row highlighted, per-row menu (watched on date, with,
  note, mark previous).
- Plays tab (sessions): table of plays (date, players, scores, winner, duration), stats
  strip (plays, H-index contribution, win rate, best player count), "Log a play".
- History tab: this item's events newest first with Edit.
- Notes tab: private note (always), "Publish as review" toggle, spoiler flag.
- States: stub without source: cover placeholder from dominant colour, "No details
  available. Add a data source or edit the title and year by hand" with an Edit button;
  partial: "Fetching details..." live line, never blocks actions; error: inline with Retry.
- Keyboard: `W` next step, `R` rating focus, `T` tag, `L` list, `1-5` rating when the rating
  control is focused.
- Mobile: cover and title stacked; the action bar becomes a sticky bottom bar above the tab
  bar; tabs scroll horizontally.

### 6.4 Search palette (dialog, any page) - `palette.html`

- Input with scope segments (All, Watch, Read, Listen, Play; `Tab` cycles), `Esc` closes and
  returns focus to the opener (APG dialog).
- Sections in order: In your library; one group per source and kind; Actions ("Add by
  hand", commands such as "Log a play of X", "Go to Lists", "New list"). At most 8 rows per
  group with "more" (Baymard autocomplete guidance, R-31).
- Row anatomy: thumbnail (thumbhash), title + year, meta line (kind, progress, author), right
  side: "In library" badge or Add + "Add as..." menu; the active row shows `Enter` and
  `Ctrl+Enter` hints.
- Footer: source status dots with words, "Asking X..." live line, and the setup link for
  missing sources (admin) or "Ask admin" (member).
- No results / no sources / offline states as in 5.3.
- Mobile: full-screen; scope segments under the input; rows 56 px tall.

### 6.5 Lists (`/lists`, `/lists/:id`, `/shared/lists/:token`) - `lists.html`

- Overview: tabs Mine / Shared with me / On this instance; cards with cover strip, count and
  kinds, visibility badge, freshness; "New list" opens a one-field sheet (name) and then the
  list page.
- List page: title (editable), description, visibility badge with Share, item rows with drag
  handle and keyboard move up/down, remove; "Add" opens the palette scoped to this list;
  "Add all to my library" for lists you do not own.
- Shared public page: no app chrome except the brand header, title, owner, posters, "Sign
  in to add these to your library"; `shared-list-missing` as a friendly 404 ("This list is
  not shared any more").

### 6.6 History (`/history`) - `history.html`

- Filters: group, person, year, "Shared with me". Stream by day, rows as sentences with the
  kind icon, source (Jellyfin, import), Edit; shared rows highlighted with "Copy to my log".
- Side panel: this year vs the same period last year (movies, episodes, books, pages,
  plays, H-index), most played with, unplayed owned games, link to All stats.
- Stats live on the Stats page (6.1, phase 1); this panel links to it.
- Export button (CSV/JSON, the documented generic format, DEC-118).

### 6.7 People (`/people`) - `companions.html`

- Requests for you as cards at the top; tabs All / Linked accounts / Game groups; person
  cards (avatar initials, name, link state, together/plays/win rate, last, History, Log
  with); duplicate suggestion card; "Add a person" sheet (name, optional avatar, note).
- Person page (`/people/:id`): header with stats, History filtered to the person, lists
  shared by them, actions (invite, link, unlink, merge, edit, delete with "keeps history").
- Privacy switch ("Linked people see the sessions I log") lives under Settings > Privacy,
  with a copy of it on the invite dialog.

### 6.8 Notifications (`/settings/notifications`, bell icon) - `notifications.html`

- Tabs Inbox / Channels / Rules / Log. Inbox: rows grouped by day, unread bold with a dot,
  "Mark all read", each row links to its subject; empty "Nothing new. You will see new
  episodes, shares and import results here." Channels: cards with health from the delivery
  log and Send test; add-channel inline form per type. Rules: matrix (event rows, channel
  switches, mode in words, instance defaults labelled, locked rules show a lock); "Add a
  rule for one list, item or person". Log: table with state and expandable error.
- The bell shows the unread count (`aria-label="Notifications, 2 unread"`), capped at "9+".

### 6.9 Integrations (`/settings/integrations`) - `integrations.html`

- Sender cards with health and last recorded item; refused calls explained inline with the
  field name and the last payload (secrets redacted); "Connect a sender" 3-step panel;
  revoke in the card menu with a confirm.

### 6.10 Data sources (admin `/admin/sources`; member `/settings/sources`) - `sources.html`

- Admin: hosting-mode banner (hosted only, until agreements are recorded); table with
  Source, Kinds, Credential (instance / per user / none, "from environment"), Status (as in
  4.4), last 24 h calls and errors, Details/Set up; a source card expands with fields, Test,
  how-to lines, attribution preview, "Disabled" switch; the attribution list (the current
  Sources page) moves to the bottom of this page and to the public About page.
- Member: the list of what works, what needs their own account, what is missing with "Ask
  admin".

### 6.11 Imports and export (`/settings/imports`)

- Cards per format (Trakt, Goodreads CSV, BG Stats, BGG, generic CSV/JSON, legacy); drop
  zone or file picker; dry-run preview (counts, sample rows, what maps to what and what
  will be lost, R-35); run; progress bar with "x of y" and cancel; result: imported,
  skipped, unmatched (with a persistent list, manual match via the palette, retry). Export:
  one button per format with a short description.

### 6.12 Settings (`/settings/*`)

- Account: display name, email, username (read-only with explanation), password change,
  TOTP setup (QR via otpauth URL as text and image generated server-side), OIDC link, theme,
  locale (new: per-user locale, falls back to the brand default), delete account (explains
  demotion to companion, DEC-017 behaviour).
- Privacy: share sessions switch; DNF/Dropped visibility; review default visibility.

### 6.13 Admin (`/admin/*`)

- Overview: health rows (database, search, storage, jobs, mail, sources) with last check and
  one action; recent sign-ups.
- Users and sign-up: sign-up mode (open / invite / closed) with a sentence per option;
  invites list (create, revoke); users table (role, last seen, actions: make admin,
  disable, delete with the companion-snapshot explanation).
- Notification defaults: the Rules matrix for instance defaults with a lock per row.
- Storage: the storage report (budget, watermarks, tiers, S3 counters) as cards and one
  table; "Run eviction now". Budget numbers are editable here; a value set in env shows
  read-only with the variable name (OD-10, DEC-172).
- Sessions: session length and cookie security, editable with the same env rule; a cookie
  change applies on the next session and warns when the public URL is not https (OD-11).
  The full admin area (eight pages) is specified in addendum-1.md section 3, which wins.
- Branding: read-only summary of `brand.toml` (name, locale, theme swatches with computed
  contrast) and a link to the white-labelling doc.

### 6.14 Auth pages (`/sign-in`, `/sign-up`, `/invite/:token`, `/setup`) - `signup.html`

- Centred 28 rem card, brand mark above, no app navigation, footer with legal links from
  the brand. Behaviour in 5.2. Invite pages state who invited you and what links on
  creation.

### 6.15 Error and status pages

- 404: "There is nothing at this address" + Home / Library links (in the app shell when
  signed in).
- Service problem: GOV.UK-style single page: what happened, your data is safe, try again
  later, support link from the brand.
- Offline banner (role=status) at the top of the shell when `navigator.onLine` is false or a
  server function fails with a network error; disappears on reconnect.

---

## 7. Component inventory

Mapped to `src/rust/crates/ui/src` (generic, no workspace deps, all labels as props) and
`src/rust/crates/web/src/widgets.rs` (domain widgets).

### 7.1 Exists in `ui`, keep

| Component | Status | Changes |
|---|---|---|
| `Button` (6 variants, 4 sizes) | keep | add `loading: Signal<bool>` (spinner, keeps label, `aria-busy`); `Lg` height 2.75 rem on touch |
| `Badge` | keep | add variants `Success`, `Warning`, `Info` (soft background + semantic text) |
| `Card` family | keep | none |
| `Avatar` | keep | initials fallback from a `name` prop; sizes sm/md/lg |
| `Dialog`, `Sheet` | keep | focus trap and return-to-opener verified; `Sheet` bottom side for mobile with grabber and medium/full height prop; discard-confirm hook when `dirty` |
| `Command`, `CommandItem` | keep | `CommandItem` gains `meta`, `thumb` (data URL), `badge`, `actions` (trailing buttons), `kind` and `source` for grouping; `on_secondary` callback for `Ctrl+Enter`; footer slot; sections capped with "more" |
| `Combobox` | keep | used for people and tag pickers with "create '<text>'" row |
| `DataGrid`, `Table` | keep | selectable rows prop, bulk bar slot, sticky header |
| `DropdownMenu`, `MenuItem` | keep | icon slot, destructive variant, keyboard shortcut hint slot |
| `FormField`, `Input`, `Textarea`, `Select`, `Checkbox`, `Switch`, `Label` | keep | `Input` gains `trailing` slot (Show/Copy buttons), `inputmode`, `autocomplete`; `FormField` gains `rules: Signal<Vec<(String, RuleState)>>` for the live checklist and `status: Ok(String)` for the green line; keep `described_by` wiring |
| `Tabs` | keep | manual activation when panels load lazily (APG); count slot |
| `Toast`, `Toaster` | keep | `role="status"` container; `action` slot (Undo); duration 10 s for undoable; errors never go through toasts |
| `Tooltip` | keep | none |
| `Skeleton` | keep | shapes: `Poster`, `Line`, `Row`; honours reduced motion |
| `Icon`, `IconName` | keep | add the names listed in 2.10 to `icons.toml` |
| `ThemeToggle` | change | becomes a three-state (system/light/dark) item in the account menu; storage key stays `app-theme` |

### 7.2 Add to `ui`

| Component | Purpose |
|---|---|
| `Chip` (toggle) | kind chips, people toggles, date chips; `aria-pressed`, count slot, icon slot |
| `Segmented` | group switcher and view toggle; `role="tablist"` or `radiogroup` depending on use |
| `Alert` (info/success/warning/destructive) | inline persistent messages with icon, title, body, actions; `role` chosen by the caller (`alert` for errors, `status` otherwise) |
| `EmptyState` | icon, title, body, primary and secondary actions; replaces the element it stands in for |
| `StatusDot` | dot + word (never dot alone), variants ok/warn/bad/off |
| `ErrorSummary` | GOV.UK-style list of field errors with anchor links; receives focus on render |
| `Steps` | wizard and multi-step panels, current/done states |
| `TaskList` | setup checklist rows with done state |
| `SideNav` | settings navigation with group headings and `aria-current` |
| `TabBar` | mobile bottom navigation with a centre action |
| `ProgressBar` (generic) | determinate bar with `aria-valuenow`, thin and regular sizes |
| `Rating` (generic stars) | move from `web` `RatingInput` into `ui` as a generic 0-100 control with half steps and keyboard support; `web` keeps the mapping to the user's scale |
| `KeyHint` | `<kbd>` pairs for shortcut hints |
| `CopyField` | read-only input with Copy button and "copied" status |

### 7.3 Domain widgets in `web`

| Widget | Status |
|---|---|
| `PosterCard` | extend: next-step pill, progress edge bar, quick menu, square variant |
| `Cover` | keep |
| `CompanionPicker` | rebuild on `Chip` toggles + `Combobox` with inline create |
| `RatingInput` | becomes a thin wrapper over `ui::Rating` |
| `NextStepButton` | new: derives label and action from entry + kind descriptor |
| `ActionBar` | new: per progress model (section 3.4) |
| `LogSheet` (`WatchedSheet`, `PlaySheet`, `SessionSheet`, `PagesInline`, `PositionInline`) | new |
| `EpisodeList`, `SeasonChips` | new |
| `ContinueCard`, `ActivityRow`, `HistoryDay` | new |
| `SourceStatusFooter`, `SourceCard` | new |
| `ChannelCard`, `RulesMatrix`, `SenderCard` | new |
| `PersonCard`, `RequestCard` | new |
| `AppShell` (top bar + mobile tab bar + offline banner + toaster + palette) | replaces `Header` |

### 7.4 Theme and CSS

- `input.css`: change `--input` and `--ring` as in 2.7; add `--success`, `--warning`,
  `--info`, `--destructive-text`, the `-soft` tints, `--shadow-1..3`; add a
  `@media (prefers-reduced-motion: reduce)` block that sets `transition-duration: 1ms` and
  `animation: none`; add `scroll-padding-top: 3.5rem` and the tab-bar bottom padding.
- Brand `[theme]` keys stay the same names; the brand crate validates contrast of
  `primary`/`primary-foreground` and logs a warning below 4.5.

---

## 8. Accessibility requirements

Targets: WCAG 2.2 AA is a requirement everywhere, together with the density budget OD-01
(addendum-1.md 1.2); decided (owner review 2026-11-06, DEC-171). Concretely: WCAG 2.2 AA throughout; the Playwright axe check already in M06 is extended to
every page in the e2e suite.

- **Keyboard:** everything operable without a mouse (2.1.1). Roving tabindex in grids and
  lists; `Esc` closes layers and restores focus; shortcuts are single keys only outside
  text fields and listed under `?`. Drag-and-drop list ordering has "Move up/down" buttons.
- **Focus:** visible 2 px ring (`--ring`, >= 3:1 against both themes) on every interactive
  element, never removed (2.4.7); sticky bars never cover a focused element
  (`scroll-padding`, 2.4.11); dialogs trap focus, label themselves and return focus (APG
  dialog pattern).
- **Screen readers:** palette is an APG combobox + listbox with `aria-activedescendant`
  (already); tabs follow the APG tabs pattern; status dots have text; icon-only buttons
  have `aria-label`; counts in nav items are part of the accessible name ("Notifications, 2
  unread"); toasts and "Asking X..." use `role="status"`/`aria-live="polite"`, errors
  `role="alert"` (4.1.3); form fields keep `aria-describedby` to hint + error and
  `aria-invalid`; error summary receives focus; poster images have `alt` of the title;
  decorative icons `aria-hidden`.
- **Contrast:** text >= 4.5:1, large text and UI boundaries >= 3:1 in both themes
  (tables in 2.7); semantic colours always paired with an icon or word (1.4.1).
- **Reduced motion:** all transforms become fades, skeleton pulse stops, auto-advance has
  no animation (2.3.3 technique; `prefers-reduced-motion`).
- **Touch:** minimum 44 x 44 px for primary controls on mobile, 24 px floor with spacing
  for dense secondary icons (2.5.8); destructive actions away from frequent targets.
- **Language and i18n scope** (decided, owner review 2026-11-06, DEC-171): every
  user-visible string lives in Fluent `.ftl` locale files (pages, server errors,
  notifications, emails); dates and numbers are locale-aware; plurals use Fluent plural
  rules; each user has a language setting with the brand default as fallback; `<html lang>`
  follows the user's locale and foreign-language fragments carry their own `lang`; a parity
  test fails the build when a key exists in one locale only. Right-to-left languages are out
  of scope. German strings are reviewed for length (buttons allow wrapping to two lines on
  mobile).
- **Zoom and reflow:** layouts work at 320 px width and 200% zoom; no horizontal scrolling
  except deliberate rails (1.4.10).
- **Time limits:** none in the UI; the session is sliding (DEC-069).

---

## 9. Implementation plan

Each phase ends green (`cargo xtask ci`, e2e) and can ship on its own. The owner's
complaints are numbered as in the brief: (1) top bar, (2) library separation, (3) search
without sources, (4) data-source credentials and modes, (5) stale pages, (6) forms.

### Phase 0: foundations (1 week)

- Tokens: `input.css` changes (2.7, 7.4); add `Alert`, `EmptyState`, `StatusDot`, `Chip`,
  `Segmented`, `ErrorSummary`, `Steps`, `TaskList`, `SideNav`, `TabBar`, `CopyField`,
  `KeyHint`, `ProgressBar`, `Rating` to `ui` with gallery entries and axe checks; add the
  icons to `icons.toml` and `ICONS.adoc`.
- `ClientConfig` gains `signed_in`, `display_name`, `is_admin`, `unread`; `AppShell`
  replaces `Header`: top bar with five sections, search pill, add, bell, account menu;
  mobile tab bar; offline banner. **Fixes (1).**
- Form discipline: `FormField` rules/status, `Input` trailing slot; sign-up and sign-in
  rebuilt with hints, live normalisation, availability check (new server fn
  `username_available`), error summary; server error codes mapped per field; remove the
  generic "check the fields" path from auth. **Fixes (6).**
- Locale strings: new keys for every string in this document (en + de), the locale test
  already enforces parity.

### Phase 1: where am I, what is next (2 weeks)

- Kind groups in `kinds` (`group()` on `KindModule`, default from the module, exposed in
  the descriptor DTO).
- Library page: group switcher, kind chips with counts (new server fn `library_counts`),
  status tabs, tag filter and tags on entries (new `entry_tags` table + use-cases), facet
  filters from descriptors, sort, grid/list toggle, URL state, filter box (server query
  through the existing search port with the library index). **Fixes (2).**
- Next-step derivation (`app::next_step(entry, descriptor, progress)`), `NextStepButton`,
  `ActionBar` per model, `PosterCard` pill and quick menu.
- Home page rebuilt: continue, up next, activity (new `activity_feed` server fn over the
  change feed), all-time stats charts, Stats page with a tab per group (native SVG with text equivalents); empty state; admin nudge (reads source status).
- Log sheets: `WatchedSheet` (date chips, people, rating, note, rewatch), `PlaySheet`
  (players, scores, winner, more), inline pages/position; toasts with Undo (new
  `undo_progress(event_id)` use-case that appends a compensating event; append-only is
  preserved).
- Item page restructure: facts card, action bar, rating/tags/lists row, tabs, episode list
  with bulk marking and the "ask once" preference, notes tab with private note (new
  `entry_note` field with HLC like `notes`).
- Stats page (`/stats`) with a tab per group and the all-time charts on Home (native SVG
  with text equivalents).
- History page (`/history`, new name, no redirect): day groups, filters, shared rows, side stats.

### Phase 2: why is nothing happening (1-2 weeks)

- Credential model: `connector_credentials` table, `Credential` state in the registry,
  connectors always registered, `SourceStatus` computed from the governed client's
  counters (calls, 429s, circuit state, last 401) exposed by a server fn; palette footer and
  no-results card; member "Ask admin" event. **Fixes (3).**
- `instance.hosting` setting, setup wizard (`/setup`, locked after completion), admin area
  with Data sources (table + cards + Test), Users and sign-up, Overview health. Per-user
  credential page for TheTVDB PIN and Hardcover. Licence-mode record per source and the
  hosted banner. **Fixes (4).**
- Settings area with `SideNav`; move Notifications, Integrations, Sources, Privacy under it.

### Phase 3: the stale pages (1-2 weeks)

- Notifications: tabs, channel cards with health from the delivery log, inline add form with
  validation, rules matrix, log table, inbox grouping. **Fixes (5).**
- Integrations: sender cards with health and last payload (redacted), the three-step connect
  panel with SSE "waiting for first call" and "send a test call". **Fixes (5).**
- People: cards with stats (new `companion_stats` use-case), request cards, invite/link/merge
  dialogs, person page. **Fixes (5).**
- Lists: cards, share dialog with visibility radios (adds "make private again", which the
  API lacks today), list page with ordering, public page polish.
- Imports page with dry-run preview, progress, unmatched list; export.

### Phase 4: fluency and measurement (ongoing)

- Keyboard map (`G then X`, `?` help), roving focus in grids, `Ctrl+Enter` actions in the
  palette, recent items and commands in the empty palette.
- Bulk bar in list view; saved views if tags and lists prove insufficient.
- Opt-in goals on the Stats page; game groups.
- Per-user fair share on the interactive lane for hosted mode.
- Instrumentation: count taps and time from "intent" (palette open or next-step click) to
  "logged", and abandonment of sheets, as `tracing` events aggregated per day (no third
  party, no per-user profiles). Review after four weeks and cut what is not used. This is the
  substitute for the public analytics that do not exist for any tracker [R-54].

---

## 10. Decisions for the owner

All items below are decided (owner review 2026-11-06, DEC-171/DEC-172). The
whole design is approved with these rulings.

1. Status label "Completed" (en) / "Abgeschlossen" (de), not "Done"/"Fertig"; keys
   unchanged. **Decided: Completed / Abgeschlossen** (owner review 2026-11-06, DEC-171).
2. `--input` border at zinc-500 (passes 1.4.11) or zinc-400 + filled field. **Decided:
   zinc-500** (owner review 2026-11-06, DEC-171).
3. Hosting mode default. **Decided: `hosting = "home"` by default; the wizard asks**
   (owner review 2026-11-06, DEC-171).
4. Routes `/sessions` -> `/history` and `/companions` -> `/people`. **Decided: rename, with
   no redirects (greenfield)** (owner review 2026-11-06, DEC-171).
5. Mobile "You" hub vs a sixth bar item. **Decided: hub, five items** (owner review
   2026-11-06, DEC-171).
6. Web font. **Decided: the default brand ships Atkinson Hyperlegible Next (OFL 1.1),
   self-hosted, never a CDN; system stack as fallback** (owner review 2026-11-06, DEC-171).
7. TMDB logo. **Decided: the required logo is committed unmodified from TMDB's official
   source (R-F3, DEC-170), plus an open-source attribution page** (owner review 2026-11-06,
   DEC-170, DEC-171).
8. Further rulings (owner review 2026-11-06, DEC-171): secrets encrypted at rest
   (addendum-1.md section 2); WCAG 2.2 AA required everywhere with the density budget OD-01;
   i18n with Fluent `.ftl` files (section 8); Home shows all-time stats as native SVG charts
   and a Stats page shows everything over time (6.1, phase 1).
