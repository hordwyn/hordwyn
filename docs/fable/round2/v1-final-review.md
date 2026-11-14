# v1.0.0 final review (2026-11-14, main at 8bb4d38)

Read-only review before the `v1.0.0` tag. Nothing was built, started or clicked: every
statement rests on the files named. Scope: a fresh install as install.adoc describes it,
internet exposure, the six kinds, the backup / restore / upgrade / staging docs against the
CLI and the release checklist, and what the owner has to decide before tagging. The
owner's run is in `docs/handoff/v1-acceptance.md`.

Severity: High = the tag cannot be set as planned, or a first install fails; Medium = a
real risk or contradiction with a way around; Low = wording, hygiene, rare. "Blocks" means:
decide or fix before `v1.0.0`; everything else can follow after v1.

## 0. Summary

- **Counts: 2 High, 8 Medium, 10 Low.** Four findings block `v1.0.0` (F-1 to F-4); none of
  them is a defect in the application code. Three are a decision or a few lines in the
  release commit, one is the acceptance run itself.
- No finding of the security reviews sec0, sec1 and sec2 that was deferred "before v1" is
  still open. What is open from them was accepted or deferred to after v1 in a decision
  (section 3); three of these are worth an explicit yes from the owner (F-7, F-8, F-9).
- The install path reads consistently end to end: `config/.env.example`, compose, the
  Dockerfile, install.adoc, reverse-proxy.adoc and the configuration reference agree on the
  key names, the `SOURCE_` prefix, the required passwords, the loopback publish address and
  the trusted-proxy rule. Empty values in `config/.env` are "not set" in the application
  (`src/rust/crates/config/src/lib.rs:688-691`), so the copied example file is safe.
- The docs and the CLI agree: `cli migrate --dry-run`, `cli reindex [--clear] [--now]`,
  `cli secrets status`, `cli refresh-failed`, `cli match-books`
  (`src/rust/apps/cli/src/main.rs:105-175`, `:355`), `cargo xtask upgrade-test`
  (`src/rust/xtask/src/main.rs:57`), and `v1.0.0-rc.1` is not taken for a release tag
  (`src/rust/xtask/src/upgrade.rs:26-31`).

## 1. Findings that block v1.0.0

| Id | Sev | Finding | Scenario | What unblocks it |
|---|---|---|---|---|
| F-1 | High | Release checklist step 4 ends with `cargo xtask site --strict` (`docs/HANDOFF.md:174-178`). Strict mode refuses while an owner slot is open, and 3 are open in `src/rust/apps/website/content/site.toml:25,31,32` (source repository, security contact, expiry), 2 in `en/security.md:25,29`, 10 in `en/imprint.md`, 4 in `de/privacy.md`, the same in the other locale | The orchestrator works through the checklist for `v1.0.0-rc.1`, step 4 fails, and the choice is made under time pressure: skip a step of a checklist that was written to be followed, or invent an imprint | Owner decision: either fill the slots before the candidate (repository address, security contact, disclosure policy, imprint, privacy), or change step 4 for v1 to `cargo xtask site` without `--strict` and say that publishing stays off (`PAGES_DEPLOY`, `.gitlab-ci.yml:245`) |
| F-2 | High | The container path has never been executed anywhere: the image build (incl. `cp target/release/hash.txt`, `src/docker/Dockerfile:30`, "not verified" in DEC-659), the compose start, every runbook command (DEC-625: "the owner's acceptance run is the first execution"; DEC-644), the Caddy and nginx snippets (DEC-664), `podman compose`. install.adoc:122-126 says so itself | The owner tags, then the first production build stops at a missing file or a compose option the installed version does not know | The acceptance run (steps 6 to 9 and 22 to 26 of the checklist) on the candidate commit. Before the candidate tag, also confirm that the CI job `docker-image` is green on that commit; whether it ever ran on a runner was not verifiable here |
| F-3 | Medium | Step 4 switches `book`, `board_game` and `manual_entry` to "shipped" (`en/features.toml:46,90,168`, `en/home.toml:48,66`, same lines in `de/features.toml`, `de/home.toml:52,70`), but the sentence beside them stays: "Catalogue sources for them come later, and so does entering a title by hand" (`en/home.toml:69`, `de/home.toml:28`) | After the switch the home page says by hand entry is shipped (features) and coming later (home) | Rewrite that sentence in both locales in the release commit and name it in step 4. The prepared id list in `release.toml:21-26` is complete: 17 ids (6 kinds, 10 `everything.points` ids, `security_headers`), all in v1 |
| F-4 | Medium | Step 8 "the owner upgrades the staging copy from a fresh production backup" (`docs/HANDOFF.md:194-197`) has no meaning for the first release: there is no production. Neither the checklist nor staging.adoc says what confirms `v1.0.0-rc.1` | The candidate is tagged and nobody knows which run releases it; or the acceptance run is done on a commit before the release commit (version 0.1.0) and the tag goes on a commit nobody started | Owner ruling, one sentence in step 8: for v1 the confirmation is a fresh install of `v1.0.0-rc.1` passing `docs/handoff/v1-acceptance.md`, with the staging procedure done once as a dry run (checklist step 26) |

## 2. Fresh install behind Caddy or nginx

Would it work end to end? By reading: yes, with the notes below. Checked: required
variables fail loudly (`src/docker/compose.yml:35,37,92,105`); `environment` wins over the
empty service keys of the env file (`config/.env.example:70-72`); an empty
`APP__SECRETS__KEY_FILE` falls back to `data/secrets.key`, which is `/data/secrets.key`
with `WORKDIR /` (`src/docker/Dockerfile:60-64`); `config/.env` is not in the build context
(`.dockerignore`); the Postgres 18 volume path is the one the image expects
(`compose.yml:94`); only the application port is published, on loopback (`compose.yml:43`);
`X-Forwarded-For` is walked from the right and only believed from a listed peer
(`src/rust/crates/api/src/request_meta.rs:118-147`), which fits both snippets; a value of
`APP__SERVER__TRUSTED_PROXIES` that is not a list or not a network stops the start with a
message (`config/src/lib.rs:823-827`), it is never silently ignored.

| Id | Sev | Finding | Scenario | Blocks |
|---|---|---|---|---|
| F-5 | Medium | The first visitor of a fresh instance becomes the administrator (`src/rust/crates/auth/src/lib.rs:386`, wizard step 1 without a session: `crates/api/src/admin_routes.rs:184-196`); install.adoc:92-93 says "Open the public URL" and no more | DNS and proxy are live, the owner starts compose and goes for a coffee; whoever opens the address first owns the instance and its data-source keys from the environment | No. One paragraph in install.adoc ("create the account at once; if the wizard does not ask for an account, `down -v` and start again"). The checklist does it that way (step 10) |
| F-6 | Medium | install.adoc does not name what an internet-facing instance needs beyond the table: the operator contact (board games are not searched without it; only v1-scope.adoc:45 and `.env.example:100-106` say so), an authenticator for the admin account (operations.adoc:540-541 relies on it), a short `APP__SERVER__HSTS_MAX_AGE_SECS` while trying the address (reverse-proxy.adoc:133-137), and a minimum Compose version (the long `env_file` form with `required: false`, `compose.yml:39-41`, needs a current Compose v2; an old distribution package fails at parse time) | The owner follows install.adoc only: board game search finds nothing, the admin has a password only, and a browser pins HTTPS for a year on a name that was a trial | No. A short "After the first start" list on the page |
| F-7 | Medium | A proxy that is not listed turns the address budget into a lock for everybody: 30 wrong sign-ins from anywhere in 15 minutes refuse every sign-in, the local one included, and a right password does not pass (`crates/auth/src/throttle.rs:417`, DEC-780 "not built"). `config/.env.example:20` ships the key empty; the only signals are the log line and the "Reverse proxy" row, and that row only sees proxies on private addresses | A typo in the list (or a rootless runtime with another gateway address) and one scanner keeps the owner out until a restart | No, if step 16 of the checklist is done: the row must read "No forwarded requests from an unlisted proxy" and the audit log must show the owner's own address |
| F-8 | Medium | The policy keeps `style-src-attr 'unsafe-inline'` (`src/rust/apps/server/src/security_headers.rs:136`); accepted for v1 in DEC-661 with a follow-up package `sec1b` | A later markup injection in a title or note could draw a fake sign-in over the page although scripts stay blocked; none is known. A public header checker will name it | No. Owner: confirm the acceptance |
| F-9 | Medium | Not built and deferred "while the owner is the only user" (v1-plan addendum 2026-11-12): account deletion leaves search documents, import files and the name in audit rows (SEC-3); sessions slide for 30 days without a hard cap, no session list, no "sign out everywhere" (SEC-10; `crates/config/src/lib.rs:347`). The changelog advertises "several users per instance, invitations" (`CHANGELOG.md:44-45`) | A second household member gets an account in week one; a lost phone stays signed in for as long as it is used | No for the owner alone. Owner decision: no second account before sec3 / sec4, or accept it for the household |
| F-10 | Medium | A staging copy or restore rehearsal runs with production's `config/.env`: same public URL, secure cookies, same sign-in provider. The browser line of the staging checklist (staging.adoc:113-117) may not be reachable (cookie dropped over plain HTTP; a provider sends the browser back to production), and nothing but the `APP_ROLE=web` in the shell helper keeps a copy from sending every due notification again (operations.adoc:263-274) | The owner signs in only through the identity provider: the staging copy cannot be opened in a browser, the check falls back to four commands. Or `dcs up -d` is typed in a new shell without the helper's variables | No. After v1: say on both pages what to change in the copy's own `config/.env` (public URL, cookies) and add a guard |

Low:

- F-11: reverse-proxy.adoc:262-265, check 6 ("four times, then once more ... Try again in
  1 second") cannot be seen by hand: the wait after the fourth failure is one second and is
  over before a person has typed again; each later failure doubles it. It needs seven or
  eight quick failures, or a loop (checklist step 17).
- F-12: `config/.env.example:88-89` lists `SOURCE_BGG_TOKEN` "the connector is not built
  yet"; no code reads it and the ruling is "no BGG connector" (HANDOFF 4.7, HUB-7). Remove.
- F-13: `config/.env.example:79-87` does not say what a key looks like: TMDB wants the API
  read access token, IGDB `<client_id>:<client_secret>`, Podcast Index `<key>:<secret>`
  (connectors.adoc:15,24,31, which names the short `SOURCE_` form for TMDB only).
- F-14: The docs write the secrets key and the dumps into the checkout (install.adoc:104-106
  `secrets.key`, operations.adoc:171 `backup/`); neither is in `.gitignore` or
  `.dockerignore`. They are untracked files in a git checkout and part of the build context
  sent to the daemon (not of the image: the Dockerfile copies named paths only).
- F-15: Environment values are read as TOML first (`config/src/lib.rs:647-652`): a value of
  digits only for a text key (a numeric OIDC client id, a numeric SMTP password) becomes a
  number. Expected result: a start-up error naming the type; not run. Quoting the value
  (`"123456"`) avoids it; the docs do not say so.
- F-16: `docs/HANDOFF.md` section 0 has no line for the merge of v1-release (8bb4d38) and
  its gate numbers, and line 23 still says the package is in a lane.

## 3. Internet exposure

Closed, checked against the decisions and spot-checked in the code: headers and policy on
every answer, HSTS for an https public URL, no-store on session answers (sec1); 413 for
bodies over the limit on every path, read timeouts, HTTP/1 only (sec0, sec2); sign-in
throttling per pair, per address and per account for second factors; outbound confinement
with one allowlist; sign-up is `invite` by default (configuration.adoc:19); compose has no
default password and publishes on loopback. All sec0 findings are applied (the one deferral,
DEC-653, was closed by DEC-769); all 14 sec2 findings and all sec1 findings except SEC1-5
are applied.

Still open from the security reviews, all with a decision:

| Item | Decision | Judgement |
|---|---|---|
| SEC1-5 inline style attributes | DEC-661, accepted for v1, `sec1b` later | F-8, owner confirms |
| SEC2-4 right password through an exhausted address budget | DEC-780, not built | F-7, covered by the acceptance check |
| SEC2-1 no notice to the admin after repeated wrong one-time codes | DEC-770, out of scope | After v1. Someone who has the password can try about 800 codes a day unseen; the per-account wait is the defence |
| Counters in memory, per process | DEC-762 | After v1; in the known issues of the release notes |
| `username-available` budget, invite and share tokens not throttled | sec2-progress "noticed", no DEC | After v1; the tokens are random and sign-up is closed |
| `form-action` on the provider's end-session redirect never run in a browser that enforces it | DEC-656 | Acceptance check (step 13), with the LAN provider |
| No report endpoint for the policy | DEC-656 | After v1 |
| SEC-3, SEC-10 (round 1) | v1-plan addendum | F-9 |

Nothing makes the default configuration unsafe on the internet by itself. The two defaults
that need the owner's hand are the trusted proxy list (F-7) and the first account (F-5).

## 4. The six kinds

From the code and the gate numbers in HANDOFF section 0 (not clicked): each kind has a
source connector (`src/rust/crates/connectors/src/{tmdb,tvdb,igdb,podcastindex,openlibrary,wikidata}.rs`),
by-hand entry, logging and statistics. What the docs and the website claim matches the code
where it was checked: seven notification channels (`crates/notify/src/model.rs:57-63`; web
push exists as a type and is not claimed), import with a preview before anything is written
(`crates/importers/src/lib.rs:119,150,386`), Trakt, the legacy export and the own format.

Notes for the acceptance run, not defects:

- Podcasts were never owner-tested and the source gives a count, no episode list
  (v1-plan.md:812). Step 20 is its first real use; a finding there comes before the tag.
- Board games: Wikidata is not asked without the operator contact, has no covers and about
  4,700 games (v1-scope.adoc:44-48). A search that finds nothing is expected for small
  titles; the by-hand row is the way in.
- Books: a Completed set by hand without a logged step is undated and counts in no month,
  year or goal (DEC-725, "the owner may reverse it"); books the legacy import set to
  Completed add no pages (v1-plan Q-C). Owner decisions, section 6.
- A show added by hand has no episode total (v1-scope.adoc:100-108, documented).
- The UI calls the Ryot file "Previous tracker export" (`crates/web/locales/en.ftl:2014`).

Low:

- F-17: the website's self-hosting page says outgoing requests go to the data sources and
  the notification channels (`en/self-hosting.md`, "Your data stays yours"; `en/home.toml`
  "Nothing phones home"). Not said there: browsers load covers that are not stored yet
  straight from the sources' hosts (operations.adoc:432-447), the operator contact is sent
  to Open Library and Wikidata, and sign-in providers and the mail server are called too.
- F-18: after the switch to "shipped" the features page promises "Search and add from
  Wikidata" (`en/features.toml:92`) without the condition (operator contact) that the docs
  and the changelog name.
- F-19: wording left for the owner: "Log a play of ..." and the tab "Plays" on a video
  game while the toast says "Session" (DEC-689, POL-16.4).

## 5. Backup, restore, upgrade, staging, release checklist

Consistent with each other and with the CLI: the `dc` helper and the three project names
and ports (3000, 3001, 3002); restore order (services, key, database, files, start,
reindex); `--now` on the worker-less staging copy and the text `indexed them` it waits for
(`apps/cli/src/main.rs:355`); `NOT LOADED` (`:457`); dry run before `up`; candidate tags
ignored by `upgrade-test`; the baseline's checksum pinned in
`crates/store-pg/tests/fixtures/released-migrations.txt`; nav entries for Staging and
Releases. The release notes and the changelog agree on scope and on the `SOURCE_` names.

Besides F-1 to F-4 and F-10:

- F-20 (Low): until the tag the baseline may still change (greenfield rule, HANDOFF 4.8),
  and a database made by the acceptance run then refuses to start on the next build (the
  checksum of `0001` differs). Either treat everything entered during acceptance as
  throwaway until `v1.0.0-rc.1`, or freeze the baseline now. The checklist assumes
  throwaway data before the candidate and real data from the candidate on.

## 6. For the owner to decide before tagging

1. F-1: fill the website slots now, or tag v1 with a non-strict site build and publishing
   off.
2. F-4: what confirms `v1.0.0-rc.1` (proposal: this acceptance checklist on a fresh install
   of the candidate).
3. F-8: accept inline style attributes in the policy for v1 (DEC-661).
4. F-9: a second account before sec3 / sec4, yes or no.
5. F-20: is the baseline frozen from now on, or from the candidate on.
6. Books: should a Completed set by hand count in the year and the goals (DEC-725); should
   imported Completed books count their pages (v1-plan Q-C).
7. The operator contact value that is sent to Open Library and Wikidata (v1-plan Q-D); the
   Open Library registration form (Q-E).
8. Standing: the wording of F-19; the 53 defaults of the round 2 page and the unreviewed
   DECISIONS entries stay in force unreviewed (HANDOFF 4.5, 4.7).

## 7. Not verified

- Nothing was run. The image build, the compose start, the proxy snippets and every command
  of the checklist are unexecuted until the owner's run (F-2).
- Image tags (`rust:1.99.1-slim-trixie`, `postgres:18.6-alpine`,
  `getmeili/meilisearch:v1.56.1`) were not looked up; a wrong tag fails at the first pull.
- Whether a browser keeps a `Secure` cookie on `http://127.0.0.1:<port>` through an SSH
  tunnel (the rehearsal and staging pages allow for both outcomes).
- The minimum Compose version in F-6 is from memory of the Compose release that introduced
  the long `env_file` form, not from a lookup.
- The reviews other than sec0, sec1 and sec2 were read by finding, decision and progress
  table, not line by line; the deferrals of v1-books, v1-manual, v1-polish and site that are
  still open (DEC-685, 732, 753, 754, 756, 757, 709) concern hosted mode, the website or
  rare cases and are all "after v1".
- F-15 is from reading the loader; the exact message was not produced.
