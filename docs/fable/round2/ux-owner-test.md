# UX pass on the owner's test findings (round 2, F2)

Status: COMPLETE (all five sections written; see "Not verified" at the end).
Date: 2026-11-10. Read-only pass: nothing was built, run or rendered. Every code claim cites
`file:line` on `main` (225b09d). Paths are relative to the repo root; `crates/` means
`src/rust/crates/`, `en.ftl` / `de.ftl` mean `crates/web/locales/en.ftl` / `de.ftl`.
German suggestions are written in ASCII here (ae, oe, ue); the locale file uses the real
letters (rule 3 exception).

Short answers per finding:

| OT | One-line proposal |
|---|---|
| 4 | Two buttons instead of one: "Mark as watched" (primary) and "Want to watch" (adds as Planned). Every add shows a toast with Undo. |
| 5 | Buttons are imperatives ("Mark as watched"), states are a pill in the header ("Planned", "Completed"). Today the action is labelled "Watched", which reads like a state. |
| 6 | "Watched with..." / "Listened with..." / "Played with..." per group verb, on the button, the sheet title and the people legend. |
| 7 | Replace "Rewatches: N" by "Watched 3 times, last on <date>" in the header. Nobody has to add 1 + N. |
| 9 | Status says where you are with the item now, history counts every time. A movie stays Completed while and after a rewatch. A show rewatch is an explicit run: In progress while it runs, back to Completed when done or stopped. |
| 10 | Centered dialog from 640 px, bottom sheet below. The odd spacing has a concrete CSS cause (full-height grid). |
| 12 | One size for all actions in a card footer; the People "History" link is 36 px / 14 px text next to 32 px / 12 px buttons. |
| 13 | Violet accent plus four group tints as brand tokens; all pairs pass AA in light and dark (numbers in 3.1). |
| 14 | Grid pages up to 96 rem, reading pages 80 rem (the design value; the code has 72 rem). |
| 15 | Bulk marks on already watched episodes ask "Log these N again?" instead of doing nothing. |
| 16 | Duration moves out of "More" into the first row of the play dialog (the approved mockup already has it there). |
| 17 | Time first: hero tile with piggy-bank and total hours, then movies h, shows h, pages, played h. The cache already stores seconds per kind; no migration for the first step. |

## 1. Library and status model (OT-4, 5, 7, 9, 15)

### 1.1 Today's model, exactly

**Statuses.** Five core values for every kind: `planned`, `in_progress`, `completed`,
`paused`, `dropped` (`crates/domain/src/kind.rs:84-95`); video games add `endless`
(`crates/kinds/src/builtin/play.rs:34-35`). One status per library entry, a last-writer-wins
field (`crates/domain/src/library.rs:57`). Labels: Planned, In progress, Completed, Paused,
Dropped, Endless (`en.ftl:141-146`); Geplant, Begonnen, Abgeschlossen, Pausiert, Abgebrochen,
Endlos (`de.ftl:141-146`). Owner ruling: "Completed", keys unchanged (HANDOFF 4.2, DEC-171).

**"Add to library".** The item page of an item without an entry shows one large button
"Add to library" (`crates/web/src/item.rs:481-499`, `en.ftl:182`). It calls
`api::add_to_library(id)`; `App::add_to_library` with no status creates the entry as Planned
(`crates/app/src/library.rs:24-58`, default at `:51`). After the call the page only refetches
(`item.rs:488-492`): no toast, no explanation, no Undo. The palette's Ctrl+Enter does the
same add but does show a toast "Added to Planned" with Undo
(`crates/web/src/palette.rs:190-216`, `en.ftl:1982`, DEC-426). So the same action is
explained in one place and silent in the other (OT-4).

**What the page shows after the add.** The header has only the kind and year badges and the
title (`item.rs:290-299`); there is no status pill. The status is a `<select>` in the facts
card under the cover (`item.rs:365-369`). The action bar now shows the next step as the
primary button, labelled "Watched" with a check icon (`item.rs:569-571`, `en.ftl:170`), then
"With..." (`item.rs:572-575`, `en.ftl:211`) and the overflow menu. A button that says
"Watched" with a check mark reads as a state ("this is watched"), not as an action; together
with the missing confirmation this explains OT-5 without any layout bug. After one watch the
button says "Watched again" (`en.ftl:171`; `crates/app/src/next_step.rs:117-119`), again a
past participle on a button.

**How logging moves the status.** `App::log_progress_stamped`
(`crates/app/src/library.rs:210-262`): a `Finished` event sets Completed; any other event
moves Planned to In progress; nothing else moves (`:223-227`). The move is stored on the
event as `status_change {from, to, stamp}` (`crates/domain/src/progress.rs:84-92`, `:110`) so
Undo can take it back (DEC-208). Consequences:

- A movie (Single model) is Completed after one watch and stays Completed on every further
  watch.
- A show (Episodic model) never becomes Completed by logging: `Finished` does not fit the
  episodic model (`progress.rs:43-47`, `:56-68`), the last episode only makes the next step
  `caught_up` ("Up to date", `next_step.rs:83-89`, `en.ftl:176`), and DEC-233 records "only a
  finished event completes an entry on its own". A show is Completed only when the person
  picks it in the status select.
- Paused and Dropped entries keep their status when something is logged.

**How a rewatch is stored.** There is no rewatch flag, column or event type (no match for
"rewatch" in any migration). A rewatch is simply one more event:

- Movie: one more `Finished`. `ProgressSummary.finished` counts them all
  (`progress.rs:134-135`, `:177`); the UI shows "Rewatches" = finished minus one, inside the
  closed "More facts" disclosure (`item.rs:358-362`, `:378`, `en.ftl:204`). The sheet adds a
  line "Finished once before: this counts as a rewatch." (`en.ftl:244-247`,
  `crates/web/src/log_sheet.rs:302`, `:311`). That is the "watched once + X rewatches"
  arithmetic of OT-7.
- Show: one more `Episode {season, episode}`. The summary keeps distinct episodes and all
  views separately (`progress.rs:136-139`, `:178-181`). The next step ignores rewatches of
  older episodes (DEC-207). There is no notion of "a rewatch run": the page cannot say
  "rewatch, 5 of 24".
- A stopped rewatch (OT-9): for a movie nothing is stored at all (the Single model has no
  partial progress), the entry stays Completed. For a show the re-logged episodes are extra
  views; the status does not change. So the answer to "is it both?" is today: it is
  Completed, and the UI says nothing about the rewatch.
- The legacy importer maps "rewatching" to In progress (DEC-354: "In Progress (wins: a
  rewatch)"), so imported libraries already contain entries that are In progress with
  earlier finishes. The model below gives that state a name.

**"Mark up to here" on watched episodes (OT-15).** The row menu item "Mark up to here"
(`crates/web/src/episode_list.rs:486-491`, `en.ftl:290`) calls `unwatched_up_to`
(`episode_list.rs:338-341`), which returns only episodes not yet watched
(`crates/web/src/episode_plan.rs:96-110`). When all of them are watched the list is empty and
`log_episodes` returns at once (`episode_list.rs:92-94`): no event, no toast, no message.
"Mark season watched" and "Mark show watched" behave the same (`episode_list.rs:332-335`,
`:364-367`, `episode_plan.rs:114-131`). Only the per-row button "Again" logs a rewatch, of
that one episode (`episode_list.rs:526-530`, `episode_plan.rs:277-285`; DEC-231: "Marking an
already watched episode logs a rewatch of it alone"). So a bulk rewatch is not possible and
the UI does not say why.

### 1.2 Proposed model (default: model A)

One sentence for the owner and for the docs: **the status says where you are with this item
now; the history counts every time you did it.** Nothing is "both".

Rules:

1. **Not in the library: two buttons, both verbs.** Primary "Mark as watched" (per kind, see
   copy table), secondary "Want to watch". "Want to watch" is today's "Add to library": it
   creates the entry as Planned. "Mark as watched" logs the event; the server already creates
   the entry on the first log (`library.rs:222`), so no new use case. The generic "Add to
   library" label stays only in the palette hint and for kinds where no verb fits.
2. **Every add is confirmed and reversible.** Item page and palette show the same toast with
   Undo (the palette's `toast_added`, `palette.rs:190-216`, moves to a shared place).
3. **States are shown as a pill in the header, actions are imperative buttons.** The pill
   sits next to the kind and year badges (`item.rs:291-294`): status word plus one fact line
   under the title. The facts card keeps the status select for changing it.
4. **Counts are "times", never "once + N rewatches".** "Watched 3 times, last on 2 Nov 2026"
   replaces "Rewatches: 2". The same number feeds the sheet line.
5. **Movies and other Single kinds:** a rewatch is one more watch. Status stays Completed
   before, during and after. A started and abandoned rewatch leaves no trace because nothing
   was logged; the page does not pretend otherwise.
6. **Shows and other Episodic kinds: a rewatch is an explicit run.**
   - Start: overflow menu "Start a rewatch" on a Completed or up-to-date show, and the answer
     "Rewatch from here" / "Log again" of the bulk question in rule 7.
   - While it runs the status is In progress (that is where you are now) and the header says
     "Rewatch: 5 of 24 episodes. Watched once before." The next step counts within the run
     ("Mark S1 E6 watched").
   - Finished run: the event that completes the run moves the status back to Completed and
     the count goes to "Watched 2 times".
   - Stopped run: overflow "Stop the rewatch" sets Completed again; the header says
     "Completed. Watched once; rewatch stopped after 5 of 24 episodes." The 5 events stay in
     the history.
   - Watching one favourite episode again ("Again" on a row) is not a run and changes no
     status, as today.
7. **Bulk marks never do nothing.** "Mark up to here", "Mark season watched" and "Mark show
   watched" on a range where everything is already watched ask inline (same pattern as the
   existing "Also mark the earlier episodes?" block, `episode_list.rs:426-470`; addendum-1
   D15: no dialog): "S1 E1 to E5 are already watched. Log them again as a rewatch?" with
   "Log 5 again" and "Cancel". A mixed range logs the unwatched ones as today and the toast
   says how many were skipped as already watched.
8. **A show can be completed in one click.** When the last known episode is logged the toast
   offers "Mark as Completed" next to Undo. Not automatic: the app does not know whether the
   show still airs (DEC-207), and R-23 says a status never depends on provider data.

How the item page header reads (movie, en):

| Situation | Pill | Line under the title | Primary button | Secondary |
|---|---|---|---|---|
| Not in the library | none | none | Mark as watched | Want to watch |
| Added, not watched | Planned | Added on 10 Nov 2026 | Mark as watched | Watched with... |
| After 1 watch | Completed | Watched once, on 10 Nov 2026 | Log a rewatch | Watched with... |
| After 3 watches | Completed | Watched 3 times, last on 10 Nov 2026 | Log a rewatch | Watched with... |

Show:

| Situation | Pill | Line | Primary button |
|---|---|---|---|
| First run | In progress | 12 of 24 episodes | Mark S2 E1 watched |
| All watched, not completed | In progress | Up to date: 24 of 24 episodes | (label "Up to date"; toast offered "Mark as Completed") |
| Completed | Completed | Watched once, finished on 10 Nov 2026 | (overflow: Start a rewatch) |
| Rewatch running | In progress | Rewatch: 5 of 24 episodes. Watched once before. | Mark S1 E6 watched |
| Rewatch stopped | Completed | Watched once; rewatch stopped after 5 of 24 episodes | (overflow: Start a rewatch) |

### 1.3 Copy (en) and the meaning of the German keys

Existing keys keep their names; only their text changes. New keys are marked (new).

| Key | en today | en proposed | de meaning (ASCII) |
|---|---|---|---|
| `next-step-watched` (`en.ftl:170`) | Watched | Mark as watched | "Als gesehen markieren" |
| `next-step-watched-again` (`:171`) | Watched again | Log a rewatch | "Nochmal gesehen eintragen" |
| `next-step-finished` (`:172`) | Finished | Mark as finished | "Als beendet markieren" |
| `next-step-finished-again` (`:173`) | Finished again | Log it again | "Nochmal eintragen" |
| `next-step-episode` (`:174`) | Watched S2 E5 | Mark S2 E5 watched | "S2 E5 als gesehen markieren" |
| `next-step-episode-listened` (`:175`) | Listened to S2 E5 | Mark S2 E5 listened | "S2 E5 als gehoert markieren" |
| `item-want-watch` (new) | - | Want to watch | "Moechte ich sehen" (adds as Geplant) |
| `item-want-read` (new) | - | Want to read | "Moechte ich lesen" |
| `item-want-listen` (new) | - | Want to listen | "Moechte ich hoeren" |
| `item-want-play` (new) | - | Want to play | "Moechte ich spielen" |
| `item-add` (`:182`) | Add to library | Add to library (fallback for group Other only) | unchanged |
| `item-added-planned` (new, replaces `search-added-planned` `:1982`) | Added to Planned | Added to your library as Planned | "Als Geplant in die Bibliothek aufgenommen" |
| `item-times-watched` (new, plural) | - | Watched once / Watched { $count } times | "Einmal gesehen" / "{ $count }-mal gesehen" |
| `item-times-read`, `-listened`, `-played` (new) | - | Read once ..., Listened once ..., Played once ... | per verb, same pattern |
| `item-last-on` (new) | - | last on { $date } | "zuletzt am { $date }" |
| `item-rewatches` (`:204`) | Rewatches | removed (replaced by `item-times-*`) | - |
| `log-rewatch` (`:244-247`) | Finished once before: this counts as a rewatch. | Watched once before. This will be watch number { $next }. | "Schon einmal gesehen. Das wird Nummer { $next }." |
| `item-rewatch-start` (new) | - | Start a rewatch | "Nochmal von vorn schauen" |
| `item-rewatch-stop` (new) | - | Stop the rewatch | "Wiederholung beenden" |
| `item-rewatch-line` (new) | - | Rewatch: { $done } of { $total } episodes. { $times } before. | "Wiederholung: { $done } von { $total } Folgen." |
| `item-rewatch-stopped` (new) | - | rewatch stopped after { $done } of { $total } episodes | "Wiederholung nach { $done } von { $total } Folgen beendet" |
| `episodes-again-ask` (new, plural) | - | S{ $season } E{ $from } to E{ $to } are already watched. Log them again as a rewatch? | "... sind schon gesehen. Nochmal als Wiederholung eintragen?" |
| `episodes-again-all` (new) | - | Log { $count } again | "{ $count } nochmal eintragen" |
| `episodes-skipped` (new) | - | { $count } already watched, not logged again | "{ $count } schon gesehen, nicht erneut eingetragen" |
| `item-complete-offer` (new) | - | Mark as Completed | "Als abgeschlossen markieren" |

Toasts (all `role=status`, 10 s, with Undo as today, design.md 5.4):

- Want to watch: "Added to your library as Planned" + title, action "Undo".
- Mark as watched (first): "Logged: watched" + title, "Undo" (key `library-logged`,
  `en.ftl:103`, with the step in the past tense; the toast keeps the past-tense words
  "watched", "S2 E5 watched" via a second key family `logged-step-*` so buttons can be
  imperative and confirmations past tense).
- Last episode: "Logged: S3 E10 watched. You are up to date." actions "Mark as Completed",
  "Undo".
- Rewatch run finished: "Rewatch finished. Watched 2 times."
- Bulk: "Logged: 5 episodes" (today's `episodes-logged`, `en.ftl:299-302`), plus
  "2 already watched, not logged again" when some were skipped.

Risk to check when building: the imperative episode label is longer than "Watched S2 E5".
It is also the pill on poster cards (`crates/web/src/widgets.rs:165`, `:189`). If it does
not fit at 360 px, cards keep a short key (`card-step-episode` = "S2 E5" with the check
icon and the full sentence as `aria-label`).

### 1.4 What changes in the domain and events

No migration for model A.

Stays as it is: the five statuses and `endless`; the Fluent status keys; append-only events
and Undo (D-12, DEC-208); `ProgressSummary`; LWW status; "Planned is the default status of
an add"; everything in the importers.

Changes:

| Layer | Change | Size |
|---|---|---|
| web, i18n | Two buttons on a not-yet-added item; shared add toast; header pill and fact line; imperative labels; "times" counts; bulk question; skipped count | S |
| api / contracts | `ItemPageDto.next_step` also for an item without an entry (today the page only uses it with an entry, `item.rs:481`; whether the server sends it then was not checked), or the web derives it from `KindDto.progress_model` | S |
| app | `log_progress`: an optional flag `rewatch_run` on the log call. With it, an `Episode` event on a Completed entry moves the status to In progress and stores that move in `status_change` (the field exists, `progress.rs:110`). The run is then derivable without new storage: the events since the newest event whose `status_change` is Completed -> In progress | M |
| app | `next_step` and the summary become run-aware for episodic kinds: inside a run the next episode and "x of y" count only the run's events; the event that completes the run moves In progress -> Completed | M |
| contracts | `NewProgressDto.rewatch_run: bool` (additive), `ProgressSummaryDto` gains `times` (full passes) and optional `run_done` / `run_total` | S |
| web offline queue | `log_command` carries the flag (`crates/web/src/episode_list.rs:186-191`) | S |

Why the run start is an event property and not a new column: undoing the first rewatch
event then also undoes the status move with the existing rules (DEC-208), and sync clients
see nothing new except one more boolean.

### 1.5 The one alternative (model B: no rewatch run)

Do rules 1-5, 7 and 8 only. A show rewatch stays "extra views": the header of a Completed
show says "Watched once. 5 episodes watched again." and the bulk question logs the episodes
without touching the status. No `app` change at all, cost S instead of M. What is lost: the
next step and the "x of y" line do not follow a rewatch, and imported "In progress with
earlier finishes" entries (DEC-354) keep looking like a first run.

Recommendation: build B's parts first in pack ot2 (they are the common part of both models
and answer OT-4, 5, 7, 15 completely), then decide on the run (rule 6) after the owner has
used the new header for a week. Not proposed: a sixth status "Rewatching". It would touch
the owner-approved status set, every filter, the stats by status and the importers.

## 2. Log sheets (OT-6, 10, 16)

### 2.1 "With..." becomes the kind's verb (OT-6)

Today: the button says "With..." (`en.ftl:211`, `item.rs:574`), its accessible name is "Log
with details: <title>" (`en.ftl:212`), the sheet title is "Log with details" (`en.ftl:229`,
`log_sheet.rs:284`) and the people legend is "With" (`en.ftl:235`, `log_sheet.rs:313`). The
episode row menu already says "Watched with..." (`en.ftl:291`), and design.md 5.4 already
names the flow "Watched with". Only single and episodic kinds have this button
(`item.rs:527`, `:544-579`); books and audiobooks log inline (`item.rs:511-517`), games use
the play sheet (`item.rs:518-526`).

Proposal: one key family by group, used in all three places.

| Key (new) | en | de meaning (ASCII) | Used by |
|---|---|---|---|
| `log-with-watch` | Watched with | "Gesehen mit" | movie, show, anime |
| `log-with-listen` | Listened with | "Gehoert mit" | podcast (episodic) |
| `log-with-read` | Read with | "Gelesen mit" | reserved: no sheet for paged kinds today |
| `log-with-play` | Played with | "Gespielt mit" | video game, visual novel; board games keep "Players" (`en.ftl:236`) because scores hang on them |

- Button: "<verb> with..." (`item-with` is replaced by the family plus the ellipsis).
- Accessible name: "Watched with...: <title>".
- Dialog title: the step itself in the past tense ("Watched S2 E5", "Watched"), description
  the item title. This removes the separate step paragraph (`log_sheet.rs:310`) and one row.
- Legend of the people picker: "Watched with" (optional, no people = watched alone; say so
  in the hint: "Leave empty if you watched alone.").

### 2.2 Centered dialog or side sheet (OT-10)

Today: both log sheets use `Sheet` with `SheetSide::Adaptive` (`log_sheet.rs:284-285`,
`:621-622`): a bottom sheet below 768 px, a right-hand panel of full height and at most
28 rem from 768 px (`crates/ui/src/dialog.rs:239-243`).

Likely cause of "much empty space at the top, uneven clustering": the panel is a CSS grid
(`dialog.rs:158`, `grid gap-4`) and the side variant gives it the full viewport height
(`md:h-full`, `dialog.rs:241`). A grid with a fixed height and auto rows stretches its rows
to fill the height (`align-content: normal` behaves as `stretch` for grid containers), so
the title row and the form row each get a share of the free space; the form is itself a
grid (`log_sheet.rs:304`, `:633`) and stretches its rows again. The centered `Dialog` has
no fixed height (`dialog.rs:207`) and does not show this. This is read from the CSS, not
from a rendered page.

The approved mockup already draws both log forms as centered dialog cards with a footer bar
(`docs/design/ux/mockups/log.html:228-234`, `:343`, `:377`); only the text of design.md 5.4
and 7.3 says "sheet". So a centered dialog is closer to what was approved.

WCAG view: both variants are the same `Panel` (`dialog.rs:91-181`): `role="dialog"`,
`aria-modal`, labelled by the title, focus moved in, Tab kept inside, Escape closes, focus
returned (`dialog.rs:116-156`). So 2.1.2, 2.4.3 and 4.1.2 are equal. Differences:

- 1.4.10 Reflow and 1.4.4 Resize: the centered `Dialog` has no maximum height and no scroll
  (`dialog.rs:207`). A long form on a low or zoomed viewport would be cut off. It needs
  `max-h-[calc(100dvh-2rem)] overflow-y-auto` and `w-[calc(100%-2rem)]` before it can carry
  the log forms.
- 2.4.11 Focus Not Obscured: a sticky footer inside the scrolling panel needs
  `scroll-padding-bottom` of the footer height (same rule as addendum-1 D11).
- 2.5.8 / addendum-1 D10: the Save button is the primary control; 44 px on phones.

Proposal (default):

| Width | Presentation |
|---|---|
| >= 640 px | Centered dialog. Watched: max 32 rem. Play: max 42 rem (room for the score table). Max height `100dvh - 2rem`, body scrolls, header and footer (Cancel, Save) stay. |
| < 640 px | Bottom sheet as today (`max-h-[85dvh]`, scroll, rounded top, `dialog.rs:240`), full width, footer with a full-width 44 px Save button sticking to the bottom, safe-area padding. No swipe gesture needed: the close button and Escape stay. |

Implementation: a third presentation in `ui` (for example `Dialog` with `adaptive=true`) so
the breakpoint moves from `md` to `sm` in one place; add `content-start` to the panel grid
in any case. `SheetSide::Right` stays for real side panels (filters, navigation).

Alternative: keep the side sheet and only add `content-start` (cost S, one class). It fixes
the spacing but not the owner's preference.

### 2.3 Field layout within OD-01 (5 inputs / 7 rows)

Watched dialog (movie, show, anime, podcast):

| Row | Content | Inputs |
|---|---|---|
| 1 | When: Today / Yesterday / Pick a date (segmented; the date field appears only for "Pick a date") | 1 (2) |
| 2 | Watched with: people chips and "Add a person" | 1 |
| 3 | More: rating and note (disclosure, remembered, as today `log_sheet.rs:314-325`) | 0 closed |
| footer | Cancel, Save | - |

2 to 3 inputs, 3 rows: inside the budget, same content as today, only one row less.

Play dialog (board game, video game, visual novel), from 640 px in two columns:

| Row | Content | Inputs |
|---|---|---|
| 1 | When (left) and Duration (right, "1:30 or 90", hint "hours:minutes or minutes") | 2 |
| 2 | Groups (chips; only when the person has game groups, `log_sheet.rs:670`) | 0-1 |
| 3 | Players / Played with (chips and add) | 1 |
| 4 | Scores: one compact table, a row per player: name, score, "won" radio | 1 group |
| 5 | More: co-op, place, notes | 0 closed |
| footer | Cancel, Save | - |

Budget note for the owner: rows 1-3 are 4 inputs. The score table has one input per player,
so a 4-player game shows 8 inputs. Proposed reading of OD-01: the score table counts as one
row group (it is one question, "who scored what"), and by kind:

- board game: scores open (addendum-1 1.3 "players and scores open");
- video game and visual novel: scores and winner move into "More" (design.md 5.4 lists
  "date, minutes, note, companions" for a video game session); the open inputs are then
  When, Duration, Played with = 3.

The separate "Winner" select (`log_sheet.rs:731-736`) becomes the "won" radio column of the
table, preselected on the highest score (same rule as today, `en.ftl:254`).

### 2.4 Play time (OT-16) and where score and win show up (link to OT-11)

Finding: the duration can be logged today, but it is hidden. The field "Duration in
minutes" exists (`log_sheet.rs:757-759`, `en.ftl:257`), is parsed
(`crates/web/src/log_form.rs:239-252`), sent (`crates/contracts/src/browse.rs:398`) and
stored (`crates/domain/src/session.rs:52`, `crates/store-pg/migrations/0001_core.sql:147`).
It sits inside the closed "More" disclosure whose summary reads "Co-op, duration, place,
notes" (`log_sheet.rs:643-646`, `en.ftl:242`). The approved mockup has Date, Duration and
Location open in the first row (`log.html:346-350`) and design.md 5.4 says the sheet is
"prefilled with now, last duration, last location". So OT-16 is a deviation from the
design, not a missing feature. No server change is needed to fix it.

Proposal:

- Duration is in row 1, always open (2.3).
- Accept `90`, `1:30` and `1h 30` (today whole minutes only, `log_form.rs:239`); label
  "Duration", hint "hours:minutes or minutes".
- Prefill with the duration of the last play of this game (the sheet already loads the last
  play for its players, `log_sheet.rs:559-570`); for a first play of a board game offer the
  catalogue's typical play time as placeholder text, not as a value
  (`playtime_min_minutes`, `crates/kinds/src/builtin/play.rs:101-102`).
- Video games: the same field covers "I played 6 hours this week"; the item header then
  says "Played 14 times, 31 h in total".

Where a logged score, win and duration appear (today: nowhere readable, OT-11; F3 owns the
diagnosis, this is the display side):

- The item page History tab builds its rows without participants and without the session
  (`item.rs:834-835`), and a session row's text is only "Session logged"
  (`crates/web/src/history.rs:52-56`, `en.ftl:663`). The row DTO's people carry a name only,
  no score or winner (`crates/contracts/src/browse.rs:231-236`).
- Proposal, three places:
  1. Activity sentence (History, Home, item History): "Played 1 h 35 min with Anna and
     Jonas. Anna won (51, 42, 38)." Co-op: "Played ... Won together." Needs
     `ActivityPersonDto.score`, `.winner` and `ActivityRowDto.duration_minutes`, `.coop`
     (additive).
  2. Item page of a sessions kind: the "Plays" tab of design.md 6.3 (table: date, players,
     scores, winner, duration; strip: plays, total time, win rate). Recorded as not built
     in DEC-233.
  3. Stats, Play tab: hours played and plays first, win rate second (section 4).

## 3. Visual pass (OT-12, 13, 14)

### 3.1 Colour (OT-13)

Today every surface token is a zinc value and `--primary`, `--accent`, `--secondary` are
grey (`src/web/style/input.css:13-42`, `:63-89`); design.md 2.7 chose that on purpose
("The default is neutral on purpose", "Covers are the only saturated surfaces"). The
default brand file sets no colour at all (`config/brand/default/brand.toml:1-19`). The
brand crate already turns `[theme]` and `[theme_dark]` into CSS variables that outrank the
stylesheet (`crates/brand/src/lib.rs:44-49`, `:155-173`) and checks the accent pair
(`crates/brand/src/contrast.rs:166-203`). Chart bars, progress and primary buttons all use
`primary` (`crates/web/src/charts.rs:227`, `:283`; `crates/ui/src/button.rs:30`), so one
token change colours them.

Proposal: one accent and four group tints, all as values in the default brand file, none in
component code (rule 9).

- `config/brand/default/brand.toml` gets `[theme]` and `[theme_dark]` with the values below.
- `input.css` only declares the new variables with neutral fallbacks
  (`--group-watch: var(--muted-foreground)`, `--group-watch-soft: var(--muted)` and so on)
  and maps them in `@theme inline` (`--color-group-watch: var(--group-watch)`), so a
  white-label brand without these keys stays grey and still passes.
- `contrast.rs` checks the new pairs at start-up the same way as the accent pair.

Values are Tailwind 4.1.0 palette colours; hex computed from the oklch definitions in
https://unpkg.com/tailwindcss@4.1.0/theme.css (fetched 2026-11-10) with the CSS Color 4
matrices, rounded to 8 bits; ratios are WCAG 2.x contrast ratios of those hex values. The
method reproduces design.md 2.7 (zinc-50 on zinc-900: 16.97 here, 16.98 there; white on
violet-700: 7.30 here, 7.29 there).

Light theme (background and card are white #ffffff, muted is zinc-100 #f4f4f5):

| Token | Value | Hex | Pair | Ratio | Needs | Result |
|---|---|---|---|---|---|---|
| `primary` | violet-700 | #7008e7 | white text on it | 7.30 | 4.5 (1.4.3) | pass |
| | | | it as text or bar on white | 7.30 | 4.5 / 3 (1.4.11) | pass |
| | | | it as bar on the muted track | 6.64 | 3 | pass |
| `primary-foreground` | white | #ffffff | | | | |
| `primary-soft` (new) | violet-50 | #f5f3ff | violet-700 text on it | 6.65 | 4.5 | pass |
| | | | zinc-950 text on it | 18.14 | 4.5 | pass |
| `group-watch` | fuchsia-700 | #a800b7 | on white / on muted | 6.27 / 5.71 | 4.5 / 3 | pass |
| `group-watch-soft` | fuchsia-50 | #fdf4ff | fuchsia-700 on it / zinc-950 on it | 5.84 / 18.53 | 4.5 | pass |
| `group-read` | orange-700 | #ca3500 | on white / on muted | 5.22 / 4.75 | 4.5 / 3 | pass |
| `group-read-soft` | orange-50 | #fff7ed | orange-700 on it / zinc-950 on it | 4.92 / 18.74 | 4.5 | pass |
| `group-listen` | teal-700 | #00786f | on white / on muted | 5.36 / 4.88 | 4.5 / 3 | pass |
| `group-listen-soft` | teal-50 | #f0fdfa | teal-700 on it / zinc-950 on it | 5.14 / 19.08 | 4.5 | pass |
| `group-play` | blue-700 | #1447e6 | on white / on muted | 6.83 / 6.22 | 4.5 / 3 | pass |
| `group-play-soft` | blue-50 | #eff6ff | blue-700 on it / zinc-950 on it | 6.28 / 18.28 | 4.5 | pass |

Dark theme (background zinc-950 #09090b, card zinc-900 #18181b, muted zinc-800 #27272a):

| Token | Value | Hex | Pair | Ratio | Needs | Result |
|---|---|---|---|---|---|---|
| `primary` | violet-400 | #a684ff | zinc-950 text on it | 6.98 | 4.5 | pass |
| | | | it on background / card / muted | 6.98 / 6.21 / 5.22 | 4.5 / 3 | pass |
| `primary-foreground` | zinc-950 | #09090b | | | | |
| `primary-soft` | violet-950 | #2f0d68 | violet-400 on it / zinc-50 on it | 5.34 / 14.60 | 4.5 | pass |
| `group-watch` | fuchsia-400 | #ed6aff | on background / card / muted | 7.68 / 6.84 / 5.75 | 4.5 / 3 | pass |
| `group-watch-soft` | fuchsia-950 | #4b004f | fuchsia-400 on it / zinc-50 on it | 5.74 / 14.25 | 4.5 | pass |
| `group-read` | orange-400 | #ff8904 | on background / card / muted | 8.37 / 7.45 / 6.26 | 4.5 / 3 | pass |
| `group-read-soft` | orange-950 | #441306 | orange-400 on it / zinc-50 on it | 6.58 / 15.00 | 4.5 | pass |
| `group-listen` | teal-400 | #00d5be | on background / card / muted | 10.67 / 9.50 / 7.99 | 4.5 / 3 | pass |
| `group-listen-soft` | teal-950 | #022f2e | teal-400 on it / zinc-50 on it | 7.77 / 13.89 | 4.5 | pass |
| `group-play` | blue-400 | #51a2ff | on background / card / muted | 7.54 / 6.72 / 5.65 | 4.5 / 3 | pass |
| `group-play-soft` | blue-950 | #162456 | blue-400 on it / zinc-50 on it | 5.60 / 14.16 | 4.5 | pass |

Violet is the example design.md 2.7 itself names as passing in both themes, and the brand
crate's tests already use exactly these two hex values (`crates/brand/src/lib.rs:264`).

Where the colour goes (and where not):

- Accent: primary buttons, links, the active navigation item, the selected segment, focus of
  attention rows (the "next up" episode, today `bg-accent`, `episode_list.rs:513`), progress
  bars and single-series charts. Focus rings stay zinc (`--ring`), unchanged.
- Group tints: the kind badge and kind icon (item header `item.rs:292`, cards, history
  rows), the group cards and tiles on Home and Stats (soft background, tinted icon, tinted
  bar), the group tabs' marker. Text on a tint is the normal foreground; the tint colour
  itself is used for icons, bars and at most a short label.
- Colour never carries meaning alone (1.4.1): a group tint always comes with the group icon
  and name; success, warning, info and destructive keep their own tokens and always have an
  icon or word (design.md 2.7 rule, unchanged). Known closeness, accepted because of that
  rule: orange (Read) next to amber (warning), teal (Listen) next to emerald (success),
  blue (Play) next to sky (info). Tints are never used on alerts or status dots.
- Not coloured: page background, cards, borders, body text, field borders (zinc-500 stays,
  1.4.11).

To check when building (cannot be done read-only): every place that uses `bg-primary` as
"selected" or `text-primary` as plain dark text becomes violet; a sweep with screenshots in
both themes is part of the package.

Alternative: accent only (violet, no group tints): two keys per theme, cost S. Or keep the
neutral default and ship the colours as a second brand folder the owner switches to.

### 3.2 Wide screens (OT-14)

Today the top bar, the status line and `main` are capped at `max-w-6xl` = 72 rem = 1152 px
(`crates/web/src/shell.rs:358`, `:404`, `:825`). design.md 2.9 says 80 rem for app pages
(`docs/design/ux/design.md:345-346`) and the mockups use 80 rem
(`docs/design/ux/mockups/log.html:82`, `:102`). On a 2560 px screen 1152 px is 45% of the
width. The poster grid already fills whatever it gets
(`crates/web/src/library.rs:903`, `auto-fill, minmax(8.5rem, 1fr)`), so widening needs no
grid change.

Proposal, max width per page type:

| Page type | Pages | Max width |
|---|---|---|
| Grid and dashboard | Home, Library, Lists, People, Stats | 96 rem (1536 px) |
| Record and stream | Item page, History, list page, person page | 80 rem (1280 px) |
| Settings and admin | Settings, Admin (side nav plus content) | 80 rem |
| Forms, wizard, auth, prose | sign-in, setup, notes, descriptions | 36-52 rem / `max-w-prose` (unchanged) |
| Top bar | all | 96 rem, so it lines up with the widest page |

Rules: more columns, not bigger cards (posters keep 8.5-11 rem); text lines stay at
`max-w-prose`; the 1 rem gutter stays at every width. Implementation: the shell reads a page
width class from the route (two values), cost S. The owner called this "not important";
it can ride along with the colour package.

### 3.3 Button sizing rule for card actions (OT-12)

Cause on the People cards: the footer mixes a hand-styled link and two buttons. "History"
is an `<a>` with `min-h-9 px-3 text-sm` (36 px high, 14 px text,
`crates/web/src/people.rs:387-390`); "Invite" and "Log with..." are `ButtonSize::Sm`
(`people.rs:333`, `:363`) = `h-8 px-3 text-xs` (32 px, 12 px text,
`crates/ui/src/button.rs:48`). Hence two heights and two font sizes in one row.

Rule (for every card in the app):

1. All actions in one card footer have the same size. Default: `Sm` (32 px high). On
   pointer-coarse or below 640 px they grow to 40 px height (`max-sm:h-10`); 24 px minimum
   is met either way (2.5.8), and a card action is not the page's primary control (D10).
2. A link that looks like a button uses the same class source as `Button`: add a
   `ui::ButtonLink` (an `<a>` with the `Button` variant and size classes) and forbid
   hand-written button classes in `web` (a grep test like the existing ones).
3. At most one filled (primary) button per card (D1); the rest are `Outline`; the overflow
   trigger is the icon size matching the row (`size-8` next to `Sm`).
4. Same order on every card: navigation link first, then actions, overflow last.

Fix for OT-12 itself: "History" becomes `ButtonLink size=Sm variant=Outline`. Cost S.
The same hand-styled pattern exists in the discard prompt (`log_sheet.rs:132-133`); the
sweep should replace it too.

## 4. Stats (OT-17)

### 4.1 Today

- Home "All-time stats": one card per group with a bar chart of entries by status (planned,
  in progress, completed, paused, dropped) and below it the amounts as small grey text
  (`crates/web/src/home.rs:354-372`, `:377-403`).
- Stats page: the totals row starts with Entries, Completed, In progress, then the group's
  measures (`home.rs:529-551`). The measure order puts time last and in minutes: Watch =
  Finished, Episodes, Minutes; Play = Plays, Minutes; Read = Pages, Completions
  (`crates/web/src/charts.rs:70-78`); `stats-measure-minutes = Minutes` (`en.ftl:614`).
- So the numbers the owner wants exist but are the least visible ones, and they are per
  group (Watch = movies + shows + anime together), not per kind.

### 4.2 Proposed layout (Home block and top of the Stats page)

Row 1, hero tile (full width on phones, half width from 768 px):

- Icon: Lucide `piggy-bank`, 40 px, decorative (`aria-hidden`), on `primary-soft`.
- Number: total hours, large (`text-display`), for example "1,284 hours".
- Sentence under it: "That is 53 days and 12 hours of movies, shows, listening and play."
- Label above: "Time banked" (de meaning: "Angesparte Zeit"). No pace wording, no per-week
  rate, no target (R-04 is about goals; the tile stays a plain total anyway).

Row 2, four tiles, time or pages first, the count second and small:

| Tile (group tint) | Big number | Small line |
|---|---|---|
| Movies (Watch) | 312 h | 164 movies watched |
| Shows (Watch) | 742 h | 1,930 episodes |
| Books (Read) | 18,420 pages | 61 books finished |
| Games (Play) | 230 h | 88 plays |

Further kinds with activity (anime, podcasts, audiobooks, music, comics, manga, visual
novels) appear as a compact list "Also: 41 h audiobooks, 12 h podcasts, 3,100 pages manga"
under the tiles, only when not zero. The total in the hero is the sum of all hours; pages
are not converted into hours (say so once: "Reading is counted in pages.").

Row 3 (Stats page only): "Hours per month" column chart, one per group tab, hours instead
of minutes; then the existing charts (episodes, pages, plays).

Moved down: entries by status become one line per group under a "Library" heading
("Watch: 212 entries, 150 completed, 14 in progress, 48 planned") without a chart on Home;
the status bars stay on the Stats page behind a "Library by status" disclosure
(native details, closed; addendum-1 D4: fewer than half need it).

Density: Home stats block = 1 hero + 4 tiles + 1 line = 6 rows of content, within OD-01
(7 rows).

### 4.3 Data: what exists, what is missing, cheapest way

The cache already has what the first step needs. `stats_months` holds per user, kind and
month: `finished`, `episodes`, `pages`, `plays`, `seconds`
(`crates/store-pg/migrations/0023_stats_cache.sql:26-38`). The read folds these rows into
groups (`crates/app/src/stats.rs:255-279`, `group_of` at `:261`, `:279`), and the DTO has
only groups (`crates/contracts/src/browse.rs:317-335`). So per-kind totals need no new SQL
and no migration: keep the kind when folding and add `kinds: [{kind, amounts}]` to
`StatsTotalsDto` (additive).

| Number | Source today | What is missing | Cheapest fix |
|---|---|---|---|
| Hours of movies | each `finished` event x the item's `runtime_minutes` (`crates/store-pg/src/repos/stats_cache.rs:263-264`; from TMDB, `crates/connectors/src/tmdb.rs:289`) | Items without a runtime (added by hand, stubs, runtime 0) count as 0 h without any hint | Step 1: footnote "Titles without a known length count as 0". Step 2 (migration): a column `untimed` (events without a length) in both cache tables, shown as "12 movies without a length" with a link to them |
| Hours of shows | each `episode` event x the show's one `episode_runtime_minutes` (`stats_cache.rs:265-267`), a series average (`tmdb.rs:320` first value of `episode_run_time`; `crates/connectors/src/tvdb.rs:393` average) | Per-episode runtimes are stored (`catalog_episodes.runtime_minutes`, `crates/store-pg/migrations/0027_catalog_episodes.sql:15-24`; filled at `tmdb.rs:370`, `tvdb.rs:453`) but not read by stats: DEC-450 item (1) | Pull DEC-450 (1) forward: in the statement `LEFT JOIN catalog_episodes` on (item, season, episode) and use `COALESCE(episode runtime, series average, 0)`; one small migration with a dirty trigger on `catalog_episodes` (same shape as the item trigger, `0023_stats_cache.sql:158-163`). The same CASE exists three times (`stats_cache.rs:262-270`, `crates/store-pg/src/repos/library.rs:420-422`, `crates/store-pg/src/repos/goals_groups.rs:242-244`) and once in memory (`crates/app/src/stats.rs:133-136`); all four must change together (a Postgres test compares them, DEC-235) |
| Pages read | sum of forward page moves (`stats_cache.rs:260`; `crates/domain/src/progress.rs:187-190`) | A book finished without a known `page_count` adds no pages (DEC-233: "Finished" logs the last page only when the total is known) | No change now. Later: ask for the page count once when "Finished" is pressed on a book without one |
| Books finished (small line) | `completions` per kind | nothing | per-kind DTO |
| Hours played | each session event x `sessions.duration_minutes` (`stats_cache.rs:235`, `:269`) | The duration is almost never filled because the field is hidden (OT-16, section 2.4) | Section 2.4 (web only). No estimate from the catalogue's typical play time: an invented number would sit next to logged ones |
| Listen hours | forward moves of the position in seconds (`stats_cache.rs:268`); podcasts via episodes x runtime | DEC-211: nothing for anime and podcasts unless the item has the attribute | none now |
| Total hours (hero) | sum of `seconds` over all kinds | nothing | per-kind DTO, or `overall.amounts.minutes` which exists (`browse.rs:311`) |

Order of work: (a) per-kind totals in the DTO and the new tiles: no migration, S-M;
(b) duration field open (section 2.4): S; (c) per-episode runtimes: M, one migration plus a
cache rebuild for everyone (the zone stamp mechanism in `stats_users` already forces a
rebuild, `0023_stats_cache.sql:46-53`; whether it can be reused to trigger this one was not
checked); (d) the `untimed` coverage column: M, optional.

### 4.4 Icon

`piggy-bank` exists in the vendored Lucide version: the project pins `lucide-static` 1.69.0
(`config/brand/ICONS.adoc:17-20`), and
https://unpkg.com/lucide-static@1.69.0/icons/piggy-bank.svg answers 200 with the header
`@license lucide-static v1.69.0 - ISC` (fetched 2026-11-10). It is not in the repo yet: the
folder `crates/ui/icons/vendor/lucide/` has no `piggy-bank.svg` (among the files there:
`clock.svg`, `timer.svg`, `trophy.svg`), and `crates/ui/icons/icons.toml` has no entry
(`:63` timer, `:74` clock, `:75` repeat are the nearest). To add it (rule 11, same steps as
DEC-183): copy the file unmodified from the 1.69.0 tarball, map it in `icons.toml`, add the
`IconName` variant, add the name to the list in `config/brand/ICONS.adoc:31-35`.

### 4.5 Text equivalents

| Visual | Text equivalent |
|---|---|
| Hero tile | It is text: label, number, sentence. The icon is `aria-hidden`. The tile is a `<section>` with the label as heading |
| Four kind tiles | A `<dl>`: `<dt>` "Movies", `<dd>` "312 hours", second `<dd>` "164 movies watched". Tint and icon are decoration |
| "Also" line | A plain sentence |
| Hours per kind as bars (Stats page, optional) | `BarList` already takes a one-sentence summary as the accessible name (`crates/web/src/charts.rs:211-217`): "Hours by kind: shows 742, movies 312, games 230, audiobooks 41." The tiles above repeat the numbers as text |
| Hours per month columns | As every chart today: summary sentence as the name (`stats-chart-summary`, `en.ftl:596`: "Hours in Watch, Jan 2026 to Nov 2026: 214 in total, most in Mar 2026.") plus the "Show as a table" disclosure (`home.rs:672`, `en.ftl:605`) with Month and Hours columns |
| Library by status bars | Summary sentence (`stats-totals-summary`, `en.ftl:598-603`) plus the line of numbers, as today |

New keys: `stats-hero-label` ("Time banked"), `stats-hero-hours` (plural: "{ $count }
hours"), `stats-hero-days` ("That is { $days } days and { $hours } hours of ..."),
`stats-tile-movies`, `-shows`, `-books`, `-games`, `stats-also`, `stats-measure-hours`
("Hours", replaces `stats-measure-minutes` in the UI), `stats-count-hours`,
`stats-untimed-note`, `stats-reading-note`, `stats-library-heading`. German meanings:
"Angesparte Zeit", "{ $count } Stunden", "Das sind { $days } Tage und { $hours } Stunden
...", "Filme", "Serien", "Buecher", "Spiele", "Ausserdem", "Stunden". Numbers go through
the locale formatter (`i18n.number`); hours with one decimal below 10, whole above.

## 5. Decision table

Cost: S = up to a day, M = a few days, L = a week or more (one agent, test-first, with
review). Packages: server (app / api / contracts / store), web, migration, i18n.

| OT | Proposed default | Alternative | Cost | Packages |
|---|---|---|---|---|
| OT-4 | Two buttons on a not-yet-added item: "Mark as watched" and "Want to watch" (= add as Planned); one shared toast "Added to your library as Planned" with Undo on item page and palette | Keep the single "Add to library" button and only add the toast and a hint line "Adds it as Planned" | S | web, i18n; server S if `next_step` must be sent without an entry |
| OT-5 | Header pill with the status and a fact line; action labels become imperatives ("Mark as watched", "Log a rewatch", "Mark S2 E5 watched"); toasts keep the past tense | Keep the labels, add only the pill | S | web, i18n |
| OT-7 | "Watched 3 times, last on <date>" in the header; "Rewatches" row removed; sheet line "This will be watch number 3" | Keep "Rewatches" but show it in the header instead of behind "More facts" | S | web, i18n; contracts S (`times`) |
| OT-9 | Model A: status = where you are now, history = every time. Movies stay Completed. Shows: explicit rewatch run (In progress while it runs, Completed when finished or stopped), derived from `status_change`, no migration | Model B: no run; rewatched episodes are extra views, status untouched | M (B: S) | server (app, contracts), web, i18n; no migration |
| OT-15 | Bulk marks on already watched episodes ask inline "Log these N again as a rewatch?"; mixed ranges say how many were skipped | Disable the menu items on fully watched ranges with a hint "All watched. Use Again on a row." | S | web, i18n (server only with the run of OT-9) |
| OT-6 | Key family `log-with-{watch,listen,read,play}`: "Watched with..." on the button, as people legend; dialog title = the step | Only rename the button to "Watched with..." | S | web, i18n |
| OT-10 | Centered dialog from 640 px (32 rem watched, 42 rem play, max height with scroll, footer), bottom sheet below 640 px; fix the stretching grid | Keep the side sheet, add `content-start` only | S-M | web (`ui` dialog), e2e updates |
| OT-16 | Duration open in row 1 of the play dialog, accepts `1:30` or `90`, prefilled from the last play; scores behind "More" for video games | Only move the field out of "More" | S | web, i18n |
| OT-11 (display side) | Activity sentence with duration, scores and winner; "Plays" tab on game pages; hours and win rate on the Stats Play tab | Sentence only | M | server (contracts, api, app activity), web, i18n |
| OT-12 | One size per card footer (`Sm`), `ui::ButtonLink` for links that look like buttons, at most one filled button per card | Fix only the People "History" link classes | S | web (`ui`) |
| OT-13 | Violet accent plus four group tints as `[theme]` / `[theme_dark]` values in the default brand file, neutral fallbacks in the stylesheet, start-up contrast check extended | Accent only; or the colours as a second brand folder | M | web (css, components), brand config and crate; no migration |
| OT-14 | 96 rem for grid pages and the top bar, 80 rem for record pages, forms unchanged | Just raise the cap to the design's 80 rem everywhere | S | web |
| OT-17 | Hero tile (piggy-bank, total hours) and four time-first tiles from per-kind totals; status counts move down; then per-episode runtimes (DEC-450 (1)) | Reorder only: hours first within today's group cards, no per-kind split | M (step 1), M (per-episode runtimes) | server (app stats, contracts), web, i18n, icon vendoring; migration only for per-episode runtimes and the optional `untimed` column |

Suggested cut for pack "ot2": first OT-4, 5, 6, 7, 12, 15, 16 (all S, web and i18n only,
together they change how the item page reads); then OT-10 and OT-17 step 1; then OT-13
with OT-14; the rewatch run (OT-9 model A rule 6), OT-11 display and per-episode runtimes
as separate packages after the owner's decision.

## Not verified

- Nothing was rendered or run. The explanation of the side sheet's spacing (2.2) and the
  cause of the uneven People buttons (3.3) are read from the classes in the code.
- Whether the server sends `next_step` for an item without a library entry (1.4), and how
  wide the poster pill is with the longer imperative labels (1.3).
- The rewatch run (1.2 rule 6, 1.4) is a design, not a checked implementation plan: its
  effect on sync clients, the offline queue stamps (DEC-455) and the importers was not
  traced.
- How many seasons actually have rows in `catalog_episodes` on a real instance (4.3), and
  whether TMDB's series-level `episode_run_time` is often empty (remembered, not looked up).
- Whether the stats cache has a ready way to force a rebuild after the statement changes
  (4.3 (c)).
- The colour numbers are computed here from the published oklch values with a small
  script, not with the brand crate's own `contrast.rs`; they should be re-run through it in
  the package. The closeness of group tints to the semantic colours (3.1) was judged by hue
  values, not seen on screen.
- The Lucide icon was checked on unpkg (URL in 4.4), not in the 1.69.0 tarball the repo
  cites; `hourglass` and `users-round` exist there too if wanted.
- The round-1 reviews (on their own branch) and `docs/design/DESIGN.md` were not read for this
  pass; design.md, addendum-1, the mockup `log.html`, DECISIONS (status, rewatch, stats
  entries) and the code named above were.
- Costs are estimates.
