# Owner review page - content (round 2, F1)

Status: COMPLETE (2026-11-10). All sections written; limits in section 13.

Date: 2026-11-10. Read-only work; nothing was built or run.

## 0. How to read this

Purpose: one ordered list of the decisions the owner has to make, taken from the eight
round-1 reviews (branch `wip/fable-reviews`, folder `docs/fable/`), the owner test
(`docs/handoff/owner-test-2026-11-10.md`) and `docs/HANDOFF.md` section 6. The orchestrator
turns this file into the review page.

Conventions:

- **Ids.** A review's own ids are kept (SEC-n, OPS-n, WP-xx, C-n, D-OWN-n, P-n). Three
  reviews number their items without a prefix; this file adds one and says so:
  `HUB-n` = question n of `hub-design.md` section 13 (lines 1497-1536);
  `TEST-n` = step n of `test-audit.md` section 7 (lines 576-604);
  `PERF-n` = step n of `perf-proposals.md` section 8 (lines 754-772);
  `DLOG-n` = step n of `decisions-current-state.md` section 6 (lines 672-699).
  Ids invented here for questions no review asked as such are marked "(new)":
  HUB-0, SEC-4a, ARCH-0, DATA-0, DATA-R, OWN-n.
- **Default** = what the reviewer recommends. If the owner says nothing, rule 2 applies and
  the default is taken and logged as a DEC.
- **Cost** S = under a day, M = one to three days, L = about a week or more (the scale of
  `architecture-review.md` section 8). Where the review gave no effort the cost is this
  file's estimate and is marked `~`.
- **Citations.** `file:line` references to code are the cited review's own references
  (Rust paths drop `src/rust/`), not re-verified here, except where the line says
  `[checked]`: those were re-read on `main` (225b09d) on 2026-11-10. Review line numbers
  refer to the files as stored on `wip/fable-reviews`.
- **Marks.** `[OWNER]` = needs the owner. `[DEFAULT]` = no owner input needed, repeated in
  section 11. `[CONFLICT]` = contradicts another review or an owner ruling, detail in
  section 12.

Suggested reading order for the owner: section 1 (HUB-0, 7, 1, 2, 4 first), section 2
(SEC-1 to SEC-5), then 7 (DATA-0) because it changes the TMDB retention question, then the
rest. Sections 3 to 6 are mostly "confirm the order of work".

Items that need the owner: 14 hub (HUB-0 and 13 of the 15 questions) + 6 security + 5 ops
+ 2 architecture + 1 performance + 2 tests + 7 data sources + 2 decision log + 12 open
items = 51. Items that proceed on defaults: section 11.

## 1. Hub questions (hub-design.md)

Context in three lines (hub-design.md 40-93): the hub is a role of the same binary
(`[hub] serve = true`), reached through one new connector `hub`; board games are its first
kind, seeded from Wikidata (CC0); contributions are reviewed change sets with trust tiers;
restricted provider data (TMDB, IGDB, BGG, MAL) is filtered on the sending instance and never
reaches the hub. Phases H1 to H7 (hub-design.md 1349-1495).

### HUB-0 (new) Build the hub, and when [OWNER]

- Question: Do we build the hub as designed, and does H1 (read-only hub + Wikidata seed)
  replace workstream W3 "board games on Wikidata" in the plan?
- Default: yes; run H1 and H2 now, open the reference hub to the owner's own instances
  only, open to third parties after H5. Reason: H1 alone already replaces BGG for board
  games; H3 is the first phase that needs a lawyer's eye, H5 completes the legal surface
  (hub-design.md 1489-1495).
- Options: (a) H1 + H2 now, third parties after H5 (default); (b) H1 only now (read-only
  hub, no peers), decide the rest after use; (c) no hub: keep W3 as an instance-level
  Wikidata connector (DEC-192) and shelve the design.
- Cost: L per phase `~` (H1 alone: config, migration, new crate, 7 routes, connector, seed
  job, e2e; hub-design.md 1356-1384). Seven phases in total.
- Blocks: W3 (board games), the owned flag and "Mark owned" (DEC-420, HANDOFF.md 82-83),
  HUB-1 to HUB-15, WP-OD8, and whether the BGG deliveries are still needed (HUB-7).

### HUB-7 BGG connector [OWNER] [CONFLICT]

- Question: Do we drop the planned BGG connector for good and keep BGG only as an
  identifier plus the importers for the owner's own collection and plays?
- Default: do not build it; retire the "optional connector" of DEC-190/193 into
  "identifier only + importers". Reason: the connector was never implemented, only
  documented; the hub seeded from Wikidata covers the kind; BGG terms forbid modification
  and copying of covers (hub-design.md 51-55, 594-597; HANDOFF.md 753-758).
- Options: (a) do not build (default); (b) keep the plan for operators with their own
  token, off by default (the owner's wording on 2026-11-07, HANDOFF.md 760); (c) decide
  when the BGG application is answered.
- Cost: S (docs and one DEC) for (a); M-L `~` for (b).
- Blocks: the owner deliveries "BGG token" and "Powered by BGG logo" (HANDOFF.md 792; they
  become unnecessary under (a)); the attribution page content for BGG; W3.
- Conflict: the owner's ruling says "keep BGG for those with a token" (HANDOFF.md 760).
  The review's default reverses that, so it must not be taken silently. See 12.2 X1.

### HUB-1 Data licence for hub facts [OWNER]

- Question: Under which licence does the hub publish its structured data?
- Default: CC0 1.0. Reason: the only choice that lets the hub import from Wikidata and
  push back to it, avoids attribution stacking across contributors, and lets mirrors and
  instances keep the data without a licence string (hub-design.md 56-62, 990-1022).
- Options: (a) CC0 1.0 (default); (b) CC BY-SA 4.0; (c) ODbL. (b) and (c) block Wikidata
  push-back and add notice duties (hub-design.md 1499-1500).
- Cost: S (a text decision); changing it after the first third-party contribution is
  practically impossible.
- Blocks: H1 (`GET /meta` states the licence), the contributor terms (H3), WP-OD2 (the
  per-field licence tag and the "only CC0 may be proposed" filter).

### HUB-2 Image licence [OWNER]

- Question: Which licences may an uploader choose for an image, and is the revocable
  operator licence the owner asked for offered?
- Default: CC BY 4.0 preselected, CC0 and the revocable operator licence ("model B")
  selectable per upload. Reason: CC licences are irrevocable by their text and GDPR does
  not require a revocable copyright licence, only that the uploader's name can be removed;
  model B is bespoke, uncertain in court, cannot go to Wikimedia Commons or to mirrors,
  and cannot reach copies made before the withdrawal (hub-design.md 1023-1073).
- Options: (a) CC BY 4.0 default + CC0 + model B opt-in (default); (b) CC BY 4.0 and CC0
  only (no bespoke text, no lawyer needed for the clause); (c) model B only (no Commons,
  no mirrors for images).
- Cost: L `~` (H4), plus a lawyer review of the model B clause (draft at hub-design.md
  1075-1163) for (a) and (c).
- Blocks: H4; the legal texts in `config/brand/default/legal/`.

### HUB-4 Movies and shows on the hub [OWNER]

- Question: Does the hub carry movies and shows?
- Default: no in v1. Reason: highest laundering risk, this is where the TMDB terms bite
  (hub-design.md 76-79, 1341).
- Options: (a) no (default); (b) later a Wikidata-id bridge only (title, year, QID; no
  descriptions, no images); (c) full kind (not recommended by the review).
- Cost: none for (a); M `~` for (b).
- Blocks: nothing now. Related: DATA-0 gives every instance a CC0 title and year for
  films and shows without the hub, which removes most of the reason for (b).

### HUB-3 Contributor identity [OWNER]

- Question: Must a person have a hub account to contribute?
- Default: no; contributions are attributed to the peer (the instance) with a
  pseudonymous subject hash, a personal hub account is optional. Reason: GDPR by
  minimisation, more contributors (hub-design.md 80-86, 1504-1506).
- Options: (a) peer-attributed, account optional (default); (b) personal account
  mandatory (more accountability, more personal data, fewer contributors).
- Cost: part of H3/H5; (b) pulls account handling from H5 forward into H3.
- Blocks: H3 change model, H5 deletion and privacy notice.

### HUB-5 Restricted identifiers on the hub [OWNER]

- Question: May the hub store `bgg`, `igdb` and `mal` ids as plain identifiers when they
  come from Wikidata or are typed by a contributor?
- Default: yes for `bgg` (DEC-193 already does); `igdb` and `mal` only after the Wikidata
  properties are verified. Reason: a bare id is a fact and Wikidata publishes these ids
  under CC0 (hub-design.md 883-890, 1509-1512).
- Options: (a) as default; (b) QID, GTIN and ISBN only.
- Cost: S.
- Blocks: import matching (BGG collection import maps by BGG id), H7.

### HUB-6 Hashes of purged TMDB text [OWNER]

- Question: May an instance keep salted hashes of TMDB text it has purged, for 180 days,
  so it can refuse proposals that retype that text?
- Default: yes, under the same six-month limit, and ask the lawyer whether a one-way hash
  is "information obtained from TMDB". Reason: without it the echo check only sees
  currently stored values (hub-design.md 1513-1516, 915-927).
- Options: (a) yes + lawyer question (default); (b) compare against currently stored
  values only.
- Cost: S within H3.
- Blocks: H3 (the provenance filter). Related: TMDB retention (OWN-1).

### HUB-9 Registration at launch [OWNER]

- Question: Who can join the reference hub at launch?
- Default: personal accounts by invite, peers by admin approval (hub-design.md 1523-1524).
  Reason: review capacity is one person at the start (HUB-10).
- Options: (a) invite + approval (default); (b) open with a captcha.
- Cost: S (config defaults in H2/H5).
- Blocks: H2 (`peer_registration = approve`).

### HUB-10 Reviewers [OWNER]

- Question: Who reviews contributions?
- Default: the owner is the first moderator; an admin appoints trusted (L2) contributors
  after nomination (hub-design.md 1525-1526).
- Options: (a) appointment (default); (b) election as with MusicBrainz auto-editors.
- Cost: none in code beyond H3; it is the owner's time.
- Blocks: H3 opening to third parties.

### HUB-11 Notice-and-action and the lawyer [OWNER]

- Question: Do we build the full report, decision and appeal flow (DSA Art. 16/17 shape)
  even if the owner may be exempt as a micro-enterprise, and does the owner engage a
  lawyer for the hub terms?
- Default: build it fully; the lawyer confirms which duties bind the owner (hosting
  service vs online platform, Impressum). Reason: it is also the right abuse-report design
  (hub-design.md 87-88, 1527-1529).
- Options: (a) full flow + lawyer (default); (b) minimal contact address until third
  parties connect; (c) no third parties, so not needed (follows from HUB-0 (b)).
- Cost: L `~` (H5) plus the lawyer's fee. The review names three lawyer inputs: the
  contributor terms before H3 opens, the image clause before H4, the DSA scope (hub-design.md
  1491-1493).
- Blocks: opening the hub beyond the owner's own instances.

### HUB-12 First monetisation step [OWNER]

- Question: If money is ever taken, what is sold first? (low priority)
- Default: hosted instances with TMDB and IGDB switched off (or licensed) plus donations;
  hub API tiers later. Reason: the data is CC0 and stays free; what is sold is delivery;
  TMDB forbids charging users without a written agreement (hub-design.md 1298-1323).
- Options: (a) hosted instances + donations (default); (b) hub API tiers first; (c) no
  monetisation, donations only; (d) decide later (nothing is built for it now).
- Cost: none now.
- Blocks: nothing. See 12.2 X6: VNDB's API is free for non-commercial use only, so a paid
  hosted instance would need VNDB off as well; the hub review lists only TMDB and IGDB.

### HUB-13 Staging hub [OWNER]

- Question: Is there a second small instance as a staging hub before third-party peers
  connect?
- Default: yes (hub-design.md 1532-1533). Reason: protocol changes are tried against a
  hub whose history may be thrown away.
- Options: (a) second instance (default; the owner hosts it); (b) a `dry_run` flag on the
  reference hub.
- Cost: S in code (H6 staging flag); running cost of one more instance.
- Blocks: H6.

### HUB-15 Open Products Facts as a barcode source [OWNER]

- Question: Do we also query open product databases by barcode for board games?
- Default: no; not needed once the hub stores GTINs, revisit if contributors ask
  (hub-design.md 1535-1536).
- Options: (a) no (default); (b) yes, as an instance connector.
- Cost: none for (a); M `~` for (b).
- Blocks: nothing. Merged with the open question in HANDOFF.md 763-764 and 796, and with
  D-OWN-3 (part). Answering this closes all three.

### HUB-8 Review timing defaults [DEFAULT]

- Question: Which timers and thresholds does the review queue use?
- Default: 4-day period, 48 h minimum for destructive changes, 3 unanimous votes, 24 h
  objection window for trusted auto-accept, 14 days + 10 accepted changes to become
  trusted (hub-design.md 1520-1522). Alternative: MusicBrainz's 7 days.
- Cost: S; all are config values. Listed again in section 11.

### HUB-14 Hub search engine [DEFAULT]

- Question: Which search backend does the hub role use?
- Default: Postgres full-text + pg_trgm first, Meilisearch optional later (hub-design.md
  1534). Pure engineering. Listed again in section 11.

## 2. Security findings (security-audit.md)

Five medium, four low, three info; no high or critical. The review's fix order is SEC-2, 1,
3, 4, 5, 6, 8, 9, then the rest (security-audit.md 742-756). The owner test plan puts "the
security fixes the owner accepts" into fix pack P2 "ot1" (owner-test-2026-11-10.md, Plan).
For each finding the plain owner question is "accept the fix as proposed?"; where a real
choice exists it is spelled out.

### SEC-1 (medium) No security headers, no cache directives [OWNER]

- Question: Does the app set the security headers itself (including a Content Security
  Policy), or do we leave them to the reverse proxy?
- Finding: the only cross-cutting layers are the CSRF guard and request meta
  (`apps/server/src/lib.rs:445-454` [checked]); no CSP, no `X-Frame-Options`, no HSTS, no
  `Cache-Control: private, no-store` on signed-in responses (security-audit.md 500-523).
- Default: one middleware in the app that sets CSP, frame denial, nosniff, referrer
  policy and `Cache-Control: private, no-store` (except `/assets`, `/brand/fonts`, `/pkg`
  and the public shared-list page), HSTS when the public URL is https; plus a
  reverse-proxy docs section. Reason: a self-hosted app cannot rely on every operator's
  proxy.
- Options: (a) all headers in the app including CSP with a nonce for the hydration and
  theme scripts (default); (b) the simple headers and cache directives now, CSP as a
  second step (CSP is the part that can break pages); (c) document the headers for the
  proxy only.
- Cost: M for (a) (CSP needs nonces and `'wasm-unsafe-eval'`, e2e must stay green); S for
  (b) and (c).
- Blocks: fix pack "ot1"; the reverse-proxy page (OPS-2); OPS-8 must be decided with it
  (both set cache headers for `/pkg`, see 12.1 M4).

### SEC-2 (medium) Server-function requests have no body limit [DEFAULT]

- Question: Accept the fix? (No real alternative.)
- Finding: the pages router has no body limit and `/_fn/*` reads the whole body before
  any session check (`apps/server/src/lib.rs:798-826` [checked: no limit layer on the
  pages router]; security-audit.md 525-542). An unauthenticated client can exhaust memory.
- Default: the REST JSON body limit (2 MiB per the audit) on the pages router and a test
  expecting 413.
- Cost: S. Smallest change in the audit; first in the review's order.
- Blocks: fix pack "ot1". Listed again in section 11.

### SEC-3 (medium) Deleting a user leaves personal data behind [OWNER]

- Question: When an account is deleted, do audit rows lose the username at once, or are
  they kept for the 365-day audit retention the owner approved (OD-13)?
- Finding: search documents with notes, reviews and companion names, the audit rows'
  `actor_name`/`detail.username`, and stored import files survive the delete
  (security-audit.md 544-569).
- Default: delete search documents and stored objects with the account (no choice
  needed); for audit rows either pseudonymise or state in the privacy page that they are
  kept 365 days as legitimate interest. The review leaves that choice open.
- Options: (a) pseudonymise audit rows on delete (the admin loses "who did it" for
  deleted accounts); (b) keep them 365 days and say so in the privacy page (matches OD-13,
  HANDOFF.md 705-706); (c) keep for a shorter fixed period, for example 30 days, then
  pseudonymise.
- Cost: M (use-case change, outbox rows, one end-to-end test).
- Blocks: fix pack "ot1"; any hosted-mode offer (GDPR erasure); the hub's deletion design
  (H5) should use the same rule.

### SEC-4 (medium) Outbound requests to admin-chosen hosts are not confined [OWNER]

- Question: May an admin point OIDC and SMTP at hosts on private networks?
- Finding: OIDC endpoint hosts from a discovery document and the SMTP host are not checked
  against private ranges; the governed HTTP client follows redirects without re-checking
  the allowlist (security-audit.md 571-595). In hosted mode an admin who is not the
  operator can probe the operator's network.
- Default: no redirects on the governed client; private-range filter for OIDC endpoint
  hosts and the mail host unless `notify.allow_private_targets` is set; categorised
  errors only.
- Options: (a) block private targets unless the switch is on, in every hosting mode
  (default as written); (b) allow private targets in home mode by default, block in
  hosted mode; (c) redirect fix and categorised errors only, no private-range filter.
- Cost: M.
- Blocks: fix pack "ot1".
- SEC-4a (new, not raised by the review): the default hosting mode is home (HANDOFF.md
  690) and a home identity provider or mail relay is typically on a private address (the
  owner runs authentik). Under option (a) such a setup stops working after the upgrade
  until the switch is set. This file recommends (b), or (a) with the switch defaulting to
  on in home mode. Not verified: where the owner's authentik actually runs.

### SEC-5 (medium) Sign-in throttling [OWNER]

- Question: Should repeated wrong passwords slow an account down instead of locking it,
  and should the limits be per client address and shared between processes?
- Finding: the throttle is per username and per process; ten wrong passwords lock the
  break-glass admin sign-in for 15 minutes, repeatable by anyone; each attempt costs one
  argon2 run; no per-address limit (security-audit.md 597-618).
- Default: a second budget per client address, a global cap on concurrent password
  verifications, progressive delay instead of a hard lock, counters in Postgres (or
  per-process, documented).
- Options: (a) per-address budget + concurrency cap + progressive delay, counters stay in
  memory and the docs say "run one web process" (S-M); (b) the same with counters in
  Postgres, correct for several web processes (M); (c) keep the hard lock and add only
  the per-address budget and the concurrency cap.
- Cost: M.
- Blocks: fix pack "ot1"; running more than one web process (OPS-12).
- Touches an owner ruling: OD-17 says the break-glass sign-in keeps "the throttle"
  (HANDOFF.md 717-719). A progressive delay is still a throttle, but the behaviour the
  owner approved changes, so it is asked here.
- Merged: C8 and C9 of decisions-current-state.md (lines 449-450) and D-AUTH there; see
  12.1 M1.

### SEC-6 (low) Compose defaults [OWNER]

- Question: May `compose up` refuse to start until the passwords are set, and bind to
  127.0.0.1 only?
- Finding: default credentials (`src/docker/compose.yml:14,16,41,53` [checked]) and
  `ports: "3000:3000"` on every interface (`compose.yml:22` [checked])
  (security-audit.md 620-637).
- Default: required variables (`${VAR:?...}`), loopback bind, a start-up warning when the
  public URL is http and the bind is not loopback, and a "Behind a reverse proxy" page.
- Options: (a) as default (the one-line quick start becomes "copy `.env.example`, set two
  values, start"); (b) keep the working defaults but print the warning; (c) two compose
  files: a strict one and a `compose.dev.yml` with defaults.
- Cost: S.
- Blocks: the first-run docs (OPS-2). Merged with step 7 of the ops plan (12.1 M3).

### SEC-10 (info) Session lifetime and session list [OWNER]

- Question: Do sessions get an absolute maximum age, and do members get a list of their
  sessions with "sign out everywhere"?
- Finding: sessions slide (default 30 days) without a hard cap; no device list
  (security-audit.md 679-686).
- Default: a hard cap (for example 90 days) and a Settings > Account session list.
- Options: (a) both; (b) hard cap only (S); (c) neither for now.
- Cost: S for the cap, M for the list (a new settings block; density budget OD-01
  applies).
- Blocks: nothing. Touches OD-11 (session length is editable in the admin UI, HANDOFF.md
  714-716): the cap would be a second editable value. Merged with the "revoking other
  sessions" deferral in C5 (decisions-current-state.md 446); see 12.1 M2.

### SEC-7, SEC-8, SEC-9, SEC-11, SEC-12 [DEFAULT]

No owner input needed; one line each, detail in section 11:
SEC-7 (low) stream import parsing or cap concurrent imports; SEC-8 (low) checksums for
downloaded build tools; SEC-9 (low) generic error text for webhook senders; SEC-11 (info)
SSO hint cookie, accepted per addendum-1 4.2; SEC-12 (info) CSRF guard and missing
`Origin`, no fix required (security-audit.md 639-705).

## 3. Ops readiness (ops-readiness.md)

Twelve findings OPS-1 to OPS-12 (index at ops-readiness.md 743-758), two high. The plan is
at ops-readiness.md 725-741. Six need the owner; the rest are in section 11.

### OPS-1 (high) The `cli` binary is not in the image [DEFAULT]

- Question: Accept the fix? (No real alternative.)
- Finding: the image copies only `server` (`src/docker/Dockerfile:29,37` [checked]); every
  documented operator procedure (storage report, reindex, key rotation, the restore's
  reindex step) needs `cli`.
- Default: build `cli` in the image build and copy it next to `server`.
- Cost: S (adds one more release compile to a build that is already tight on memory).
- Blocks: the backup/restore runbook (OPS-9), every `operations.adoc` procedure for image
  users. Listed again in section 11.

### OPS-2 (high) First run behind TLS is undocumented [DEFAULT]

- Finding: `APP__SERVER__PUBLIC_URL`, `APP__SERVER__TRUSTED_PROXIES` and
  `APP__AUTH__SECURE_COOKIES` are not in `config/.env.example` ([checked]: no match for
  the three names); with the defaults a sign-in from any host other than localhost fails
  silently or with a 403 (ops-readiness.md 305-322). The owner hit exactly this
  (HANDOFF.md 43-45).
- Default: add the three keys with one-line comments; write a reverse-proxy page with a
  Caddy and an nginx snippet and the "do not buffer SSE" note.
- Cost: S. Listed again in section 11. Merged with the docs part of SEC-1 and SEC-6.

### OPS-3 + OPS-4 (medium) Where the secrets key lives, and the backup reminder [OWNER]

- Question: Do we move `secrets.key` out of the data directory and add a "back up this
  key now" step or banner after the setup wizard?
- Finding: the key is at `/data/secrets.key`, inside the blob root `/data`
  (`src/docker/Dockerfile:46-47` [checked]); syncing or clearing `/data` ships or deletes
  the key; nothing in the UI says that the key was generated and must be backed up
  (ops-readiness.md 358-377). Losing the key is the one unrecoverable loss.
- Default: blobs to `/data/blobs`, key stays outside the blob root; a wizard step or a
  persistent banner until an admin confirms the backup.
- Options: (a) move + banner on the admin overview until confirmed (default; no new
  wizard step); (b) move + a wizard step "download or copy this key now"; (c) docs only.
- Cost: S-M.
- Blocks: the backup runbook (OPS-9). Note for the owner: an existing instance (the
  owner's home instance) needs a one-time move of the blob directory or an explicit
  `APP__STORAGE__FS_ROOT`; the review does not describe the migration. The wizard step
  counts against the density budget (OD-01) and needs en/de strings.

### OPS-6 (medium) Rollback policy [OWNER]

- Question: Is "the only rollback is restoring the database dump taken before the
  upgrade" acceptable as the documented policy?
- Finding: an older binary refuses a newer schema; TOTP sealing is one-way; nothing is
  documented (ops-readiness.md 445-462).
- Default: yes, document it together with "take the dump first"; log the migrations that
  will run at start; keep the expand/contract discipline; a `cli migrate --dry-run`.
- Options: (a) dump-restore only, documented (default); (b) additionally reversible
  migration pairs from a new baseline (WP-D3, section 4); (c) nothing until there are
  external operators.
- Cost: S for (a); (b) is part of WP-D3.
- Blocks: the upgrade runbook; publishing images (OPS-I).

### OPS-12 (medium) More than one web process [OWNER]

- Question: Do we support several web processes now, or state "one web process" as the
  supported shape?
- Finding: two `--role=all` processes do not share live events (ops-readiness.md 757); the
  sign-in, outage, emailed-link and funnel throttles are per process (C8); no test runs
  `--role web` + `--role worker` as two processes (test-audit.md 472-476).
- Default (ops): document which roles may be duplicated and ship a commented `web` /
  `worker` service pair in compose. Default (decisions review, C8): decide once: document
  "one web process" or move the counters to a shared table.
- Options: (a) supported shape = one `web` + n `worker`, documented; throttles stay per
  process (S); (b) make several web processes correct: shared throttles (SEC-5 option b),
  shared events, a two-process test (L `~`); (c) (a) now, (b) when a hosted offer exists.
- Cost: S for (a).
- Blocks: SEC-5 option choice; sizing docs; TEST-10.

### OPS-I (new) Publish images [OWNER]

- Question: Do we publish tagged container images from CI, and where?
- Source: step 11 of the ops plan ("push tagged images on tags", ops-readiness.md 741)
  and docs gap 9 ("pinning the app image to a tag once images are published",
  ops-readiness.md 720-722). The review treats it as a CI detail; it is the first public
  distribution of the product, so it is asked here.
- Default (this file): not before OPS-1, OPS-2, SEC-1, SEC-2 and SEC-6 are done.
- Options: (a) GitLab registry, private, for the owner's own hosts; (b) public registry
  on tags; (c) no images, operators build themselves.
- Cost: S-M (kaniko cache, tag job).
- Blocks: the attribution page must be in the image before a public image exists (R-F3,
  HANDOFF.md 671-675); the upgrade and rollback docs (OPS-6).

### OPS-B (new) Build profile: wasm size against build memory [OWNER]

- Question: May the wasm bundle get somewhat larger so that the image builds reliably on a
  16 GB host?
- Source: the proposed Dockerfile (ops-readiness.md 188-301: cargo-chef dependency layer,
  separate frontend and server stages, per-step job arguments, `WASM_LTO=thin`) and WP-E1
  of the architecture review (architecture-review.md 774-779). Both name fat LTO with one
  codegen unit as the largest single-process memory peak. Neither measured it.
- Default: adopt the Dockerfile structure (no trade-off in that); measure thin against fat
  LTO (peak memory, size before and after wasm-opt), then the owner picks with numbers.
- Options: (a) measure first, then decide (default); (b) thin LTO in the image build, fat
  only in CI; (c) keep fat LTO everywhere.
- Cost: S for the measurement, M for the Dockerfile.
- Blocks: the owner's own podman build (closed as a network problem on the company
  laptop, owner-test-2026-11-10.md "Closed threads"; the memory analysis still holds for
  small hosts). The measurement needs a build, so it waits until the VM is free.

### Other ops findings [DEFAULT]

OPS-5, OPS-7, OPS-8, OPS-9, OPS-10, OPS-11: section 11.

## 4. Architecture packages (architecture-review.md)

Twelve packages (architecture-review.md 666-786; HANDOFF.md 30 says "10 work packages",
the review lists twelve). None fixes a correctness defect; all are cost: build time, build
memory, latency, lines to maintain (architecture-review.md 39-40). The review's order:
A1, A2 -> C2 -> B3 -> B2 -> B1 -> C1 -> D1 -> D2 -> D3 -> C3 -> E1.

### ARCH-0 (new) When does the restructuring happen [OWNER]

- Question: Do the three large packages (WP-B3 split `api`, WP-B2 remove the Backend
  pass-through, WP-B1 split `web`) run before the feature workstreams W1 to W7, between
  them, or not at all?
- Default (review): do them, in the order above, after the quick wins. Reason: `web` is
  one 32 k-line crate compiled three times per full build and is what needs 3-4 GB per
  compiler process; about 1,250 lines of forwarding code carry no logic; `api` (12.7 k
  lines) has grown into a second application layer that bypasses `app`
  (architecture-review.md 20-31).
- Options: (a) quick wins and rules now (A1, A2, C2), the B packages as their own
  workstream before the hub (the hub adds a crate, routes and pages to exactly the crates
  that would be split); (b) quick wins now, B packages after P2/P3 and W1; (c) quick wins
  only, accept the build cost.
- Cost: L + M + L for B3, B2, B1 (review's figures); S + M + S-M for A1, A2, C2.
- Blocks: build memory on small hosts (OPS-B), the hub's file layout (hub-design.md
  1367-1371 names files in today's crates), test run time for `web` logic.
- This file's note: no user-visible change comes out of B1-B3, and each ends with a full
  `cargo xtask ci` + e2e run on a shared VM. Option (b) fits the owner test plan P0-P4.

### WP-B2 Remove the Backend pass-through (reverses DEC-162) [OWNER] [CONFLICT]

- Question: Do we reverse DEC-162 and let server functions call the operations directly?
- Default (architecture review): yes, delete `Backend`, `AdminBackend`, `WebBackend`,
  `WebAdmin` after B3 (architecture-review.md 713-721).
- Contradiction: the decisions review recommends the opposite for the same DEC: accept
  the split "reads = server functions, writes = REST" as the architecture and close
  DEC-162's "revisit" note (decisions-current-state.md 447, C6). See 12.2 X3.
- Options: (a) reverse DEC-162 (architecture review); (b) keep the layers and record the
  split as intended (decisions review); (c) decide after B3, when the size of the
  remaining forwarding code is known.
- Cost: M, risk M.
- Blocks: WP-B1 (the split of `web` is simpler without the trait); C6.
- DEC-162 is in the range the owner has not reviewed (HANDOFF.md 742-744), so it is in
  force but not owner-approved; either answer is open to the owner.

### Other architecture packages [DEFAULT]

WP-A1, WP-A2, WP-C2, WP-C1, WP-D1, WP-D2, WP-D3, WP-C3, WP-E1: section 11. WP-D3 contains
one owner question, asked as OWN-9 in section 9 (squash the migrations into one baseline:
may the owner's instance be re-created?).

## 5. Performance (perf-proposals.md)

Nine steps plus a measuring step 0 (perf-proposals.md 754-772). All mixed-set endpoints are
inside the 300 ms p95 budget today; the zone-change read is at ~750 ms against a 1000 ms
budget and a 500 ms target (perf-proposals.md 26-29, HANDOFF.md 86-88). The one predicted
failure is `/people`, which is not measured: one user read per shared session (2,000 at
the load shape) and a full read of all own sessions; predicted 600-1,200 ms single-client
in debug, "a prediction, not a measurement" (perf-proposals.md 366-387).

### PERF-T (new) Targets and place in the plan [OWNER]

- Question: Are the review's targets the ones we work to, and does the performance work
  run before or after the fix and UX packs?
- Default (review): every mixed-set line <= 150 ms p95 (worst today ~210, the library
  query first page), Home <= 90, `/people` <= 40, zone change <= 400, on the shape
  "release, 8 clients + 2 writers, 40k events, 20k sessions, 2k shared"
  (perf-proposals.md 778-782).
- Options: (a) steps 0 and 1 now (measure `/people`, fix its N+1: both S, no migration),
  the rest as workstream W2 after P2/P3; (b) the whole plan as W2, as scheduled;
  (c) steps 0 to 4 only (no migration, all S or M), stop when everything is under 150 ms.
- Cost: S, S, M, S, S, M, S, M, S, M for steps 0 to 9; each step needs one release load
  run (about 40 minutes and 15 GB of disk, perf-proposals.md 756-757).
- Blocks: nothing user-visible today. The owner test reported no slowness
  (owner-test-2026-11-10.md has no performance finding). OT-17 (hours on the stats page)
  will change the stats queries, so PERF-5 (zone rebuild) should come after the stats
  redesign is decided.

### PERF-0 to PERF-9 [DEFAULT]

Section 11. Order conflict with the architecture review (which package is first): 12.2 X4.

## 6. Tests (test-audit.md)

About 840 Rust tests and 121 Playwright tests; the weaknesses are structural
(test-audit.md 13-32). Ten plan steps (test-audit.md 576-604). Two touch what the owner
sees or pays for.

### TEST-8 Import job lane [OWNER]

- Question: Should each member's import run in its own lane, so that one member's large
  import does not delay another's?
- Finding: all imports share the strictly ordered `user_import` lane
  (`crates/jobs/src/lib.rs:5` and `crates/importers/src/runner.rs:120` [checked]; DEC-029);
  e2e timeouts of 120-240 s are the symptom (test-audit.md 246-255).
- Default: a per-user or per-import lane, then lower the test timeouts to 60 s. The audit
  calls it "product-visible".
- Options: (a) per-user import lane (product change, M `~`); (b) test-only worker
  concurrency or a direct seeding endpoint for tests (S, no product change); (c) leave it.
- Cost: S for (b) (review: half a day), M `~` for (a).
- Blocks: e2e wall time; hosted mode with several importing members. See 12.2 X5: the
  security audit states the lane already serialises per user.

### TEST-1 Flaky tests in CI [OWNER]

- Question: When a browser test fails and then passes on the retry, does the pipeline
  fail, or only report it?
- Finding: CI retries every failed Playwright test once
  (`src/web/playwright.config.ts:21` [checked]; the audit cites line 22) and nothing
  records how often that happens (test-audit.md 21-22).
- Default: keep the retry, add a JSON reporter, let `cargo xtask e2e-scan` "fail or print
  a prominent line" when a test was flaky. The audit leaves fail-or-print open.
- Options: (a) fail the pipeline on any flaky test (matches the Definition of Done "no
  test workarounds for app races", HANDOFF.md 57-58); (b) print and count, fail above a
  threshold; (c) print only.
- Cost: S (half a day).
- Blocks: TEST-2 (isolating the specs that change instance-wide state) should land first
  or together, otherwise (a) turns known order dependence into red pipelines.

### TEST-2 to TEST-7, TEST-9, TEST-10 [DEFAULT]

Section 11. TEST-9 (a nightly job) needs CI runner time; if the owner's runner is the
shared VM, say so and it is dropped to "on demand".

## 7. Data sources (open-data-sources.md)

One finding dominates: every kind except anime has an open source for identity, so TMDB
can become optional enrichment (open-data-sources.md 49-64, 366-376). Ten packages WP-OD1
to WP-OD10 (open-data-sources.md 396-409). Pages the reviewer could not open are listed at
open-data-sources.md 411-440 and summarised in section 13.

### DATA-0 (new) Demote TMDB to optional enrichment [OWNER]

- Question: Should title, year and ids of films and shows come from Wikidata (CC0) and,
  for shows, TVmaze, with TMDB only an operator-enabled source of posters and overviews?
- Default: yes (WP-OD1, then WP-OD3). Reason: TMDB terms forbid caching beyond six months
  and commercial use without a written agreement; with a CC0 identity layer the retention
  job purges only TMDB-sourced fields and the item keeps its real title, so the
  placeholder "Title unavailable - source data expired" disappears for every item with a
  Wikidata or TVmaze id (open-data-sources.md 366-371).
- Options: (a) Wikidata identity above every proprietary source for all kinds (default);
  (b) Wikidata identity only as a fallback when TMDB data has expired; (c) leave TMDB as
  the identity source.
- Cost: L `~` for WP-OD1 (generalised connector, a property map per kind, field
  priority), M `~` for WP-OD2 (licence tag per field and asset), which the review wants
  first or alongside.
- Blocks: OWN-1 (TMDB retention: the strict reading stops hurting under (a)); the hub
  seed for video games (H7); the attribution page (generated from the licence tags, F4's
  topic); search ranking (OT-3: a second identity source changes which result is "the
  original"; not analysed by the review).
- Merged: C1, D-OWN-4 and the HANDOFF 6 TMDB item are the same question seen from the
  retention side; they are asked once as OWN-1 with this item as context (12.1 M5).

### WP-OD3 TVmaze as the default show source [OWNER]

- Question: Is TVmaze (CC BY-SA 4.0, no key) the default source for shows, episodes and
  air dates, with TMDB and TheTVDB behind it?
- Default: yes. Reason: key-less, no retention limit, better episode data than TMDB
  (open-data-sources.md 402).
- Options: (a) TVmaze default, TMDB enrichment (default); (b) TVmaze as an additional
  source, TMDB stays first; (c) not now.
- Cost: M `~`.
- Blocks: the "Up to date" badge and per-episode runtime (DEC-450), which OT-17 pulls
  forward (owner-test-2026-11-10.md, Triage 4): hours watched need episode runtimes, and
  the source of those runtimes is decided here. Share-alike data must carry a credit link
  and can never be proposed to the CC0 hub.

### WP-OD7 Wikipedia summaries as descriptions [OWNER]

- Question: May item descriptions come from Wikipedia (CC BY-SA 4.0, credited link)?
- Default: yes, as the open description source for films, shows, games and books on the
  instance; never exported to the hub. Reason: lets an instance switch TMDB off entirely
  (open-data-sources.md 356-357, 406).
- Options: (a) yes, default on; (b) yes, off by default; (c) no.
- Cost: M `~`.
- Blocks: nothing; depends on WP-OD1.

### DATA-K (new) Order of the new connectors [OWNER]

- Question: In which order do books, music and visual novels get their connectors?
- Default (review): WP-OD4 Open Library + DNB (books have no connector at all today; DNB
  is CC0 and German), then WP-OD5 MusicBrainz + Cover Art Archive + ListenBrainz importer,
  then WP-OD6 VNDB (open-data-sources.md 403-405).
- Options: (a) as default; (b) the owner's own use first (state which kinds the owner
  tracks); (c) after the hub H1.
- Cost: M each `~`.
- Blocks: W5. All three were unblocked by ruling R-P1 (HANDOFF.md 666-670). Note: Open
  Library publishes no licence text and VNDB's API is free for non-commercial use only
  (open-data-sources.md 59, 64).

### WP-OD8 Audiobook editions as a hub kind [OWNER] [CONFLICT]

- Question: Does the hub get a kind for audiobook editions (narrator, duration,
  abridged)?
- Default (data sources review): yes, "the one kind with no open home"
  (open-data-sources.md 58, 390-394).
- Contradiction: the hub design says for book / audiobook "not needed" (hub-design.md
  1336). See 12.2 X2.
- Options: (a) add after H3, seeded from MusicBrainz audiobook release groups; (b) no hub
  kind, audiobooks from Open Library / DNB / MusicBrainz on the instance only; (c) decide
  when WP-OD5 exists and the gap is visible.
- Cost: M `~` once the hub exists.
- Blocks: nothing now (depends on H1-H3, WP-OD1, WP-OD5).

### WP-OD9 Comics [OWNER] (a delivery, not a decision)

- Question: Will the owner open the licence pages of the Grand Comics Database, Metron
  and Inducks in a browser and save them to `owner-inbox/`?
- Reason: the build host is blocked by all three (HTTP 403 or a bot challenge,
  open-data-sources.md 364-365, 416-424); the GCD licence is `UNVERIFIED`.
- Default: until then no comics connector; afterwards each behind a flag that is off by
  default (rule Q11).
- Cost: owner time; M `~` per connector afterwards.
- Blocks: comics only. See 12.2 X7: the hub design calls Metron "open"; the data sources
  review could not find its licence.

### DATA-R (new) Confirm the "never build" list [OWNER]

- Question: Is it agreed that these are not built: Google Books, OMDb, IMDb datasets,
  Comic Vine, Giant Bomb, RAWG, MobyGames, TheGamesDB, Steam Web API, Discogs API,
  Last.fm, Deezer, Spotify, TheAudioDB, iTunes Search, Audnexus (plus AniList per DEC-108
  and BGG per HUB-7)?
- Default: yes (open-data-sources.md 378-382).
- Options: (a) confirm; (b) name the ones to re-check. Several rejections rest on pages
  that could not be opened (MobyGames, RAWG, Giant Bomb, TheGamesDB: section 13).
- Cost: none.
- Blocks: nothing.

### WP-OD2, WP-OD10 [DEFAULT]

Section 11.

## 8. Decisions-log restructuring (decisions-current-state.md)

The review read DEC-001 to DEC-494: 93 supersession rows, 12 standing contradictions
C1 to C12, 58 open backlog items of which 9 wait for the owner
(decisions-current-state.md 701-710). DEC-495 was added afterwards
(`docs/DECISIONS.md:499` [checked]) and is not covered.

### DLOG-1 Split the decision log into three files [OWNER]

- Question: Do we restructure the log into a maintained current-state summary, the
  append-only history with a status prefix, and a backlog?
- Default: yes: `CURRENT.md` (what is true now, by area, citing DEC ids; what a new agent
  reads first), `DECISIONS.md` (history, never edited except a status prefix `active |
  superseded by DEC-n | amended by DEC-n | done | record | owner`), `BACKLOG.md` (open
  deferrals) (decisions-current-state.md 672-699). Reason: 29 entries are superseded and
  43 amended without a note in the file itself; about 60 entries are records, not
  decisions.
- Options: (a) three files as proposed; (b) keep one file, add only the status prefix and
  the two new fields `follow-up` and `supersedes` (DLOG-2); (c) leave the log, keep the
  review file as the index.
- Cost: M for the one-time migration (DLOG-6), S for the repository check (DLOG-4).
- Blocks: OWN-2 (the owner's review of about 150 unreviewed entries is far shorter
  against a current-state summary); every agent's start-up reading.
- Conflict with the KICKOFF layout: the rules fix the path `docs/DECISIONS.md`
  (HANDOFF.md 207 and rule 2) and say everything written in `docs/` is AsciiDoc
  (HANDOFF.md 224); the proposal moves the log to `docs/decisions/` and adds Markdown
  files. Only the owner can change a KICKOFF rule, so (a) needs an explicit yes. A variant
  that needs no rule change: keep `docs/DECISIONS.md` where it is and add
  `docs/decisions/CURRENT.adoc` and `BACKLOG.adoc` next to it.

### C10 Where personal UI preferences live [OWNER]

- Question: Should the shortcuts switch, remembered disclosures, remembered group, nudge
  snooze and the episode "ask once" follow the user to another browser?
- Finding: all five live in the browser's `localStorage`; five DECs each say "a server
  preference later" (decisions-current-state.md 451).
- Default: one decision instead of five open notes: a `user_preferences` store, or an
  explicit "browser-only by design".
- Options: (a) server-side store for all five (M `~`); (b) browser-only by design,
  close the five notes (S); (c) server-side only for the shortcuts switch and the
  remembered group.
- Cost: S to M.
- Blocks: nothing. Related ruling: OD-02 "remember disclosure state" (HANDOFF.md 702-703)
  does not say where.

### C3 and C7 [OWNER]

Two deviations from owner-approved text; asked as OWN-6 and OWN-7 in section 9.

### C1 [OWNER]

Asked as OWN-1 in section 9.

### C2, C4, C5, C6, C8, C9, C11, C12 and DLOG-2 to DLOG-6 [DEFAULT]

Section 11 (C6 is contested, see WP-B2 in section 4; C8 and C9 are merged into SEC-5).

## 9. Open owner items (HANDOFF section 6 and the reviews' "waiting for the owner")

Sources: HANDOFF.md 789-807, decisions-current-state.md 464-482 (D-OWN-1 to D-OWN-9).

### OWN-1 TMDB retention: strict reading or keep title and year [OWNER]

- Ids merged: HANDOFF 6 "TMDB retention", C1, D-OWN-4, DEC-453 vs DEC-466.
- Question: After six months without a possible refresh, does an item lose its title and
  year too (strict, in force), or are title and year kept as the person's own record?
- Default in force: strict (DEC-466): placeholder "Title unavailable - source data
  expired", refetch once TMDB is usable, unreferenced items deleted (HANDOFF.md 801-805).
- Options: (a) strict, as now; (b) keep title and year (DEC-453's reading); (c) strict
  for TMDB fields, but copy title and year into the library entry at purge time so no
  library ever shows a placeholder (the decisions review's suggestion,
  decisions-current-state.md 442); (d) strict, and make the question moot with DATA-0
  (title and year from Wikidata, CC0).
- Cost: S for (a)-(c); (d) is DATA-0.
- Blocks: nothing is waiting, but whichever stands, the other DEC must be marked
  superseded.
- Not verified: whether (b) or (c) is compatible with TMDB terms 1.C; the reviews do not
  settle it and this file did not read the terms.

### OWN-2 The unreviewed DECISIONS entries [OWNER]

- Ids merged: HANDOFF 6 first item, HANDOFF 4.5, D-OWN-6 (DEC-174).
- Question: How does the owner want to review DEC-019 to 088 and 091 to 168 (and the
  later ones), which are in force but not owner-approved (HANDOFF.md 742-744)?
- Default (this file): review the current-state summary by area
  (decisions-current-state.md section 1, lines 21-327) instead of the entries one by one,
  and raise only what looks wrong; the 12 contradictions and the 9 owner waits are already
  on this page.
- Options: (a) as default; (b) entry by entry on the open-decisions page (HANDOFF.md
  895); (c) declare the range accepted as a whole, reversible per entry.
- Cost: owner time only.
- Blocks: nothing.

### OWN-3 Deliveries [OWNER]

- Ids merged: HANDOFF 6 "Deliveries", D-OWN-1, D-OWN-2, R-P2, R-P4.
- Sample exports still expected in `owner-inbox/`: Goodreads, BG Stats, BGG collection,
  and the Trakt "Raw Export" ZIP (decisions-current-state.md 467-468). They block the
  corresponding importers (HANDOFF.md 54).
- BGG token and the "Powered by BGG" logo files: needed only if HUB-7 is answered (b);
  under (a) they are dropped. The BGG collection export is needed either way.

### OWN-4 Anime and manga sources [OWNER]

- Ids merged: HANDOFF 6 "Questions with safe defaults", D-OWN-3 (without the barcode
  part, which is HUB-15).
- Four small actions, all with safe defaults in force (everything off): (1) read the
  MyAnimeList developer agreement when registering a client, then set `KK_MAL_CLIENT_ID`;
  (2) save the Kitsu terms page into `owner-inbox/`; (3) MangaDex metadata-only: yes or
  no; (4) read the AniDB rules in a browser (DEC-221; listed by the decisions review, not
  by HANDOFF 6).
- Blocks: W4 (anime/manga connectors).

### OWN-5 Funnel review on 2026-12-06, saved views [OWNER]

- Ids merged: HANDOFF 6, D-OWN-5 (DEC-431).
- A date, not a decision today: read Admin > Storage and jobs > Usage measurement, cut
  what is not used, promote disclosures opened on more than half of their visits, decide
  on saved views (HANDOFF.md 797-800).
- Note: the owner test changes several of the measured flows (OT-4, 5, 9, 10, 17). If the
  UX pack "ot2" ships before 2026-12-06, four weeks of data on the old flows lose value;
  the owner may want to move the date to four weeks after "ot2".

### OWN-6 C3: language for people who have not chosen one [OWNER]

- Question: The ruling says "per-user language with the brand default as fallback"
  (HANDOFF.md 678-680); the code puts the browser's Accept-Language before the brand
  default (DEC-201). Accept the deviation?
- Default: accept and record it (better signal) (decisions-current-state.md 444).
- Options: (a) accept; (b) follow the ruling literally.
- Cost: S either way.

### OWN-7 C7: where sign-up lands [OWNER]

- Question: The approved design says a new account lands on Home; the code lands on
  `/library` (`crates/web/src/auth_pages.rs:481`, DEC-189). Which one?
- Default: either change the landing page and the e2e suites, or record the deviation
  (decisions-current-state.md 448). The review does not pick.
- Options: (a) Home, as designed; (b) `/library`, record the deviation.
- Cost: S.
- Related: OT-4/OT-5 (what "Add to library" does) are in the UX pass; a new account has an
  empty library, so the better landing page depends on that pass.

### OWN-8 Live checks only the owner can run [OWNER]

- Ids merged: D-OWN-7, D-OWN-8, HANDOFF.md 750-752.
- (1) Does the owner's authentik honour `max_age=0`? (ignored test
  `live_provider_honours_max_age_zero`, DEC-325; fallback in
  `docs/research/od-12-reauth-window.md` 4.4). (2) Does compose report "healthy within
  60 s" on a real Docker host (DEC-165)? (3) The Tauri spike on a host with disk
  (DEC-125).
- Blocks: (1) the re-auth design for password-less admins.

### OWN-9 Squash the migrations into one baseline [OWNER]

- Source: WP-D3 and architecture-review.md 597-600 ("before the first external
  deployment, squash 0001-0031 into one baseline (the owner's instance is the only
  one)").
- Question: May the owner's instance be re-created from an export (or its migration table
  be rewritten by hand) so that the 31 migrations become one baseline?
- Default: decide and record before the first external deployment.
- Options: (a) squash now, owner re-imports; (b) squash with a one-time script that
  rewrites the owner's migration table; (c) never squash.
- Cost: S-M.
- Blocks: OPS-I (publishing images is the first external deployment), reversible
  migration pairs (OPS-6 option b).
- Not verified: whether the owner's home instance holds data the owner wants to keep.

### OWN-10 Rotate the TMDB token [OWNER]

- The read access token was posted in the chat; "the owner will rotate it" (HANDOFF.md
  49). An action, listed so it is not lost.

### OWN-11 Merge the round-1 reviews into `main` and finalise the plan [OWNER]

- Question: After this walkthrough, are the eight review files merged into `main`, and
  is plan P0 to P4 final (owner-test-2026-11-10.md, Plan)?
- Default: merge the folder (HANDOFF.md 36-37), then P2 fix pack "ot1" (F3's bugs plus
  the accepted security fixes), P3 UX pack "ot2", P4 = W0 to W7.
- Options: (a) merge as `docs/fable/`; (b) merge into the folders the KICKOFF layout
  names for such outputs, `docs/reviews/` and `docs/research/` (HANDOFF.md 209-210).
- Note: rule 10 forbids model names in docs and file headers. The folder name on the
  branch is the alias of the review tool, and the layout rule itself uses that word for
  `docs/reviews/`. Whether a folder name counts is the owner's reading of rule 10. Not
  checked: whether the review files themselves carry such names in their text.
- Proposed content of "ot1" from this page: SEC-2, SEC-1, SEC-3, SEC-4, SEC-5 (as
  answered), SEC-9, OPS-1, OPS-2. Proposed order after that: section 0 of this file.

### OWN-12 More RAM for the VM [OWNER] (optional)

- HANDOFF.md 806. The VM has 4 vCPU / 16 GB and is shared with another agent
  (docs/handoff/fable-briefs/common.md). Relevant to ARCH-0 and PERF-T: every package
  ends with a full build, and a release load run needs about 15 GB of disk.

## 10. OT-* UX questions

OT-4, 5, 6, 7, 9, 10, 12, 13, 14, 15, 16, 17 (status model, log sheets, layout, colour,
stats led by hours): see `docs/fable/round2/ux-owner-test.md` (F2). Bugs OT-1, 2, 3, 8,
11, 15, 16: see `docs/fable/round2/bug-diagnosis.md` (F3). Neither file was read for this
page; whether they exist yet was not checked.

Points on this page that the UX pass should know: WP-OD3 (episode runtimes for OT-17),
DATA-0 (a second identity source and OT-3 ranking), OWN-5 (funnel date), OWN-7 (landing
page), OPS-3/4 (a possible wizard step), SEC-10 (a session list in Settings).

## 11. Proceed with defaults unless the owner objects

Pure engineering or hygiene; under rule 2 the reviewer's default is taken and logged as a
DEC. One line each: id, default, cost, source.

Hub
- HUB-8 review timers (4 days, 48 h destructive, 3 votes, 24 h objection, 14 days + 10
  changes); config values. S. hub-design.md 1520-1522.
- HUB-14 Postgres full-text + pg_trgm for hub search. S. hub-design.md 1534.

Security
- SEC-2 body limit on the pages router (the REST JSON limit, 2 MiB per the audit) + 413
  test. S. security-audit.md 525-542.
- SEC-7 stream JSONL/CSV parsing or cap concurrent import jobs. S-M `~`.
  security-audit.md 639-650.
- SEC-8 pin a sha256 per downloaded build tool in `src/sh/ci-tools.sh`. S.
  security-audit.md 652-666. Same change as step 3 of the ops plan.
- SEC-9 generic 500 text in `crates/api/src/hooks.rs:48-50`. S. security-audit.md 668-677.
- SEC-11 SSO hint cookie: accepted as is (addendum-1 4.2). None.
- SEC-12 CSRF guard without `Origin`: no fix required. None.

Ops
- OPS-1 ship `cli` in the image. S. ops-readiness.md 729.
- OPS-2 three keys in `.env.example`, reverse-proxy page. S. ops-readiness.md 730.
- OPS-5 rule for hot tables: batched backfills, `CONCURRENTLY` + `-- no-transaction`; log
  applied migration versions at start. S. ops-readiness.md 739.
- OPS-7 compose `stop_grace_period: 30s` (default 10 s is shorter than the 20 s job
  grace). S. ops-readiness.md 752.
- OPS-8 hashed file names for `/pkg` (or `Cache-Control: no-cache`), so an upgrade does
  not leave a stale wasm in the browser. S. ops-readiness.md 753. Do it with SEC-1.
- OPS-9 backup, restore and upgrade runbook with commands, restore order and
  verification. S. ops-readiness.md 496-576, 732.
- OPS-10 `/metrics` in Prometheus text format and an optional access-log line. M.
  ops-readiness.md 740. Lowest ops priority; skip until someone asks.
- OPS-11 worker heartbeat shown in the admin overview; `/healthz` checks the database
  pool and reports the search backend. M. ops-readiness.md 738.
- CI: build cache, no duplicate release compile per pipeline. S-M. ops-readiness.md 741.
  (Pushing images is OPS-I, an owner item.)

Architecture (order as in architecture-review.md 781-786)
- WP-A1 cache the parsed Fluent bundles per (locale, brand overrides) on the server. S,
  risk low. Same as PERF-4.
- WP-A2 batch the DTO builders (covers, users) to remove per-row queries. M, risk low.
  Contains PERF-3.
- WP-C2 architecture rules and leaf crates before any split; rename our `config` crate
  to `app-config`; `cli` no longer compiles `web`. S-M, risk low.
- WP-C1 typed contracts (enums instead of strings, wire format unchanged). M, risk M.
- WP-D1 decompose `Server::build`. M. WP-D2 one contract suite for MemStore and PgStore.
  M. Same goal as TEST-6.
- WP-D3 migration conventions and a docs page. S. (The baseline squash inside it is
  OWN-9.)
- WP-C3 dependency hygiene; includes "evaluate replacing `meilisearch-sdk` by a thin
  client". S-M.
- WP-E1 measure the wasm link profile. S. The choice that follows is OPS-B.
- WP-B3 and WP-B1 are engineering in content but large; their timing is ARCH-0.

Performance (perf-proposals.md 759-770)
- PERF-0 measure: `/people` in the load set, covers, a filtered query, a second zone
  read. S.
- PERF-1 remove the per-shared-session user read on `/people`. S.
- PERF-2 total count out of the library query on the first page; Home reads. M.
- PERF-3 batched cover lookups. S. PERF-4 i18n cache. S.
- PERF-5 zone-change rebuild (target <= 500 ms debug, <= 400 release). M.
- PERF-6 participant-driven shared reads, no new index. S.
- PERF-7 `/people` aggregates in SQL. M.
- PERF-8 unlogged stats cache tables (migration). S. PERF-9 library indexes and a
  generated `last_activity` column (migration). M.
- Not done by design: per-filter total caches, a Home cache, a companion-stats cache, an
  asset record cache, the global sessions time index (perf-proposals.md 784-787).

Tests (test-audit.md 576-604)
- TEST-2 a serial Playwright project for specs that change instance-wide state; per-test
  admin accounts. 1 day.
- TEST-3 break each never-seen-red test's code path once and confirm it goes red. 1 day.
- TEST-4 replace sleeps before negative assertions. 2 hours.
- TEST-5 HTTP tests for the nine untested REST routes, including "user B on A's goal".
  1 day.
- TEST-6 parity harness: the `app` scenarios again on Postgres. 1-2 days.
- TEST-7 template test database, merge the 25 server test binaries into about 5. 1 day.
- TEST-9 nightly job (axe in both themes and widths, repeats of three specs, the load
  test). Half a day; needs runner time.
- TEST-10 one test each for `--role web` + `--role worker`, the `fs` blob store (the
  compose default, untested), SSE resume, cookie attributes. 1 day.

Data sources
- WP-OD2 a licence tag per field and asset (`CC0-1.0`, `CC-BY-SA-4.0`, `ODbL-1.0`,
  `unspecified-open`, `proprietary:<connector>`); attribution page, retention job and hub
  filter keyed by it. M `~`. open-data-sources.md 384-388, 401. F4 covers attribution.
- WP-OD10 LibriVox connector, Discogs-dump hub seed: later, nice to have.

Decision log
- C2 correct DEC-328/331 (hosting change is not gated on the server) or gate it; a server
  decision per the review. S.
- C4 the History year filter uses the user's time zone, not UTC
  (`crates/app/src/activity.rs:49,190`). S. A real bug for people far from UTC.
- C5 record that the CLI is the proof for "reopen wizard"; email update and session
  revocation stay deferrals (see SEC-10). S.
- C11 vendor the TMDB logo per R-F3 with the attribution workstream. S.
- C12 unpin the cover on "Remove from library" or build the sweep. S.
- DLOG-2 fields `follow-up` and `supersedes`; DLOG-3 mark owner rulings and records;
  DLOG-4 repository check for DEC references; DLOG-5 the rule for new work; DLOG-6 the
  one-time migration. All follow from DLOG-1; if DLOG-1 is (c), none is done.

## 12. Merged duplicates and contradictions

### 12.1 Merged (asked once)

| # | Asked as | Merged ids | Why the same |
|---|---|---|---|
| M1 | SEC-5 | SEC-5, C8, C9, D-AUTH-2 | per-process throttles and per-client limits (security-audit.md 597-618; decisions-current-state.md 449-450, 518-521) |
| M2 | SEC-10 | SEC-10, C5 (session part), D-AUTH-3 (session part) | session revocation for members |
| M3 | SEC-6 | SEC-6, ops plan step 7, ops docs gap 9 | compose credentials and loopback bind (ops-readiness.md 720-722, 737) |
| M4 | SEC-1 (one package) | SEC-1, OPS-8, OPS-2, docs part of SEC-6 | SEC-1 exempts `/pkg` from `no-store`; OPS-8 wants `no-cache` or hashed names on `/pkg`; both and SEC-6 ask for the same docs page. One package |
| M5 | OWN-1 | HANDOFF 6 TMDB item, C1, D-OWN-4; DATA-0 as context | DEC-453 vs DEC-466 |
| M6 | HUB-15 | HUB-15, HANDOFF 4.6/6 barcode question, D-OWN-3 (part) | open product databases |
| M7 | HUB-7 | HUB-7, "retire `bgg`" in open-data-sources.md 380, D-OWN-1, HANDOFF 6 BGG deliveries | build the BGG connector or not |
| M8 | SEC-8 | SEC-8, ops plan step 3 (checksums) | `ci-tools.sh` integrity |
| M9 | WP-A1 / WP-A2 | WP-A1 = PERF-4 (I-A); WP-A2 contains PERF-3 (C-A) and overlaps PERF-1 | perf-proposals.md 688 names WP-A1 itself |
| M10 | WP-D3 | WP-D3, OPS-5, ops plan step 9, OPS-6 (c) | migration rules for hot tables |
| M11 | WP-D2 | WP-D2, TEST-6 | MemStore vs PgStore parity |
| M12 | OPS-B | WP-E1, the `WASM_LTO` argument of the proposed Dockerfile | wasm link profile |
| M13 | OWN-6, OWN-7 | D-OWN-9 = C3 + C7 | the two owner-visible deviations |
| M14 | OPS-12 | OPS-12, C8 (deployment side), TEST-10 (role-split test) | more than one process |
| M15 | TEST-8 | TEST-8, SEC-7 | concurrency of imports |
| M16 | OWN-2 | HANDOFF 6 first item, HANDOFF 4.5, D-OWN-6 | unreviewed entries |
| M17 | OWN-3 | HANDOFF 6 deliveries, D-OWN-1, D-OWN-2 | owner hand-overs |

### 12.2 Contradictions

- **X1 HUB-7 against an owner ruling.** hub-design.md 1517-1519 and open-data-sources.md
  380: do not build the BGG connector. HANDOFF.md 760: the owner said keep BGG for those
  with a token (DEC-190, DEC-193). Owner rulings win until the owner changes them.
- **X2 Audiobooks on the hub.** hub-design.md 1336: book / audiobook "not needed", no
  phase. open-data-sources.md 58 and 390-394: audiobook editions are a new hub kind, "the
  one kind with no open home". The data sources review is the later and more detailed
  one on this point and says it updates the hub's section 11. Asked as WP-OD8.
- **X3 DEC-162, the Backend layers.** architecture-review.md 713-721 (WP-B2): delete
  them, reverse DEC-162. decisions-current-state.md 447 (C6): accept reads through server
  functions and writes through REST as the architecture, close the "revisit" note. Asked
  as WP-B2.
- **X4 What is done first for performance.** architecture-review.md 672 and 781: WP-A1
  (i18n cache) first, then WP-A2. perf-proposals.md 759-764: measure first, then the
  `/people` N+1, then the query total; the i18n cache is step 4 because the load harness
  speaks REST only and cannot see it (perf-proposals.md 36-38). Also: the architecture
  review says `library_query` has "the right shape" (as quoted at perf-proposals.md
  49-52), while the performance review's largest step changes that query (P2-A, P2-B).
  Not a hard conflict (shape against cost), but two orders exist. Suggested resolution:
  the performance order, with WP-A1 taken at any time since it is S and independent.
- **X5 Is the import lane per user?** security-audit.md 649: "the queue lane already
  serialises per user". test-audit.md 248-253: the import lane is strictly ordered and
  shared, a per-user lane is the proposed fix. Code read for this page: one
  `user_import` lane (`crates/jobs/src/lib.rs:5`), every import is pushed with
  `Lane::UserImport` (`crates/importers/src/runner.rs:120`). The test audit's reading
  matches what was read; whether another mechanism serialises per user was not found and
  not excluded.
- **X6 Which sources must be off in a paid hosted instance.** hub-design.md 1312: TMDB
  and IGDB. open-data-sources.md 64: VNDB's API is "free for non-commercial use", and
  hub-design.md 1340 calls VNDB "open". A paid instance would need VNDB off as well.
  Affects HUB-12 only.
- **X7 Metron and Hardcover.** hub-design.md 1336-1337 lists "Hardcover" as a current
  book source and "Metron (open)". open-data-sources.md 55 and 422-424: Metron's licence
  was not found and its docs were unreachable; the Hardcover docs could not be opened
  (open-data-sources.md 421) and Hardcover is not recommended anywhere. Treat both as
  unverified. Asked as WP-OD9.
- **X8 Migration numbers.** hub-design.md 1358 and 1363 take `0032_hub.sql` and
  `0033_hub_client.sql`; perf-proposals.md 769-770 take 0032 and 0033 for the stats and
  library migrations. Whoever lands second renumbers. Engineering only.
- **X9 Wrong cross-reference.** open-data-sources.md 400 cites "DEC-191 rules:
  User-Agent, serial, maxlag" for the Wikidata client. DEC-191 is the re-authentication
  window (`docs/DECISIONS.md:195` [checked]); the Wikidata decision is DEC-192
  (`docs/DECISIONS.md:196` [checked]). hub-design.md 577 points at
  `docs/research/board-game-sources.md` 2.1 for the same rules.
- **X10 Counts.** HANDOFF.md 30 says the architecture review has 10 work packages; it
  lists 12 (architecture-review.md 672-779). Harmless.
- **X11 The log proposal against the KICKOFF layout.** See DLOG-1.
- **X12 SEC-4 against the default hosting mode.** Not between two reviews but between a
  review default and a ruling: see SEC-4a.
- **X13 SEC-5 against OD-17, SEC-3 against OD-13, SEC-10 against OD-11.** The fixes are
  compatible with the rulings but change behaviour the owner approved; each is flagged at
  its item.

No contradiction was found between the security audit and the ops review; the ops review
refers to SEC ids instead of repeating them (ops-readiness.md 14-15).

## 13. Not verified

- Nothing was built or run. Every code statement on this page is the cited review's,
  re-read here only where marked `[checked]` (seven places: the guard layers and the
  pages router in `apps/server/src/lib.rs`, the Dockerfile, `config/.env.example`,
  `compose.yml`, `playwright.config.ts`, the import lane, three lines of
  `docs/DECISIONS.md`).
- The reviews were read through their summaries, plans, finding lists and the sections
  cited above, not line by line: of hub-design.md (1,593 lines) sections 0, 3.6, 6.3,
  6.4, 7.4, 10 to 13; of the others the summary, the findings or packages, and the plan.
  A question raised only in a body section and not repeated in a plan or index may be
  missing here.
- Costs marked `~` are this file's estimates. The hub review and the data sources review
  give no effort figures.
- The reviews' own "not checked" lists stand: run-time headers and body limits, the
  client bundle, job queue fairness in depth (security-audit.md 729-738); build time,
  memory and image size, cargo-leptos and cargo-chef flags, the SSE path for the proxy
  snippet (ops-readiness.md 769-772); the `/people` numbers are predictions
  (perf-proposals.md 383-387).
- Web sources: no page was opened for this file. Licence and terms statements are the
  reviews', dated 2026-11-09. Pages the data sources review could not open: Discogs
  developers, Grand Comics Database, MobyGames, RAWG, Hardcover, Metron, Inducks, fyyd,
  Podcast Index, TheGamesDB, BookBrainz API docs, ListenBrainz data page, archive.org
  terms, Giant Bomb, TheAudioDB guide, one DNB page (open-data-sources.md 411-440). The
  GCD licence is `UNVERIFIED` there.
- Legal points are for a lawyer, not settled here: the revocable image licence (HUB-2),
  hashes of purged TMDB text (HUB-6), DSA scope (HUB-11), whether keeping title and year
  is compatible with TMDB terms (OWN-1), audit rows after account deletion (SEC-3).
- Not known: where the owner's identity provider and mail relay run (SEC-4a); whether the
  owner's instance holds data to keep (OWN-9, OPS-3); whether the CI runner is the shared
  VM (TEST-9); whether F2's and F3's files exist yet (section 10).
