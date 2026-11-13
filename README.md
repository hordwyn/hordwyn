# Hordwyn

Self-hosted, multi-user, white-label tracker for movies, shows, anime, manga, books,
audiobooks, podcasts, music, video games, visual novels, comics and board games.
Hordwyn (Old English `hordwynn`, "joy of the hoard") is the name of the default brand;
deployments can set their own name through the brand config.

Status: under construction. See `docs/design/DESIGN.md` (spec) and `docs/DECISIONS.md`.

## Quick start

```sh
cp config/.env.example config/.env   # set POSTGRES_PASSWORD and MEILI_MASTER_KEY (no defaults)
docker compose --env-file config/.env -f src/docker/compose.yml up -d --build
```

The app listens on `127.0.0.1:3000`; put a reverse proxy with TLS in front of it. First run,
reverse proxy, backup and restore: `docs/modules/ROOT/pages/install.adoc`,
`reverse-proxy.adoc` and `operations.adoc`. Back up the secrets key (`/data/secrets.key`)
together with the database.

Upgrading an instance that is already running: `operations.adoc`, "Upgrade", and the release
notes of the version (`CHANGELOG.md`, `docs/modules/ROOT/pages/releases/`).

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
