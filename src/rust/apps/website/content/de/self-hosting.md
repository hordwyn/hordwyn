+++
title = "Selbst hosten"
description = "{name} läuft auf deinem eigenen Server mit Docker Compose oder Podman Compose: Anwendung, Postgres und Meilisearch auf einem Host, hinter deinem Reverse Proxy."
eyebrow = "Selbst hosten"
+++

## Was du brauchst

- Einen Linux-Host mit Docker Compose oder Podman Compose.
- Etwa 8 GB Arbeitsspeicher für den ersten Start, weil v1 sein Image aus dem Quelltext baut. Veröffentlichte Images sind geplant und ersparen diesen Schritt.
- Einen Reverse Proxy mit TLS vor der Anwendung (Caddy, nginx, Traefik oder der, den du schon betreibst) und einen Hostnamen dafür.

## Die Kurzfassung

```sh
git clone {source} tracker && cd tracker
cp config/.env.example config/.env   # POSTGRES_PASSWORD und MEILI_MASTER_KEY setzen
chmod 600 config/.env
docker compose --env-file config/.env -f src/docker/compose.yml up -d --build
```

Die Anwendung lauscht dann auf `127.0.0.1:3000`. Richte deinen Reverse Proxy darauf, trage die öffentliche Adresse in `config/.env` ein, öffne sie und lege im Einrichtungsassistenten das erste Konto an. Das erste Konto ist das Administratorkonto.

Die <a href="{install}" hreflang="en">Installationsanleitung</a> (englisch) enthält jeden Schritt, die Einstellungen hinter einem Reverse Proxy und was nach dem ersten Start zu prüfen ist.

## Drei Dinge für den ersten Tag

1. **Sichere den Geheimnisschlüssel.** Schlüssel der Datenquellen und Zwei-Faktor-Geheimnisse werden verschlüsselt gespeichert. Ohne die Schlüsseldatei kann eine Datenbanksicherung sie nicht öffnen, und nichts kann sie neu erzeugen.
2. **Richte eine Sicherung ein.** Datenbank und Schlüssel gehören zusammen. Die Dokumentation enthält die Befehle und eine Wiederherstellungsprobe.
3. **Trage deine Datenquellen-Schlüssel ein.** TMDB, IGDB und der Podcast Index brauchen einen eigenen Schlüssel; der Einrichtungsassistent fragt danach und speichert sie verschlüsselt.

## Deine Daten bleiben bei dir

- Alles liegt in deiner Postgres-Datenbank und deinem Datenverzeichnis.
- {name} hat keine Telemetrie und meldet sich nirgends. Ausgehende Anfragen gehen an die Datenquellen, die du eingeschaltet hast, um Titel und Cover zu holen, und an die Benachrichtigungskanäle, die du eingerichtet hast.
- Der Export gibt dir deine Bibliothek - Titel, Status, Bewertungen und Verlauf - in einem dokumentierten, offenen Format. Mitspielende, Punkte und Siege von Spielsitzungen gehören noch nicht zum Export.
- {name} führt Metadaten. Es sucht, indiziert und lädt niemals Medien.

## Weiterlesen

- <a href="{install}" hreflang="en">Installation und erster Start</a> (englisch)
- <a href="{docs}" hreflang="en">Die ganze Dokumentation</a> (englisch): Reverse Proxy, Konfiguration, Betrieb und Sicherung, White-Labelling, Konnektoren, Importformat
- [Sicherheit]({root}/security/)
