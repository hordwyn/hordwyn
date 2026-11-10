# Owner answers from the round 2 review page, 2026-11-10 (F9)

Status: COMPLETE. Docs only; nothing was built or run.

Sources:

- The answers: 64 JSON files, one per decision id, written by the review page between
  16:48 and 17:21 UTC on 2026-11-10 (`choice` = option letter or `other`, `note` = the
  owner's words). All 64 decision items of the page are answered.
- The letters are resolved against `docs/fable/round2/review-page.html` (array `ITEMS`,
  lines 161-513; option `a` is the first entry of `o`, and so on; `other` is the page's
  "Something else (see my note)").
- Background per item: `docs/fable/round2/owner-review-page.md` (F1) and
  `docs/fable/round2/ux-owner-test.md` (F2).
- The owner's words on 2026-11-10: "Answered all open questions. Didn't go through
  decisions".

How to read this file:

- Sections 1 to 10 list every answered id: question, the chosen option in words, the
  owner's note verbatim (in quotes; "-" when the note is empty), and the consequence.
- Section 11 holds the notes that need interpretation (HUB-2, HUB-4, SEC-3 with the review
  the owner asked for, SEC-5 with the answer to the owner's question, DATA-R, OPS-3+4,
  OWN-1).
- Section 12 lists the 53 "proceed with defaults" items: **not owner-reviewed**, in force
  under rule 2 until the owner reconsiders one, exactly like the DEC entries in
  `docs/HANDOFF.md` 4.5.
- Section 13 is the list of questions back to the owner. Section 14 says what could not be
  verified.
- DEC entries for the rulings that change or reverse an earlier decision are DEC-530 to
  DEC-552 in `docs/DECISIONS.md`. The work packages are in
  `docs/fable/round2/next-packages.md`.

Marks: **[=default]** the owner confirmed the page's default; **[changed]** the owner chose
something other than the preselected or review default; **[was open]** the page had no
default; **[other]** free text.

Tally: 64 answered; 3 `other` (HUB-4, SEC-3, and OPS-3+4 with an empty note); 7 notes
(HUB-2, HUB-4, HUB-7, SEC-3, SEC-5, DATA-R, OWN-1); choices that differ from the page's
default or settle a contested item: HUB-2, HUB-7, HUB-12, SEC-4, WP-B2, WP-OD8, DATA-R,
OWN-1, OWN-4; items without a default now decided: SEC-3, OPS-I, TEST-1, C10, OWN-7, OWN-8,
OWN-9, OWN-10.

## 1. Hub

**HUB-0 Build the hub, and when** [=default]
- Question: Do we build the hub as designed, and does H1 (read-only hub plus Wikidata seed)
  replace workstream W3 "board games on Wikidata"?
- Answer (a): H1 and H2 now, the reference hub open to the owner's own instances only,
  third parties after H5.
- Note: -
- Consequence: the hub is built. H1 (hub role, read API, `hub` connector, Wikidata seed) and
  H2 (peer registration, signed requests) become work packages; W3 is replaced by H1. The
  rules of DEC-192 to DEC-198 (Wikidata as the CC0 seed, etiquette, Commons images, the
  per-value source marker) stay and are carried out by the hub instead of an instance-level
  connector. The owned flag and "Mark owned" (DEC-420) come with H1. Nothing is opened to
  third parties before H5. DEC-531.

**HUB-7 BGG connector** [contested; the owner took the review default]
- Question: Do we drop the planned BGG connector for good and keep BGG only as an
  identifier plus the importers for the owner's own collection and plays?
- Answer (a): do not build it; BGG becomes "identifier only plus importers".
- Note: "I may reverse it once we have a valid token."
- Consequence: reverses the ruling of 2026-11-07 "keep BGG for those with a token"
  (`docs/HANDOFF.md` 4.6, "Board games direction") and the "optional enrichment connector"
  part of DEC-193; the terms record DEC-190 stays as it is, because the owner keeps the
  reversal open. Not built: the connector, the "From BoardGameGeek" panel, the purgeable BGG
  store. Kept: the BGG id as a plain identifier (HUB-5 a) and the BGG collection and plays
  importers (the sample export is still needed, OWN-3). The deliveries "BGG token" and
  "Powered by BGG logo" are no longer waited for; the BGG application is left running
  (do not withdraw it). `docs/connectors/bgg.md` gets the line "not built; identifier and
  importers only; the owner may reverse this once a token exists". The attribution page
  needs no BGG entry. DEC-530.

**HUB-1 Data licence for hub facts** [=default]
- Question: Under which licence does the hub publish its structured data?
- Answer (a): CC0 1.0.
- Note: -
- Consequence: `GET /meta` of H1 states CC0 1.0; the contributor terms (H3) and the licence
  tag per field (WP-OD2) are written for it; only CC0-compatible values may be proposed to
  the hub (so TVmaze and Wikipedia text, both CC BY-SA, and every TMDB, IGDB, MAL or BGG
  value never go there). Practically irreversible after the first third-party contribution.

**HUB-2 Image licence** [changed, with a note] - see 11.1
- Question: Which licences may an uploader choose for an image, and is the revocable
  operator licence the owner asked for offered?
- Answer (b): CC BY 4.0 and CC0 only (no bespoke text, no lawyer needed for the clause).
- Note: "Not sure I got this right. I didn't know there were irrevocable licenses. It was my
  unknowing. Fable suggestion may be used"
- Consequence: the revocable operator licence ("model B") is not built in H4; the draft
  clause in the hub design is shelved, not deleted. The owner's earlier wish for a revocable
  licence is withdrawn. Interpretation and the remaining open point: 11.1. DEC-532.

**HUB-4 Movies and shows on the hub** [other] - see 11.2
- Question: Does the hub carry movies and shows?
- Answer: other (none of: no in v1 / a Wikidata-id bridge only / full kind).
- Note: "I would like a full feature hub. Including everything we do. I am okey with
  starting without movies and shows."
- Consequence: the target changes from "board games first, a few kinds, never movies and
  shows" to "every kind the product tracks, movies and shows last". Interpretation, what
  that means against the CC0 licence, and the open points: 11.2. DEC-532.

**HUB-3 Contributor identity** [=default]
- Question: Must a person have a hub account to contribute?
- Answer (a): no; contributions are attributed to the instance with a pseudonymous subject
  hash, a personal hub account is optional.
- Note: -
- Consequence: H3 change model without mandatory accounts; account handling stays in H5.
  The subject hash must follow the rule of SEC-3 (11.3): an identifier that can be cut from
  the person.

**HUB-5 Restricted identifiers on the hub** [=default]
- Question: May the hub store BGG, IGDB and MAL ids as plain identifiers when they come from
  Wikidata or are typed by a contributor?
- Answer (a): yes for BGG now; IGDB and MAL only after the Wikidata properties are verified.
- Note: -
- Consequence: the BGG id is a hub field from H1; IGDB and MAL ids wait for the property
  check in H7.

**HUB-6 Hashes of purged TMDB text** [=default]
- Question: May an instance keep salted hashes of TMDB text it has purged, for 180 days, so
  it can refuse proposals that retype that text?
- Answer (a): yes, under the same six-month limit, and ask the lawyer whether a one-way hash
  is "information obtained from TMDB".
- Note: -
- Consequence: part of H3 (the provenance filter); one more question for the lawyer
  (HUB-11). Until the lawyer answers, the hashes are kept no longer than the retention limit.

**HUB-9 Registration at launch** [=default]
- Question: Who can join the reference hub at launch?
- Answer (a): personal accounts by invite, instances by admin approval.
- Note: -
- Consequence: config defaults `peer_registration = approve` (H2) and invite mode (H5).

**HUB-10 Reviewers** [=default]
- Question: Who reviews contributions?
- Answer (a): the owner is the first moderator; an admin appoints trusted contributors after
  nomination.
- Note: -
- Consequence: no election machinery in H3; the owner's time is the review capacity.

**HUB-11 Notice-and-action and the lawyer** [=default]
- Question: Do we build the full report, decision and appeal flow (DSA Art. 16/17 shape),
  and does the owner engage a lawyer for the hub terms?
- Answer (a): full flow plus lawyer.
- Note: -
- Consequence: H5 is built in full. Owner action: engage a lawyer; three inputs are needed
  (contributor terms before H3 opens to third parties, the image terms before H4, the DSA
  scope), plus the two points raised by other answers: HUB-6 (hashes) and OWN-1 (title and
  year kept after a TMDB purge), and for HUB-4 the question of movies and shows on a CC0 hub.

**HUB-12 First monetisation step (low priority)** [changed]
- Question: If money is ever taken, what is sold first?
- Answer (d): decide later (nothing is built for it now).
- Note: -
- Consequence: nothing is built; the review default (hosted instances plus donations) is
  not adopted. No DEC needed (no earlier decision is changed).

**HUB-13 Staging hub** [=default]
- Question: Is there a second small instance as a staging hub before third-party instances
  connect?
- Answer (a): yes, a second instance that the owner hosts.
- Note: -
- Consequence: H6 gets the staging flag; the owner runs one more instance before H3 opens.

**HUB-15 Open Products Facts as a barcode source** [=default]
- Question: Do we also query open product databases by barcode for board games?
- Answer (a): no; revisit if contributors ask.
- Note: -
- Consequence: closes the open question in `docs/HANDOFF.md` 4.6 ("offered, not answered")
  and the item in section 6. No connector.

## 2. Security

**SEC-1 Security headers and cache directives** [=default]
- Question: Does the app set the security headers itself, including a Content Security
  Policy, or do we leave them to the reverse proxy?
- Answer (a): all headers in the app, including CSP with a nonce for the hydration and theme
  scripts, plus a reverse-proxy docs section.
- Note: -
- Consequence: package sec1 (M): headers and CSP in the server, cache directives and hashed
  bundle names (OPS-8 is done with it), every browser test must stay green under the CSP.

**SEC-3 Deleting a user leaves personal data behind** [other; the page had no default] - see 11.3
- Question: When an account is deleted, do audit rows lose the username at once, or are they
  kept for the 365-day audit retention (OD-13)?
- Answer: other.
- Note: "Audit rows don't need the name. It's fine to keep an identifier that is never stored
  anywhere else. That means I remove the user from the identifier list which means all the
  other data can't be retraced to that user. A fable agent may review this. It should be the
  normal flow for this globally"
- Consequence: audit rows carry no name at all, only an identifier; deleting the account
  deletes the one row that maps the identifier to a person. The owner wants this as the
  general pattern, not only for the audit log. The review the owner asked for, what must
  hold and what breaks: 11.3. DEC-533.

**SEC-4 Outbound requests to admin-chosen hosts** [contested; changed against the audit's default]
- Question: May an admin point OIDC and SMTP at hosts on private networks?
- Answer (b): allow private targets in home mode by default, block in hosted mode.
- Note: -
- Consequence: the audit's fix "block everywhere unless a switch is on" is not taken as
  written; the recommendation of the review page (SEC-4a: a home identity provider or mail
  relay is usually on a private address) is. Built in package sec2: no redirects on the
  governed client (or a per-hop allowlist check), categorised errors for discovery failures,
  and a private-range filter for OIDC endpoint hosts and the mail host whose default follows
  the hosting mode (off in home, on in hosted); the existing switch
  `notify.allow_private_targets` (`crates/config/src/lib.rs:381`, default false at `:391`)
  stays the explicit override. The owner's home setup keeps working after the upgrade
  without a new setting. DEC-534.

**SEC-5 Sign-in throttling** [=default, with a question] - see 11.4
- Question: Should repeated wrong passwords slow an account down instead of locking it, and
  should the limits be per client address and shared between processes?
- Answer (b): per-address budget, concurrency cap and progressive delay, with the counters
  in Postgres (correct for several web processes).
- Note: "Does it make sense to do that much login logic if we use an idm anyway?"
- Consequence: the hard lock (10 failures / 15 minutes per name) is replaced by a
  progressive delay; OD-17's "keep the throttle" on the break-glass sign-in is kept in
  substance but its behaviour changes. The answer to the owner's question and a proposed
  reduction: 11.4. DEC-535.

**SEC-6 Compose defaults** [=default]
- Question: May "compose up" refuse to start until the passwords are set, and bind to
  127.0.0.1 only?
- Answer (a): required variables, loopback bind, a start-up warning, a "Behind a reverse
  proxy" page.
- Note: -
- Consequence: compose and `.env.example` change; the quick start becomes "copy the example
  env file, set two values, start". Part of package ops1 with OPS-1, OPS-2, OPS-7.
  DEC-536.

**SEC-10 Session lifetime and session list** [=default]
- Question: Do sessions get an absolute maximum age, and do members get a list of their
  sessions with "sign out everywhere"?
- Answer (a): both: a hard cap (for example 90 days) and a session list in Settings >
  Account.
- Note: -
- Consequence: a second session value editable in the admin UI under the OD-11 rule (env
  override shown read-only), a new block in Settings > Account inside the density budget
  (OD-01), en/de strings. Closes the deferred "revoking other sessions" (C5). DEC-536.

## 3. UX from the owner test (all option a, no notes)

The fix pack ot1a already built the minimum of some items (uncommitted at the time of
writing, `docs/fable/round2/fix-pack-ot1a-progress.md`); where the owner's answer goes
further than ot1a, the DEC of ot1a is superseded or its deferral is now due.

**OT-4 What "Add to library" does** [=default]
- Answer (a): two buttons on an item not yet in the library, "Mark as watched" and "Want to
  watch" (adds as Planned); one shared toast "Added to your library as Planned" with Undo,
  on the item page and in the palette.
- Consequence: pack ot2a. The palette's Ctrl+Enter hint (DEC-426, "Add as Planned or next
  step") gets the same toast.

**OT-5 Finding the Watched button; states against actions** [=default]
- Answer (a): a header pill with the status and a fact line; action labels become
  imperatives ("Mark as watched", "Log a rewatch", "Mark S2 E5 watched"); toasts keep the
  past tense.
- Consequence: pack ot2a; en/de keys per kind verb.

**OT-7 Rewatch count wording** [=default]
- Answer (a): "Watched 3 times, last on <date>" in the header; the "Rewatches" row is
  removed; the log sheet says "This will be watch number 3".
- Consequence: pack ot2a; one small contract field (`times`).

**OT-9 Status model and rewatches** [=default]
- Answer (a): model A. Status is where you are now, history counts every time. Movies stay
  Completed. A show rewatch is an explicit run: In progress while it runs, Completed when
  finished or stopped. No migration.
- Consequence: own package ot2e (M: app, contracts, web, i18n). The page's flag still
  holds: the effect of the run on sync clients, the offline queue and the importers was not
  traced and is the first task of that package. DEC-537.

**OT-15 Bulk marks on already watched episodes** [=default]
- Answer (a): ask inline "Log these N again as a rewatch?"; mixed ranges say how many were
  skipped.
- Consequence: supersedes the choice of DEC-501 (a status line that only says "already
  watched"). The web part is in ot2a; the server part (the rewatch run) is ot2e. DEC-538.

**OT-6 "With..." becomes the kind's verb** [=default]
- Answer (a): a key family per verb, "Watched with...", "Listened with...", "Read with...",
  "Played with..." on the button and as the people legend; the dialog title is the step.
- Consequence: pack ot2a; en/de.

**OT-10 Centered dialog or side sheet** [=default]
- Answer (a): a centered dialog from 640 px (32 rem for watched, 42 rem for play, scrolls
  inside, fixed footer), a bottom sheet below 640 px; the stretching grid is fixed.
- Consequence: pack ot2b; changes the approved log-sheet layout of design.md (side sheet)
  by the owner's own wish (OT-10). The browser tests that open sheets change. DEC-539.

**OT-16 Play time on a game** [=default]
- Answer (a): duration is open in row 1 of the play dialog, accepts 1:30 or 90, prefilled
  from the last play; scores move behind "More" for video games.
- Consequence: ot1a moved the field (DEC-498); the three parts DEC-498 left for the owner
  are now decided and go into ot2a: `h:mm` input, prefill, scores behind "More" for video
  games.

**OT-11 Where score and win show up (display side)** [=default]
- Answer (a): an activity sentence with duration, scores and winner; a "Plays" tab on game
  pages; hours and win rate on the Stats Play tab.
- Consequence: ot1a built the sentence and the item History rows (DEC-502). Still to
  build (ot2f): the "Plays" tab and the Stats Play tab figures.

**OT-12 Button sizes on cards** [=default]
- Answer (a): one rule: one small size per card footer, a link-as-button component for links
  that look like buttons, at most one filled button per card.
- Consequence: ot1a built `ui::ButtonLink` and fixed the People card (DEC-497); the sweep
  over the other hand-styled links and the card-footer rule, which DEC-497 left out, go
  into ot2a.

**OT-13 Colour** [=default]
- Answer (a): a violet accent plus four group tints as theme values in the default brand
  file, neutral fallbacks in the stylesheet, the start-up contrast check extended.
- Consequence: pack ot2d. Changes the KICKOFF default Q8 "neutral palette" for the default
  brand; white-label holds because the colours live in `config/brand/default` and the
  stylesheet keeps neutral fallbacks (rule 9). The contrast numbers of the UX pass must be
  re-run through the brand crate's own check, and the result must be seen on screen in both
  themes (the page's flag). DEC-540.

**OT-14 Wide screens** [=default]
- Answer (a): 96 rem for grid pages and the top bar, 80 rem for record pages, forms
  unchanged.
- Consequence: supersedes DEC-496 (80 rem everywhere, set by ot1a "until the owner
  decides"). Pack ot2d. DEC-541.

**OT-17 Stats led by time** [=default]
- Answer (a): a hero tile (piggy-bank icon, total hours) and four time-first tiles from
  per-kind totals; status counts move down; then per-episode runtimes as a second step.
- Consequence: step 1 is ot2c (no migration; the Lucide `piggy-bank` icon is already
  vendored in the working tree by ot1a's preparation). Step 2 (per-episode runtimes, a
  migration) pulls DEC-450 item (1) forward and takes its runtimes from TVmaze (WP-OD3 a),
  so it follows the TVmaze package. Amends the ruling "Home stats: all-time totals per
  group" (`docs/HANDOFF.md` 4.2) in what leads the block. DEC-542.

## 4. Operations

**OPS-3+4 Where the secrets key lives, and the backup reminder** [other, empty note] - see 11.6
- Question: Do we move the secrets key out of the data directory and add a "back up this key
  now" step or banner after the setup wizard?
- Answer: other, and the note is empty.
- Note: -
- Consequence: no ruling can be read from it. Question back to the owner (13, Q1). Until
  answered only the part common to every option is done: the backup runbook (OPS-9) says
  where the key is and that it must be backed up separately. No code change.

**OPS-6 Rollback policy** [=default]
- Answer (a): yes, dump-restore only, documented with "take the dump first"; log the
  migrations that will run; add a migration dry run.
- Consequence: runbook text plus two small server changes (log, dry run) in ops1.

**OPS-12 More than one web process** [=default]
- Answer (a): the supported shape is one web process plus any number of workers,
  documented; throttles stay per process.
- Consequence: a docs statement and a commented `web` / `worker` pair in compose. Note the
  tension with SEC-5 b (counters in Postgres), discussed in 11.4.

**OPS-I Publish images** [was open]
- Question: Do we publish tagged container images from CI, and where?
- Answer (b): a public registry, on tags.
- Note: -
- Consequence: the first public distribution of the product. Preconditions from the page
  stay: OPS-1, OPS-2, SEC-1, SEC-2 and SEC-6 done, the attribution page inside the image
  (R-F3), the upgrade and rollback docs (OPS-6), and the migration baseline squashed first
  (OWN-9 a), because a public image is the first external deployment. Not said: which
  registry (13, Q4). DEC-543.

**OPS-B Build profile: bundle size against build memory** [=default]
- Answer (a): adopt the new Dockerfile structure, measure thin against fat link-time
  optimisation (peak memory, size), then the owner picks with numbers.
- Consequence: one measuring package when the build host is free; a follow-up question to
  the owner with the numbers.

## 5. Architecture

**ARCH-0 When does the restructuring happen** [=default]
- Answer (b): quick wins now, the large packages after the fix pack, the UX pack and W1.
- Consequence: WP-A1, WP-A2, WP-C2 early; WP-B3 (split `api`), WP-B2, WP-B1 (split `web`)
  after ot1, ot2 and the attribution page, before the hub adds files to those crates.

**WP-B2 Remove the Backend pass-through** [contested; the owner took the architecture review's side]
- Answer (a): reverse DEC-162: delete the pass-through layers after the API split.
- Consequence: DEC-162 ("rejected for now") is reversed; the decisions review's
  recommendation C6 (keep the layers, record "reads through server functions, writes
  through REST") is not taken. Runs after WP-B3. The reason DEC-162 gave (the desktop shell
  may plug in another backend, `web` stays free of server crates, DEC-092) must be answered
  in the package: the architecture test has to keep `web` free of server crates under
  `hydrate`. DEC-544.

## 6. Performance

**PERF-T Targets and place in the plan** [=default]
- Answer (b): the whole plan as W2, as scheduled.
- Consequence: the targets are accepted (every mixed-set line at most 150 ms p95, Home 90,
  People 40, zone change 400); PERF-0 to PERF-9 run as one workstream after the packs, with
  PERF-5 after the stats redesign (OT-17).

## 7. Tests

**TEST-8 Import job lane** [=default]
- Answer (a): a per-user import lane (a product change), then lower the test timeouts to
  60 s.
- Consequence: changes the single strictly ordered `user_import` lane (DEC-029 lane model;
  `crates/jobs/src/lib.rs:5`, cited by the review page as checked). The package first
  settles the contradiction between the security audit ("already per user") and the code
  read for the review page (one shared lane). DEC-545.

**TEST-1 Flaky tests in CI** [was open]
- Answer (a): fail the pipeline on any flaky test.
- Consequence: a JSON reporter and a failing scan step; TEST-2 (a serial project for specs
  that change instance-wide state) lands first or in the same package, otherwise known
  order dependence turns pipelines red. DEC-545.

## 8. Data sources

**DATA-0 Demote TMDB to optional enrichment** [=default]
- Answer (a): Wikidata identity above every proprietary source, for all kinds.
- Consequence: the largest change to the catalogue since M09: title, year and ids come from
  Wikidata (CC0); TMDB, IGDB and the others become operator-enabled enrichment. Needs the
  licence tag per field (WP-OD2, a default) first or alongside. Search ranking (OT-3) must
  be re-checked with two identity sources. DEC-546.

**WP-OD3 TVmaze as the default show source** [=default]
- Answer (a): TVmaze default, TMDB as enrichment.
- Consequence: shows, episodes and air dates from TVmaze (CC BY-SA 4.0, credit link, never
  proposed to the CC0 hub); the source of per-episode runtimes for OT-17 step 2 and of the
  "Up to date" badge (DEC-450). Terms were recorded in `docs/connectors/tvmaze.md`
  (verified 2026-11-10 per commit b526830). DEC-546.

**WP-OD7 Wikipedia summaries as descriptions** [=default]
- Answer (a): yes, on by default.
- Consequence: a description source under CC BY-SA 4.0 with a credited link; never exported
  to the hub; depends on DATA-0. DEC-546.

**DATA-K Order of the new connectors** [=default]
- Answer (a): Open Library plus DNB (books), then MusicBrainz plus Cover Art Archive and the
  ListenBrainz importer, then VNDB.
- Consequence: the order of W5. Open Library terms are recorded
  (`docs/connectors/openlibrary.md`, commit b526830); VNDB is free for non-commercial use
  only, which matters once anything is sold (HUB-12 is "decide later").

**WP-OD8 Audiobook editions as a hub kind** [contested; the owner took the data review's side]
- Answer (a): add after H3, seeded from MusicBrainz audiobook release groups.
- Consequence: the hub design's "not needed" for books and audiobooks is overruled; fits
  the HUB-4 note (every kind). Depends on H1 to H3 and the MusicBrainz connector. DEC-532.

**WP-OD9 Comics: licence pages (a delivery)** [=default]
- Answer (b): not now; no comics connector until then.
- Consequence: comics stay without a connector; nothing waits on the owner here.

**DATA-R Confirm the "never build" list** [changed, with a note] - see 11.5
- Answer (b): re-check some (named in the note).
- Note: "Steam web API. Steamdb.info, maybe protondb for Linux compatibility."
- Consequence: the list is confirmed except the Steam Web API, which moves from "never
  build" to "check the terms, then decide"; SteamDB and ProtonDB are two new candidates
  that were not on any list. Terms first (rule 5 context: `docs/connectors/<id>.md` before
  any code). Interpretation and what is already known: 11.5. DEC-547.

## 9. Decision log

**DLOG-1 Split the decision log into three files** [=default; includes a rule change]
- Answer (a): three files as proposed: current state, history, backlog.
- Consequence: the page said that (a) "needs your explicit yes" because it moves the log
  and adds Markdown files, against the KICKOFF layout (fixed path `docs/DECISIONS.md`,
  AsciiDoc in `docs/`) and DEC-009. The owner chose (a) with that flag in front of him, so
  the rule change is approved: `docs/decisions/CURRENT.md`, `docs/decisions/DECISIONS.md`
  (history with a status prefix), `docs/decisions/BACKLOG.md`. The migration must run at a
  moment when no other agent appends DEC entries. DLOG-2 to DLOG-6 follow. DEC-548.

**C10 Where personal UI preferences live** [was open]
- Answer (a): a server-side store for all five (shortcuts switch, remembered disclosures,
  remembered group, nudge snooze, episode "ask once").
- Consequence: a `user_preferences` store (migration, contracts, web); closes the five
  "a server preference later" notes (DEC-182, DEC-223, DEC-231, DEC-268, DEC-423 as listed
  by the decisions review) and settles where OD-02 "remember disclosure state" lives.
  DEC-549.

## 10. Open owner items

**OWN-1 TMDB retention** [changed, with a note] - see 11.7
- Question: After six months without a possible refresh, does an item lose its title and
  year too (strict, in force), or are title and year kept as the owner's record?
- Answer (c): strict for TMDB fields, but copy title and year into the library entry at
  purge time so no library ever shows a placeholder.
- Note: "The movie name is technically entered by the user. So is the release year (it's
  chosen by the user for identification) so it's not tmdb data. The user entered it"
- Consequence: DEC-466's clearing of title and year stays for the catalogue item, but the
  person's library entry keeps a title and a year of its own, so the placeholder "Title
  unavailable - source data expired" no longer shows in a library. Uses the reversal path
  DEC-466 names. With DATA-0 a the case shrinks to items without a Wikidata or TVmaze id.
  The page's flag stands: whether this is compatible with the TMDB terms is a lawyer's
  point (added to the lawyer list, HUB-11). Details: 11.7. DEC-550.

**OWN-2 The unreviewed decision entries** [=default]
- Answer (a): review the current-state summary by area and raise only what looks wrong.
- Consequence: depends on DLOG-1 (the summary must exist). Until then all unreviewed
  entries stay in force and not owner-approved (`docs/HANDOFF.md` 4.5).

**OWN-3 Deliveries: sample exports** [=default]
- Answer (a): yes, the owner will deliver them (Goodreads, BG Stats, BGG collection, the
  Trakt "Raw Export" ZIP).
- Consequence: those importers keep waiting for `owner-inbox/`. The BGG token and logo
  files are no longer expected (HUB-7 a).

**OWN-4 Anime and manga sources** [changed from the preselected "not now"]
- Answer (a): yes, the owner will do the four actions (MAL developer agreement and client
  id, Kitsu terms page into the inbox, MangaDex metadata-only yes or no, AniDB rules).
- Note: - (the page asked for the MangaDex answer in the note; it is empty)
- Consequence: W4 waits for the owner's actions; everything stays off until then. MangaDex
  is still unanswered (13, Q7).

**OWN-5 Funnel review date** [=default]
- Answer (a): keep 2026-12-06.
- Consequence: no change to `docs/HANDOFF.md` section 6. If ot2 ships before that date the
  four weeks mix old and new flows; the review has to read the numbers with that in mind.

**OWN-6 Language for people who have not chosen one** [=default]
- Answer (a): accept and record the deviation (the browser's language comes before the
  brand default).
- Consequence: amends the i18n ruling in `docs/HANDOFF.md` 4.1 ("per-user language with the
  brand default as fallback"): the order is the user's choice, then the browser's language,
  then the brand default. DEC-201 is now owner-approved. DEC-551.

**OWN-7 Where sign-up lands** [was open]
- Answer (a): Home, as designed.
- Consequence: the deviation of DEC-189 item 4 (sign-up lands on the library,
  `crates/web/src/auth_pages.rs:481` as cited by the decisions review) is reversed; the
  landing page and the browser tests change. Goes into ot2a, because what a new account
  sees first depends on OT-4 and OT-5. DEC-551.

**OWN-8 Live checks only the owner can run** [was open]
- Answer (a): yes, the owner will run them (authentik and `max_age=0`; compose "healthy
  within 60 s" on a real Docker host; the Tauri spike).
- Consequence: owner action; the first result decides the re-authentication fallback
  (`docs/research/od-12-reauth-window.md` 4.4).

**OWN-9 Squash the migrations into one baseline** [was open]
- Answer (a): squash now; the owner re-imports.
- Consequence: migrations 0001 to 0031 (plus whatever the packages before it add) become
  one baseline; the owner's instance is re-created from an export. Preconditions: the
  product's own export and import must carry everything the owner wants to keep (a
  round-trip test first), and no migration-adding package may be in flight. Must happen
  before the first public image (OPS-I b). DEC-543.

**OWN-10 Rotate the TMDB token** [action]
- Answer (b): done.
- Consequence: the thread in `docs/HANDOFF.md` section 0 ("the owner will rotate it") is
  closed.

**OWN-11 Merge the round-1 reviews and finalise the plan** [=default]
- Answer (a): merge the folder as it is on the branch.
- Consequence: `docs/fable/` from `wip/fable-reviews` goes to `main` as it is, not into
  `docs/reviews/` and `docs/research/` as the KICKOFF layout names them; plan P0 to P4 is
  final (fix pack ot1, UX pack ot2, then W0 to W7). The page's flag about rule 10 was in
  front of the owner; choosing (a) accepts the folder name. Still to do at the merge: check
  the text of the eight files for model names and tool attribution (not checked by the
  page) and remove what rule 10 forbids. DEC-552.

**OWN-12 More RAM for the VM** [=default]
- Answer (a): no change.
- Consequence: 4 vCPU / 16 GB stays; one building package at a time.

## 11. Notes that need interpretation

### 11.1 HUB-2 image licence

The words: choice (b) "CC BY 4.0 and CC0 only", and "Not sure I got this right. I didn't
know there were irrevocable licenses. It was my unknowing. Fable suggestion may be used".

Reading: the revocable operator licence was the owner's own earlier request. The note
takes that request back as made without knowing that the Creative Commons licences cannot
be revoked once granted. Two parts of the answer point in slightly different directions:

- the choice is (b), which drops the revocable licence;
- "Fable suggestion may be used" permits the review's suggestion, which was (a): CC BY 4.0
  preselected, CC0 and the revocable licence selectable.

Taken as the ruling: (b). Reasons: it is the ticked option; it is the smaller build; it
needs no lawyer for a bespoke clause; and the reason for model B has gone with the owner's
note. The sentence about the suggestion is read as "I do not insist on my earlier wish;
follow the review's advice where it matters", not as an order to build model B.

What (b) means for uploaders and what must hold:

- The upload form must say in plain words, before the upload, that the licence cannot be
  withdrawn later; the image can be removed from the hub on request, but copies others made
  under the licence stay licensed.
- GDPR needs the uploader's name to be removable, not the licence to be revocable (the hub
  design's argument, hub-design.md 1023-1073). CC BY 4.0 lets a licensor ask for the
  attribution to be removed (section 3(a)(3) of the legal code: "If requested by the Licensor, You must remove any of
  the information required by Section 3(a)(1)(A) to the extent reasonably practicable";
  the grant in section 2(a)(1) is "irrevocable";
  https://creativecommons.org/licenses/by/4.0/legalcode.en, read 2026-11-10);
  the hub must support "remove my name" without removing the image, and "remove my image
  from the hub" as a courtesy take-down.
- With CC BY 4.0 and CC0 only, images can go to Wikimedia Commons and to mirrors and into
  the dump; the model-B exclusions in H4 ("dump excludes model-B images") fall away.

Open point (13, Q5, low urgency; H4 is far away): confirm "(b), no revocable licence at
all". If the owner meant (a), model B comes back as an opt-in and needs the lawyer.

### 11.2 HUB-4 "a full feature hub"

The words: "I would like a full feature hub. Including everything we do. I am okey with
starting without movies and shows."

Reading taken as the ruling:

1. Target: the hub carries every kind the product tracks (board games, video games, books
   and audiobooks, music, comics, podcasts, visual novels, anime and manga, movies and
   shows), not only the kinds "where the open world is thin" (the hub design's rule of
   thumb, hub-design.md 1326-1346, which marks book, comic, podcast, music and visual novel
   "not needed" and movie / show "out of scope v1, not planned").
2. Order: movies and shows may come last. So for now HUB-4 behaves like option (a) "no, not
   in v1", and the destination is option (c) "full kind".

What this changes: the kind table of the hub design is no longer the scope; it is the
order. WP-OD8 (a) (audiobook editions after H3) already fits. H7 "hub-kinds" grows from
"video games, then anime and manga identifiers" into a series of kind packages.

What limits it (these do not go away with the ruling):

- The hub is CC0 (HUB-1 a). Only CC0-compatible values can be stored: Wikidata facts,
  MusicBrainz core data (CC0 per the data sources review), DNB (CC0 per the same review),
  and what contributors type themselves. Not storable: TVmaze and Wikipedia text (CC BY-SA),
  Open Library (no licence text published, per the review), TMDB, IGDB, MAL, BGG, VNDB
  (non-commercial terms) values.
- So "movies and shows on the hub" can mean: Wikidata ids, titles, years, contributor-typed
  facts and contributor-uploaded images. It cannot mean a copy of TMDB. The review's reason
  for keeping the kind out ("highest laundering risk; this is where the TMDB terms bite")
  stays true: people will retype TMDB overviews. The provenance filter and the hashes of
  H3 / HUB-6 are the guard, and the kind should not open before the lawyer has seen that
  design.
- For kinds with a good open source (MusicBrainz, Open Library) a full hub kind duplicates
  an existing community database. The honest minimum there is a bridge (ids plus facts the
  source lacks), which is what the hub design proposed.

Open points for the owner (13, Q2):

- (i) Does "everything we do" mean every media kind (the reading above), or also features
  beyond the catalogue (lists, reviews, plays, people)? The hub design is a catalogue of
  facts and images only; personal data stays on the instance. The reading above keeps it
  that way.
- (ii) For kinds that already have an open home (music, books, podcasts, visual novels): a
  full kind with its own records, or a bridge (identifier plus the facts the open source
  lacks)? Default taken until answered: bridge first, full kind only where contributors
  add facts the source lacks (audiobook editions are the first such case).
- (iii) The order after board games. Default taken: video games (H7), audiobook editions
  (WP-OD8), then one kind per package by the owner's own use, movies and shows last and
  only after the lawyer's answer on the laundering risk.

### 11.3 SEC-3 pseudonymised audit: the review the owner asked for

The words: "Audit rows don't need the name. It's fine to keep an identifier that is never
stored anywhere else. That means I remove the user from the identifier list which means all
the other data can't be retraced to that user. A fable agent may review this. It should be
the normal flow for this globally"

**What the owner describes.** Every record that must outlive an account (the audit log
first) stores an opaque identifier instead of a name. One table, the "identifier list",
maps identifier to person. Deleting the account deletes that one row; the remaining rows
then point at nothing. "Globally": this is the standard way the product keeps anything
about a person in a record that is not deleted with the account, and (HUB-3) the same on
the hub.

**Today's state (code).** `admin_audit` has `actor_id uuid REFERENCES users (id) ON DELETE
SET NULL`, plus `actor_name text`, `detail jsonb`, `client_addr text`, `target_id text`
(`src/rust/crates/store-pg/migrations/0016_admin.sql:105-116`). So the identifier half
exists already (the user id, set to NULL on delete); what survives a delete is the name
copy `actor_name`, usernames inside `detail`, the client address, and target ids that name
a user (security-audit.md 544-569 on `wip/fable-reviews`). Outside the audit log the same
audit lists search documents, stored import files and invite e-mail addresses; and
`App::delete_user` writes the deleted user's display name as a snapshot into other members'
people entries before the delete (`src/rust/crates/app/src/users.rs:113-125`).

**Is it sound? Yes, as a design, with five conditions.** The legal frame (Art. 4(5) and recital 26 read on 2026-11-10 at
https://gdpr-info.eu/art-4-gdpr/ and https://gdpr-info.eu/recitals/no-26/; the other
articles are named from memory, see 14): pseudonymisation is
defined in Art. 4(5) as processing so that data can no longer be attributed to a person
without additional information that is kept separately; pseudonymised data is still
personal data while that additional information exists (recital 26); data that can no
longer be attributed to a person by any means reasonably likely to be used is anonymous and
outside the regulation (recital 26). So:

- While the account lives, the audit rows are pseudonymised personal data. That is fine
  and better than today (data minimisation, Art. 5(1)(c); pseudonymisation is named as a
  safeguard in Art. 25 and Art. 32).
- After the mapping row is deleted, the rows are anonymous **only if nothing else in or
  around them leads back to the person**. Deleting the key is an accepted way to carry out
  erasure (Art. 17) exactly under that condition. This is the part that needs care; the
  design is not sound by itself.

What must hold:

1. **No second path to the person in the row itself.** Free text and structured detail must
   not carry the username, display name, e-mail address or the subject of an identity
   provider. Today `detail.username` does. The audit writer needs a typed detail (an
   allowlist of keys per action) instead of free JSON, and a test that fails when a new
   action writes a name.
2. **Targets too, not only actors.** "Admin X deleted user Y", "changed role of Y": Y must
   be the same kind of identifier. `target_id` for target type `user` follows the same rule.
3. **The client address is personal data of its own.** `client_addr` in an audit row can
   identify a household. Keep it only for a short, stated period (the hub design uses 30
   days for address hashes) or blank it when the account is deleted. Otherwise rows are
   re-identifiable through the address.
4. **The identifier is random and used nowhere else.** Not the user id that also sits in
   backups of other tables, not a hash of the username or e-mail (a hash of a guessable
   value can be recomputed). The cleanest form: a random `audit_subject` per user, stored
   only in the mapping table and the audit rows. The existing `actor_id` foreign key with
   `ON DELETE SET NULL` is a weaker variant: after the delete all rows of all deleted users
   look the same (NULL), so "the same deleted person did these five things" is lost. A
   random subject that stays in the rows keeps that link, which is what an admin needs to
   read an incident, and still names nobody.
5. **Backups and logs.** A database dump taken before the deletion still holds the mapping
   row. Erasure is complete only when those backups have aged out; the privacy page and the
   backup runbook must state the backup retention, and a restore must re-apply deletions
   made since the dump (keep a small list of deleted subject ids, which names nobody, to
   replay). Server logs must not carry usernames next to the identifier for longer than
   their rotation.

What breaks, and what does not:

- The admin loses "who did it" for deleted accounts (this is option (a) of the page, and
  the owner accepts it: "Audit rows don't need the name"). For living accounts the admin UI
  resolves the identifier to the current name at read time, so nothing is lost there, and a
  rename shows the new name everywhere (today the copy shows the old one).
- Singling out: a row that says "the only admin of this instance did X" identifies the
  person without any name. On a home instance with three members the audit rows of a
  deleted member may stay attributable to those who know the household. For the product
  this is acceptable (the people who can read the audit log already know), but it means the
  rows should not be called "anonymous" in the privacy page; say "kept without name for up
  to 365 days" (OD-13 stays as the outer limit).
- The break-glass notification and security e-mails (OD-17) name the account at send time;
  they are messages, not stored records, and are not affected.
- "Globally" has limits. The pattern fits records about a person's actions: audit log,
  job history, import reports, notification log, usage rows (the funnel already has no
  user column, DEC-431), hub change history (HUB-3 subject hash, H5 deletion). It does not
  fit a person's own content: notes, reviews, people entries, lists, uploaded import files
  and search documents are personal data in themselves and must be deleted with the
  account, as the audit's fix says. Cutting the identifier does not anonymise a note that
  says who watched what with whom.
- One case needs its own decision: the display-name snapshot that deletion writes into
  other members' people entries (`users.rs:113-125`). It keeps the deleted person's name on
  purpose, as part of another member's own record ("I watched this with Anna"). Under the
  owner's rule "can't be retraced to that user" it is a retraceable name. It is defensible
  as the other member's data (they could have typed the name themselves), and it is the
  current, approved behaviour of companion demotion; but it should be a conscious choice
  and named in the privacy page (13, Q3).

Proposed rule for the product (DEC-533), in one paragraph: records that outlive an account
carry a random per-user subject id and never a name, address or provider subject; names are
resolved at read time; deleting the account deletes the subject mapping, the person's own
content, their search documents, stored objects and invite addresses; client addresses in
kept records are blanked at deletion (and at the latest after 30 days); retention of the
kept records stays 365 days (OD-13); backups are covered by a stated retention and a
replay list of deleted subject ids. The hub (H5) uses the same rule for its change
history.

Cost: the page said M for the audit part; with the typed detail, the migration of existing
rows (drop `actor_name`, strip `detail.username`, backfill subjects) and the content
deletion of the audit's fix it is M to L, one package (sec3).

### 11.4 SEC-5: "Does it make sense to do that much login logic if we use an idm anyway?"

Short answer: yes for most of it, no for one part.

Why the password door still matters with an identity provider:

- **Home installs without an identity provider** are the default hosting mode (home,
  `docs/HANDOFF.md` 4.2); there the local sign-in is the only door and often faces the
  internet through a reverse proxy.
- **Break-glass.** In auto mode `/sign-in/local` stays reachable and, by the owner's
  ruling OD-17, needs the password only. It is the door used when the identity provider is
  down. Today ten wrong passwords for a name lock that name for 15 minutes, per process and
  repeatable by anyone (security-audit.md 597-618: `crates/auth/src/lib.rs:192-249`; the
  constants are at `lib.rs:192-200` on `main`). So anyone who knows the admin's username
  can keep the break-glass door shut exactly when it is needed. That is the defect worth
  fixing, and an identity provider does not fix it.
- **Other password endpoints** exist regardless: re-authentication, account linking,
  password reset, the username check (same audit section).
- **Cost of an attempt.** Every attempt, also for unknown names, runs one argon2
  verification (19 MiB, per the audit). Without a per-address budget and a concurrency cap
  a flood is a memory and CPU problem for the whole instance, with or without an identity
  provider.

What the identity provider does take over: throttling, lockout and second factors for the
provider's own sign-in form. Nothing of that is rebuilt here.

Which part is not worth it now: the difference between option (a) and option (b) is only
where the counters live. Postgres counters matter when several web processes share the
load. OPS-12 was answered (a) on the same page: one web process is the supported shape and
"throttles stay per process". With one web process, in-memory counters are correct.

Proposed reduction, to be confirmed by the owner (13, Q6): build the three behaviours
(per-address budget, concurrency cap around the password check, progressive delay instead
of the hard lock) behind a small counter interface with the in-memory implementation;
add the Postgres implementation when several web processes become supported (OPS-12 b) or
a hosted offer exists. That is option (a) in effect, with the door to (b) open at the cost
of one table and no rework.

Until the owner confirms, the ruling as ticked is (b), and the package (sec2) is cut so
that the Postgres store is its last, separable step. Either way the break-glass sign-in
keeps a throttle, an audit row and the admin notification (OD-17).

### 11.5 DATA-R: Steam Web API, SteamDB, ProtonDB

The words: choice (b) "re-check some", and "Steam web API. Steamdb.info, maybe protondb for
Linux compatibility."

Reading: every other entry of the "never build" list is confirmed (Google Books, OMDb, IMDb
datasets, Comic Vine, Giant Bomb, RAWG, MobyGames, TheGamesDB, Discogs API, Last.fm,
Deezer, Spotify, TheAudioDB, iTunes Search, Audnexus, AniList; BGG per HUB-7). The Steam
Web API leaves the list and is re-checked. SteamDB and ProtonDB are new candidates; the
purpose named for ProtonDB is a "runs on Linux" fact on video games.

Rule context: before any connector code, `docs/connectors/<id>.md` with API docs URL, terms
URL, licence, attribution, auth, rate limits and the date checked (KICKOFF section 5); with
unclear terms the connector sits behind a flag that is off by default (Q11); no invented or
undocumented endpoints (rule 1); no reading of GPL-family code (rule 6). So the next step
is a terms check, not a build.

What is known already, and how well:

- **Steam Web API.** The data sources review read the terms
  (https://steamcommunity.com/dev/apiterms, dated July 2010, read for the review in
  2026-11): 100,000 calls per day, a key, users must be told which Steam data is stored,
  Valve links must not be `nofollow`. It rejected the API because it "serves player and app
  data, not a catalogue; the storefront `appdetails` endpoint is undocumented"
  (open-data-sources.md 281 on `wip/fable-reviews`). So the rejection was about fit, not
  about forbidden terms. The API's documented strength is the player's own library and
  playtime, which is an import or sync of personal data (and answers OT-16 "playtime on a
  game" for Steam games without typing), not a metadata source. Wikidata links 129,148
  games to their Steam app id (same review, line 62), so matching is possible without
  Steam.
- **SteamDB (steamdb.info).** Its FAQ (https://steamdb.info/faq/, read 2026-11-10) says
  there is no API ("This will most likely mean an API at some point"), "We also do not allow
  scraping/crawling on SteamDB", and "Please get the information from Steam itself". So
  SteamDB cannot be a data source; the only use left is a link out to the game's SteamDB
  page built from the Steam app id. The FAQ points to a client library for Steam's own
  network; its licence must be checked before anyone reads it (rule 6), and a protocol
  that Valve does not document as a public API falls under rule 1 anyway.
- **ProtonDB.** Not checked. Remembered, not verified: no documented public API; periodic
  data exports of the reports exist under an open database licence. The JSON endpoints its
  own site uses are undocumented and fall under rule 1. If the export's licence allows it,
  a compatibility tier per Steam app id could be imported as an instance-side fact; whether
  it may enter the CC0 hub depends on that licence (a share-alike database licence could
  not).
- Valve also publishes its own compatibility ratings for the Steam Deck; whether a
  documented endpoint exposes them was not checked.

Open points for the owner (13, Q8): what is wanted from Steam: (i) import of the owner's
Steam library and playtime, (ii) store metadata for games, (iii) both? And for ProtonDB:
is a link out to the ProtonDB page enough if the data may not be copied?

Next step: a read-only research task "Steam, SteamDB, ProtonDB: terms and documented
endpoints" that writes `docs/connectors/steam.md`, `steamdb.md`, `protondb.md` (package
data-r in next-packages.md). Nothing is built before it. DEC-547.

### 11.6 OPS-3+4: "other" without a note

The owner ticked "Something else (see my note)" and left the note empty. The three options
were: (a) move the key out of the blob directory plus a banner on the admin overview until
an admin confirms the backup; (b) move it plus a wizard step "download or copy this key
now"; (c) docs only. No default is taken, because the owner explicitly did not take one.
The page's flag may be the reason: the owner's existing home instance needs a one-time move
of the blob directory or an explicit storage root setting, and the review does not describe
that migration. Question back: 13, Q1.

### 11.7 OWN-1: title and year after a TMDB purge

The words: choice (c), and "The movie name is technically entered by the user. So is the
release year (it's chosen by the user for identification) so it's not tmdb data. The user
entered it".

Reading: the owner's position is that the title and the year by which a person identified
what they watched are that person's own record, not data "obtained from TMDB". The ruling
is (c): TMDB fields are purged strictly at the limit (DEC-466 stays for the catalogue
item), and the library entry carries its own title and year.

Two remarks for the package, neither blocks it:

- The owner's argument is strongest when the copy is made when the person adds the item
  (the moment they "enter" it by picking it), not at purge time, when it would be a copy of
  provider data taken at the last allowed minute. Proposed implementation: the library
  entry gets `title` and `year` columns written when the entry is created or first logged
  (backfilled once for existing entries), and the purge leaves them alone. The option text
  says "at purge time"; the result for the owner is the same (no placeholder), the
  reasoning is cleaner. The package records this as its DEC.
- Lists, sessions, progress events and reviews can refer to an item that has no library
  entry. For those the placeholder could still show. The package decides whether adding to
  a list or logging also writes the person's label (proposed: yes, one `user_label` per
  person and item, not per library entry).
- The exact title string (spelling, localisation) does come from the provider's response.
  Whether TMDB's terms 1.C reach a title the person picked is the lawyer's point the page
  flagged; it goes on the lawyer list. With DATA-0 (a) the title and year of most films and
  shows come from Wikidata (CC0), so the question remains only for items without a Wikidata
  or TVmaze id.

## 12. The 53 defaults: in force, not owner-reviewed

The owner did not go through the "proceed with defaults" list of the page
(`review-page.html`, array `DEFAULTS`, lines 515-577). Under rule 2 each stays in force as
the default; none is owner-approved; the owner can reverse any of them. Same standing as
the entries in `docs/HANDOFF.md` 4.5.

| Group | Ids (count) |
|---|---|
| Hub | HUB-8, HUB-14 (2) |
| Security | SEC-2, SEC-7, SEC-8, SEC-9, SEC-11, SEC-12 (6) |
| Operations | OPS-1, OPS-2, OPS-5, OPS-7, OPS-8, OPS-9, OPS-10, OPS-11, CI (9) |
| Architecture | WP-A1, WP-A2, WP-C2, WP-C1, WP-D1, WP-D2, WP-D3, WP-C3, WP-E1 (9) |
| Performance | PERF-0 to PERF-9, PERF-N (11) |
| Tests | TEST-2 to TEST-7, TEST-9, TEST-10 (8) |
| Data sources | WP-OD2, WP-OD10 (2) |
| Decision log | C2, C4, C5, C11, C12, DLOG-2-6 (6) |

Answers above that touch a default:

- SEC-1 a carries OPS-8 (hashed bundle names) with it.
- TEST-1 a makes TEST-2 a precondition.
- DLOG-1 a makes DLOG-2-6 due.
- SEC-10 a completes C5 (session revocation).
- OWN-9 a decides the squash inside WP-D3.
- OPS-I b makes OPS-1, OPS-2, SEC-2 preconditions of a public image.
- OPS-12 a keeps TEST-10's two-process test at "web plus worker", not "two web".
- DATA-0 a makes WP-OD2 (licence tag per field) a precondition.
- HUB-4 (every kind) widens WP-OD10's "Discogs-dump hub seed: later" from "nice to have"
  to "a candidate seed for the music kind"; still later.

## 13. Questions back to the owner

Short; none blocks the next package (ot2a).

1. **OPS-3+4:** you chose "something else" without a note. What should happen with the
   secrets key: move it out of the data directory with a banner (a), with a wizard step (b),
   docs only (c), or what else? Is the one-time move on your home instance the concern?
2. **HUB-4:** "everything we do" is read as "every media kind, catalogue facts and images
   only, movies and shows last". Right? For kinds that already have an open database
   (music, books, podcasts, visual novels): a bridge (ids plus missing facts) or a full
   copy? Which kind after board games and video games?
3. **SEC-3:** the review in 11.3 says your rule is sound with five conditions. One choice
   is yours: when an account is deleted, other members' people entries keep the deleted
   person's display name as a snapshot ("watched with Anna"). Keep that (the other member's
   own record), or replace the name with a neutral "Deleted member"?
4. **OPS-I:** which public registry (the project's GitLab registry made public, Docker Hub,
   another), and under which image name?
5. **HUB-2:** confirm "CC BY 4.0 and CC0 only, no revocable licence at all" (your tick),
   not "revocable licence as an opt-in" (the review's suggestion your note allows).
6. **SEC-5:** your tick is "counters in Postgres". With one supported web process (your
   OPS-12 answer) in-memory counters are correct; may the Postgres part wait until several
   web processes are supported? (Everything else of SEC-5 is built either way; 11.4.)
7. **OWN-4:** MangaDex metadata-only: yes or no? (The note was empty.)
8. **DATA-R:** what do you want from Steam: your library and playtime imported, store
   metadata, or both? Is a link to ProtonDB enough if its data may not be copied?
9. **Lawyer (HUB-11 a):** engaging one is now an owner action. The list for the first
   conversation: contributor terms, image terms, DSA scope, hashes of purged TMDB text
   (HUB-6), title and year kept by the person after a TMDB purge (OWN-1), movies and shows
   on a CC0 hub (HUB-4).

Owner actions already promised by the answers (no question, listed so they are not lost):
sample exports (OWN-3), the four anime and manga actions (OWN-4), the three live checks
(OWN-8), re-import after the squash (OWN-9), a second instance as staging hub (HUB-13).

## 14. Not verified

- Nothing was built, run or rendered.
- Legal statements: GDPR Art. 4(5) and recital 26 and the two CC BY 4.0 passages in 11.1
  were read on 2026-11-10 (URLs at the place of use) through a page summariser, so the
  wording between the quoted parts is paraphrase. GDPR Art. 5(1)(c), 17, 25 and 32 are
  named from memory and were not re-read (https://eur-lex.europa.eu/eli/reg/2016/679/oj).
  11.3 is a design review, not legal advice; it goes to the lawyer with the hub terms.
- ProtonDB terms (11.5) are remembered, not read; the Steam Web API terms are quoted from
  the round-1 data sources review, not re-read; the SteamDB FAQ was read on 2026-11-10.
- Code citations: `0016_admin.sql:105-116`, `app/src/users.rs:108-132`,
  `config/src/lib.rs:381,391` and `auth/src/lib.rs:192-200` were read on the working tree
  of `main` on 2026-11-10; every other `file:line` is the cited review's own reference and
  was not re-read.
- DEC numbers of other entries (DEC-182, 223, 231, 268, 423 for C10; DEC-189 and DEC-201
  for OWN-7 and OWN-6) are taken from the decisions review's table, not re-read in the log.
- Whether the owner's home instance holds data that the product's export does not carry
  (OWN-9) is unknown.
- The ot1a fix pack and its review fixes were uncommitted while this file was written; the
  statements about what ot1a built come from its progress log.
