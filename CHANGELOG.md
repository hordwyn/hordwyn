# Changelog

All notable changes of each release are listed here, newest first. The format is based on
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/); version numbers follow
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). Each release also has
release notes with its upgrade steps in the documentation
(`docs/modules/ROOT/pages/releases/`), and `operations.adoc` describes the routine upgrade.

Every change that is merged after v1 adds its line under "Unreleased" in the same commit.

## [Unreleased]

### Security

- lettre 0.11.24 for the mail it sends (notifications, test mail, password resets): fixes
  RUSTSEC-2026-0560 to RUSTSEC-2026-0565 (STARTTLS reply injection, dot-stuffing after a
  bare line ending, a panic on a blank EHLO line, quoted local parts and others; DEC-857).

### Fixed

- Undo of a view logged "Watched with ..." also deletes its session with those people, so the
  person card and "History with ..." no longer count it; `cli sessions orphaned [--apply]`
  removes the sessions an earlier Undo left behind (#1, DEC-853).
- Shows from TheTVDB list all their seasons and episodes: the connector reads the episode
  count and first air date of each season from the aired-order episode list; `cli
  refresh-source tvdb --apply` refreshes the shows stored before (#3, DEC-854).
- People: "Merge with..." is offered only when there is another person to merge with, and Swap
  in the merge dialog is disabled until a person to keep is chosen (#2, DEC-856).
- With the language on "automatic", the sources are searched in the language the page shows
  (the browser's), not in the instance language; REST searches use the first language of
  `Accept-Language` (#4, DEC-858).
- Books from Open Library: one edition with a wrong date no longer sets the year; the year
  comes from the work's editions, leaving out impossible and lone outlier dates (#5, DEC-859).

### Added

- The image carries the licence notices of its third-party parts (Rust crates, icons, fonts,
  Tailwind CSS) in `/usr/share/doc/app/THIRD-PARTY-NOTICES.txt`, generated at build time;
  the build and the CI job `notices` fail when they cannot be made (#9, DEC-855).

## [1.0.0] - 2026-11-17

The first release: a home instance for one household, one web process behind a reverse
proxy with TLS, reachable from the internet. The documentation page "What v1 covers"
(`docs/modules/ROOT/pages/v1-scope.adoc`) is the full statement of scope; its "Limits to
know" and the known issues of the release notes (`releases/v1.0.0.adoc`) say what v1 does
not do yet.

### Added

- The container image is published to the project's GitLab registry: `main` and the short commit
  from the default branch, `X.Y.Z`, `X.Y` and `latest` from a release tag, `X.Y.Z-rc.N` from a
  candidate (DEC-843, DEC-846).
- Tracking of movies, shows, video games, podcasts, books and board games with a data source
  for each: TMDB for movies and shows (TheTVDB for shows when the instance has a key), IGDB
  for video games, Podcast Index for podcasts, Open Library for books, Wikidata for board
  games (once the admin has entered the operator contact).
- The kinds anime, manga, comic, audiobook, music and visual novel without a source:
  statuses, progress, lists, statistics and sharing work for them; items get in by hand or
  with an import file.
- Adding a title by hand in every kind that is turned on, from the last row of the search,
  with "Already here" for titles of that kind the instance has; "Edit details" (title, year,
  picture address) for items no source answers for.
- Library with statuses, ratings, notes and reviews; progress by episode, page, position or
  play; rewatch runs; game sessions with players, scores, winner, co-op result and duration.
- History by day and year in the member's time zone; statistics by kind and year; yearly
  goals; lists that hold items of every kind and can be shared.
- People: an address book of companions, link requests and invitations, game groups;
  linked accounts see the sessions they took part in.
- Import from Trakt (its JSON files), from the legacy JSON export and from this
  application's own format (CSV or JSON Lines); export of the library in that format.
- Notifications in an inbox, with copies by e-mail, ntfy, Gotify, Discord, Matrix, Apprise
  or webhook.
- Integrations: a watched webhook for media servers and scripts, and a REST API.
- Accounts: several users per instance, invitations, two-factor sign-in (TOTP with recovery
  codes), single sign-on with OpenID Connect, an admin area.
- Light and dark theme, English and German, built to WCAG 2.2 AA and usable from the
  keyboard.
- White-label: name, colours, font and wording from a brand directory.
- Operation: a container image with the admin CLI (`cli migrate --dry-run`, `cli reindex`,
  key rotation, storage report), a compose file without default passwords that publishes on
  loopback, backup and restore with a rehearsal beside the live instance, a storage budget
  for pictures and cached responses, Postgres or Meilisearch search.

### Fixed before the release

Found while the release candidate was checked on a production host and on the CI runner.

- Settings > Privacy: the "Linked participants see the sessions I log" switch says "Saved." once
  the server has the change; a save that fails puts the switch back and says so (DEC-848).
- Background jobs: a worker whose job lease lapsed can no longer overwrite the state of the
  worker that took the job over; jobs stay at-least-once (DEC-844).
- Data sources: a source the admin switched off showed as working to members, and the search
  footer then left out that its kind group has no source (DEC-850).

### Security

- Security headers and a content security policy that allows the cover hosts of the data
  sources; sign-in throttling per account and client address; outbound requests confined to
  the hosts of the data sources (private addresses only where allowed); time limits for
  request heads and bodies; secrets at rest encrypted with a key file kept outside the blob
  directory, with an admin reminder until its backup is confirmed.
