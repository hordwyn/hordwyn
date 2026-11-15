STATUS: COMPLETE (2026-11-09)

# Design: the community hub (any instance can serve as a shared catalogue)

Date: 2026-11-09. Author: design agent (Fable), for the owner. Design document only, no
code. Repository state read: branch `main` = tag `ux4`.

This document is not legal advice. Every legal text, clause and interpretation in it is a
draft for a lawyer to check; it is marked as such where it appears.

Grounding: `docs/HANDOFF.md` sections 2 and 4, `docs/design/DESIGN.md`,
`docs/design/ux/design.md` (sections 3.7 and 4), `docs/connectors/*.md`,
`docs/research/board-game-sources.md`, DEC-190..198, DEC-446, DEC-453, DEC-466, and the
catalogue code (`crates/domain/src/catalog.rs`, `crates/app/src/catalog.rs`,
`crates/catalog-core/src/{merge,model,connector}.rs`, migrations 0001, 0003, 0004, 0016,
0028, 0030, 0031). External sources are cited inline with URL and fetch date (all fetched
2026-11-09 through the web-fetch agent unless stated otherwise); where a page could not be
read it says so. No source code of any GPL-family project was opened; stash-box turned out
to be MIT-licensed (its GitHub sidebar, fetched 2026-11-09), and only its README and the
stashapp documentation site were read anyway.

## Contents

0. Summary and recommendation
1. Goals and non-goals
2. How the reference projects work and what to borrow
3. Architecture
4. Contribution and review workflow
5. Spam, abuse and validation
6. Restricted-source protection (TMDB and friends)
7. GDPR and licences, with the draft image clause (en, de)
8. Moderation and operations (incl. notice-and-action)
9. Security considerations
10. Monetisation options
11. Scope beyond board games
12. Phased implementation plan (work packages with acceptance tests)
13. Open questions for the owner, with recommended defaults
14. Sources

## 0. Summary and recommendation

- Build the hub as a **role of the same binary** (`[hub] serve = true`), not a second
  product. A hub is an instance that additionally keeps a public, versioned, CC0 catalogue
  and accepts signed contributions from connected instances ("peers"). Any instance can
  turn it on; the owner's instance is the first hub ("the reference hub").
- Instances talk to a hub through **one new connector, `hub`**, which implements the
  existing `Search`, `Details` and `Images` capabilities (pull) and a new `Contribute`
  capability (push). Nothing in the tracker changes shape: hub data arrives as a
  `Snapshot` with connector id `hub`, is merged by the existing per-field priority and
  keeps per-field provenance like every other source.
- **Scrap the BGG connector** (it was never implemented, only documented): board games
  become the hub's first kind. The hub is seeded from Wikidata (CC0, DEC-192), users add
  what is missing, and the hub becomes the shared cache in front of Wikidata for all
  connected instances. The BGG id stays an identifier (DEC-193); the BGG importers for the
  user's own collection and plays stay.
- **Licence: CC0 for the hub's structured data** (same as Wikidata and MusicBrainz core).
  It is the only choice that lets the hub import Wikidata and push back to Wikidata, avoids
  attribution stacking across thousands of contributors, and lets every mirror and every
  instance keep the data without a licence string. Images are the exception: the uploader
  chooses CC0 or CC BY 4.0 (standard, irrevocable, Commons-compatible); the owner's wish for
  a revocable operator licence is analysed in section 7 and offered as a third option with
  the limits spelled out (withdrawal stops the hub's own distribution for the future; it
  cannot reach copies lawfully made before).
- **Review model: MusicBrainz-style edits with trust tiers.** Every contribution is a
  change set against a base revision; new accounts and new peers are reviewed before
  anything becomes public; trusted contributors and peers get auto-accept for
  non-destructive changes; destructive changes (merge, delete, image removal) always wait.
  History is immutable, rollback is a new revision, legal takedowns redact content but keep
  the chain.
- **Restricted sources never reach the hub.** The sending instance filters by provenance
  (only `user`, `import:*`, `wikidata`, `commons`, `hub` fields may be proposed), blocks
  edits that reproduce a restricted provider's stored value (it has that value locally and
  can compare; the hub cannot), and signs an attestation. The hub refuses restricted
  identifier namespaces and image hosts, demands a declared source for long text, limits
  description length, and keeps movies and shows out of scope in v1 because that is where
  the laundering risk and the TMDB terms bite. Peers caught laundering lose trust and their
  contributions are rolled back in bulk.
- **GDPR by minimisation:** members of a connected instance never need a hub account;
  contributions are attributed to the peer (the instance) unless the member opts into a
  personal, pseudonymous hub account. Public history shows handles only; IPs are kept 7
  days for abuse handling; account deletion pseudonymises the handle in history (Open Food
  Facts model). Notice-and-action follows DSA Art. 16/17 shape even where a small operator
  may be exempt, because it is also the right abuse-report design.
- **Money, low priority:** sell delivery, not data (Wikimedia Enterprise model): hosted
  instances, higher hub API quotas and priority sync for commercial peers, an image CDN
  with large renditions, managed backups, sponsorship. TMDB-related revenue remains
  forbidden without TMDB's written agreement; the hub itself has no TMDB data, so hub
  revenue is clean.
- **Phasing:** H1 read-only hub + `hub` connector + Wikidata seed (replaces BGG), H2 peers
  and signed push, H3 review queue and trust, H4 images and licences, H5 GDPR lifecycle and
  notice-and-action, H6 operations and quotas, H7 further kinds. Each with acceptance
  tests in section 12.

## 1. Goals and non-goals

### 1.1 Goals

G1. One tool: an operator enables the hub role on an ordinary instance; there is no second
    codebase, deployment or database engine (Postgres only, DESIGN.md 1).
G2. Pull: any instance can add a hub as a data source and get search, details and images
    for the kinds the hub serves, with the usual stale-while-revalidate behaviour (no page
    waits on the hub, DESIGN.md 6).
G3. Push: a connected instance can propose new items, field corrections and images; the
    proposer sees the state of each proposal (pending, accepted, rejected with reason).
G4. Review: nothing becomes public without passing validation and either review or an
    earned auto-accept; the hub keeps full history and can roll back any change, any
    contributor and any peer.
G5. Protection: spam, vandalism, duplicates and "idiotic input" are caught by layered
    checks (schema, sanity rules, duplicate detection, rate limits, trust, reports).
G6. Clean room: data from restricted providers (TMDB, TheTVDB, IGDB, BGG, MAL, Kitsu, ...)
    cannot be pushed; the hub stores only data that is CC0-compatible by origin.
G7. Lawful: GDPR data minimisation, clear roles (controller/processor), consent where
    needed, deletion that works, an image licence that holds up, DSA-shaped
    notice-and-action.
G8. Open data: hub data is published under an open licence with dumps and a change feed,
    so the community is not locked to one operator and mirrors are possible.
G9. Sustainable: the owner can earn money from services around the hub without breaking
    source terms or the open licence.
G10. Works at the owner's scale first (one 4 vCPU / 16 GB VM, build-host-constraints
    memory), designed to grow.

### 1.2 Non-goals (v1)

- No ActivityPub/fediverse federation of user activity (ratings, plays, reviews). The hub
  federates catalogue facts and images only. User data stays on the instance.
- No real-time multi-master replication between hubs. One hub is the authority for its
  items; a second hub may mirror (pull the change feed) and may later forward
  contributions, but hub-to-hub conflict resolution is out of scope.
- No movies and shows on the hub in v1 (section 6 and 11): the laundering risk from TMDB
  and the owner's own TMDB obligations make it the wrong first kind.
- No AI-generated descriptions or AI review decisions. Validation is rule-based; a human
  decides disputed changes (DSA Art. 17 also asks to disclose automated means).
- No general-purpose forum, talk pages or private messaging. Discussion happens in change
  notes (MusicBrainz "edit notes" model) and in the existing notification system.
- No payment processing inside the binary in v1. Monetisation hooks (quotas, tiers) are
  designed; billing is external.
- No BGG connector. Section 6 and HANDOFF 4.6 record why; this document supersedes the
  "implement later behind a flag" expectation of DEC-190/193 in that respect (a DEC entry
  must record it when the owner agrees).

## 2. How the reference projects work and what to borrow

All facts in this section were fetched 2026-11-09 from the pages named; quotes are short.

### 2.1 Open Food Facts (OFF)

Source pages: https://world.openfoodfacts.org/terms-of-use ,
https://world.openfoodfacts.org/data , https://openfoodfacts.github.io/openfoodfacts-server/api/ ,
https://world.openfoodfacts.org/privacy , https://world.openfoodfacts.org/faq ,
https://world.openfoodfacts.org/who-we-are , https://world.openfoodfacts.org/donate-to-open-food-facts .
Not readable: https://wiki.openfoodfacts.org/Data_quality and
https://wiki.openfoodfacts.org/Moderation (bot-protection page instead of content); the
Producers platform host was not fetched.

- Licences: database ODbL 1.0, individual contents DbCL 1.0, images CC BY-SA 3.0, with the
  caveat that images "may contain graphical elements subject to copyright or other
  rights". Contributors "agree to place irrevocably their contributions under the Database
  Contents Licence 1.0" and under CC BY-SA for photos, and "agree to be credited by re-user
  by a link to the product".
- Source rules: data must come from the label and packaging; photos "must have been taken
  by the contributors themselves"; no copying from other sites or databases.
- Workflow: registration required; edits go live immediately (no review step is described;
  OFF "has no obligation to verify"); every edit is a numbered revision retrievable with
  `?rev=N`; history is public and "the public character of contributions cannot be
  revoked". Only moderators can remove photos. Repeat offenders lose access and all their
  contributions may be removed.
- Identity and anti-abuse at the API: writes need an account; apps send `app_name`,
  `app_version`, `app_uuid` ("a salted random uuid for the user so that Open Food Facts
  moderators can selectively ban any problematic user"); User-Agent `AppName/Version
  (ContactEmail)`; read limits 15 req/min/IP for product reads, 10 for search, "no limit on
  product write queries"; a staging host (`.net`) with its own account database.
- Duplicates: the barcode is the identity; same-barcode records are merged.
- Producers platform: manufacturers get a free pro platform; a producer's own entries take
  priority and only that producer can edit them once filled.
- Funding: non-profit, "No ads, no industry money", donations (2026 goal EUR 170,000),
  public funding, a permanent team of about 7.
- Privacy: username public, e-mail confidential; on account deletion "the user ID is
  replaced with a random but unique username" kept indefinitely for traceability;
  IP/logs kept 3 years; moderation data 1 to 3 years under legal obligation/legitimate
  interest.

Borrow: the app/user identification triple (app name, version, per-user salted uuid) for
instance-forwarded contributions; the "taken by yourself" and "no copying from other
databases" contributor rules; revision numbers with public history; moderator-only image
removal; pseudonymisation on deletion; the producers-platform idea as a later "verified
publisher" tier. Do not borrow: publish-before-review (we review first), ODbL (section 7).

### 2.2 StashDB / stash-box

Source pages: https://github.com/stashapp/stash-box (sidebar: MIT licence) and its README
(https://github.com/stashapp/stash-box/blob/master/README.md); https://docs.stashapp.cc/ ,
https://docs.stashapp.cc/guides/scraping-scenes-via-stash-box/ ,
https://docs.stashapp.cc/metadata-sources/stash-box-instances/ ,
https://docs.stashapp.cc/in-app-manual/tagger/ . Not readable: `docs.stashapp.cc/stash-box/`
(404); the StashDB guidelines site (guidelines.stashdb.org) was not fetched.

- Philosophy stated in the README: edits "should follow the same principles as
  MusicBrainz".
- Configuration-driven review knobs: `vote_application_threshold` (default 3 same votes
  for immediate application), `voting_period` (default 345600 s = 4 days),
  `min_destructive_voting_period` (default 172800 s = 48 h before a destructive edit may
  be applied early), `vote_cron_interval` (5 m), `edit_update_limit` (1: the creator may
  update an edit once), `guidelines_url`.
- Roles: `default_user_roles` = READ, VOTE, EDIT; `vote_promotion_threshold` grants VOTE
  automatically after N approved edits; `require_tag_role` for tag edits; a root admin with
  an API key is generated on first start; `mod_audit_retention_days` (30).
- Registration: `require_invite` (default true, invite keys "generated by existing
  users"), optional e-mail activation.
- Client integration: the client registers endpoints `https://<instance>/graphql` with a
  per-user API key ("Test Credentials"); pulls by fingerprint or keyword; saved objects
  carry a `stash_id`; submissions are "tied to the user API key" and can be managed and
  deleted on the box website; drafts expire (`draft_time_limit` 24 h).
- Images: local or S3 backend, max size and JPEG quality settings; no takedown process in
  the pages read.

Borrow: the knob set (thresholds and periods as instance settings with these defaults as
the starting point), a stable foreign id on the client side (our `hub` external id),
drafts that expire, invite-gated registration on the hub, per-user API key for a personal
hub account. Do not borrow: GraphQL (the project's API is REST/JSON), publish-before-vote.

### 2.3 MusicBrainz / MetaBrainz

Source pages: https://musicbrainz.org/doc/How_Editing_Works ,
https://musicbrainz.org/doc/Introduction_to_Voting , https://musicbrainz.org/doc/Editor ,
https://musicbrainz.org/doc/About/Data_License , https://metabrainz.org/supporters/account-type ,
https://metabrainz.org/supporters/tiers/2 , https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting ,
https://metabrainz.org/privacy , https://musicbrainz.org/doc/Code_of_Conduct . Not fetched:
MusicBrainz_Database (core vs supplementary split), How_to_Report_a_User, metabrainz.org/gdpr.

- Edits stay open 7 days unless an auto-editor approves them or 3 unanimous votes close
  them early (3 Yes applies, 3 No rejects); at expiry no votes means accepted, otherwise
  accepted only if Yes > No. Some additions show immediately and are removed if voted
  down. Destructive edits (removals, merges) stay open at least 48 hours even with 3 Yes.
  The first No vote extends the period to at least 72 hours.
- Every new account is a Beginner: can edit and write edit notes, cannot vote; graduates
  after more than two weeks, a verified e-mail and more than 10 accepted voted edits.
- Auto-editors' edits are applied without voting and they can approve others' edits; they
  are elected; the Code of Conduct forbids using the privilege on disputed edits and asks
  them to revert and re-enter for voting if challenged.
- There is no rollback feature; an open edit can be cancelled by its author; otherwise
  correction is a new edit, and destructive edits are "either impossible or very hard to
  revert". Account admins can accept/reject any open edit immediately.
- Licence: core data CC0, supplementary data and the Live Data Feed replication packets CC
  BY-NC-SA 3.0; contributors grant MetaBrainz "the right to license this data for
  commercial use", and licence income "keeps MusicBrainz running". Cover art is not in the
  dataset (Cover Art Archive).
- Commercial model: dataset and Live Data Feed access need a verified MetaBrainz account;
  tiers Non-commercial/Personal $0, Non-profit $0, Stealth start-up $0, Bronze $100+/month
  (negotiated $100-$599), Silver $600+, Gold $1250+, Unicorn $2000+.
- Rate limits: about 1 request/s per IP, 300 req/s global, anonymous or blacklisted
  User-Agents share 50 req/s; UA must carry app name, version and contact.
- Privacy: username is the only required public identity; edits and notes are visible to
  logged-in users under the username; web and API logs keep IP and UA. Account deletion is
  not described on the privacy page (GDPR page not fetched).
- Abuse: captcha at sign-up; sock-puppet voting "completely unacceptable"; account admins
  can revoke edit/vote/note privileges; edit notes are the mandatory explanation channel
  for No votes.

Borrow: the whole edit life cycle (open period, early close by unanimous votes,
destructive minimum, No-vote extension, beginner gate, auto-editor behaviour and its code
of conduct), mandatory reason on rejection, "approve = trivial and obvious only", the CC0
core + separate licence for the paid feed idea (we keep the feed CC0 but sell SLA and
quota, section 10). Do not borrow: no rollback (we add revision-based revert), CC
BY-NC-SA anywhere (it would block Wikidata push-back and commercial self-hosting).

### 2.4 Wikidata

Source pages: https://www.wikidata.org/wiki/Wikidata:Licensing ,
https://www.wikidata.org/wiki/User:Lydia_Pintscher_(WMDE)/CC-0 ,
https://www.wikidata.org/wiki/Wikidata:Notability , https://wiki.creativecommons.org/wiki/Data ;
plus `docs/research/board-game-sources.md` 2.1 (data access, etiquette, properties).

- "All structured data (i.e. the main, Property, Lexeme, and EntitySchema namespaces) is
  released into the public domain under" CC0; "Other than public-domain data and CC0 data,
  no other data license or designation is compatible" (CC BY is explicitly excluded).
- Why CC0 (Pintscher): data is "aggregated, combined, mashed-up, filtered" far more than
  articles; attribution "would need to go back to potentially millions of editors and
  sources"; protectability varies by jurisdiction so attribution "would only be enforceable
  in some jurisdictions"; share-alike and attribution hurt "the smaller start-up,
  institution or hacker" more than big companies. Acknowledged downside: "some data can't
  be imported and some re-users do not attribute us".
- Notability: an item needs a sitelink, or must be a "clearly identifiable conceptual or
  material entity" with "serious and publicly available references", or fill a structural
  need. (So many obscure board games, expansions and self-published titles are not
  notable there: the hub fills that gap and pushes back only what qualifies.)
- Editing model: anyone edits live; vandalism is reverted after the fact by patrollers,
  with bots and abuse filters; every revision is kept (common knowledge about MediaWiki,
  not re-fetched for this document).

Borrow: CC0, the per-statement reference ("source pairs") idea as per-field source URL,
the notability idea inverted (the hub accepts what Wikidata will not), QuickStatements
push-back (DEC-196). Do not borrow: edit-live-revert-later for a small team.

### 2.5 What the hub takes from each, in one table

| Concern | OFF | stash-box | MusicBrainz | Wikidata | Hub (this design) |
|---|---|---|---|---|---|
| Publish before or after review | before | after (votes) | mostly after (7 d) | before | after; auto-accept for trusted |
| Identity for API writes | account + app triple | per-user API key | account | account/OAuth | signed peer + optional personal account |
| Votes | none | 3 same votes, 4 d | 3 unanimous, 7 d, 48 h destructive | none | 3 unanimous or reviewer, 4 d, 48 h destructive |
| Beginner gate | none | VOTE after N edits | 2 weeks + 10 edits | none | 14 d + 10 accepted for vote; first 5 changes reviewed |
| History | numbered revisions, public | edits | edits, no revert | revisions, revert | immutable revisions, revert = new revision, legal redaction |
| Duplicate key | barcode | fingerprints (phash) | MBIDs, merges | QID, merges | QID, GTIN/ISBN, title+year+publisher candidates, merge changes |
| Data licence | ODbL+DbCL | per instance | CC0 core, BY-NC-SA rest | CC0 | CC0 |
| Image licence | CC BY-SA 3.0 | per instance | n/a (CAA) | Commons (CC0/BY/BY-SA) | CC0 or CC BY 4.0 (uploader's choice) |
| Deletion of account | pseudonymise, keep edits | not documented | not documented | pseudonymise | pseudonymise, keep facts |
| Money | donations, grants | none | tiers $0..$2000+ | donations, Enterprise | services, quotas, hosting |

## 3. Architecture

### 3.1 Roles in one binary

The server already runs with `[instance] hosting = "home" | "hosted"` (design.md 4.1) and
job lanes. The hub adds one orthogonal switch and one connector:

```toml
[hub]
serve = false            # true: this instance is a hub (public catalogue + contributions)
public_base_url = ""     # https://hub.example.org, required when serve = true
kinds = ["board_game"]   # kinds this hub serves (allowlist; movie/show refused in v1)
registration = "invite"  # personal hub accounts: open | invite | closed
peer_registration = "approve"   # new peers: open | approve | closed
data_licence = "CC0-1.0" # fixed in v1; shown on every page and in the API

[[hub.upstreams]]        # the hubs this instance USES as data sources (client side)
id = "reference"         # local slug, becomes the external id prefix (see 3.3)
url = "https://hub.example.org"
push = true              # members may propose contributions to it
kinds = ["board_game"]   # subset of what the hub offers
```

Environment overrides follow the existing `APP__HUB__SERVE` pattern; the admin UI edits
the same values (OD-10/OD-11 rule: env wins and shows read-only). An instance may be a
hub and a client of another hub at the same time (mirror use case).

Crate placement (D-3 dependency rule: `domain`/`app` stay free of sqlx, axum, reqwest):

| Piece | Crate | Notes |
|---|---|---|
| Hub domain types (HubItem, Revision, Change, Vote, Peer, Account, Grant, Report) | new `crates/hub-domain` or module in `domain` | plain types, no IO |
| Hub use cases (submit change, validate, review, apply, revert, quotas, trust) | new `crates/hub` (depends on `domain`, `app` ports) | ports for repo, clock, signer |
| Postgres repos and migrations `0032_hub_*.sql` | `store-pg` | same migration stream |
| HTTP API `/api/hub/v1/*` (server side) | `api` | axum routes behind `hub.serve` |
| Connector `hub` (client side: Search, Details, Images, Contribute) | `connectors` | feature `hub`, on by default |
| Signing (Ed25519, RFC 9421) | `catalog-core::http` (client) and `api` (verifier) | shared `hub-wire` crate for canonicalisation |
| Web pages (hub public pages, review queue, admin) | `web`/`ui` | reuse admin anatomy (design.md 3.7) |

### 3.2 Data model on the hub

Everything public is append-only; the current state is a view over the latest accepted
revision. Column lists are indicative (migration work is in WP-H1..H4).

```
hub_items          id uuid (UUIDv7), kind, created_change, current_revision, merged_into,
                   deleted_at, lock_level (none|fields|all), search_doc tsvector
hub_revisions      id bigserial (the global sequence = change feed cursor), item_id,
                   number (per item), change_id, state jsonb (full item: title,
                   original_title, year, description, attributes, external_ids,
                   images[]), sources jsonb (field -> {kind, ref, url, licence}),
                   created_at, created_by_account, created_by_peer, reverts bigint null,
                   redacted_at, redaction_reason
hub_changes        id uuid, item_id null (create), base_revision null, kind
                   (create|update|merge|delete|image_add|image_remove), ops jsonb
                   (per field: set/clear/append with value + source), note text,
                   submitter_account, submitter_peer, peer_subject_hash (OFF app_uuid
                   idea), attestation jsonb (signed by the peer: provenance_ok, filter
                   version), status (pending|accepted|rejected|withdrawn|superseded|
                   expired), risk_score, risk_flags jsonb, opened_at, closes_at,
                   decided_at, decided_by, decision_reason_code, decision_note
hub_votes          change_id, account_id, vote (yes|no|abstain), note, at
hub_images         hash (blake3), item_id, kind (cover|box_back|component|other),
                   width, height, bytes, mime, uploader_account, uploader_peer,
                   licence (CC0-1.0|CC-BY-4.0|operator-licence-v1|commons:<short name>),
                   attribution_name (nullable; the uploader's chosen credit),
                   source_url (Commons only), status (pending|public|withdrawn|
                   taken_down), withdrawn_at, tombstone_revision
hub_accounts       id, handle (unique, pseudonymous), email (nullable, hashed
                   lookup), oidc link (reuse auth crate), trust_level, points,
                   created_at, email_verified_at, last_seen_at, state
                   (active|limited|blocked|deleted), deleted_handle_replacement
hub_peers          id, name, origin (https URL), public_key (Ed25519, multibase),
                   key_id, previous_keys jsonb, contact_email, admin_account,
                   trust_level, quotas jsonb, state (pending|active|limited|blocked),
                   registered_at, last_seen_at, user_agent
hub_reports        id, target (item|revision|image|account|change), target_id,
                   reporter_name, reporter_email, reason_code, explanation, url,
                   bona_fide bool, received_at, ack_sent_at, status, decision,
                   statement_sent_at, handled_by
hub_locks          item_id, field (null = all), level, reason, set_by, at
hub_audit          id, actor, action, target, at, details (reuse existing admin audit
                   pattern, retention OD-13 365 days)
hub_rate           key (account|peer|ip-hash), window, counters (or in-memory + Redis-free
                   Postgres upsert, like the existing governor)
```

Design points:

- **Per-field sources** in `hub_revisions.sources` follow the existing provenance map
  (`field -> connector`) but with structure: `{"kind": "contributor" | "wikidata" |
  "commons" | "import" | "publisher", "ref": "Q17271" | "<account id>", "url":
  "https://...", "licence": "CC0-1.0"}`. `kind` and `licence` are from closed sets;
  everything the hub stores must have `licence` in {CC0-1.0} for data, and
  {CC0-1.0, CC-BY-4.0, CC-BY-SA-4.0 (Commons images only, see 7.4), operator-licence-v1}
  for images. A revision whose sources carry anything else is refused by a database
  CHECK and by the application.
- **Immutable revisions** with a global `bigserial` make the change feed trivial
  (`GET /changes?since=<id>`) and give mirrors an exact cursor. A revert writes a new
  revision with `reverts = <id>`. Legal redaction (section 8.4) sets `redacted_at` and
  blanks `state`/`sources` of that revision only; the sequence and the hash chain stay.
- **Hash chain:** each revision stores `prev_hash` and `hash = blake3(canonical JSON of
  (item_id, number, change_id, state, sources, prev_hash))`. Mirrors can verify a feed;
  redaction stores the hash but not the content.
- **Merges:** a `merge` change sets `merged_into` on the loser; its external ids move to
  the winner; the loser's id stays resolvable (`GET /items/{id}` returns 301 to the
  winner). Instances follow the redirect and rewrite their external id.
- **Images** are content-addressed with the existing asset pipeline (blake3, variants
  thumb/card/detail/full, AVIF/WebP, DESIGN.md 7) and the storage budget. The hub stores
  originals of uploads (unlike covers from providers) because the licence grant covers the
  original and reviewers need it; originals have a size cap (default 20 MB, 6000 px).

### 3.3 Data model on the client instance

No new table shape for the catalogue. A hub item arrives as a `Snapshot { connector:
"hub", external_ids: [ExternalId("hub_<upstream id>", "<hub item uuid>"), ...], ...}`
and is merged by `catalog_core::merge` with the existing `FieldPriority`. Provenance
values therefore read `hub_reference` per field, which section 6 uses. The namespace
rule in `app::catalog::check_untrusted` (lowercase, digits, underscore, 1..32 chars)
allows `hub_reference`; the upstream slug is validated against the same rule.

New client-side tables (`0033_hub_client.sql`):

```
hub_links           item_id, upstream, hub_item_id, hub_revision (what we last pulled),
                    pulled_at
hub_proposals       id, upstream, item_id, change payload (as sent), hub_change_id,
                    status mirror, submitted_by (local user), submitted_at, last_polled,
                    decision_reason_code, decision_note
hub_upstream_keys   upstream, private_key (encrypted with the existing secrets key,
                    DEC-140 pattern), key_id, peer_id, registered_at
hub_personal_tokens user_id, upstream, token (encrypted), handle, linked_at
```

Default field priority per kind: `user` values always win locally (they are stored in
the item's attributes with provenance `user`, as today), then `hub_*`, then `wikidata`,
then restricted providers. For `board_game` the priority is `[user, hub_*, wikidata]`
and BGG is not a catalogue source at all (DEC-193).

### 3.4 Federation protocol

Transport: HTTPS, JSON, versioned base path `/api/hub/v1`. Content negotiation is not
needed; all bodies are `application/json` except image uploads (multipart) and image
downloads (the existing `/assets/{hash}/{variant}.{ext}`). Every error body uses the
existing API error shape (`crates/api/src/error.rs`) with a stable `code`.

#### 3.4.1 Identity and authentication between instances

Three callers exist:

1. **Anonymous readers** (any HTTP client, browsers): read endpoints are public because the
   data is CC0; they get the anonymous rate limit (section 5.5) and must send a
   User-Agent (Wikimedia/MusicBrainz/OFF all demand it; we follow).
2. **Peers** (connected instances): every request is signed with **RFC 9421 HTTP Message
   Signatures** using **Ed25519** and a **RFC 9530 `Content-Digest`** for bodies.
   RFC 9421 (https://www.rfc-editor.org/rfc/rfc9421.html, February 2024, Standards Track)
   requires an application profile to fix covered components, parameters, key retrieval,
   algorithms and error codes; ours:
   - covered components: `@method`, `@target-uri`, `content-digest` (when a body exists),
     `content-type` (when a body exists; RFC 9530 section 6.3 warns that signing the
     digest but not the type invites tampering);
   - parameters: `created` (required), `expires` (required, at most 300 s after
     `created`), `nonce` (required, 16+ random bytes, base64url), `keyid` (= the peer's
     `key_id`), `alg="ed25519"`; exactly one signature with label `sig1`;
   - verification: resolve `keyid` to `hub_peers.public_key` (never fetched from the
     request); reject if `created` is more than 300 s in the past or 60 s in the future,
     if `expires` passed, or if `(keyid, nonce)` was seen within 10 minutes (table or
     in-memory set, the same technique as `oidc_used_jti` in migration 0024); 401 for
     a bad or missing signature, 403 for a known but blocked peer;
   - `Content-Digest: sha-256=:...:` is mandatory on POST/PUT and verified before the
     body is parsed.
   Mastodon's documented scheme (https://docs.joinmastodon.org/spec/security/, fetched
   2026-11-09) uses the older cavage draft with a 12-hour `Date` window and is moving to
   RFC 9421 with `@method`, `@target-uri` and `content-digest` mandatory; we start on
   RFC 9421 directly and keep a much tighter window because our peers are servers with
   NTP, not browsers.
   Key life cycle: the instance generates the Ed25519 key pair when the admin adds an
   upstream; the public key is registered through `POST /peers` (3.4.3); rotation is
   `POST /peers/{id}/keys` signed with the old key, the old key stays valid for 7 days
   (`previous_keys`). Private keys are stored encrypted with the existing secrets key.
3. **Persons** on the hub itself (reviewers, moderators, personal accounts): the existing
   session cookie and the existing auth crate (local password + TOTP, OIDC) serve the
   hub pages; a personal **contribution token** (random 32 bytes, stored hashed) can be
   created on the hub profile and pasted into the instance (Settings > Data sources >
   Hub); the instance sends it in the change body (`actor.personal_token`), never in a
   header that proxies might log, and the request is still signed by the peer.

Attribution of a change on the hub is therefore `(peer, subject)` where `subject` is
either a personal account or the pseudonymous `peer_subject_hash` = `blake3(peer_id ||
local user id || peer secret salt)` that the peer computes (OFF's `app_uuid` idea): the
hub can rate-limit and block one person behind a peer without knowing who they are, and
the peer admin can resolve the hash locally if asked by the hub operator about abuse.

#### 3.4.2 Pull (read) endpoints

| Endpoint | Purpose | Notes |
|---|---|---|
| `GET /meta` | hub name, kinds, licence, limits, contact, public keys for the feed hash, protocol version | cached 1 h |
| `GET /search?kind=&q=&lang=&limit=` | search (Postgres FTS + trigram; Meilisearch optional) | `Snapshot`-shaped results with `complete=false` |
| `GET /items/{id}` | current state + revision number + sources + image list | 301 on merged items; ETag = revision |
| `GET /items/{id}/revisions?before=` | history | public, redactions shown as such |
| `GET /items/by-external/{ns}/{value}` | lookup by QID/GTIN/ISBN/BGG id | ns from the allowlist (6.3) |
| `GET /changes?since=<revision id>&limit=` | the change feed for sync and mirrors | tombstones for delete/merge/image withdrawals; cursor is `hub_revisions.id` |
| `GET /dump/latest` | weekly `jsonl.zst` of current states (no accounts, no e-mails, handles only) | behind the storage budget |
| `GET /assets/{hash}/{variant}.{ext}` | images | existing route; attribution in `GET /items` |

Sync on the client: the `hub` connector's `details` is called by the existing refresh job
(stale-while-revalidate, `fresh_for_secs` from `hub.meta.ttl`, default 7 days). A cheap
cron `hub.sync` (lane `background_refresh`) polls `GET /changes?since=` once an hour and
queues `catalog.refresh` only for linked items that changed, so a hub with 50,000 items
and 500 connected instances costs one small request per instance per hour instead of
per-item polling. A tombstone (`image_remove`, `delete`, `merge`) is applied within one
sync cycle: the client removes the variant files of a withdrawn image and rewrites the
external id of a merged item. This is the technical hook that makes a withdrawn image
disappear from connected instances (section 7.3).

#### 3.4.3 Push (write) endpoints

| Endpoint | Caller | Purpose |
|---|---|---|
| `POST /peers` | unsigned, from the instance admin's browser or the instance | register: name, origin, public key, contact; answer: peer id, state (`pending` when `peer_registration = approve`) |
| `POST /peers/{id}/keys` | signed (old key) | rotate |
| `POST /changes` | signed peer | one change set (3.2), with `actor` and `attestation`; answer: change id, status, risk flags the sender may show |
| `GET /changes/{id}` | signed peer or owner | status, votes, decision reason |
| `POST /changes/{id}/withdraw` | the submitter | withdraw while pending |
| `POST /images` | signed peer | multipart: bytes + `licence` + `attribution_name` + `item_id` + `kind` + `declaration` (3 checkboxes, 7.4); creates an `image_add` change |
| `POST /reports` | anyone (unsigned, captcha-free but rate-limited) | DSA Art. 16-shaped notice (8.4) |
| `POST /accounts/me/tokens` | hub session | personal contribution token |
| `DELETE /accounts/me` | hub session | deletion (7.2) |

Idempotency: `POST /changes` carries a client-generated `change_id` (UUIDv7); a repeat
with the same id returns the stored result (the job queue may retry).

#### 3.4.4 Conflict handling

A change references `base_revision`. On arrival the hub performs a three-way check per
field: if the field is unchanged on the hub since `base_revision`, the op applies; if the
hub value changed and equals the proposed value, the op is dropped as already done; if it
changed to something else, the change is flagged `conflict` and goes to review with both
values shown (never auto-accepted, never auto-rejected). Creates are checked against
duplicates (5.3) and flagged `possible_duplicate` with candidates; a reviewer can turn a
create into an update of the candidate with one click.

On the client, user overrides (provenance `user`) always win locally. When a pull brings a
hub value that differs from a local user override, the item page shows a small "The hub
now says X" note with two actions: "Use the hub value" (drops the override) or "Propose my
value" (opens the contribution form prefilled). No silent change of user values.

### 3.5 Contribution capability of the `hub` connector

```rust
// catalog-core::connector (sketch, not code)
pub trait Contribute: Send + Sync {
    async fn propose(&self, ctx: &Ctx, change: ProposedChange) -> Result<Receipt, ConnectorError>;
    async fn status(&self, ctx: &Ctx, id: ChangeId) -> Result<ChangeStatus, ConnectorError>;
    async fn withdraw(&self, ctx: &Ctx, id: ChangeId) -> Result<(), ConnectorError>;
    async fn upload_image(&self, ctx: &Ctx, img: ProposedImage) -> Result<Receipt, ConnectorError>;
}
```

`ProposedChange` is built by `app` from the diff between the local item and the hub
revision it is linked to, filtered by section 6; the connector never sees restricted
fields. Everything goes through the governed HTTP client (rate limit per hub from
`/meta`, cache for reads, circuit breaker).

### 3.6 Wikidata seeding and the end of BGG

- The reference hub runs `hub.seed.wikidata` (lane `prefetch`): a SPARQL query per batch
  (board game class tree, 500 items a page, `maxlag`, one request at a time, descriptive
  User-Agent, etiquette from `docs/research/board-game-sources.md` 2.1) followed by
  `wbgetentities` in batches of 50 to resolve labels; each seeded item becomes a
  `create` change by the system account `wikidata-import` that is auto-accepted but
  logged, with per-field `sources = {kind: "wikidata", ref: "Qnnn", licence: "CC0-1.0"}`
  and a P18 image fetched from Commons under the DEC-194 rules (licence tag required).
- A weekly `hub.resync.wikidata` compares seeded fields whose latest source is still
  `wikidata` with the live entity and applies changes as system changes; contributor
  values are never overwritten (same rule as the instance-level design in
  board-game-sources.md 3).
- Instances keep the direct Wikidata connector (DEC-192) for operators who do not want a
  hub; with a hub configured, the hub ranks above Wikidata in the field priority and most
  Wikimedia traffic disappears.
- Push-back: a moderator action "Suggest to Wikidata" builds the QuickStatements batch
  (DEC-196) for hub items whose sources are all `contributor`/`import` and that meet
  notability; the hub stores the resulting QID when the moderator pastes it back.
- BGG: no connector is built. The BGG id remains an external id namespace on the hub
  (`bgg`, as a plain identifier from Wikidata P2339 or typed by a contributor), used for
  outbound links and import matching only (DEC-193). The owner's non-commercial BGG
  application token, if it arrives, is not needed by this design.

## 4. Contribution and review workflow

### 4.1 Trust levels

Two ladders, one for persons (accounts and peer subjects) and one for peers, because a
careful person on a sloppy instance and a sloppy person on a careful instance are
different risks. The effective level of a change is the **lower** of the two.

| Level | Who | What it may do | How it is reached |
|---|---|---|---|
| L0 new | fresh account, fresh peer subject, fresh peer | create and update (reviewed), 10 open changes at once, 1 image per change | registration |
| L1 basic | 5 accepted changes, 0 rejected for abuse, 7 days old | 30 open changes, votes on nothing, images 5 per change | automatic |
| L2 trusted | 14 days and 10 accepted voted changes (MusicBrainz gate), e-mail verified (personal accounts) | non-destructive updates auto-accepted after a 24 h "objection window", may vote | automatic, revocable |
| L3 reviewer | nominated by a moderator | approve or reject pending changes (trivial and obvious ones; disputed ones go to votes), auto-accept own non-destructive changes immediately | moderator action, logged |
| L4 moderator | hub admin appoints | everything in L3 plus merges, deletions, image removals, locks, blocks, notice handling, bulk rollback | admin action, re-auth (OD-12) |
| admin | hub operator | settings, peers, appointments, redaction | config / first admin |

Peers: `pending` (no writes), `active L0..L2` (same meaning for the whole instance:
caps per peer, auto-accept of the peer's L2 subjects only when the peer itself is L2),
`limited` (all changes reviewed, images refused), `blocked`.

Points: +2 accepted change, +5 accepted create that survives 30 days without correction,
-3 rejected as wrong, -20 rejected as abuse or restricted-source, -50 and demotion to L0
on a confirmed laundering case. Levels are a function of points, age and the abuse
counters; demotion is automatic, promotion to L3/L4 is human.

### 4.2 Change life cycle

```
            +-----------+   validation fails    +----------+
 submit --->| received  |---------------------->| refused  | (synchronous 422 with codes)
            +-----------+                       +----------+
                  | passes
                  v
            +-----------+  auto-accept rule     +----------+
            | pending   |---------------------->| accepted |---> new revision, feed entry
            +-----------+                       +----------+
              |   |   |                              ^
              |   |   +--- 3 yes (L2+) or reviewer --+
              |   +------- 3 no or reviewer -------> rejected (reason code + note)
              +----------- submitter -------------> withdrawn
              closes_at reached: no votes -> accepted (non-destructive) / rejected
              (destructive); otherwise yes > no decides. First "no" extends to >= 72 h.
              A newer accepted change on the same field -> superseded (re-based if clean).
              Peer or account blocked while pending -> expired.
```

Defaults (hub settings, each with the stash-box/MusicBrainz value it comes from):
`voting_period` 4 days; `destructive_min_period` 48 h; `vote_close_threshold` 3 unanimous;
`objection_window` 24 h for L2 auto-accept; `edit_update_limit` 1 (the submitter may
amend a pending change once, re-running validation); `draft_ttl` 24 h on the client for
unsent drafts.

Destructive changes: `merge`, `delete`, `image_remove`, clearing a field that has a
non-`wikidata` source, and any change to a locked field. They never auto-accept, always
wait `destructive_min_period`, and need a reviewer or three yes votes.

What is "accepted" technically: the hub writes a `hub_revisions` row (new state, sources
merged per field from the ops), updates `hub_items.current_revision`, appends to the feed,
queues the image variants if any, and notifies the submitter (webhook-free: the peer polls
`GET /changes/{id}` or sees it in the feed; a personal account gets an in-app
notification on the hub).

### 4.3 What a change looks like to the submitter

On the instance the member sees the item page as today. When the item is linked to a hub
and the upstream has `push = true`, each editable field shows "Propose to <hub>" (or the
whole form does, for a new item that has no hub id). The proposal form:

- shows only fields that pass the section 6 filter; restricted fields appear greyed with
  the sentence "This value comes from <provider> and cannot be proposed";
- requires a one-line **note** (why / where from) and, for text longer than 200
  characters or any new item, a **source URL** or the declaration "own observation of
  the physical product" (OFF rule: data from the box);
- shows duplicate candidates returned by the hub (`GET /items/by-external`, search) before
  sending, with "This is the same game" (turns the create into an update);
- after sending, the item page carries a badge "Proposed, pending" linked to
  Settings > Contributions where all proposals, states and reasons are listed; a rejection
  arrives as a notification with the reason text (the existing notify crate, en/de keys).

### 4.4 Review tooling

Reviewer queue on the hub (desktop admin anatomy, design.md 3.7, OD-01 density):

- Filters: kind, status, risk flags (conflict, duplicate, long text, new account, new
  peer, images), peer, account, age.
- One change per card: side-by-side diff (hub value / proposed value / source), the note,
  the risk flags in words, duplicate candidates, the submitter's level and history
  counts, links to the external references (Wikidata, publisher page) that the change
  claims. Reasons are **reason codes** with templated texts in en/de (`duplicate`,
  `no_source`, `wrong_value`, `restricted_source`, `spam`, `not_in_scope`, `copyright`,
  `personal_data`, `other` + mandatory note), because DSA Art. 17 asks for a specific
  statement of reasons and because contributors learn faster from consistent feedback.
- Keyboard: j/k, a (accept), r (reject -> reason picker), v (vote), m (merge with
  candidate), l (lock field), reuse the phase-4 palette mechanics.
- Bulk: select by peer or account within a time window -> reject all pending / revert all
  accepted since <time> (a `revert` change per item, applied by a moderator with one
  confirmation; OD-12 re-auth).
- Every decision writes `hub_audit`.

### 4.5 History, rollback and redaction

- `GET /items/{id}/revisions` and the hub item page show the full history with handles,
  peers, notes and sources; "Restore this revision" (L4) creates a `revert` change that is
  accepted immediately and recorded as destructive in the audit.
- Rollback of a contributor or a peer: the moderator picks a time window; the hub lists
  the affected revisions, creates one `revert` per item to the last revision not authored
  by them (skipping items that later changed by others, which are flagged for manual
  care), and applies them in one job.
- Redaction (legal, personal data): a revision's `state`/`sources` are replaced by
  `{"redacted": true}`, `redacted_at` and a reason code are set, the hash chain keeps the
  original hash. Images: the object is deleted from storage and the tombstone goes to the
  feed; the `hub_images` row keeps hash, status and reason (so the same bytes are refused
  on re-upload, 5.6).

## 5. Spam, abuse and validation

Layered, cheapest first; every layer produces a machine-readable code so the submitter's
instance can show a good error (rule: no silent drops, DSA Art. 17 spirit).

### 5.1 Synchronous validation (HTTP 422, before anything is stored)

- Envelope: body <= 256 KB, at most 50 ops per change, UTF-8, NFC-normalised, no control
  characters except newline in descriptions, ops only on fields the kind schema knows.
- Kind schema: the existing `kinds.validate_attributes` (JSON Schema per kind) runs on the
  resulting state, not only on the ops.
- Sanity rules (per kind, table-driven, with the first set for `board_game`):
  title 1..200 chars (hub is stricter than the instance's 500), not all upper-case unless
  <= 5 chars, not equal to the description, no URL in title or original title; year in
  1800..current+2; players min <= max, 1..99; playtime 1..6000 minutes, min <= max; min
  age 0..21; weight 1.0..5.0; description 20..1200 chars, no more than 2 URLs, no HTML;
  external ids checksum-valid where the namespace has one (GTIN-8/12/13/14 mod 10,
  ISBN-10/13); QIDs `^Q[1-9][0-9]*$`; BGG ids numeric; language codes BCP 47 from the
  locale list.
- "Idiotic input" heuristics (soft: flag, not refuse, unless marked hard): title and
  description from the placeholder list (`test`, `asdf`, `title`, `lorem`, brand-config
  placeholders), repeated characters (`aaaaaa`), description equal to title, description
  shorter than 20 chars (hard), year equal to current year for an item whose publisher
  field is empty and title matches an existing item (duplicate flag), players max > 20 for
  a non-party game (flag), emoji-only fields (hard), mixed-script titles with confusable
  characters (flag; Unicode confusables table), same value proposed for several unrelated
  items within an hour by one subject (flag `copy_paste`).

### 5.2 Rate limits and quotas

Per key, sliding windows in Postgres (the existing governor pattern), all configurable:

| Key | Default |
|---|---|
| anonymous IP (hashed) reads | 60 search/min, 300 item reads/min, 2 dump downloads/day |
| peer reads (signed) | 600 search/min, 3000 reads/min, feed 1/min |
| peer writes | L0: 20 changes/day, 2 images/day; L1: 200/day, 20 images; L2: 2000/day, 200 images |
| subject (account or subject hash) writes | L0: 10/day; L1: 60/day; L2: 300/day |
| reports (`POST /reports`) | 5/hour per IP, 20/day |
| account registration | 3/day per IP; invite mode by default |
| peer registration | 5/day total when `open`, else admin approval |

Over the limit: HTTP 429 with `Retry-After`; the client connector respects it (it already
does for every connector). Repeated 429 from one subject within an hour raises a risk flag
on their next changes.

### 5.3 Duplicate detection

Order of confidence (DEC-195, applied on the hub for every create and for external id
additions): QID equal; GTIN/ISBN equal (edition-level: two GTINs may belong to one game, so
a GTIN match proposes a merge, it does not force it); BGG id equal; then candidates by
normalised title (lower-case, diacritics folded, articles and punctuation removed,
`pg_trgm` similarity >= 0.6) with the same year +-1 or the same publisher; expansions
(`expansion_of` set) are never merged into their base. Candidates are returned to the
submitter before sending (4.3) and attached to the change as `possible_duplicate` for the
reviewer. A create with an exact identifier match of an existing item is refused with
`duplicate_identifier` and the existing id, so the client links instead.

### 5.4 Risk scoring

A small additive score (0..100) from flags, used only to order the queue and to decide
whether an L2 auto-accept may happen (score < 20): new subject +15, new peer +15, conflict
+30, possible duplicate +25, long text without source +20, external URL in text +10,
destructive +100 (never auto), image +10, copy-paste +20, 429 history +10. No machine
learning; every flag has a name shown to reviewers and to the submitter.

### 5.5 Reputation and sock puppets

- One personal account per e-mail (hashed), e-mail verification before L2; invite codes
  (stash-box) record the inviter, and an abuse ban can cascade to accounts invited by the
  banned account in the last 30 days (reviewed, not automatic).
- Votes count only from L2+ and only when the voter is not the submitter, not from the
  same peer subject hash, and not from the same peer as the submitter when the peer has
  fewer than 10 L2 subjects (prevents a small instance from voting its own changes
  through). Sock-puppet voting is a blockable offence (MusicBrainz code of conduct).
- A peer's trust is capped by its oldest active key age and by its ratio of rejected
  changes over the last 90 days (> 20 % -> `limited`).

### 5.6 Images

- Accepted: JPEG, PNG, WebP, AVIF; decoded with limits (max 6000x6000, max 20 MB, decode
  budget per request) in the existing assets pipeline; animated and HDR inputs are
  flattened; SVG refused.
- Metadata is stripped (EXIF incl. GPS, XMP, ICC kept only if needed for colour) before
  storage; the original hash is computed on the stripped file.
- Duplicate and blocklist: exact blake3 match against existing and taken-down images
  (refused with `image_known`), perceptual hash (dHash/pHash, Hamming <= 8, stash-box's
  recommendation ceiling) against existing images of the same item (flag `near_duplicate`)
  and against the takedown list (hard refusal).
- Provider image URLs (`image.tmdb.org`, `artworks.thetvdb.com`, `images.igdb.com`,
  `cf.geekdo-images.com`, ...) are refused as `source_url`; only `upload.wikimedia.org`
  is accepted as a remote source and only for Commons-sourced images with a licence tag
  (DEC-194 rules), fetched by the hub itself (no SSRF: fixed host allowlist).
- Each upload carries three declarations (checkboxes stored with the grant, 7.4): "I
  took this photo myself / I am the rights holder", "No identifiable person is in the
  picture" (KUG section 22), "I choose licence X".
- Review: images always go through the queue (even for L2) until the uploader has 20
  accepted images; moderators see the picture at `detail` size with the item's other
  images, the declared licence and the pHash neighbours.
- No automated nudity or content classifier in v1 (false positives, no training data
  policy); a reviewer looks at every image from L0/L1 uploaders anyway.

### 5.7 Reports and flags

Any reader may report an item, revision, image or account (8.4). Three independent
reports on the same target within 7 days hide images from the public API pending review
(`status = pending`); text is never hidden by report count alone (no heckler's veto), a
moderator decides.

## 6. Restricted-source protection

### 6.1 The problem, stated precisely

The instance legitimately holds TMDB, TheTVDB, IGDB (and, if ever built, BGG, MAL, Kitsu)
data under each provider's terms. TMDB's terms (https://www.themoviedb.org/api-terms-of-use,
last updated 2023-10-20, fetched 2026-11-09) forbid to "Make derivatives of the TMDB APIs
or TMDB Content", to "Sell, lease, or sublicense ... TMDB Content", to cache "for longer
than 6 months" (1.C) and require purging on termination (1.D); the AI clause mentions
"providing archived or cached data sets containing TMDB Content to another person or
entity". A hub that received TMDB-derived fields would hold TMDB content without a key, for
ever, under CC0: a breach the owner would be liable for. BGG's terms forbid modification
and non-commercial use only (DEC-190). The hub therefore must contain only data whose
origin is a contributor's own observation, a CC0 source (Wikidata), a free-licensed image
(Commons, own photo) or the hub itself.

The hub cannot compare against TMDB text because it must not hold TMDB text. So the strong
checks live on the **sending instance**, which has both the restricted value and the
proposal, and the hub adds **structural** checks, **policy** and **consequences**.

### 6.2 Sending side (the instance): provenance filter

The item already records `provenance: field -> connector` (D-6.7, `catalog_items.provenance`
jsonb, `merge.rs`). The proposal builder in `app` applies:

1. **Allowlist by provenance.** A field may be proposed only if its provenance is in
   `{user, import:*, wikidata, commons, hub_*}`. `import:*` is restricted to the user's
   own data formats (BG Stats, BGG collection export, Goodreads: only fields that are the
   user's own values, DEC-193: "never with BGG's catalogue fields"). Fields with
   `tmdb`, `tvdb`, `igdb`, `bgg`, `mal`, `kitsu`, `anilist`, `hardcover` and any
   connector whose `ConnectorInfo.attribution.licence` is not CC0-compatible are
   excluded. The list is derived from a new `ConnectorInfo.hub_shareable: bool` flag
   (false by default; true only for `wikidata`, `commons`, `hub`), not hand-maintained.
2. **No prefill from restricted values.** When a member edits a field whose current
   provenance is restricted, the editor starts empty and says so; the stored restricted
   value is still shown read-only beside it (this is how BGG values are already meant to
   be shown, DEC-190).
3. **Echo check.** Before a user-provenance value is included, the instance compares it
   with every restricted value it holds for that item and field (current and the last
   purged value, kept as a salted hash for 180 days so the TMDB retention purge does not
   disable the check): exact match after normalisation (case, whitespace, punctuation)
   -> hard refusal `restricted_echo`; for text >= 80 chars also 5-gram shingle Jaccard
   similarity >= 0.5 -> refusal; 0.3..0.5 -> sent with flag `near_restricted` which the
   hub treats as risk +40 and never auto-accepts. The comparison runs on the instance
   only; nothing restricted leaves it. (A hash of TMDB text kept for 180 days is itself
   "information obtained from TMDB" under the strict reading of 1.C, DEC-466; keep the
   hashes under the same retention limit, so the check works for 6 months and no longer.
   This is noted as a lawyer question in section 13.)
4. **Images.** Only assets with `assets.kind = 'upload'` and an `owner_id` can be
   proposed; anything with `assets.connector` set (provider covers) or `source_url` on a
   provider host is not offered at all.
5. **Attestation.** The change carries `attestation = {filter_version, provenance_ok:
   true, echo_check: "exact|shingle|none", instance_version}` inside the signed body. A
   peer running a build without the filter (old version) is detectable by
   `filter_version` and gets `limited` until it upgrades.
6. **No echo of hub data.** Fields whose provenance is `hub_<this upstream>` and whose
   value equals the pulled revision are never proposed (no-op changes are not sent).

### 6.3 Hub side: what it can do without TMDB data

- **Kind allowlist.** `hub.kinds` excludes `movie` and `show` in v1 (section 11). This
  alone removes the bulk of the TMDB risk; TheTVDB too.
- **Identifier namespaces.** Accepted external id namespaces per kind are an allowlist:
  `wikidata`, `gtin`, `isbn`, `bgg` (identifier only), `luding`, `sdj`, `tric_trac`,
  `igdb` (identifier only, for `video_game`, because Wikidata carries P5794 IGDB ids
  under CC0 - verify before enabling), `mal` likewise via Wikidata P4086 (verify). A bare
  numeric id is a fact, not content, and Wikidata publishes these ids under CC0; still,
  the hub stores them only when they come with a `wikidata` source or from a
  contributor's declaration, and the owner may switch them off (section 13).
- **Image hosts.** Remote image references from provider hosts are refused (5.6);
  uploaded bytes are checked against pHashes of images that were taken down for
  copyright, which over time builds a blocklist of provider artwork that was attempted.
- **Text policy.** Descriptions are capped at 1200 characters and must be "a factual
  description in your own words"; any description > 200 chars needs a source URL or the
  own-observation declaration; publisher marketing blurbs are rejected under reason
  `copyright` when a reviewer recognises them (reviewers get a "search this sentence"
  link that opens a web search in their own browser; the hub itself calls no search
  engine and no provider).
- **Stylometric flags (soft).** Descriptions in the typical shape of a synopsis
  ("When X discovers Y, ...", present-tense narrative, > 400 chars) for kinds where the
  hub expects rules/components facts are flagged `synopsis_like`; descriptions with
  provider boilerplate markers ("Overview", "Plot", trailing "...", a copyright sign,
  "All rights reserved") are flagged. Flags only order the queue.
- **Attestation checks.** Missing or old `filter_version` -> `limited`; changes without
  attestation from a peer -> refused `attestation_missing`.
- **Consequences.** A reviewer who rejects with `restricted_source` triggers: -20 points,
  the peer's counter +1; three such rejections for one peer in 30 days -> peer `limited`
  and the peer admin notified; a confirmed laundering case (moderator) -> bulk revert of
  that subject's accepted changes (4.5) and `blocked`.
- **Statement in the terms.** The hub's contributor terms forbid copying from any
  database or website (OFF rule) and name TMDB, TheTVDB, IGDB, BGG as examples; the
  instance shows the rule in the proposal form.

### 6.4 What is not feasible, said plainly

- The hub cannot prove a description is not from TMDB; it can make copying costly,
  rare and reversible. A determined contributor could retype a TMDB overview on an
  instance that has no TMDB key (so the echo check has nothing to compare). This is the
  same residual risk Wikidata, OFF and MusicBrainz carry, and the remedy is the same:
  policy, review, takedown and rollback, plus the kind allowlist that keeps the hub away
  from the kinds where this is likely.
- A shared hash list of TMDB texts published by the hub would itself be a derived
  dataset of TMDB content; not done. The salted per-instance hashes in 6.2 stay on the
  instance.
- Images cannot be checked against provider artwork the hub has never seen; the pHash
  blocklist only grows from takedowns and reviewer rejections.

## 7. GDPR and licences

Draft for a lawyer: this section states the design intent and the legal texts it rests
on; nothing here is legal advice.

### 7.1 Roles and legal bases

- **Hub operator** (the owner for the reference hub; any operator who sets `hub.serve`)
  is the **controller** for: hub accounts (handle, e-mail, OIDC subject, tokens), the
  public contribution history (handle, time, change content, note), peer records
  (origin, contact e-mail of the admin, key), reports (reporter name and e-mail, DSA Art.
  16(2)(c)), moderation records and security logs (hashed IP, User-Agent).
- **Instance operator** stays controller for their members. When a member proposes a
  change without a personal hub account, the instance sends the peer id and the
  `peer_subject_hash`, not the member's name or e-mail; the hash is a pseudonym the hub
  cannot resolve, the instance can. Under GDPR a pseudonym is still personal data for the
  party who can resolve it, so the instance's privacy notice must mention that
  contributions are forwarded to the configured hub (one sentence in the existing
  settings page and in the proposal form: "Your contribution is sent to <hub> and
  published there under CC0 with the reference `<handle or pseudonym>`").
- Legal bases (draft): accounts and contributions: Art. 6(1)(b) (the contributor asks
  for the service; https://gdpr-info.eu/art-6-gdpr/, fetched 2026-11-09); public
  attribution in history: 6(1)(f) legitimate interest in the integrity and traceability
  of an open database (OFF and MusicBrainz operate the same way) with the opt-in choice
  of a pseudonymous handle at sign-up and no real-name requirement; security logs and
  rate limiting: 6(1)(f); reports and moderation records: 6(1)(c)/(f) (DSA duties, legal
  claims); the e-mail of a reporter: 6(1)(c) where DSA Art. 16 applies, else (f).
- Records of processing: Art. 30(5) exempts organisations with fewer than 250 persons
  unless the processing is "not occasional" (https://gdpr-info.eu/art-30-gdpr/); a hub
  processes continuously, so keep a short record anyway (template in the admin docs).
- Hosted instances the owner runs for others: the owner is a **processor** for those
  tenants' member data (Art. 28 contract needed, https://gdpr-info.eu/art-28-gdpr/) and
  controller for the hub. Keep the two roles in two documents.

### 7.2 Data minimisation, retention and deletion (design rules)

| Data | Where | Retention | Deletion behaviour |
|---|---|---|---|
| Hub account e-mail | `hub_accounts.email` (encrypted) + blake3 lookup | while the account exists | erased on deletion |
| Handle in public history | `hub_revisions.created_by_account`, votes, notes | indefinitely (integrity of the public database) | replaced by `deleted-<n>` (OFF model: "random but unique username"); notes authored by the account are kept unless they contain personal data, then redacted on request |
| Contribution content (facts) | revisions | indefinitely, CC0 | not personal data; stays. Exception: a field that itself contains personal data (a name typed into a description) is redacted (4.5) |
| Personal tokens | hashed | until revoked | deleted |
| Peer admin contact | `hub_peers.contact_email` | while the peer exists + 90 days | erased |
| IP addresses | security log, hashed with a daily rotating salt for rate limiting; raw IP only in the web server access log | raw: 7 days; hashed: 30 days | automatic |
| Reports | `hub_reports` | 3 years (legal claims; OFF keeps 1-3 years) | reporter data erased after 3 years, decision kept |
| Moderation audit | `hub_audit` | 365 days (OD-13) | automatic |
| Dumps and feed | public | current dump replaced weekly; feed is the revision stream | a deletion request rewrites the handle in the dump at the next build; the feed carries a `account_pseudonymised` event so mirrors do the same |

Deletion flow (`DELETE /accounts/me`, re-auth OD-12): confirmation page explains what
stays (facts, pseudonymised history), pseudonymises in one transaction, erases e-mail,
tokens, OIDC link, invite relations, queues the feed event, and notifies mirrors through
the feed (this is the Art. 17(2) "reasonable steps ... to inform controllers which are
processing the personal data", https://gdpr-info.eu/art-17-gdpr/, fetched 2026-11-09;
and Art. 19's notification to recipients "unless this proves impossible or involves
disproportionate effort", https://gdpr-info.eu/art-19-gdpr/). Connected instances that
cached a handle in `hub_proposals` or on item pages apply the event within one sync.

Member on an instance without a hub account: deleting the member on the instance makes
the `peer_subject_hash` unresolvable (the instance deletes its mapping); nothing on the
hub needs to change because the hub never had more than the hash.

### 7.3 Why the data licence is CC0 (and what that means for seeding)

Compared (all legal codes fetched 2026-11-09; details in 2.5 and the sources):

| | CC0 1.0 | CC BY-SA 4.0 | ODbL 1.0 + DbCL 1.0 |
|---|---|---|---|
| Revocable by the licensor | no ("fully, permanently, irrevocably") | no ("irrevocable license", 2(a)(1); 6(c)) | no while compliant (9.2, 9.5); DbCL "perpetual, irrevocable" |
| Covers EU sui generis database right | yes (1(vi)) | yes (1(d), section 4) | yes, plus contract (2.1) |
| Attribution | none | yes (3(a)) | yes (4.2/4.3 notices) |
| Share-alike | no | yes (3(b); 4(b) for databases) | yes for derivative databases (4.4), not for produced works |
| Can take CC0 data in | yes | yes | yes |
| Can be pushed to Wikidata (CC0 only, "no other data license ... is compatible") | yes | no | no |
| Attribution stacking across thousands of contributors | none | severe | severe |
| Used by | Wikidata, MusicBrainz core | Wikipedia text | OSM, OFF |

Decision: **CC0 1.0 for all structured hub data.** Reasons: (1) seeding from Wikidata is
only clean if the result stays CC0-compatible, and CC0 in means CC0 out is the simplest
true statement; (2) DEC-196 "Suggest to Wikidata" needs CC0 data; under BY-SA or ODbL the
hub could never give anything back; (3) connected instances and mirrors would otherwise
have to carry licence notices per field and offer derivative databases (ODbL 4.4/4.6),
which no tracker UI does well; (4) the hub's defensibility does not come from the licence
but from being the place where contributions are reviewed and the feed is live (section
10). Trade-off accepted: a competitor may copy the whole hub (Wikidata accepts this too).
Contributors agree at account creation (and the instance shows it in the proposal form)
that they release their contributions under CC0 and waive database rights (Wikimedia
Terms of Use model); for peers the peer admin accepts the same on registration and must
show the proposal-form sentence to their members (a protocol requirement checked by
`filter_version`).

Attribution anyway: the hub asks (does not require) re-users to credit "<hub name>" with a
link, and connected instances show "Data from <hub name> (CC0)" on item pages and on the
data-sources page, as they do for Wikidata.

### 7.4 Images: licence options and the revocation question

Facts first:

- CC0 and CC BY 4.0 are irrevocable by their text; CC states "Our licenses are
  irrevocable" in the legal code preamble of both CC0 and BY-SA 4.0
  (https://creativecommons.org/publicdomain/zero/1.0/legalcode and
  https://creativecommons.org/licenses/by-sa/4.0/legalcode, fetched 2026-11-09); the
  licensor may stop distributing, which "will not terminate this Public License" (BY-SA
  6(c)).
- GDPR Art. 7(3) gives the right to withdraw **consent to the processing of personal
  data** (https://gdpr-info.eu/art-7-gdpr/). A photo of a board game box is not personal
  data; the uploader's **name** in the credit line is. So GDPR does not require the
  copyright licence to be revocable; it requires that the person can have their name
  removed (and that is what the hub can do everywhere it controls). A photo that shows a
  person is different: KUG section 22 requires the depicted person's consent for
  distribution (https://www.gesetze-im-internet.de/kunsturhg/__22.html, fetched
  2026-11-09), which is why upload declaration 2 in 5.6 exists and why such photos are
  removed on request regardless of licence.
- German copyright law allows the author to "grant to all a non-exclusive right of use
  free of charge" (UrhG section 32(3) sentence 3, English translation at
  https://www.gesetze-im-internet.de/englisch_urhg/englisch_urhg.html, fetched
  2026-11-09): free public licences are valid. Statutory revocation rights exist
  regardless of the contract: section 42 (changed conviction; the author "must adequately
  compensate the holder of the right of use", and revocation takes effect only after the
  costs are reimbursed or secured) and section 41 (non-exercise of an exclusive right,
  not relevant to a non-exclusive licence). Transfer of a right of use to a third party
  needs the author's consent (section 34(1)), which is why the clause must grant the
  right to **sublicense/redistribute** expressly and why "revocable" cannot reach
  copies that third parties already hold under a sublicence lawfully granted.

Three models, analysed:

| Model | Hub can stop distributing | Reaches connected instances | Reaches mirrors and unknown re-users | Compatible with Commons / re-use | Operator risk |
|---|---|---|---|---|---|
| A. CC0 or CC BY 4.0 chosen by the uploader (recommended default) | yes, as goodwill ("withdrawn" status + tombstone) | yes, through the tombstone within one sync; they are not legally obliged but the protocol requires it | no (licence is irrevocable; copies stay lawful) | yes | lowest; standard texts, courts know them |
| B. Operator licence, revocable for the future (the owner's wish) | yes, must | yes, through the tombstone (the protocol makes peers sublicensees who must honour withdrawals within 14 days) | no for copies made before revocation (the clause must say so, otherwise the grant is worthless to re-users); yes for future distribution if the re-user is bound by the hub terms | no: Commons and most re-users refuse revocable licences; the image cannot be pushed to Commons | medium; bespoke text, uncertain in court, every mirror must implement withdrawal |
| C. Hub-only licence, no redistribution | yes | no (instances could only hotlink) | n/a | no | kills the pull use case; rejected |

Recommendation: **A as the default, B as an opt-in choice** per upload ("Licence: CC BY
4.0 (default) / CC0 / Hub licence (revocable)"), with the consequence of B shown: "Hub
licence images are not forwarded to mirrors and cannot be suggested to Wikimedia
Commons; connected instances receive them and must remove them within 14 days of a
withdrawal." Withdrawal is the same button for all three; the difference is only what
the hub can promise. A metadata field `licence` per image drives the behaviour: the feed
marks B images `redistribution: "peers_only"`; the dump excludes them; peers that are not
`active` do not receive them.

Why not make A also "revocable as GDPR requires": GDPR does not require it (above), and a
revocable CC licence is not a CC licence. The name/credit line is the GDPR part and is
handled by pseudonymisation and `attribution_name` edits at any time.

### 7.5 Draft clause: hub image licence (model B) - draft for a lawyer, not legal advice

English (operator licence v1):

```
Hub image licence (version 1, draft)

1. Ownership. You keep all rights in the photo you upload ("the Image"). Nothing in this
   licence transfers ownership.
2. Grant. You grant the operator of this hub ("the Operator") a free of charge,
   worldwide, non-exclusive right to store, reproduce, resize, convert, display and make
   the Image available to the public as part of the hub, and to sublicense these rights
   to connected instances and to users of the hub's API for the purpose of showing the
   Image together with the catalogue entry it belongs to. The right to sublicense
   includes the right of sublicensees to store a copy and to show it to their users.
3. Credit. The Operator shows the credit line you chose next to the Image where the hub
   shows licence information. You may change or remove your credit line at any time in
   your account settings; connected instances receive the change with the next sync.
4. Withdrawal. You may withdraw this licence at any time in your account settings or by
   writing to the contact address of the hub. Withdrawal takes effect for the future:
   the Operator stops distributing the Image within 14 days, marks it as withdrawn in
   the hub's change feed and removes it from the hub's downloadable data set; connected
   instances that receive the feed are required by the hub terms to delete their copies
   within 14 days of the withdrawal notice. Copies that were lawfully made before the
   withdrawal became effective are not affected. Caches may keep the Image for up to 30
   additional days for technical reasons. Your statutory rights under sections 41 and 42
   of the German Copyright Act (UrhG) remain unaffected.
5. Your promises. You confirm that you took the Image yourself or otherwise hold the
   rights needed to grant this licence, that the Image shows no identifiable person
   (or that every identifiable person has consented to publication), and that the Image
   was not taken from another website, database or publisher material.
6. Personal data. Your account handle and credit line are personal data; see the hub's
   privacy notice for how they are processed and how you can have them removed.
   Removing your name is independent of this licence and is always possible.
7. Law. German law applies. If a court finds a provision invalid, the rest remains in
   force.
```

German (Hub-Bildlizenz, Version 1, Entwurf; written in ASCII, umlauts as ae/oe/ue):

```
Hub-Bildlizenz (Version 1, Entwurf)

1. Rechteinhaberschaft. Sie behalten alle Rechte an dem von Ihnen hochgeladenen Foto
   ("das Bild"). Diese Lizenz uebertraegt keine Rechte am Bild als solchem.
2. Rechteeinraeumung. Sie raeumen dem Betreiber dieses Hubs ("der Betreiber") ein
   unentgeltliches, weltweites, einfaches Nutzungsrecht ein, das Bild zu speichern, zu
   vervielfaeltigen, in Groesse und Format zu veraendern, anzuzeigen und als Teil des
   Hubs oeffentlich zugaenglich zu machen, sowie das Recht, diese Nutzungsrechte an
   verbundene Instanzen und an Nutzer der Schnittstelle des Hubs weiterzugeben, damit
   diese das Bild zusammen mit dem zugehoerigen Katalogeintrag anzeigen koennen. Die
   Weitergabe umfasst das Recht der Unterlizenznehmer, eine Kopie zu speichern und ihren
   Nutzern anzuzeigen.
3. Namensnennung. Der Betreiber zeigt die von Ihnen gewaehlte Urheberangabe dort an, wo
   der Hub Lizenzinformationen zum Bild anzeigt. Sie koennen Ihre Urheberangabe jederzeit
   in Ihren Kontoeinstellungen aendern oder entfernen; verbundene Instanzen erhalten die
   Aenderung mit der naechsten Synchronisation.
4. Widerruf. Sie koennen diese Lizenz jederzeit in Ihren Kontoeinstellungen oder durch
   Nachricht an die Kontaktadresse des Hubs widerrufen. Der Widerruf wirkt fuer die
   Zukunft: Der Betreiber stellt die Verbreitung des Bildes innerhalb von 14 Tagen ein,
   kennzeichnet es im Aenderungsfeed des Hubs als zurueckgezogen und entfernt es aus dem
   herunterladbaren Datenbestand des Hubs; verbundene Instanzen, die den Feed beziehen,
   sind nach den Hub-Bedingungen verpflichtet, ihre Kopien innerhalb von 14 Tagen nach
   der Widerrufsmitteilung zu loeschen. Kopien, die vor Wirksamwerden des Widerrufs
   rechtmaessig angefertigt wurden, bleiben unberuehrt. Zwischenspeicher duerfen das Bild
   aus technischen Gruenden bis zu 30 weitere Tage vorhalten. Ihre gesetzlichen Rechte
   nach den Paragraphen 41 und 42 UrhG bleiben unberuehrt.
5. Ihre Zusicherungen. Sie versichern, dass Sie das Bild selbst aufgenommen haben oder
   sonst ueber die fuer diese Lizenz noetigen Rechte verfuegen, dass auf dem Bild keine
   erkennbare Person abgebildet ist (oder dass jede erkennbare Person der
   Veroeffentlichung zugestimmt hat) und dass das Bild nicht von einer anderen Website,
   Datenbank oder aus Verlagsmaterial uebernommen wurde.
6. Personenbezogene Daten. Ihr Kontoname und Ihre Urheberangabe sind personenbezogene
   Daten; die Datenschutzhinweise des Hubs erklaeren deren Verarbeitung und wie Sie deren
   Entfernung verlangen koennen. Die Entfernung Ihres Namens ist von dieser Lizenz
   unabhaengig und jederzeit moeglich.
7. Recht. Es gilt deutsches Recht. Sollte eine Bestimmung unwirksam sein, bleibt der
   Rest wirksam.
```

Lawyer checklist for this clause: whether clause 4 (future-only effect, 14-day and
30-day windows) is a valid contractual shaping next to UrhG section 42's compensation
rule; whether the sublicensing chain to unknown API users holds up (section 34/35 UrhG
consent) or whether API users must accept the hub terms first (the design requires a
peer registration for images under model B, so the sublicensee is always known);
consumer-contract rules (AGB control, sections 305 ff. BGB) for the whole hub terms;
whether the hub operator is a "hosting service" under DSA Art. 3(g)(iii) and an "online
platform" under Art. 3(i) (it "stores and disseminates information to the public"), and
the Impressum duty (section 5 DDG) for a German operator.

### 7.6 Contributor terms (what the account sign-up and the peer registration must say)

Checkbox text (en; de to be produced with the clause): "I release my contributions to
the catalogue data under CC0 1.0 and waive any database rights in them. I will only
contribute facts I observed myself or took from sources that allow it, never from TMDB,
TheTVDB, IGDB, BoardGameGeek or similar databases. My handle and my contributions are
public; I can delete my account and my handle will be replaced by a pseudonym while my
contributions remain." Plus a link to the privacy notice (controller, contact, legal
bases, retention table 7.2, rights, supervisory authority) and to the hub terms (review
rules, reason codes, complaints, DSA points of contact).

## 8. Moderation and operations

### 8.1 Who moderates

- Day one: the owner is admin and the only L4 moderator; the queue is small (seeded data
  is system-accepted, contributions trickle in from the owner's own instances).
- Growth: promote L2 contributors with a clean record to L3 reviewers (MusicBrainz:
  auto-editors are nominated and elected; here the admin appoints after a reviewer or
  moderator proposes, logged). Target ratio: one active reviewer per 200 pending changes
  a month; the admin overview shows "median time to decision" and "oldest pending".
- Peer admins moderate nothing on the hub but see their own instance's proposals and
  rejection reasons, and are the first contact when their subjects misbehave (the
  hub sends them the subject hash and the reason).
- A code of conduct for reviewers (adapted from MusicBrainz's: approve only trivial and
  obvious changes, never decide your own disputes, revert and re-enter when challenged).

### 8.2 Tooling (hub admin area, reuse design.md 3.7 anatomy)

- Hub > Overview: pending by age, decisions per day, acceptance rate, open reports, peer
  states, feed lag of mirrors, storage used by images, seed job state.
- Hub > Queue (4.4), Hub > Reports (8.4), Hub > Peers (approve, limit, block, quotas,
  key history, last seen, rejection ratio), Hub > Accounts (levels, points, invites,
  blocks, deletion requests), Hub > Locks, Hub > Audit, Hub > Settings (the knobs in
  4.2 and 5.2, kinds, registration modes, licence texts and their versions).
- Every destructive admin action requires the OD-12 re-auth window and writes audit.

### 8.3 Operations

- Backups: the hub tables live in the same Postgres; the existing backup documentation
  applies; images are in the existing blob store under the storage budget, class
  `upload`, tier `pinned` for public images (they are the product; eviction must not
  remove them), `cache` for variants.
- Dumps: weekly `jsonl.zst` plus a `SHA256SUMS` file; the feed is the incremental path.
  Mirrors are ordinary instances with `hub.serve = true` and an upstream configured with
  `mirror = true`: they pull the feed and serve reads; writes are forwarded (`POST
  /changes` relayed with the mirror as peer and the original attestation nested) in a
  later phase.
- Metrics (`tracing` + the existing usage funnel): requests per endpoint and caller
  class, 429 counts, queue depth, decisions, feed consumers, image bytes.
- Storage budget: image originals count against the budget; the admin sets a hub share;
  when full, uploads are refused with `storage_full` instead of evicting public images.
- Staging: a second hub instance (`hub.example.org` -> `staging.hub.example.org`) with
  its own database, like OFF's `.net`; the client connector has a "test upstream" flag so
  developers do not pollute the reference hub.

### 8.4 Notice-and-action, takedowns and complaints

Shaped after DSA Art. 16 and 17 (https://www.eu-digital-services-act.com/Digital_Services_Act_Article_16.html
and ..._Article_17.html, fetched 2026-11-09) even if a small operator may be exempt from
parts of the online-platform section (Art. 19 exempts micro and small enterprises from
"This Section" except Art. 24(3); Arts. 16 and 17 are addressed to hosting services and
are not covered by that exemption on the pages read - a lawyer should confirm which
duties apply to the owner):

- Public form `/hub/report` and `POST /reports`, no account needed, electronic only,
  with the four Art. 16(2) elements: reasons, exact URL(s), name and e-mail, bona fide
  statement. Reason codes: copyright, trademark, personal data / portrait right,
  restricted source, wrong information, spam, other.
- Confirmation of receipt "without undue delay" (automatic e-mail when an address was
  given, Art. 16(4)); decision notice with redress information (Art. 16(5)); every report
  is handled by a person, the form says that no automated decision is made (Art. 16(6)).
- Statement of reasons to the affected contributor (Art. 17(3)): measure and duration,
  facts, whether a notice or own initiative triggered it, legal or contractual ground,
  redress (internal complaint, courts). Sent as an in-app notification and e-mail; the
  reason code templates carry the structure.
- Internal complaint: `POST /changes/{id}/appeal` or from the report decision, handled by
  a different moderator than the one who decided; one appeal per decision.
- Points of contact (Art. 11, 12): one e-mail address for authorities and one for users,
  shown on `/hub/about` with the hub terms, the privacy notice, the licence texts in all
  their versions, and the Impressum where required.
- Copyright takedown procedure: on a plausible notice the image is set `taken_down`
  immediately (visibility, not deletion), the tombstone goes to the feed, the uploader
  gets the statement of reasons and may counter-notice; after 14 days without a
  counter-notice the object is deleted from storage and the pHash joins the blocklist.
  Text claims (copied descriptions) are handled as a `revert` to the previous revision
  plus redaction of the offending revision when the text itself is the problem.

## 9. Security considerations

- **Peer authentication** (3.4.1): Ed25519 only, 300 s window, nonce replay store, keys
  bound to `keyid` from the database never from the request, `Content-Digest` verified
  before parsing, one signature per request, `@target-uri` covers host and path so a
  signature cannot be replayed against another hub or endpoint.
- **Private keys** at rest with the existing AES-256-GCM secrets key (OD-05), zeroized in
  memory (`zeroize` is already a dependency), never logged, rotation UI.
- **Personal tokens** stored as blake3 hashes, shown once, revocable, scoped to
  contributions only (no read of the account's e-mail through the API).
- **Input handling**: body size limits before deserialisation; JSON depth limit; NFC
  normalisation and confusable detection for titles; HTML never rendered from user
  text (plain text with the existing escaping); URLs in sources validated as `https`
  with a host, never fetched by the hub except Commons (fixed allowlist, no redirects
  followed off-host; SSRF guard reuses the connector host allowlist in `catalog-core`).
- **Image decoding** with dimension and byte limits, a time budget per decode, formats
  limited to those the `image` pipeline already handles; decode happens in the job
  worker, not in the request handler; metadata stripped; no SVG; no ICC parsing beyond
  what the library does; polyglot files (image + zip) refused by magic-byte check of the
  tail.
- **Rate limits and quotas** for every caller class (5.2); 429 everywhere; anonymous
  readers keyed by hashed IP with a daily salt so the hub never stores raw IPs in the
  database.
- **Authorization**: every review/moderation route checks the hub role of the session
  user, destructive routes need the OD-12 re-auth window; peer-signed requests can never
  perform moderation; a peer can only withdraw its own changes.
- **Integrity**: the revision hash chain lets mirrors and auditors detect tampering;
  the feed is served with ETags and the hash of the last revision in `/meta`.
- **Abuse of the review system**: votes limited to L2+, same-peer vote exclusion, appeals
  to a different moderator, audit of all decisions, reviewer code of conduct.
- **Availability**: hub endpoints run on the public lane with their own governor so that
  a flood of anonymous searches cannot starve the instance's own members; the hub role
  can be served from a separate process of the same binary (`--role hub-api`) later if
  needed; the DB is shared.
- **Supply chain**: new crates must pass `cargo-deny` (allowed licences list in HANDOFF
  2.4); candidates: an RFC 9421 implementation if a permissively licensed one exists,
  else an in-house signer over `ed25519-dalek` (BSD-3) with the canonicalisation rules
  from the RFC (the covered component set is small enough).
- **Secrets in logs**: tokens and signatures redacted by the existing tracing filters;
  `Signature` headers are never logged in full.
- **Threats considered and accepted**: a malicious peer floods with valid-looking
  changes (caps + limited + block); a reviewer goes rogue (audit, appeals, bulk revert of
  decisions by reviewer); hash collisions on images (blake3, not a concern); a mirror
  serves stale or altered data (hash chain, mirror shows its upstream and lag).

## 10. Monetisation options (low priority)

Principle: the data is CC0 and stays free; what is sold is **delivery, convenience and
hosting** (Wikimedia Enterprise sells SLAs, realtime feeds and snapshots over free
content: free tier with monthly snapshots and 50,000 on-demand requests a month, paid
tiers with daily snapshots, realtime API and "Up to 99% SLA", "Pay only for data egress
beyond your free allocation", https://enterprise.wikimedia.com/pricing/, fetched
2026-11-09; MetaBrainz sells supporter tiers from $100/month for commercial users while
non-commercial use is free, https://metabrainz.org/supporters/account-type, fetched
2026-11-09).

| Option | What is sold | Checked against | Verdict |
|---|---|---|---|
| 1. Hosted instances (subscription) | the owner runs instances for paying users (design.md 4.1 hosted mode already exists) | TMDB: "does not permit any commercial use" incl. "Charging users a fee for Your Application" without a written agreement (api-terms-of-use 2.A); IGDB non-commercial without partnership; TheTVDB has commercial tiers; Podcast Index free for any use; Wikidata/hub CC0 | allowed only if TMDB and IGDB are either licensed commercially by the owner or **switched off** in paid instances (the hosted-mode licence checkbox already records "agreement on file"); the hub itself is clean. Recommended first product. |
| 2. Hub API tiers | higher quotas, priority lane, realtime feed, SLA for commercial peers (shops, publisher tools, other trackers) | CC0 permits any use; limiting access speed is not a licence restriction (Wikimedia Enterprise precedent) | allowed; implement as `hub_peers.quotas` + a `tier` field; billing external |
| 3. Image CDN | large renditions, signed CDN URLs, high availability for peers | images CC0/CC BY (fine) or model B (peers only, fine within the clause) | allowed; attribution must survive in API metadata |
| 4. Managed backups / restore for instances | an operational service | no data licence involved | allowed |
| 5. Sponsorship and donations | OFF model ("No ads, no industry money"); donors page | none | allowed; the attribution page can carry sponsors |
| 6. Verified publisher accounts | a publisher maintains its own games (OFF Producers platform: free, the producer's values take priority) | CC0: the publisher's data becomes CC0, which publishers must accept; no exclusivity | allowed; recommended free for the data, paid only for bulk tooling or a badge; never let payment buy priority in review outcomes |
| 7. Selling the data or dumps | - | CC0 makes this pointless (anyone can mirror) | not a product |
| 8. Advertising on the hub | - | TMDB forbids ad revenue "for driving traffic or generating revenue" with TMDB content; the hub has no TMDB content, so legally possible | not recommended (trust; OFF's stance) |
| 9. White-label hub for a company (e.g. a shop's own catalogue) | consulting, support, white-label build (D-10 already) | MIT code, CC0 data | allowed |

Guard rails to write into the hub terms: review decisions and trust levels are never
for sale; quota tiers never change what is public; the free tier must stay generous
enough for home instances (every home instance is a potential contributor).

## 11. Scope beyond board games

Which kinds the hub should serve, by (a) whether the restricted provider is the only
realistic source today, (b) whether an open source exists already (then a hub adds
little), (c) laundering risk, (d) Wikidata seed quality.

| Kind | Current source | Open alternative | Laundering risk | Hub role | Phase |
|---|---|---|---|---|---|
| board_game | none built (BGG blocked by terms) | Wikidata (partial), own entry | low (rules/components facts; descriptions are the risk) | **primary** | H1 |
| video_game | IGDB (non-commercial, Twitch) | Wikidata (good coverage of notable games), hub contributions | medium (store blurbs) | second kind; identifiers via Wikidata | H7 |
| anime / manga | MAL/Kitsu planned (restrictive), AniList out | Wikidata (titles, ids), hub | high (synopses copied from MAL are common) | identifiers and facts only, descriptions off by default | H7 later |
| book / audiobook | Open Library (open), Hardcover | Open Library already open | low | not needed; maybe ISBN -> Wikidata/OL bridge | no |
| comic | Metron (open) | - | low | not needed | no |
| podcast | Podcast Index (free for any use) | - | low | not needed | no |
| music | MusicBrainz (CC0) | - | low | not needed | no |
| visual_novel | VNDB (open) | - | low | not needed | no |
| movie / show | TMDB, TheTVDB | Wikidata (ids, titles, years) | **highest** | **out of scope v1**; later at most a Wikidata-id bridge (title, year, QID, no descriptions, no images) if the owner wants independence from TMDB | not planned |
| tabletop RPG, puzzles, accessories | none | hub | low | later kinds (new KindDescriptors) | later |

Rule of thumb: the hub serves kinds where the open world is thin and the restricted
provider is the default; where an open project already exists (Open Library,
MusicBrainz, Podcast Index, VNDB, Metron) the instance uses it directly and the hub does
not duplicate it.

## 12. Phased implementation plan

Format follows `docs/handoff/phase4-plan.md`: each work package names scope, files, tests
first, acceptance, defaults. All tests run with `cargo xtask ci`; Postgres tests in
`store-pg/tests`; e2e in `src/web/tests`. Every WP writes its DECs (rule 2) and keeps
ASCII (rule 3). Kickoff rules apply (no GPL code, TDD, no AI attribution).

### WP-H1 hub-read (hub role, read API, `hub` connector pull, Wikidata seed)

- Scope: `[hub]` config (3.1) with env override and admin UI; migrations `0032_hub.sql`
  (items, revisions, images, accounts, peers, locks, audit, rate); `crates/hub` with
  `HubRepo` port, revision writer (hash chain), feed reader; `api` routes `GET /api/hub/v1/
  {meta,search,items/{id},items/{id}/revisions,items/by-external,changes,dump/latest}`
  behind `hub.serve`; connector `hub` (Search, Details, Images) reading `/meta` for
  limits and TTL; `hub.sync` cron; client tables `0033_hub_client.sql`; seed job
  `hub.seed.wikidata` and `hub.resync.wikidata` with the etiquette rules; "Data from <hub>
  (CC0)" attribution on item pages and the data-sources page; remove the BGG connector
  expectation from docs (connectors/bgg.md -> "not built, superseded by the hub";
  DEC entry).
- Files: `crates/config/src/*.rs`, `crates/hub/src/{lib,revision,feed,seed}.rs`,
  `crates/store-pg/src/repos/hub.rs`, `crates/api/src/hub_routes.rs`,
  `crates/connectors/src/hub.rs`, `apps/server/src/handlers.rs` (cron entries),
  `docs/connectors/hub.md`, `docs/connectors/bgg.md`.
- Tests first: revision hash chain unit test (same input same hash; redaction keeps
  hash); feed cursor test (`since` returns exactly newer rows, tombstones included);
  Postgres test "accepted revision updates current state and feed"; connector tests on
  recorded fixtures of our own API (synthetic, we own the format); seed mapper test from
  a saved Wikidata entity JSON (Q17271-shaped, synthetic values) to a `board_game`
  attributes object that passes the kind schema; `hub.sync` test: one changed item
  queues exactly one `catalog.refresh`; e2e: enable hub in admin, search a seeded game
  from a second instance config, item page shows the attribution.
- Acceptance: `GET /meta` lists kinds and licence; a seeded item is searchable on the hub
  within one job run; an instance with the upstream configured finds and links it;
  `/changes?since=0` streams the seed; p95 of `GET /items/{id}` <= 30 ms with 50,000
  items (debug, 4 vCPU); the dump builds under the storage budget.
- Defaults: `hub.serve = false`; kinds `["board_game"]`; TTL 7 days; seed batch 500;
  Postgres FTS + pg_trgm for search (Meilisearch optional later).

### WP-H2 hub-peers (peer registration, RFC 9421 signing, signed reads)

- Scope: Ed25519 key generation on the instance when an upstream is added; `POST /peers`,
  approval UI; `POST /peers/{id}/keys`; signer in the governed client for the `hub`
  connector; verifier middleware in `api` (300 s window, nonce store, `Content-Digest`);
  peer rate-limit class; `hub.meta.protocol_version`.
- Files: `crates/hub-wire/src/{sign,verify,canon}.rs`, `crates/catalog-core/src/http.rs`,
  `crates/api/src/hub_auth.rs`, `crates/store-pg/src/repos/hub_peers.rs`, admin pages.
- Tests first: canonicalisation vectors (hand-written from RFC 9421 appendix B.2.6 shape:
  ed25519 request signing; our own vectors, not copied text); verify rejects: bad
  signature, expired, future `created`, replayed nonce, missing digest, wrong `@target-uri`
  host; accepts a correct one; rotation keeps the old key 7 days; blocked peer -> 403.
- Acceptance: an instance registers, is approved, and its signed `GET /search` is counted
  in the peer class (visible in Hub > Peers "last seen"); an unsigned request to a
  peer-only endpoint gets 401 with a stable code.
- Defaults: `peer_registration = approve`; skew 300 s; nonce retention 10 min.

### WP-H3 hub-contribute (changes, validation, queue, trust, votes)

- Scope: change model and ops; synchronous validation with codes (5.1); duplicate
  detection (5.3); risk flags (5.4); trust levels and points (4.1); life cycle timers
  (4.2) as a cron `hub.changes.close`; reviewer queue UI with reason codes (en/de);
  votes; `POST /changes`, `GET /changes/{id}`, withdraw, appeal; client side: proposal
  builder with the provenance filter (6.2: allowlist, no prefill, echo check with salted
  hashes, attestation), proposal form, Settings > Contributions, notifications on
  decisions; `ConnectorInfo.hub_shareable`.
- Files: `crates/hub/src/{change,validate,dedupe,risk,trust,review}.rs`,
  `crates/app/src/hub_proposals.rs`, `crates/connectors/src/hub.rs` (Contribute),
  `crates/api/src/hub_routes.rs`, `crates/web/src/pages/hub/*`, `crates/notify` keys,
  migration `0034_hub_changes.sql`.
- Tests first (unit): each sanity rule has a failing and a passing case; echo check:
  exact and shingle matches refused, 0.3..0.5 flagged, unrelated text passes;
  provenance filter never emits a `tmdb` field even when the user edited it to the same
  value (regression test with a crafted item); three-way conflict detection; risk score
  table; life cycle: L2 non-destructive accepted after 24 h with no objection,
  destructive waits 48 h, first No extends to 72 h, 3 yes closes, tie at expiry rejects
  destructive and accepts non-destructive; votes from the submitter's small peer are
  ignored. Postgres: change -> accepted -> revision -> feed; bulk revert by subject.
  e2e: member proposes a corrected player count, reviewer accepts with keyboard, the
  member's instance shows "accepted" after sync and the item updates.
- Acceptance: all codes documented in `docs/connectors/hub.md`; a change with a TMDB
  field is impossible to construct through the UI and the API returns
  `restricted_echo`/`provenance_not_shareable` for crafted requests; queue p95 render
  <= 150 ms with 1,000 pending.
- Defaults: as in 4.2 and 5.2.

### WP-H4 hub-images (uploads, licences, withdrawal, tombstones)

- Scope: `POST /images` with declarations and licence choice (CC BY 4.0 default, CC0,
  operator licence v1), stripping, pHash, blocklist, review, `image_add/remove` changes,
  tombstones in the feed, client applies tombstones (deletes variants, clears
  `cover_asset`), `attribution_name` edits propagate, dump excludes model-B images,
  Commons images via DEC-194 rules with licence tag stored.
- Files: `crates/assets` (strip, phash), `crates/hub/src/images.rs`, routes, pages,
  licence texts in `config/brand/default/legal/hub-image-licence.{en,de}.md`
  (versioned, section 7.5).
- Tests first: strip removes EXIF GPS (fixture with synthetic EXIF); pHash distance
  tests; refusal of provider hosts; upload without declarations -> 422; withdrawal sets
  status, writes tombstone, dump excludes; client sync removes the variant files; CC BY
  attribution text appears in `GET /items/{id}`.
- Acceptance: an uploaded photo is public only after review; after withdrawal it is gone
  from the hub pages, the API, the dump and (after one sync) a connected instance.
- Defaults: 20 MB, 6000 px, pHash threshold 8, 14-day withdrawal propagation stated in
  the clause, 30-day cache allowance.

### WP-H5 hub-lawful (accounts, deletion, privacy notice, notice-and-action)

- Scope: personal hub accounts (invite mode, e-mail verification, OIDC reuse), personal
  tokens, `DELETE /accounts/me` with pseudonymisation and feed event, retention jobs (IP
  hashes 30 d, raw access log 7 d, reports 3 y, audit 365 d), `/hub/about` with terms,
  privacy notice, licence versions, points of contact; `/hub/report` + `POST /reports`,
  acknowledgement, decision and statement-of-reasons templates (en/de), appeals; peer
  registration acceptance text; proposal-form sentence on the client.
- Tests first: deletion replaces handle everywhere (revisions, votes, notes marked),
  erases e-mail and tokens, emits the feed event, mirrors apply it; report without the
  four elements -> 422 naming the missing one; acknowledgement sent when an e-mail was
  given; statement of reasons contains the mandatory parts; retention job deletes what
  it must and nothing else (fake clock).
- Acceptance: a lawyer-review package can be generated: the texts as shown to users,
  with version and date, exported from `/hub/about`.
- Defaults: as in 7.2.

### WP-H6 hub-ops (quotas, tiers, mirrors, metrics, staging)

- Scope: `hub_peers.quotas` and `tier`, admin editing, 429 behaviour, mirror mode
  (`upstream.mirror = true`: pull feed, serve reads, show upstream and lag), metrics in
  the usage funnel, dump signing, staging flag on the client, operations docs page.
- Tests first: quota exceeded -> 429 with `Retry-After`, tier change applies live;
  mirror reproduces the upstream state and hash after replaying the feed; feed lag shown.
- Acceptance: a second instance mirrors the reference hub from zero within one run of
  the feed and verifies the chain.

### WP-H7 hub-kinds (video games, then anime/manga identifiers)

- Scope: `hub.kinds += video_game`; Wikidata seed mapping for video games (P31 tree,
  P400 platform, P123 publisher, P178 developer, P577, P136; verify properties before
  use); IGDB id as identifier only when it arrives with a Wikidata source; sanity rules
  per kind; descriptions off by default for anime/manga.
- Acceptance: the same tests as H1/H3 for the new kind; the provenance filter blocks
  IGDB fields.

### Dependencies and order

H1 -> H2 -> H3 -> H4 -> H5 -> H6 -> H7. H1 alone already replaces BGG for board games
(read-only hub seeded from Wikidata plus local user entry). H3 is the first phase that
needs a lawyer's eye on the contributor terms before the reference hub opens to peers
outside the owner's own instances; H4 needs the image clause reviewed; H5 completes the
legal surface. Recommendation: run H1 and H2 now, open the reference hub to the owner's
own instances only, and open to third parties after H5.

## 13. Open questions for the owner (recommended default in bold)

1. Data licence for hub facts: **CC0 1.0** (alternatives CC BY-SA 4.0, ODbL; both block
   Wikidata push-back and add notice duties).
2. Default image licence: **CC BY 4.0 chosen by default, CC0 and the revocable
   operator licence (model B) selectable per upload**; alternative: model B only (no
   Commons, no mirrors for images).
3. Contributor identity: **peer-attributed with pseudonymous subject hash by default,
   personal hub account optional**; alternative: personal account mandatory (more
   accountability, more personal data, fewer contributors).
4. Movies and shows on the hub: **no in v1**; alternative: Wikidata-id bridge only
   (title, year, QID) later.
5. Restricted identifiers (`bgg`, `igdb`, `mal` ids) stored on the hub as plain
   identifiers when sourced from Wikidata or typed by a contributor: **yes for `bgg`
   (DEC-193 already does), `igdb`/`mal` only after verifying the Wikidata properties**;
   alternative: QID and GTIN/ISBN only.
6. Salted hashes of purged TMDB text kept 180 days on the instance for the echo check:
   **yes, under the same 6-month limit, ask the lawyer whether a one-way hash is
   "information obtained from TMDB"**; alternative: compare only against currently
   stored values.
7. BGG connector: **do not build; retire DEC-190/193's "optional connector" into
   "identifier only + importers"**; alternative: keep the plan for operators with a
   token.
8. Review defaults: **4-day period, 48 h destructive minimum, 3 unanimous votes, 24 h
   objection window for L2 auto-accept, 14 days + 10 accepted changes for L2**;
   alternative: MusicBrainz's 7 days.
9. Hub registration modes at launch: **personal accounts by invite, peers by admin
   approval**; alternative: open with captcha.
10. Who may be a reviewer: **admin appoints L2 contributors after nomination; owner is
    the first moderator**; alternative: election like MusicBrainz auto-editors.
11. Notice-and-action scope: **implement DSA Art. 16/17 shape fully regardless of the
    micro-enterprise question; lawyer confirms which duties bind the owner (hosting
    service vs online platform, Impressum)**.
12. First monetisation step: **hosted instances with TMDB/IGDB off (or licensed) plus
    donations; hub API tiers later**; alternative: start with hub tiers.
13. Staging hub: **yes, a second small instance before third-party peers connect**;
    alternative: a `dry_run` flag on the reference hub.
14. Search engine on the hub: **Postgres FTS + pg_trgm first, Meilisearch optional**.
15. Open Products Facts as a barcode source for board games (HANDOFF 6, unanswered):
    **not needed once the hub stores GTINs; revisit if contributors ask**.

## 14. Sources

Repository (read 2026-11-09): `docs/HANDOFF.md` (sections 2, 4, 5, 6), `docs/design/DESIGN.md`
(1, 5, 6, 7, 9a), `docs/design/ux/design.md` (3.7, 4.1-4.4), `docs/connectors/{tmdb,bgg,
tvdb,igdb,podcastindex,musicbrainz,openlibrary,trakt}.md`, `docs/research/board-game-sources.md`,
`docs/DECISIONS.md` DEC-190..198, DEC-446, DEC-453, DEC-466, `docs/handoff/phase4-plan.md`,
`src/rust/crates/domain/src/{catalog,user}.rs`, `src/rust/crates/app/src/{catalog,users}.rs`,
`src/rust/crates/catalog-core/src/{merge,model,connector}.rs`, migrations
`0001_core.sql`, `0003_catalog_provenance.sql`, `0004_storage.sql`, `0016_admin.sql`,
`0024_admin_ops.sql`, `0031_catalog_audience.sql`.

External, all fetched 2026-11-09 through the web-fetch agent (pages marked "not readable"
returned a bot-protection page or 404):

- Open Food Facts: https://world.openfoodfacts.org/terms-of-use ;
  https://world.openfoodfacts.org/data ; https://openfoodfacts.github.io/openfoodfacts-server/api/ ;
  https://world.openfoodfacts.org/privacy ; https://world.openfoodfacts.org/faq ;
  https://world.openfoodfacts.org/who-we-are ; https://world.openfoodfacts.org/donate-to-open-food-facts ;
  https://openfoodfacts.github.io/openfoodfacts-server/api/tutorials/how-to-create-data-quality-controls-in-your-app/ .
  Not readable: https://wiki.openfoodfacts.org/Data_quality , https://wiki.openfoodfacts.org/Moderation .
- stash-box (MIT): https://github.com/stashapp/stash-box ;
  https://github.com/stashapp/stash-box/blob/master/README.md ; https://docs.stashapp.cc/ ;
  https://docs.stashapp.cc/guides/scraping-scenes-via-stash-box/ ;
  https://docs.stashapp.cc/metadata-sources/stash-box-instances/ ;
  https://docs.stashapp.cc/in-app-manual/tagger/ . Not readable: https://docs.stashapp.cc/stash-box/ (404).
  Not fetched: guidelines.stashdb.org.
- MusicBrainz / MetaBrainz (docs only, server code is GPL and was not opened):
  https://musicbrainz.org/doc/How_Editing_Works ; https://musicbrainz.org/doc/Introduction_to_Voting ;
  https://musicbrainz.org/doc/Editor ; https://musicbrainz.org/doc/About/Data_License ;
  https://metabrainz.org/supporters/account-type ; https://metabrainz.org/supporters/tiers/2 ;
  https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting ; https://metabrainz.org/privacy ;
  https://musicbrainz.org/doc/Code_of_Conduct .
- Wikidata / Wikimedia / Creative Commons: https://www.wikidata.org/wiki/Wikidata:Licensing ;
  https://www.wikidata.org/wiki/User:Lydia_Pintscher_(WMDE)/CC-0 ;
  https://www.wikidata.org/wiki/Wikidata:Notability ; https://commons.wikimedia.org/wiki/Commons:Licensing ;
  https://wiki.creativecommons.org/wiki/Data ; https://creativecommons.org/publicdomain/zero/1.0/legalcode ;
  https://creativecommons.org/licenses/by-sa/4.0/legalcode ; https://creativecommons.org/faq/
  (revocation entries read from a model-extracted summary, not verbatim);
  https://creativecommons.org/share-your-work/cclicenses/ ; https://enterprise.wikimedia.com/ ;
  https://enterprise.wikimedia.com/pricing/ .
- Open Data Commons: https://opendatacommons.org/licenses/odbl/1-0/ ;
  https://opendatacommons.org/licenses/dbcl/1-0/ ; https://wiki.openstreetmap.org/wiki/Open_Database_License
  (index page only; OSMF guideline pages not fetched).
- Protocol: https://www.rfc-editor.org/rfc/rfc9421.html (HTTP Message Signatures, Feb 2024;
  registry strings partly from a summary of the later sections); https://www.rfc-editor.org/rfc/rfc9530.html
  (Digest Fields); https://docs.joinmastodon.org/spec/security/ .
- TMDB: https://www.themoviedb.org/api-terms-of-use (last updated 2023-10-20);
  https://developer.themoviedb.org/docs/faq .
- Law (texts, not advice): https://gdpr-info.eu/art-6-gdpr/ ; https://gdpr-info.eu/art-7-gdpr/ ;
  https://gdpr-info.eu/art-17-gdpr/ ; https://gdpr-info.eu/art-19-gdpr/ ; https://gdpr-info.eu/art-28-gdpr/ ;
  https://gdpr-info.eu/art-30-gdpr/ ; DSA via https://www.eu-digital-services-act.com/Digital_Services_Act_Article_3.html ,
  ..._Article_11.html , ..._Article_12.html , ..._Article_14.html , ..._Article_16.html ,
  ..._Article_17.html , ..._Article_19.html (eur-lex itself not fetched; section
  boundaries for Art. 19 should be checked there); UrhG English translation
  https://www.gesetze-im-internet.de/englisch_urhg/englisch_urhg.html (sections 31, 32,
  34, 35, 41, 42); KUG https://www.gesetze-im-internet.de/kunsturhg/__22.html and __23.html .
