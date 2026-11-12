# Domains for the project site and the hub

Date checked: 2026-11-12 (all lookups in this file, unless a line says otherwise).
Scope: research only. Nothing was registered, no registrar and no person was contacted.
Question (owner): "check which domain might be good for the hub and a website for the new
app."

Read first: section 1 (short answer) and section 3 (the name collision). The rest is
evidence.

## 1. Short answer

1. **Primary domain: `kithkeep.org`.** It shows no registration. `.com` and `.app` are
   already registered by someone else, so `.org` is the best free fit for an MIT-licensed,
   self-hosted project.
2. **Hub address: a neutral, separate domain, not a `kithkeep` subdomain.** Suggested:
   `shelfcommons.org` (no registration found in `.org`, `.com`, `.net`, `.eu`; no
   collision found). Staging at `staging.shelfcommons.org`. The name is a proposal; the
   reasoning in section 5 holds for any neutral name.
3. **Defensive registrations: `kithkeep.de`, `kithkeep.eu`, `kithkeep.dev`.** Optional
   fourth: `kithkeep.net`. Not worth it: `.io`, `.social`, `.games`, `.community`, `.club`
   (high renewal prices, no use planned).
4. **Cost:** about 45 to 60 EUR per year for all five domains at regular renewal prices
   (section 7).
5. **The name itself has a collision the owner must decide on before anything is
   published** (section 3): an iOS app called "KithKeep: Stay Close" has been in the App
   Store since 2026-10-06, `kithkeep.com` was registered 2026-01-12 and `kithkeep.app`
   2026-05-17. Different product field (personal relationship reminders), same name, same
   broad class (software). Not a proven legal blocker, but this project would be the later
   user of the name. Renaming is cheap now (white-label, greenfield until v1) and expensive
   after v1.

Limits of this research: RDAP, WHOIS and DNS show whether a name is registered right now.
They do not show the price, whether the registry lists the name as "premium", or whether
it is reserved or blocked. A registrar check is needed immediately before buying. The
trademark registers could not be queried automatically (section 3.3).

## 2. Registration status of kithkeep.<tld>

Method: RDAP at the registry server named in the IANA bootstrap file
(https://data.iana.org/rdap/dns.json). `.io`, `.eu` and `.de` have no entry in that file,
so port-43 WHOIS at the registry was used (`whois.nic.io`, `whois.eu`, `whois.denic.de`);
`.de` was also confirmed at https://rdap.denic.de/domain/kithkeep.de (404 "No such
domain"). https://rdap.org answered "No RDAP service is available" for `.io`, `.de`, `.eu`.
The build host has no `dig`; NS and A records were read through DNS over HTTPS
(https://dns.google/resolve?name=<domain>&type=NS and type=A), which gives the same answer
as `dig +short`.

| Domain | RDAP / WHOIS | DNS | Result |
|---|---|---|---|
| kithkeep.com | RDAP 200 (rdap.verisign.com): registered 2026-01-12, expires 2027-01-12, registrar DNSPod, Inc. | NS fair.dnspod.net, library.dnspod.net; apex has no A record; www -> 62.234.165.63, no HTTP answer | **taken** |
| kithkeep.org | RDAP 404 (rdap.publicinterestregistry.org) | NXDOMAIN | no registration found |
| kithkeep.net | RDAP 404 (rdap.verisign.com) | NXDOMAIN | no registration found |
| kithkeep.app | RDAP 200 (pubapi.registry.google): registered 2026-05-17, expires 2027-05-17, registrar Hosting Concepts B.V. d/b/a Registrar.eu (Openprovider) | NS artemis/hermes.dns-parking.com; A 2.57.91.91; page title "Parked Domain name on Hostinger DNS system" | **taken** (parked) |
| kithkeep.dev | RDAP 404 (pubapi.registry.google) | NXDOMAIN | no registration found |
| kithkeep.io | WHOIS whois.nic.io: "Domain not found." | NXDOMAIN | no registration found |
| kithkeep.eu | WHOIS whois.eu: "Status: AVAILABLE" | NXDOMAIN | no registration found |
| kithkeep.de | WHOIS whois.denic.de: "Status: free"; RDAP 404 | NXDOMAIN | no registration found |
| kithkeep.social | RDAP 404 (rdap.identitydigital.services) | NXDOMAIN | no registration found |
| kithkeep.club | RDAP 404 (rdap.nic.club) | NXDOMAIN | no registration found |
| kithkeep.games | RDAP 404 (rdap.identitydigital.services) | NXDOMAIN | no registration found |
| kithkeep.community | RDAP 404 (rdap.identitydigital.services) | NXDOMAIN | no registration found |

Also looked at: `kithkeep.xyz`, `kithkeep.info`, `kithkeep.one`, `getkithkeep.com`: RDAP
404, NXDOMAIN.

Notes on the two taken names:

- `kithkeep.com`: the registrar and name servers (DNSPod, a Tencent Cloud brand) and the
  Tencent Cloud address of `www` fit the Chinese seller of the iOS app in section 3.1, but
  RDAP does not name the holder, so the link is likely, not proven. The app's own site is
  on another domain (`kithkeep.sanva.top`).
- `kithkeep.app`: certificate transparency (https://crt.sh/?q=kithkeep.app) shows
  certificates from 2026-05-17 to 2026-08-08, so something was served there for a while;
  today it is a parking page. Holder unknown.
- Both expire in 2027. Waiting for a drop is not a plan.

### Special rules per TLD

- **.app, .dev** (Google Registry): the whole TLD is on the HSTS preload list. Browsers
  refuse plain HTTP for every name under it; a valid TLS certificate is required from the
  first request. Fine for a project site (all three hosting options in section 6 issue
  certificates), wrong for anything that must answer on port 80. Sources: https://get.app/
  (".app made history as the world's first HSTS-preloaded top-level domain"),
  https://get.dev/ ("The .dev domain is included on the HSTS preload list, making HTTPS
  required on all connections").
- **.eu** (EURid): only for EU citizens (wherever they live), residents of an EU or EEA
  country, and organisations established there (Regulation (EU) 2019/517, Art. 3). The
  holder must stay eligible; EURid can revoke otherwise. Page:
  https://eurid.eu/en/register-a-eu-domain/ (reached; the eligibility text itself was not
  extracted automatically, the rule is quoted from the regulation as known).
- **.de** (DENIC): no residency requirement and no admin-c any more, but: "If the Domain
  Holder is not domiciled in Germany, they shall name within two weeks of a corresponding
  request by DENIC an authorised representative domiciled in Germany for receiving the
  service of official or court documents" (DENIC Domain Terms and Conditions, section 3
  (4), https://www.denic.de/en/domain-terms-and-conditions). No issue for a holder
  domiciled in Germany. Holder data must be real; a `.de` holder is easier to identify
  through DENIC than a gTLD holder.
- **.io**: country code of the British Indian Ocean Territory, run commercially by
  Identity Digital. Two concerns. (a) Price: renewal about 50 to 70 per year (section 7).
  (b) Future: the UK and Mauritius signed a treaty on the Chagos Archipelago; ICANN wrote
  on 2024-11-14 that "IO" is still in ISO 3166-1 and that a change of sovereignty "is
  likely to trigger an evaluation" by the body that maintains the standard
  (https://www.icann.org/en/blogs/details/the-chagos-archipelago-and-the-io-domain-14-11-2024-en).
  Press reports in 2026 say ratification is stalled (search result summary, e.g.
  https://domainincite.com/tag/chagos; not verified at a primary source). Retirement, if
  it ever comes, takes years. Not needed here; skip.
- **.org, .net, .com**: no special rules.
- **.social, .club, .games, .community**: no eligibility rules; cheap first year, high
  renewal (section 7).

## 3. Name collisions for "kithkeep"

### 3.1 Products and companies

| Where | Finding | Weight |
|---|---|---|
| Apple App Store | **"KithKeep: Stay Close"**, bundle id `com.sanva.kithkeep`, seller an individual (name in Chinese characters), version 1.0 released 2026-10-06, free, category Lifestyle, 0 ratings. A private "people who matter" journal with keep-in-touch reminders. Site https://kithkeep.sanva.top/ (Chinese and English). Source: https://itunes.apple.com/lookup?id=6782482143 and https://apps.apple.com/us/app/kithkeep-stay-close/id6782482143. Listed in the US and DE storefronts. | **high**: same name, software, already public |
| kithkeep.com | registered 2026-01-12 (section 2), probably the same party | high for the domain, medium otherwise |
| kithkeep.app | registered 2026-05-17, parked, holder unknown | medium |
| GitHub | one repository: `andryous/kithkeep-project`, "Birthday and anniversary reminder app (iOS/Android) built with React Native (Expo) and a Next.js API", created 2025-11-06, last push 2025-11-27, no licence, 0 stars. Source: https://api.github.com/search/repositories?q=kithkeep | low (dormant), but it shows the name is an obvious pick for "keep in touch with your kith" apps |
| Google Play | search page for "kithkeep" returned no app hit; `details?id=com.sanva.kithkeep` returns 404. Source: https://play.google.com/store/search?q=kithkeep&c=apps (page is script-rendered, so "no hit" is weak evidence) | none found |
| Web search | no company or product called kithkeep besides the above. Near names: KITH (New York fashion retailer, kith.com, has its own shopping app), Kithly (household app), "KITH + KIN" (US mark for document-sharing software, per https://trademarks.justia.com/owners/kith-and-kin-llc-4714349). | low: different names |

Assessment. The word is a natural coinage ("kith and kin" plus "keep"), and at least three
unrelated parties reached it within a year. The other uses are about keeping in touch with
people; this project is a media and play tracker. That is a different purpose, but the same
kind of goods (software, Nice classes 9 and 42), and the iOS app was public first. Practical
consequences if the name stays:

- `kithkeep.com` and `kithkeep.app` are gone; the project lives on `.org`.
- A later mobile app of this project could not use the plain name in the App Store without
  confusion, and Apple requires unique app names per storefront.
- If the other party registers a trade mark, this project is the junior user. No filing was
  found, but the registers could not be checked properly (3.3).
- Search results for the name will be shared from day one.

Options: (a) keep the name, take `.org`, accept the shared name; (b) rename before v1 while
it costs one directory (`config/brand/default/`), the repository path and a few docs. A
neutral hub domain (section 5) is unaffected by either choice, which is one reason to
separate it. This is a point for the lawyer list (HUB-11) if the owner wants certainty; this
file is not legal advice.

### 3.2 Namespaces (all free unless noted)

| Namespace | Check | Result |
|---|---|---|
| GitHub user/org `kithkeep` | https://api.github.com/users/kithkeep | 404, free (`kith-keep` also 404) |
| ghcr.io/kithkeep | follows the GitHub org name | free while the org is free |
| GitLab group `kithkeep` | https://gitlab.com/kithkeep answers 302 (sign-in redirect); group search API returned nothing | probably free, confirm when signed in. Today the repo lives under `hordwyn/hordwyn` |
| Codeberg `kithkeep` | https://codeberg.org/kithkeep | 404, free |
| crates.io `kithkeep` | https://crates.io/api/v1/crates/kithkeep | 404, free; search for "kithkeep": 0 crates. (`kith` exists: "A TUI for accessing Teleport databases", unrelated.) |
| npm `kithkeep`, scope `@kithkeep` | https://registry.npmjs.org/kithkeep, .../@kithkeep%2fcore | 404, free; search: 0 packages. The npm org page answered 403 to the script, not conclusive for the scope. |
| PyPI `kithkeep` | https://pypi.org/pypi/kithkeep/json | 404, free |
| Docker Hub user/org `kithkeep` | https://hub.docker.com/v2/users/kithkeep/, /v2/orgs/kithkeep/ | 404 "User not found", free; repository search: 0 |
| Apple App Store | see 3.1 | **taken** |

Namespaces are first come, first served. If the name stays, reserve the GitHub org (also
covers ghcr.io), the GitLab group, the Codeberg org, the Docker Hub org, the crates.io name
and the npm scope on the same day the domain is bought. That is account creation, not
purchase; the owner decides.

### 3.3 Trade marks

None of the official registers could be queried by script:

- EUIPO eSearch plus (https://euipo.europa.eu/eSearch/): script-rendered application; a
  direct request returned an error page ("Problem detected").
- TMview (https://www.tmdn.org/tmview/): request rejected by the firewall ("Request
  Rejected").
- DPMAregister (https://register.dpma.de/DPMAregister/marke/basis): returned the generic
  start page, no result list (needs a browser session).
- USPTO trademark search (https://tmsearch.uspto.gov/search/search-results?q=kithkeep):
  returned the application shell without results (script-rendered).

Substitute: two web searches ("kithkeep", and "KITHKEEP trademark application USPTO OR
EUIPO OR WIPO") found no KITHKEEP mark or filing; they found only the near names in 3.1
(KITH, KITH + KIN, KITHWILD, KITKEEPERS). This is weak evidence. **To do by hand (10
minutes, free): search "kithkeep" and "kith keep" in TMview (covers EUIPO, DPMA, USPTO,
WIPO) and in DPMAregister**, classes 9, 41, 42. The well-known mark KITH (fashion, retail)
is in other classes; a composite word for tracker software is unlikely to conflict, but a
lawyer decides that, not this file.

## 4. What the repository already says about the hub (context)

- The hub is a role of the same binary; any instance can serve as a hub. Design:
  `git show wip/fable-reviews:docs/fable/hub-design.md`. It has no section on naming or
  domains; examples use `https://hub.example.org` (config `public_base_url`, client `url`)
  and `staging.hub.example.org` for the staging hub (HUB-13).
- Peers are identified by origin (https URL) and an Ed25519 key; `GET /meta` returns the hub
  name. So the hub's address is part of every peer's configuration and of every item URL
  that gets shared or cited.
- Owner answers 2026-11-10 (`docs/fable/round2/owner-answers-2026-11-10.md`): HUB-0 the
  reference hub is open to the owner's own instances first, third parties after H5; HUB-1
  facts are CC0; HUB-4 "a full feature hub. Including everything we do" (every kind, not
  only board games); HUB-13 a staging hub exists.
- Rule 9 (HANDOFF): no product name, colour, logo or domain hard-coded outside the brand
  directory. The reference hub URL must therefore be a configuration default, not a
  constant.

## 5. Hub naming

### 5.1 Three ways to address the hub

| Option | For | Against |
|---|---|---|
| A. Subdomain of the app: `hub.kithkeep.org` | free, one domain, obvious for kithkeep users | The hub then belongs to one brand. A white-label operator (shop, club, fork) sends its users' contributions to "kithkeep"; the data attribution line ("Data from <hub>") advertises another brand inside their product. If the app is renamed (section 3) or forked, every peer config and every published item URL changes. |
| B. Separate domain with the brand in it: `kithhub.org` | short | Same branding problem as A, plus "kith" alone invites confusion with KITH. No gain over A. |
| C. Neutral name on its own domain | The catalogue is CC0 and seeded from Wikidata; it is a commons, and a neutral name says so. Any brand, fork or third-party client can federate without carrying another product's name. Stable identifiers: the hub address survives a rename of the app. Clean separation of legal roles: the hub is a public service with accounts, reports and notice-and-action (HUB-9 to HUB-11); the app site is a brochure. Follows the reference projects in the design: MusicBrainz, Open Food Facts and Wikidata are named after the data, not after a client app. | One more domain (about 10 to 12 EUR per year). A second name to explain. |

**Proposal: C.** Details:

- The reference hub is served at the apex of the neutral domain (`https://<hubname>.org`),
  staging at `staging.<hubname>.org`. API under a versioned path on the same origin; no
  separate `api.` host needed at this scale (one VM).
- The app keeps a convenience alias only if wanted: `hub.kithkeep.org` as an HTTP redirect
  to the neutral domain. Peers must be configured with the neutral origin, because request
  signatures are bound to the origin.
- The hub name shown in the UI comes from the hub's own `GET /meta`, never from the client
  brand. The default hub URL lives in configuration (rule 9), so a white-label build can
  point at the reference hub, at its own hub, or at none.
- Other people's hubs use their own domains; the neutral name is the name of the reference
  hub, not of the protocol. If the protocol ever needs a name, it should be a third,
  descriptive one (for example "hub protocol v1" in docs), not a domain.
- Mail for the hub (abuse, notice-and-action contact) is on the hub domain, separate from
  project mail.

### 5.2 Candidate names (checked 2026-11-12, same method as section 2)

The hub covers every kind (HUB-4), so names that say only "games" or only "media" are too
narrow. "Shelf" covers books, games, films and records; "commons" says CC0.

| Candidate | .org | Other TLDs | Collisions (GitHub user, repo search, crates.io, npm, App Store search, web search) | Verdict |
|---|---|---|---|---|
| **shelfcommons** | no registration (RDAP 404, NXDOMAIN) | `.com`, `.net` RDAP 404; `.eu` WHOIS "AVAILABLE" | none found | **first choice** |
| playcommons | no registration | `.com` taken (registered 2024-03-03, Squarespace, has a site) | none found in registries | usable, but "play" undersells books and films, and the `.com` is in use |
| pastimebase | no registration | `.com` taken (2020-08-29, XServer, Japan) | none found | usable, weaker name |
| commoncatalogue | no registration | `commoncatalog.org` (US spelling) taken since 2008 | none found | no: two spellings, one owned by someone else |
| openmediafacts | no registration | not checked further | none found | no: copies the "Open ... Facts" naming of the Open Food Facts family and would suggest an affiliation that does not exist |
| kithhub | no registration (`.org`, `.net`) | - | - | no: branded (option B) |

Rejected because the `.org` is taken: `commonshelf.org` (registered 2025-09-30, Cloudflare,
live; `.com` taken since 2021), `openshelf.org` (2009), `sharedshelf.org` (2009),
`opencatalogue.org` (2022), `opencollection.org` (2003).

No trade mark search was possible for the candidates either (3.3); do the TMview search for
the chosen name before buying.

## 6. Project website

### 6.1 What the site needs at v1

v1 is internet-facing (HANDOFF 4.8), so the site is where a stranger decides whether to
trust and install the software.

Needed:

1. **Landing page**: what it is in two sentences (self-hosted tracker for movies, shows,
   books, games, board games, podcasts; your data stays on your server), three or four
   screenshots (light and dark), the install quick start (compose file, link to
   `install.adoc`), licence (MIT), link to the source and the issue tracker.
2. **Documentation**: the Antora site that CI already builds (`antora` job, output
   `build/site`), published under `/docs/`. The playbook title is still "Tracker
   documentation"; the site title should come from the brand.
3. **Release notes, changelog and upgrade notes** (HANDOFF 4.8: every tag after v1 needs an
   upgrade path). One page per release generated from the changelog is enough.
4. **Security page** and `/.well-known/security.txt` with a contact and the supported
   versions.
5. **Legal**: imprint and privacy notice if the site is run from Germany, plus the
   third-party attribution page (crates, fonts, icons, data sources) that HANDOFF already
   asks for.
6. **"About the hub"**: one page that explains the shared catalogue and links to the hub
   domain. The hub's own pages (terms, licences, reports) live on the hub.

Not needed at v1:

- **A public demo instance.** It would be a second internet-facing deployment with open
  sign-up or a shared login, provider keys with their own terms, abuse handling and a reset
  job. Screenshots and a short screen recording do the job. Once the hub is public (after
  H5), its read-only catalogue is a live demo for free.
- Blog, forum, newsletter, analytics. Issues on the forge are enough; no trackers on the
  site.

Suggested layout on one static site: `/` landing, `/docs/` Antora, `/releases/`,
`/security/`, `/legal/`. One host name (`kithkeep.org`, `www` redirecting to it) keeps
certificates and redirects simple; `docs.kithkeep.org` is not needed.

### 6.2 Where to host it (static files)

| Host | Terms in one line | Fit |
|---|---|---|
| GitLab Pages | Available on the Free tier of GitLab.com; custom domains with automatic Let's Encrypt certificates; site size limited by the artifact limit (1 GB); up to 150 custom domains per site. Sources: https://docs.gitlab.com/user/project/pages/, https://docs.gitlab.com/user/gitlab_com/ | **best**: the repo and the `antora` job are already on GitLab; one `pages` job publishes `build/site` plus the landing page. No new account, no new party. |
| Codeberg Pages | Run by the non-profit Codeberg e.V. for free and libre software only (Terms of Use section 1 and 2: public content must carry a free licence); repository must be public; custom domains supported on the new git-pages server (the legacy v2 server is closed to new sign-ups); no uptime commitment stated. Sources: https://docs.codeberg.org/codeberg-pages/, https://codeberg.org/Codeberg/org/src/branch/main/TermsOfUse.md | good fallback and a good home for a public mirror; needs a push of the built site to a Codeberg repo. Volunteer-run, so not for anything that must be up. |
| Cloudflare Pages | Free plan: 500 builds per month, 20,000 files per site, 25 MiB per file, 100 custom domains per project; no bandwidth limit stated on the limits page. Source: https://developers.cloudflare.com/pages/platform/limits/ | works, fast, but adds a US provider and an account for no gain at this size; consider only if traffic outgrows GitLab Pages. |

Proposal: GitLab Pages for `kithkeep.org`. The hub is not a static site; it runs on the
owner's VM behind the reverse proxy like any instance.

DNS: keep DNS at the registrar or one DNS host for all domains; enable DNSSEC and CAA
records; defensive domains get a redirect to the primary and a null MX plus SPF `-all` so
they cannot be used to send mail.

## 7. Prices (public price lists, 2026-11-12)

Two registrars' public pages. Prices change often and exclude or include VAT differently;
treat them as ranges.

- Porkbun, public price API https://api.porkbun.com/api/json/v3/pricing/get (same data as
  https://porkbun.com/products/domains), USD, without VAT.
- INWX (Berlin), per-TLD pages https://www.inwx.de/en/<tld>-domain (regular price and
  first-year promotion, promotion "valid for the first year" until 2027-01-01) and the
  price list https://www.inwx.de/en/domain/pricelist ("net prices plus the applicable
  VAT"), EUR. It was not clear from the page text whether the per-TLD page prices include
  VAT; where both pages gave a figure, both are listed.

| TLD | Porkbun first year / renewal (USD) | INWX regular per year (EUR) | INWX first-year promo (EUR) | Rough range per year after year one |
|---|---|---|---|---|
| .com | 11.81 / 11.81 | 15.50 (price list: 13.10 net) | - | 12 to 16 |
| .org | 7.98 / 11.84 | 15.00 (price list: 11.91 net) | 9.50 | 12 to 15 |
| .net | 12.52 / 12.52 | price list: 14.61 net | - | 12.50 to 17 |
| .app | 8.75 / 14.93 | 20.10 | 9.50 | 15 to 20 |
| .dev | 8.75 / 12.87 | 16.60 | 9.50 | 13 to 17 |
| .io | 28.12 / 51.80 | 69.50 | 26.89 | 52 to 70 |
| .eu | 5.88 / 5.88 | price list: 9.40 net | - | 6 to 11 |
| .de | 2.90 / 4.07 | price list: 4.71 net first year, 3.60 net renewal | - | 3.60 to 4.50 |
| .social | 6.69 / 34.50 | 51.00 | 6.30 | 35 to 51 |
| .club | 4.12 / 15.96 | 35.50 | 2.10 | 16 to 36 |
| .games | 8.24 / 30.38 | 44.50 | 7.56 | 30 to 45 |
| .community | 8.24 / 45.83 | 63.00 | 7.56 | 46 to 63 |

The first-year promotions on `.social`, `.club`, `.games` and `.community` hide renewals of
16 to 63 per year; that is why they are not on the buy list.

## 8. Recommendation

If the owner keeps the name (section 3):

| Buy | Purpose | About per year |
|---|---|---|
| `kithkeep.org` | project site (landing, docs, releases), project mail | 12 to 15 |
| `shelfcommons.org` (or another neutral name from 5.2) | reference hub, `staging.` for the staging hub | 12 to 15 |
| `kithkeep.de` | defensive, redirect; owner's home market, German UI exists | 4 |
| `kithkeep.eu` | defensive, redirect; cheap, fits a GDPR-minded self-hosting audience | 6 to 11 |
| `kithkeep.dev` | defensive, redirect to the docs or the repository | 13 to 17 |

Total about 45 to 60 per year. Optional: `kithkeep.net` (12.50 to 17) as the classic
look-alike, and `shelfcommons.net`/`.com` if the hub name sticks.

Order of steps for the owner (nothing here has been done):

1. Decide: keep "kithkeep" or rename before v1 (section 3). If in doubt, do the TMview
   search by hand first and put the question on the lawyer list.
2. Check the chosen names at the registrar (price, premium flag) and buy the primary and
   the hub domain on the same day; then the defensive ones.
3. Reserve the namespaces in 3.2 on that day.
4. Add a `pages` job that publishes the Antora output and the landing page to GitLab Pages;
   point `kithkeep.org` at it.
5. Put the reference hub URL into configuration as a default (rule 9), with the neutral
   domain.

If the owner renames: sections 5 and 6 stay as they are; repeat sections 2 and 3 for the new
name. The neutral hub domain can be bought now in either case.

## 9. Sources (all checked 2026-11-12)

Registration data
- IANA RDAP bootstrap: https://data.iana.org/rdap/dns.json
- Verisign RDAP (.com, .net): https://rdap.verisign.com/com/v1/domain/kithkeep.com,
  https://rdap.verisign.com/net/v1/domain/kithkeep.net
- PIR RDAP (.org): https://rdap.publicinterestregistry.org/rdap/domain/kithkeep.org
- Google Registry RDAP (.app, .dev): https://pubapi.registry.google/rdap/domain/kithkeep.app
- Identity Digital RDAP (.social, .games, .community):
  https://rdap.identitydigital.services/rdap/domain/kithkeep.social
- .club RDAP: https://rdap.nic.club/domain/kithkeep.club
- DENIC RDAP: https://rdap.denic.de/domain/kithkeep.de; WHOIS whois.denic.de, whois.eu,
  whois.nic.io (port 43)
- rdap.org: https://rdap.org/domain/kithkeep.io (no service for .io, .de, .eu)
- DNS over HTTPS: https://dns.google/resolve
- Certificate transparency: https://crt.sh/?q=kithkeep.app, https://crt.sh/?q=kithkeep.com
  (no entries for .com)

TLD rules
- https://get.app/, https://get.dev/ (HSTS preload)
- https://eurid.eu/en/register-a-eu-domain/ (.eu), Regulation (EU) 2019/517 Art. 3
- https://www.denic.de/en/domain-terms-and-conditions (section 3 (4))
- https://www.icann.org/en/blogs/details/the-chagos-archipelago-and-the-io-domain-14-11-2024-en
- https://domainincite.com/tag/chagos (press, via search summary)

Collisions
- https://itunes.apple.com/search?term=kithkeep&entity=software (US and country=de),
  https://itunes.apple.com/lookup?id=6782482143,
  https://apps.apple.com/us/app/kithkeep-stay-close/id6782482143, https://kithkeep.sanva.top/
- https://api.github.com/users/kithkeep, https://api.github.com/search/repositories?q=kithkeep,
  https://api.github.com/repos/andryous/kithkeep-project
- https://gitlab.com/kithkeep, https://gitlab.com/api/v4/groups?search=kithkeep,
  https://codeberg.org/kithkeep
- https://crates.io/api/v1/crates/kithkeep, https://crates.io/api/v1/crates?q=kithkeep
- https://registry.npmjs.org/kithkeep, https://registry.npmjs.org/-/v1/search?text=kithkeep
- https://pypi.org/pypi/kithkeep/json
- https://hub.docker.com/v2/users/kithkeep/, https://hub.docker.com/v2/orgs/kithkeep/,
  https://hub.docker.com/v2/search/repositories/?query=kithkeep
- https://play.google.com/store/search?q=kithkeep&c=apps
- Web search results: https://kith.com/, https://hunted.space/dashboard/kithly,
  https://trademarks.justia.com/owners/kith-and-kin-llc-4714349
- Trade mark registers (not queryable by script): https://euipo.europa.eu/eSearch/,
  https://www.tmdn.org/tmview/, https://register.dpma.de/DPMAregister/marke/basis,
  https://tmsearch.uspto.gov/search/search-results

Hosting
- https://docs.gitlab.com/user/project/pages/, https://docs.gitlab.com/user/gitlab_com/
- https://docs.codeberg.org/codeberg-pages/,
  https://codeberg.org/Codeberg/org/src/branch/main/TermsOfUse.md
- https://developers.cloudflare.com/pages/platform/limits/

Prices
- https://api.porkbun.com/api/json/v3/pricing/get (https://porkbun.com/products/domains)
- https://www.inwx.de/en/domain/pricelist, https://www.inwx.de/en/app-domain (and the same
  pattern for dev, io, social, club, games, community, com, org)

Repository context
- `config/brand/default/brand.toml`, `docs/HANDOFF.md` (rule 9, 4.7, 4.8),
  `docs/fable/round2/owner-answers-2026-11-10.md` (HUB-0, HUB-1, HUB-4, HUB-13),
  `git show wip/fable-reviews:docs/fable/hub-design.md` (3.4, 9)
