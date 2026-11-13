+++
title = "Self-hosting"
description = "{name} runs on your own server with Docker Compose or Podman Compose: the application, Postgres and Meilisearch on one host, behind your reverse proxy."
eyebrow = "Self-hosting"
+++

## What you need

- A Linux host with Docker Compose or Podman Compose.
- About 8 GB of memory for the first start, because v1 builds its image from the source. Published images are planned and will remove this step.
- A reverse proxy with TLS in front of the application (Caddy, nginx, Traefik or the one you already run), and a host name for it.

## The short version

```sh
git clone {source} tracker && cd tracker
cp config/.env.example config/.env   # set POSTGRES_PASSWORD and MEILI_MASTER_KEY
chmod 600 config/.env
docker compose --env-file config/.env -f src/docker/compose.yml up -d --build
```

The application then listens on `127.0.0.1:3000`. Point your reverse proxy at it, set the public address in `config/.env`, open that address and create the first account in the setup wizard. The first account is the administrator.

The [install guide]({install}) has every step, the settings behind a reverse proxy and what to check after the first start.

## Three things to do on day one

1. **Back up the secrets key.** Keys of data sources and two-factor seeds are stored encrypted. Without the key file a database backup cannot open them, and nothing can regenerate it.
2. **Set up a backup.** The database and the key belong together. The documentation has the commands and a restore rehearsal.
3. **Add your data-source keys.** TMDB, IGDB and the Podcast Index need a key of your own; the setup wizard asks for them and stores them encrypted.

## Your data stays yours

- Everything is stored in your Postgres database and your data directory.
- {name} has no telemetry and does not call home. Outgoing requests go to the data sources you switched on, to fetch titles and covers, and to the notification channels you configured.
- The export gives you your library - titles, status, ratings and history - in a documented, open format. Players, scores and winners of game sessions are not part of the export yet.
- {name} is a tracker of metadata. It never searches for, indexes or downloads media.

## Read on

- [Install and first run]({install})
- [All documentation]({docs}): reverse proxy, configuration reference, operations and backup, white-labelling, connectors, import format
- [Security]({root}/security/)
