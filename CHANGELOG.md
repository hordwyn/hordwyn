# Changelog

All notable changes of each release are listed here, newest first. The format is based on
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/); version numbers follow
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). Each release also has
release notes with its upgrade steps in the documentation
(`docs/modules/ROOT/pages/releases/`), and `operations.adoc` describes the routine upgrade.

Every change that is merged after v1 adds its line under "Unreleased" in the same commit.

## [Unreleased]

### Removed

- The project website (its generator, content, browser checks and the GitLab Pages job) and
  `cargo xtask site`: the website now lives in its own repository (`website` in the same
  GitLab group, DEC-841). Nothing changes for an instance.

## [1.0.0] - unreleased

The first release: a home instance for one household, one web process behind a reverse
proxy with TLS, reachable from the internet. The documentation page "What v1 covers"
(`docs/modules/ROOT/pages/v1-scope.adoc`) is the full statement of scope; its "Limits to
know" and the known issues of the release notes (`releases/v1.0.0.adoc`) say what v1 does
not do yet.

### Added

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

### Security

- Security headers and a content security policy that allows the cover hosts of the data
  sources; sign-in throttling per account and client address; outbound requests confined to
  the hosts of the data sources (private addresses only where allowed); time limits for
  request heads and bodies; secrets at rest encrypted with a key file kept outside the blob
  directory, with an admin reminder until its backup is confirmed.
