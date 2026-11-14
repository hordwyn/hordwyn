# v1 acceptance checklist (owner, one evening)

For the owner's own server: a Linux host with Docker, a reverse proxy with TLS (Caddy or
nginx), reachable from the internet. Work top to bottom and tick each step. A step that
does not give the expected result is a finding: note the step number, the command and the
output (`dc logs app | tail -n 100` helps), and go on where that is possible. Findings and
the decision are collected in section I.

Sources: `docs/modules/ROOT/pages/` install.adoc, reverse-proxy.adoc, operations.adoc,
staging.adoc, v1-scope.adoc; review `docs/fable/round2/v1-final-review.md` (F-numbers below
refer to it). No command of this list was run on the build host: this run is the first
execution of the image build, the compose file and the runbook.

Placeholders: `tracker.example.org` is your host name; `<...>` is a value of yours. Never
paste a secret into a finding, a chat or a commit.

Which commit: the commit the orchestrator names as the acceptance commit (a green gate on
`main`, or the candidate `v1.0.0-rc.1`). Everything you enter before the candidate tag is
throwaway (F-20); the instance you keep is the one built from the candidate.

Time: A 30 min, B 30 to 60 min (mostly the image build), C to E 45 min, F 45 min, G 30 min,
H 45 min.

## A. Prepare

- [ ] **1. DNS and TLS.** The host name points at the proxy and the proxy has a certificate.

  ```sh
  dig +short tracker.example.org          # your public address
  docker compose version                  # Compose v2, 2.24 or newer
  free -g | head -2                       # 8 GB free for the build, else CARGO_BUILD_JOBS=1
  df -h .                                 # 20 GB free
  ```

- [ ] **2. Checkout.**

  ```sh
  git clone <repository address> tracker && cd tracker
  git checkout <acceptance commit or v1.0.0-rc.1>
  git rev-parse --short HEAD              # write it down
  ```

- [ ] **3. The environment file.**

  ```sh
  cp config/.env.example config/.env && chmod 600 config/.env
  openssl rand -hex 24                    # for POSTGRES_PASSWORD
  openssl rand -hex 24                    # for MEILI_MASTER_KEY
  ```

  Set these lines in `config/.env` (values without surrounding quotes; leave every other
  key empty):

  | Key | Value |
  |---|---|
  | `POSTGRES_PASSWORD` | the first random value (letters and digits only) |
  | `MEILI_MASTER_KEY` | the second random value |
  | `APP__SERVER__PUBLIC_URL` | `https://tracker.example.org` (scheme, no path, no trailing slash) |
  | `APP__SERVER__TRUSTED_PROXIES` | required. Proxy on the same host: `["172.16.0.0/12"]`. Proxy on another machine: `["<address of the proxy>"]` and also `APP_PUBLISH_ADDR=<LAN address of this host>` |
  | `APP__AUTH__SECURE_COOKIES` | empty (secure cookies stay on) |
  | `APP__INSTANCE__HOSTING` | `home` |
  | `APP__SERVER__HSTS_MAX_AGE_SECS` | `300` for this evening; empty again after the go (step 28) |
  | `SOURCE_TMDB_API_KEY` | the TMDB API read access token (the long one) |
  | `SOURCE_IGDB_API_KEY` | `<client_id>:<client_secret>` of the Twitch application |
  | `SOURCE_TVDB_API_KEY` | the TheTVDB key (optional) |
  | `SOURCE_PODCASTINDEX_API_KEY` | `<key>:<secret>` |
  | `SOURCE_OPERATOR_CONTACT` | your e-mail address (it is sent to Open Library and Wikidata, to no other source); or leave it empty and set it in step 11 |
  | `APP__OUTBOUND__ALLOW_PRIVATE_HOSTS` | only if a notification target is in your LAN: `["ntfy.lan"]` or `["10.0.0.5:8443"]`. Not needed for the identity provider or the mail relay on a home instance |

  A key set here is shown read-only in the admin area. Check the file:

  ```sh
  dc() { docker compose --env-file config/.env -f src/docker/compose.yml "$@"; }
  dc config --quiet && echo ok            # ok; a missing password is named here
  grep -c '^APP__SERVER__TRUSTED_PROXIES=\[' config/.env    # 1
  ```

  The `dc` helper is used in every later step; define it again in a new shell.

- [ ] **4. The proxy.** Configure the site as in reverse-proxy.adoc ("Caddy" or "nginx":
  pass `Host`, set `X-Forwarded-For` and `X-Forwarded-Proto`, 64 MiB bodies, no buffering
  on `/api/v1/events`), pointing at `127.0.0.1:3000` (or the LAN address). Set no security
  header at the proxy. Port 80 redirects to HTTPS.

- [ ] **5. Be ready for step 10.** The first person who opens a fresh instance creates the
  administrator (F-5). Have the browser open and go to step 10 as soon as step 7 says
  healthy; or keep the proxy site switched off until then.

## B. Build and start

- [ ] **6. Build and start.**

  ```sh
  dc up -d --build                        # note how long the build takes
  ```

  Expected: the image builds without an error (the last build step runs `cli help`), three
  containers start. If the build is killed for memory: `CARGO_BUILD_JOBS=1` in
  `config/.env`, again.

- [ ] **7. Healthy.**

  ```sh
  dc ps                                   # app, postgres, meilisearch: healthy
  curl -fsS http://127.0.0.1:3000/healthz # answers
  dc logs app | head -n 60
  ```

  Expected in the log: the migration `0001` (v1 baseline) is named before it runs, one line
  says the secrets key was generated, then `listening`. No `panicked`, no restart loop.
  Note the seconds from `up` to healthy (target: within 60 s after the build).

- [ ] **8. The admin tool and the settings as the application sees them.**

  ```sh
  dc exec app cli help
  dc exec app cli migrate --dry-run       # nothing pending
  dc exec app cli secrets status          # v1 loaded; no line says NOT LOADED
  dc exec app printenv APP__SERVER__PUBLIC_URL APP__SERVER__TRUSTED_PROXIES
  ```

- [ ] **9. Nothing but the application port is published.**

  ```sh
  ss -ltn | grep -E ':(3000|5432|7700)\b'
  ```

  Expected: one line, `127.0.0.1:3000` (or the LAN address you set). No 5432, no 7700.

## C. Setup wizard and first hour as admin

- [ ] **10. The wizard.** Open `https://tracker.example.org`. Expected: it redirects to
  `/setup`, "Create the admin account". Create it (10 characters or more). Then "How do you
  host?": Home, shown as fixed. Then "Data sources": the sources with their keys from the
  environment. Then "Ready".
  If the first page is a sign-in page and not the wizard, somebody else was first:
  `dc down -v`, then step 6 again.

- [ ] **11. Admin > Overview.** Every health row is fine: Database, Search, Storage,
  Background jobs, Data sources, Secrets at rest, and **Reverse proxy: "No forwarded
  requests from an unlisted proxy"**. Mail and Identity provider follow in D.
  Admin > Data sources: TMDB, IGDB, TheTVDB (if set), Podcast Index, Open Library and
  Wikidata are on; "Test" passes for each; the operator contact is shown (read-only if it
  came from the environment; otherwise enter it here now, Wikidata is not asked without it).

- [ ] **12. The secrets key, and the reminder.**

  ```sh
  mkdir -p backup && chmod 700 backup
  dc exec -T app cat /data/secrets.key > backup/secrets.key && chmod 600 backup/secrets.key
  grep -c '^v[0-9]*:' backup/secrets.key  # 1
  ```

  Copy `backup/secrets.key` off the host (password manager). Then Admin > Overview, "Back
  up the secrets key": "I have backed up the key". Expected: the reminder is gone after a
  reload. Then Settings > "Password and authenticator": set up an authenticator for the
  admin account and keep the recovery codes; sign out and in once with the code.

## D. Sign-in provider and mail in the LAN (skip what you do not use)

- [ ] **13. Identity provider.** Admin > Sign-in and SSO: add the provider (issuer URL,
  client id, client secret; register the redirect address the page shows at the provider).
  "Test" passes. If it says "its address is in a private network", the instance is not in
  home mode (step 3). Then, in a private browser window: sign in through the provider; sign
  out with the button (the browser ends up at the provider's sign-out or back at the
  sign-in page, not at an error or a blank page; try Safari too if you have one, DEC-656).
  `https://tracker.example.org/sign-in/local` still signs the admin in with the local
  password. Admin > Overview: "Identity provider" is fine.

- [ ] **14. Mail relay.** Admin > Instance > Mail: host, port, sender; "Send a test"
  arrives. Admin > Overview: "Mail" is fine.

## E. Seen from outside

Use a machine outside your network (a phone hotspot, a rented shell) for steps 15 and 17.

- [ ] **15. Headers and TLS.**

  ```sh
  curl -sI https://tracker.example.org/healthz
  curl -sI http://tracker.example.org/ | head -n 3            # a redirect to https
  curl -m 5 -sS http://<public address>:3000/healthz          # fails: refused or timeout
  ```

  Expected on the first: `200`, and each of these exactly once:
  `content-security-policy`, `x-content-type-options: nosniff`, `x-frame-options: DENY`,
  `referrer-policy: same-origin`, `permissions-policy`,
  `strict-transport-security: max-age=300`. A header twice means the proxy sets it too:
  remove it there.
  Then a public header checker (for example securityheaders.com or the Mozilla HTTP
  Observatory) and a TLS checker (for example SSL Labs) on the host name. Expected remarks,
  both accepted: inline style attributes are allowed in the policy (F-8), HSTS has no
  `includeSubDomains`. Anything else is a finding. In the browser's developer tools the
  session cookie is `Secure` and `HttpOnly`.

- [ ] **16. The proxy is believed.** Sign in from the outside machine. Admin > Audit log
  shows that machine's public address on the sign-in row, not `172.x` or the proxy's
  address. Admin > Overview, "Reverse proxy", still reads "No forwarded requests from an
  unlisted proxy". If not: correct `APP__SERVER__TRUSTED_PROXIES`, `dc up -d`, again. Do
  not go on to step 17 before this holds (F-7).

- [ ] **17. Throttling.** From the outside machine, eight wrong passwords for a name that
  does not matter (stay well below 30: the address is refused for 15 minutes from the
  thirtieth failure):

  ```sh
  for i in 1 2 3 4 5 6 7 8; do
    curl -s -o /dev/null -w '%{http_code} ' -X POST https://tracker.example.org/api/v1/auth/sign-in \
      -H 'content-type: application/json' -d '{"username":"nobody","password":"wrong-wrong-wrong"}'
  done; echo
  curl -si -X POST https://tracker.example.org/api/v1/auth/sign-in \
    -H 'content-type: application/json' -d '{"username":"nobody","password":"wrong-wrong-wrong"}' \
    | grep -i -E '^HTTP|retry-after'
  ```

  Expected: the first answers are `401`, then `429` appears; the last command shows `429`
  and a `Retry-After`. At the same moment, from your normal network, the admin signs in
  without any wait (the wait belongs to the other address).

- [ ] **18. Limits and live updates.**

  ```sh
  head -c 3145728 /dev/zero | curl -s -o /dev/null -w '%{http_code}\n' \
    -X POST --data-binary @- https://tracker.example.org/
  ```

  Expected: `413`. Then open one title in two browser windows and log it in one: the other
  changes without a reload (if not, the proxy buffers `/api/v1/events`).

## F. The six kinds

For each row: find it with the search (`Ctrl+K`), add it, log it, then look at the item
page, History and Stats. Tick the row when all four columns held.

| | Kind | Find and add | Log | Stats and History show |
|---|---|---|---|---|
| [ ] 19a | Movie | a film by title (TMDB), cover in the row | "Mark as watched", then once more | 1 movie, 2 viewings, hours from the runtime |
| [ ] 19b | Show | a series; of two with the same name the right one | three episodes one by one, a whole season at once, start and stop a rewatch | episodes; hours, or "runtimes not known yet" |
| [ ] 19c | Video game | a game (IGDB) | a session with duration, two people, a winner | plays, hours, win rate; the Plays tab lists it |
| [ ] 19d | Podcast | a podcast (Podcast Index) | two episodes; the rows say "Listened" | episodes |
| [ ] 19e | Book | a book by title, one by ISBN (Open Library) | set a page; the total reads "of N pages (estimate)"; "Change the page total" to the real number, the "(estimate)" goes; then finish it | pages read; the book counts as finished |
| [ ] 19f | Board game from Wikidata | a well-known game (Catan, Azul); no cover is expected; the page links Wikidata | a play with three people and scores; a co-op play without a result | plays, hours, win rate |
| [ ] 19g | Board game by hand | search a title Wikidata does not have, last row `Add "..." by hand...`, kind Board game, year, optional `https://` picture address | one play | counted like 19f; "Edit details" in the item menu changes title and year |

- [ ] **20. Podcasts are new ground** (never owner-tested, v1-plan B6): the source gives a
  number of episodes, not a list. If the episode rows are not usable for you, that is a
  finding before the tag.

- [ ] **21. Around the kinds.** One item of a kind without a source (an audiobook or a
  comic) added by hand and logged once. The same pages once on a phone (390 px): search,
  log dialog, a play, Stats. Once in German: no English leftovers. Covers: a new item
  shows its cover at once (loaded from the source's host) and still shows it after a
  minute and a reload (stored).

## G. Import and notifications

- [ ] **22. Import, as a check only.** Settings > Imports and export, format "Previous
  tracker export" (the Ryot JSON export). Upload the file. Expected: "Checking the file",
  then "What this file holds" with counts per kind, sample rows and the row errors. That
  preview is the dry run: nothing is in the library yet. Read it: movies, shows, video
  games, books and anime are counted; other kinds of the file are row errors (known). Stop
  here on a throwaway instance if you like; on the candidate instance run the import, then:

  ```sh
  dc exec app cli refresh-failed          # a dry run: lists items that never got their data
  dc exec app cli match-books             # a dry run: the match table for books without an id
  ```

  Apply either one (`--apply`) only after reading its output. Known: imported Completed
  books have no date and add no pages.

- [ ] **23. Notifications to one channel.** Settings > Notifications: add one channel
  (ntfy, Gotify, a webhook or e-mail) and send its test. Expected: it arrives, and the page
  shows the delivery. A target in the LAN is refused ("private network") unless its host is
  in `APP__OUTBOUND__ALLOW_PRIVATE_HOSTS`: try it once without the entry if you have such a
  target, then add the host, `dc up -d`, and it arrives.

## H. Backup, restore rehearsal, staging dry run

- [ ] **24. Backup** (operations.adoc, "Backup"; the application stays up).

  ```sh
  dc exec -T postgres pg_dump -Fc -U app app > backup/app-$(date +%F).dump
  dc exec -T app cat /data/secrets.key > backup/secrets.key && chmod 600 backup/secrets.key
  dc exec -T app tar -C /data/blobs -cf - up > backup/up-$(date +%F).tar
  dc exec -T postgres pg_restore --list < backup/app-$(date +%F).dump | grep -c 'TABLE DATA'
  tar -tf backup/up-$(date +%F).tar | head -3
  dc exec -T postgres psql -U app -d app -c 'SELECT count(*) FROM users'    # remember it
  ```

  Expected: a dump of some hundred kilobytes or more, a table count well above 50, the
  archive lists `up/...` (after the import of step 22; "No such file or directory" if
  nothing was ever uploaded). Copy `backup/` off the host.

- [ ] **25. Restore rehearsal beside the live instance** (operations.adoc, same title).
  The live instance is not touched.

  ```sh
  dcr() {
    APP_PUBLISH_ADDR=127.0.0.1 APP_PUBLISH_PORT=3001 APP_ROLE=web \
    APP_IMAGE=localhost/tracker-restore-app:local \
    docker compose -p tracker-restore --env-file config/.env -f src/docker/compose.yml "$@"
  }
  dcr build
  dcr up -d --wait postgres meilisearch
  dcr run --rm --no-deps -T --user 0 --entrypoint sh app -c \
    'cat > /data/secrets.key && chown app /data/secrets.key && chmod 600 /data/secrets.key' \
    < backup/secrets.key
  dcr exec -T postgres dropdb -U app --force --if-exists --maintenance-db=postgres app
  dcr exec -T postgres createdb -U app --maintenance-db=postgres app
  dcr exec -T postgres pg_restore --no-owner -U app -d app < backup/app-$(date +%F).dump
  dcr run --rm --no-deps -T --entrypoint sh app -c \
    'mkdir -p /data/blobs && tar -C /data/blobs -xf -' < backup/up-$(date +%F).tar
  dcr up -d
  dcr ps                                             # three services, healthy
  curl -fsS http://127.0.0.1:3001/healthz
  dcr exec app cli secrets status                    # no NOT LOADED
  dcr exec -T postgres psql -U app -d app -c 'SELECT count(*) FROM users'   # as in step 24
  ```

  Always type `dcr`, never a bare compose command with `-p tracker-restore`: the helper's
  `APP_ROLE=web` is what keeps the copy from sending your notifications a second time.
  Optional: `ssh -L 3001:127.0.0.1:3001 <host>` and open
  `http://127.0.0.1:3001/sign-in/local`; sign in with the local password (the copy shares
  the live public URL and sign-in provider, so a provider sign-in would end on the live
  instance, F-10) and look at Admin > Data sources (keys set, not locked). If the sign-in
  page just comes back, the cookie was dropped over plain HTTP: the four commands above are
  the check. Then remove the copy and confirm the live one is unharmed:

  ```sh
  dcr down -v && docker image rm localhost/tracker-restore-app:local
  dc ps                                              # still three, healthy
  ```

- [ ] **26. Staging, as a dry run** (staging.adoc). There is no older release to upgrade
  from, so this rehearses the procedure with the same version; from v1.0.1 on it is the
  real check.

  ```sh
  git worktree add ../tracker-staging "$(git rev-parse HEAD)"
  cp config/.env ../tracker-staging/config/.env && chmod 600 ../tracker-staging/config/.env
  ln -s "$PWD/backup" ../tracker-staging/backup
  cd ../tracker-staging
  dcs() {
    APP_PUBLISH_ADDR=127.0.0.1 APP_PUBLISH_PORT=3002 APP_ROLE=web \
    APP_IMAGE=localhost/tracker-staging-app:local \
    docker compose -p tracker-staging --env-file config/.env -f src/docker/compose.yml "$@"
  }
  dcs build
  ```

  Then the restore commands of step 25 with `dcs` in place of `dcr`, up to and including
  the `tar` line (do not start the application yet), and:

  ```sh
  dcs run --rm --no-deps --entrypoint cli app migrate --dry-run   # nothing pending (same version)
  dcs up -d
  dcs logs app | tail -n 20                                       # listening, no error
  dcs ps && curl -fsS http://127.0.0.1:3002/healthz
  dcs exec app cli secrets status                                 # no NOT LOADED
  dcs exec -T postgres psql -U app -d app -c 'SELECT count(*) FROM users'
  dcs exec app cli reindex --clear --now                          # ends with "indexed them"
  ```

  Remove it:

  ```sh
  dcs down -v && docker image rm localhost/tracker-staging-app:local
  cd ../tracker && git worktree remove --force ../tracker-staging
  dc ps                                                           # production: three, healthy
  ```

  Note what was unclear on the staging page while doing it.

## I. Go or no-go

- [ ] **27. Collect.** Steps ticked: ___ of 26. Findings (step, what happened, expected):

  | Step | Finding | Blocks the tag? |
  |---|---|---|
  | | | |

  Owner decisions (review section 6). Two have an orchestrator default you may reverse: v1
  ships without publishing the website, with a non-strict site build (F-1, DEC-831); the
  candidate is confirmed by a fresh install of `v1.0.0-rc.1` passing this list, staging
  once as a dry run (F-4, DEC-832). Still open: inline style attributes accepted
  (F-8); a second account before account deletion and the session cap are built (F-9); the
  two book questions; the operator contact value.

  **Go** when: steps 6 to 12, 15 to 18, 19a to 19g, 24 and 25 hold, no finding is marked as
  blocking, and the decisions above are answered. **No-go** otherwise: the findings go back
  to the orchestrator, are fixed on `main`, and the failed steps are repeated.

- [ ] **28. What happens next** (release checklist, `docs/HANDOFF.md` section 1).

  1. Orchestrator: changelog date, release notes date, version 1.0.0 with `Cargo.lock`,
     website release file and the three "coming" marks (site built without `--strict`, not
     published), the migration fixture; full gate;
     commit; tag `v1.0.0-rc.1`; push.
  2. Owner: if this run was on an earlier commit, a fresh install of `v1.0.0-rc.1`
     (`dc down -v`, `git fetch --tags`, `git checkout v1.0.0-rc.1`, steps 6 to 12 and 15 to
     18 again, one item per kind). If this run was already on `v1.0.0-rc.1`, nothing is
     repeated. Say "confirmed" or name the step that failed (then `rc.2`).
  3. Orchestrator: `git tag -a v1.0.0 -m "v1.0.0" v1.0.0-rc.1^{commit}`, push. Nothing
     changes between the candidate and the tag.
  4. Owner, production: the candidate instance is production; `git fetch --tags` and
     `git checkout v1.0.0` (the same commit, no rebuild needed). Set
     `APP__SERVER__HSTS_MAX_AGE_SECS=` back to empty (one year) and `dc up -d`. Take the
     first real backup (step 24), copy it and the key off the host, and schedule the daily
     dump (keep seven daily, one per month). From here on every upgrade goes through
     Staging first, then Operations, Upgrade.
