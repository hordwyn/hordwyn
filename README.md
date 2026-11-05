# kithkeep

Self-hosted, multi-user, white-label tracker for movies, shows, anime, manga, books,
audiobooks, podcasts, music, video games, visual novels, comics and board games.
`kithkeep` is a codename; deployments set their own name through the brand config.

Status: under construction. See `docs/design/DESIGN.md` (spec) and `docs/DECISIONS.md`.

## Quick start

```sh
docker compose -f src/docker/compose.yml up
```

## Development

```sh
nix develop
cd src/rust
cargo xtask ci      # fmt, clippy, cargo-deny, tests
cargo xtask dev     # development server
```

## Scope boundary

This project is a metadata tracker. It never searches for, indexes or downloads media
releases, and it never talks to indexers, trackers, Prowlarr/Jackett or torrent/usenet
clients. Radarr/Sonarr integration is read-only.

## Licence

MIT, see `LICENSE`. Provider data and images keep their own licences; see the connector
pages in the documentation.
