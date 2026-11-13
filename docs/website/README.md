# The project website

The website of the product (landing page, features, self-hosting, hub, changelog, security,
credits, imprint, privacy) is a static site generated from this repository. It is published
with GitLab Pages at the custom domain, with the Antora documentation below `/docs/`.

## Where things are

| What | Where |
|---|---|
| Generator (Leptos views rendered once, no hydration) | `src/rust/apps/website/` |
| Content: every text, the product name, addresses | `src/rust/apps/website/content/` |
| Stylesheet (the application's `input.css` plus the website's sources) | `src/web/style/site.css` |
| Theme script (the only JavaScript, 0.6 kB) | `src/web/site/theme.js` |
| Browser checks (axe, widths, theme, menu, pictures) | `src/web/site-tests/`, `src/web/playwright.site.config.ts` |
| Captures of the application and the script that makes them | `src/rust/apps/website/content/shots/`, `src/web/site-tests/capture/` |
| The publishing gate | `src/rust/apps/website/src/gate.rs`, `content/release.toml` |
| Assembly of what Pages publishes | `src/sh/pages-assemble.sh` |
| CI jobs `website`, `website-browser`, `pages` | `.gitlab-ci.yml` |

The pages use the components of the `ui` crate (`ButtonLink`, `Card`, `Badge`, `Alert`,
`Icon`, `Tint`) and the tokens, font and colours of `config/brand/default`, so the website
and the application share one design basis. Layout that only a website needs (top bar, hero,
footer, sections) lives in the website crate.

## Content

- `content/site.toml`: product name, public address, documentation and hub addresses, the
  source repository and the security contact. The name is written down here only; content
  files say `{name}`, code never names the product (a test checks both).
- `content/<locale>/strings.toml`: labels of the navigation and the footer.
- `content/<locale>/home.toml`, `features.toml`: the structured pages.
- `content/<locale>/<page>.md`: text pages, Markdown with a `+++` TOML head (`title`,
  `description`, `eyebrow`, optional `updated` and `notice`).
- Tokens: `{name}`, `{root}` (empty or `/de`), `{base}`, `{docs}`, `{install}`, `{hub}`,
  `{source}` (the repository address), `{security_contact}` (a link, from `site.toml`),
  `{kinds_shipped}` (the number of kinds marked shipped; `home.toml` only). A token nobody
  fills fails a test. The name must not contain `"`, `\`, `{` or `}` (tokens are filled into
  the TOML text before it is parsed).
- `content/release.toml`: the release gate, see below.
- A block of a text page between `<!-- shipped:ID -->` and `<!-- /shipped -->` is shown only
  when `release.toml` confirms `ID` (used for the security headers sentence and for the
  credits of Open Library and Wikidata).
- The documentation is English only: links to it from German pages are written as
  `<a href="{docs}" hreflang="en">` (a test checks every one); Old English words are
  written `<i lang="ang">`.
- English lives at `/`, German at `/de/`. Both locales must have the same shape (a test
  compares keys, kinds, sections and open slots). English files are ASCII.

### What blocks publishing (the gate)

`cargo run -p website -- check` lists everything that blocks publishing; `--strict` (also
`cargo xtask site --strict`) fails while the list is not empty, and the `pages` job publishes
only a build whose list is empty. The gate is `website::gate::publish_blockers` and is tested.
It blocks on:

1. **Open owner slots**: every line of a `.toml` or `.md` content file that carries
   `PLACEHOLDER` (comment lines of TOML files excepted; a Markdown heading counts). Nothing
   was invented for them. Fill the value and delete the marker. They are:
   - `site.toml`: `source_url` (the public repository, `https:`; the self-hosting guide starts
     with `git clone` of it), `security_contact` and `security_expires`.
   - `imprint.md` (en, de): provider name, address, e-mail, second contact channel, and where
     they apply representative, register, VAT id and the person responsible for the content.
   - `privacy.md` (en, de): controller, hosting details and legal basis, e-mail contact,
     supervisory authority. The text is a draft and needs a legal check.
   - `security.md` (en, de): the disclosure policy ("What to expect") and the scope with the
     supported versions. These are the owner's decisions; no promise is written for them.
     The contact line is filled from `security_contact`.
2. **A page that still shows its notice.** The warning box of a page is derived from its open
   slots: it appears while the page has one and disappears with the last (the `notice` line
   in the page head is only its wording and may stay).
3. **security.txt** (RFC 9116): `security_contact` must be a `mailto:` or `https:` URI,
   `security_expires` an RFC 3339 date and time (`2027-11-01T00:00:00Z`) that is in the
   future and at most a year ahead. Renew it before it runs out: an expired date blocks the
   next deployment.
4. **The release gate** (`content/release.toml`): every kind and feature that
   `features.toml` or `home.toml` marks `status = "shipped"` must be listed in `confirmed`,
   and `version` must name the tag they were checked against. Set this when `v1` is tagged,
   after reading the features page against the tag: flip `book`, `board_game` and
   `manual_entry` from `coming` to `shipped` only if they are in the release, then list every
   shipped id. Until then the site shows them as "Coming" and cannot be published.
5. **The documentation** (read from `docs/antora-playbook.yml`): its title must carry the
   product name (today it is the working title, changed by the rename lane), and
   `kroki-fetch-diagram` must be `true`, so the diagrams are fetched when the documentation is
   built and no reader's browser loads them from kroki.io.

Before publishing, also read and confirm what the gate cannot judge: the hub page (CC0,
invitation-only start, moderation from the first day) against the hub requirements, and the
privacy draft.

### Captures of the application

`content/shots/` holds real captures of the application with invented sample data, per view,
language and theme (`content/shots/README.md`). `home.toml` names the views and their text
alternatives (`[screenshots.hero]`, `.wide`, `[[screenshots.phones]]`); a missing file fails
the build. No provider image is shown: the mock catalogue serves none. Redo them after a
visible change of the application or of the product name:

```sh
cd src/rust && cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery
cd ../web && node site-tests/capture/capture.mjs    # about 3 minutes, ports 3510/3511, database ls_shots
```

The script starts the application under the product name of `site.toml` (a temporary copy of
the default brand), so the pictures never show a working name.

## Build and check locally

```sh
nix develop
cd src/rust
cargo xtask site                      # Tailwind, then the generator -> <repo>/build/public
cargo test -p website                 # content, structure, links, parity of the locales
cd ../web && npm ci
npx playwright test -c playwright.site.config.ts   # serves build/public on port 3500
```

`SITE_PORT` changes the port, `SITE_SHOTS=<dir>` also writes full-page screenshots (1280 and
390 px, light and dark), `SITE_STRICT=1` makes the browser test insist on a real
`security.txt`. `cargo xtask site --strict` fails while anything blocks publishing. The
output directory is emptied on every build (only if it is empty or an earlier build).

## Publishing (GitLab Pages)

Nothing is published until the owner turns it on:

1. Settle everything `cargo run -p website -- check` lists (slots, security.txt, the source
   address, `release.toml`, the documentation).
2. Project > Settings > CI/CD > Variables: add `PAGES_DEPLOY` = `true`. The job `pages` then
   runs on every pipeline of the default branch. It compiles nothing: it takes the build of
   the job `website` after `website-browser` has passed on it, refuses when the gate report
   of that build (`build/website-blockers.txt`) is not empty, copies the Antora site to
   `public/docs/`, and then checks that every `/docs/` link of the website exists in the
   Antora output and that no published page loads a picture, script or stylesheet from
   another server (`src/sh/pages-assemble.sh`).
3. The project is private: set Settings > General > Visibility > Pages to "Everyone", or the
   site asks visitors to sign in to GitLab.
4. Deploy > Pages: turn off "Use unique domain" if the default address should be the plain
   `<namespace>.gitlab.io/<project>` one. The pages use root-relative addresses (`/assets/`,
   `/de/`), so the site only works at the root of a host: use the custom domain (or the
   unique domain) to look at it, not the `/<project>/` path address.

### Custom domain and DNS (owner)

In Deploy > Pages > New Domain add `hordwyn.com` (and `www.hordwyn.com`), leave "Automatic
certificate management using Let's Encrypt" on. The dialog shows the exact records; for
GitLab.com they are, at the time of writing:

| Name | Type | Value |
|---|---|---|
| `hordwyn.com` | A | `35.185.44.232` |
| `hordwyn.com` | AAAA | `2600:1901:0:7b8a::` |
| `_gitlab-pages-verification-code.hordwyn.com` | TXT | `gitlab-pages-verification-code=<code from the dialog>` |
| `www.hordwyn.com` | CNAME | `hordwyn.gitlab.io.` |
| `_gitlab-pages-verification-code.www.hordwyn.com` | TXT | `gitlab-pages-verification-code=<code of the www domain>` |

- The dialog is authoritative. The table matches GitLab's page "Custom domains and SSL/TLS
  certificates for GitLab Pages"
  (https://docs.gitlab.com/user/project/pages/custom_domains_ssl_tls_certification/) as read
  on 2026-11-13; it was not checked against a live zone from here.
- The project lives in a subgroup, so its default address is
  `<namespace>.gitlab.io/<subgroup path>/<project>` and shows the old working name of the
  repository path; with "Use unique domain" on, the generated host name starts with the
  project slug as well. Once the custom domain works, set it as the primary domain
  (Deploy > Pages), so the gitlab.io address redirects to it; otherwise the old address
  stays a public duplicate of the site.
- After the records are visible press "Verify" in the dialog; the certificate is issued
  some minutes later. Then turn on "Force HTTPS" (Deploy > Pages).
- If the zone has a CAA record, it must allow `letsencrypt.org`. GitLab's Let's Encrypt
  troubleshooting asks for a single A or CNAME record per name; if the certificate is not
  issued, remove the AAAA record and try again.
- After the first deployment open `https://hordwyn.com/.well-known/security.txt` once:
  GitLab's documentation does not say whether Pages serves a dot directory from the
  artifact.
- GitLab Pages serves only `/404.html` for a wrong address, whatever its language; the
  English page carries a German line with a link to `/de/`.
- The documentation under `/docs/` has the Antora default look and no link back to the
  website; its UI bundle is fetched unpinned at build time (pin a version before the first
  deployment).
- `www`: GitLab serves the same site on both names; the pages name `https://hordwyn.com` as
  canonical. A redirect from `www` to the apex can be done at the DNS provider instead.
- `hordwyn.de`: simplest is the registrar's HTTP forwarding to `https://hordwyn.com/de/`.
  Alternatively add it as a further Pages domain with the same four kinds of records.
- The hub address `hub.hordwyn.com` is only linked; it needs no record until there is a hub.

The public address is `base_url` in `content/site.toml`; canonical links, the site map,
`robots.txt` and `security.txt` are built from it.

## What the website promises about itself

No cookies, no trackers, no request to another origin (fonts, styles and the script are
served by the site; a browser test fails on any foreign request). One inline script sets the
theme before first paint. Without JavaScript the site is complete and light; the theme
button is hidden then, the menu is a `<details>` element and works without script.
