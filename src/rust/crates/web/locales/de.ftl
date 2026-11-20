# Deutsche Texte (Fluent). Marken können jeden Schlüssel in brand.toml unter [strings.de]
# überschreiben.

nav-library = Bibliothek
nav-lists = Listen
nav-sources = Datenquellen
nav-sign-in = Anmelden
nav-sign-up = Konto erstellen
nav-sign-out = Abmelden
nav-main = Hauptnavigation
skip-to-content = Zum Inhalt springen

home-title = Halte fest, was du schaust, liest und spielst
home-lead = Filme, Serien, Bücher, Spiele und mehr, zusammen mit den Menschen, mit denen du sie teilst.

auth-username = Benutzername
auth-display-name = Anzeigename (optional)
auth-password = Passwort
auth-email = E-Mail (optional)
auth-totp = Einmalcode
auth-sign-in-title = Anmelden
auth-redirect-title = Anmeldung
auth-redirect-status = Du wirst zu { $provider } weitergeleitet...
auth-redirect-continue = Weiter zu { $provider }
auth-sign-up-title = Konto erstellen
auth-sign-in-submit = Anmelden
auth-sign-up-submit = Konto erstellen
auth-have-account = Schon ein Konto?
auth-no-account = Noch kein Konto?
auth-error-invalid_credentials = Benutzername oder Passwort falsch.
auth-error-totp_required = Gib den Einmalcode aus deiner Authenticator-App ein.
auth-error-invalid_totp = Dieser Einmalcode ist ungültig.
auth-error-signup_closed = Registrierung ist auf dieser Instanz nicht offen.
auth-error-invalid_invite = Diese Einladung ist nicht mehr gültig.
error-generic = Etwas ist schiefgelaufen. Bitte versuche es erneut.
# eine Seite, deren Start fehlschlug (Hydration, Race-Paket RACE-4, RACE-13)
boot-failed = Diese Seite ist nicht richtig gestartet. Lade sie neu; wenn das nicht hilft, sag der Person Bescheid, die diese Instanz betreibt.
boot-reload = Neu laden
boot-reloaded = Diese Seite ist nicht richtig gestartet und wurde neu geladen.

library-title = Bibliothek
library-empty = Deine Bibliothek ist leer
library-empty-body = Suche einen Film, eine Serie, ein Buch oder ein Spiel und füge es hinzu. Strg+K öffnet die Suche auf jeder Seite.
library-find = Etwas finden
library-add = Hinzufügen
library-group = Bereich
library-kinds = Arten
library-kinds-all = Alle Arten
library-status = Status
library-status-all = Alle
library-view = Ansicht
library-view-grid = Raster
library-view-list = Liste
library-sort = Sortierung
library-sort-last-activity = Letzte Aktivität
library-sort-title = Titel
library-sort-added = Hinzugefügt
library-sort-year = Jahr
library-sort-rating = Deine Bewertung
library-sort-plays = Partien
library-sort-reverse = Reihenfolge umkehren
library-filter = Nach Titel oder Notiz filtern
library-filter-placeholder = Filtern...
library-filters = Filter
library-filters-active = { $count ->
    [one] einer aktiv
   *[other] { $count } aktiv
}
library-tags = Schlagwörter
library-tags-none = Noch keine Schlagwörter. Du fügst sie über das Menü einer Karte hinzu.
library-facets = { $kind } filtern nach
library-facets-pick = Wähle oben eine Art, um nach Genre und weiteren Angaben zu filtern.
library-facet-any = Beliebig
library-active = Aktive Filter
library-remove-filter = Filter { $filter } entfernen
library-filter-tag = Schlagwort: { $tag }
library-filter-facet = { $name }: { $value }
library-filter-text = Text: { $text }
library-clear = Filter zurücksetzen
library-no-match = Nichts passt zu diesen Filtern
library-no-match-body = Aktiv: { $filters }. Setze die Filter zurück oder schau in einem anderen Status nach.
library-show-status = Stattdessen { $status } zeigen
library-results = Einträge
library-showing = { $shown } von { $total } angezeigt
library-error = Deine Bibliothek konnte nicht geladen werden.
library-retry = Erneut versuchen
library-loading = Deine Bibliothek wird geladen...
library-actions = Aktionen für { $title }
library-open = Öffnen
library-mark = Als { $status } markieren
library-edit-tags = Schlagwörter bearbeiten
library-tags-title = Schlagwörter für { $title }
library-tag-new = Neues Schlagwort
library-tag-add = Schlagwort hinzufügen
library-tag-remove = Schlagwort { $tag } entfernen
library-col-title = Titel
library-col-kind = Art
library-col-progress = Fortschritt
library-col-rating = Bewertung
library-col-tags = Schlagwörter
library-col-activity = Letzte Aktivität
library-col-next = Nächster Schritt
library-rating-value = { $stars } von 5
library-no-rating = Nicht bewertet
library-status-changed = Als { $status } markiert
library-next-step-named = { $step }: { $title }
library-logged = Eingetragen: { $step }
library-undo = Rückgängig
library-undone = Rückgängig gemacht
library-step-failed = Das hat nicht geklappt: { $error }
library-progress = Fortschritt
library-progress-episodes = { $done } von { $total } Folgen
library-progress-pages = Seite { $page } von { $pages }
library-progress-percent = { $percent } % geschafft
library-facet-genres = Genre
library-facet-authors = Autor:in
library-facet-narrators = Sprecher:in
library-facet-language = Sprache
library-facet-languages = Sprache
library-facet-original_language = Originalsprache
library-facet-categories = Kategorie
library-facet-artists = Künstler:in
library-facet-release_type = Veröffentlichungsart
library-facet-platforms = Plattform
library-facet-game_modes = Spielmodus
library-facet-tags = Schlagwort der Quelle
library-facet-subjects = Thema
library-facet-publishing_status = Erscheinungsstatus
library-facet-publisher = Verlag
library-facet-series = Reihe
library-facet-network = Sender
library-facet-airing_status = Ausstrahlungsstatus
library-facet-format = Format
library-facet-studios = Studio
library-facet-players_min = Mindestens Spielende
library-facet-players_max = Höchstens Spielende
library-facet-players_best = Beste Spielerzahl
library-facet-mechanics = Mechanik
library-facet-weight = Komplexität
library-count = { $count ->
    [one] Ein Eintrag
   *[other] { $count } Einträge
}

status-planned = Geplant
status-in_progress = Begonnen
status-completed = Abgeschlossen
status-paused = Pausiert
status-dropped = Abgebrochen
status-endless = Endlos

kind-movie = Film
kind-show = Serie
kind-anime = Anime
kind-manga = Manga
kind-book = Buch
kind-audiobook = Hörbuch
kind-podcast = Podcast
kind-music = Musik
kind-video_game = Videospiel
kind-visual_novel = Visual Novel
kind-comic = Comic
kind-board_game = Brettspiel

# activity groups (design.md 3.1)
kind-group-all = Alle
kind-group-watch = Sehen
kind-group-read = Lesen
kind-group-listen = Hören
kind-group-play = Spielen
kind-group-other = Sonstiges

# next step buttons (design.md 3.4); the server names the key
next-step-watched = Als gesehen markieren
next-step-watched-again = Nochmal gesehen eintragen
next-step-finished = Als beendet markieren
next-step-finished-again = Nochmal eintragen
next-step-episode = S{ $season } E{ $episode } als gesehen markieren
next-step-episode-listened = S{ $season } E{ $episode } als gehört markieren
next-step-caught-up = Auf dem neuesten Stand
next-step-update-page = Seite eintragen
next-step-update-position = Position eintragen
next-step-log-play = Partie eintragen
next-step-log-session = Sitzung eintragen
logged-step-watched = Gesehen
logged-step-watched-again = Nochmal gesehen
logged-step-finished = Beendet
logged-step-finished-again = Nochmal beendet
logged-step-episode = S{ $season } E{ $episode } gesehen
logged-step-episode-listened = S{ $season } E{ $episode } gehört
next-step-episode-short = S{ $season } E{ $episode }

item-add = Zur Bibliothek hinzufügen
item-want-watch = Möchte ich sehen
item-want-read = Möchte ich lesen
item-want-listen = Möchte ich hören
item-want-play = Möchte ich spielen
item-added-planned = Als „Geplant“ in die Bibliothek aufgenommen
item-logged-added = { $title }, in die Bibliothek aufgenommen
item-added-kept = Bleibt in der Bibliothek: Der Eintrag wurde seit dem Hinzufügen geändert.
item-added-on = Hinzugefügt am { $date }
item-times-watch = { $count ->
    [one] Einmal gesehen
   *[other] { $count }-mal gesehen
}
item-times-read = { $count ->
    [one] Einmal gelesen
   *[other] { $count }-mal gelesen
}
item-times-listen = { $count ->
    [one] Einmal gehört
   *[other] { $count }-mal gehört
}
item-times-play = { $count ->
    [one] Einmal gespielt
   *[other] { $count }-mal gespielt
}
item-times = { $count ->
    [one] Einmal beendet
   *[other] { $count }-mal beendet
}
item-times-on = { $times }, am { $date }
item-times-last-on = { $times }, zuletzt am { $date }
item-times-finished-on = { $times }, beendet am { $date }
item-fact-up-to-date = Auf dem neuesten Stand: { $progress }
item-rewatch-start = Nochmal von vorn schauen
item-rewatch-start-listen = Nochmal von vorn hören
item-rewatch-stop = Wiederholung abbrechen
item-rewatch-stop-listen = Erneutes Hören abbrechen
item-rewatch-started = Wiederholung begonnen
item-rewatch-started-listen = Erneutes Hören begonnen
item-rewatch-stopped-toast = Wiederholung abgebrochen
item-rewatch-stopped-toast-listen = Erneutes Hören abgebrochen
item-rewatch-finished = Wiederholung beendet: { $title }
item-rewatch-finished-listen = Erneutes Hören beendet: { $title }
item-rewatch-before = { $count ->
    [one] Davor einmal gesehen.
   *[other] Davor { $count }-mal gesehen.
}
item-rewatch-before-listen = { $count ->
    [one] Davor einmal gehört.
   *[other] Davor { $count }-mal gehört.
}
item-rewatch-line = Wiederholung: { $done } von { $total } Folgen. { $before }
item-rewatch-line-listen = Erneutes Hören: { $done } von { $total } Folgen. { $before }
item-rewatch-line-open = { $done ->
    [one] Wiederholung: eine Folge. { $before }
   *[other] Wiederholung: { $done } Folgen. { $before }
}
item-rewatch-line-open-listen = { $done ->
    [one] Erneutes Hören: eine Folge. { $before }
   *[other] Erneutes Hören: { $done } Folgen. { $before }
}
item-rewatch-stopped = { $times }; Wiederholung nach { $done } von { $total } Folgen abgebrochen
item-rewatch-stopped-listen = { $times }; erneutes Hören nach { $done } von { $total } Folgen abgebrochen
item-rewatch-stopped-open = { $done ->
    [one] { $times }; Wiederholung nach einer Folge abgebrochen
   *[other] { $times }; Wiederholung nach { $done } Folgen abgebrochen
}
item-rewatch-stopped-open-listen = { $done ->
    [one] { $times }; erneutes Hören nach einer Folge abgebrochen
   *[other] { $times }; erneutes Hören nach { $done } Folgen abgebrochen
}

item-rating = Deine Bewertung
item-rate = Mit { $stars } von 5 bewerten
item-history = Verlauf
item-history-empty = Noch nichts eingetragen.
item-loading-details = Details werden geladen...
item-source = Quelle: { $name }
item-cover-alt = Cover von { $title }
item-lists = Zu Liste hinzufügen
item-original-title = Originaltitel: { $title }
item-facts = Angaben zum Eintrag
item-status = Status
item-progress = Fortschritt
item-not-started = Noch nicht begonnen
item-not-rated = Nicht bewertet
item-rated = { $stars } von 5
item-more-facts = Weitere Angaben
item-more-facts-summary = Hinzugefügt, begonnen, zuletzt eingetragen
item-added = Hinzugefügt
item-started = Begonnen
item-last = Zuletzt eingetragen
item-plays = Partien
item-plays-count = { $count ->
    [one] Eine Partie
   *[other] { $count } Partien
}
item-actions = Weitere Aktionen für { $title }
item-action-bar = Nächster Schritt
item-with = Mit...
item-with-watch = Gesehen mit...
item-with-read = Gelesen mit...
item-with-listen = Gehört mit...
item-with-play = Gespielt mit...
item-with-named = { $with }: { $title }
item-tags = Tags
item-edit-tags = Tags bearbeiten
item-lists-title = Listen
item-added-to-list = Zu { $list } hinzugefügt.
item-sections = Bereiche
item-section-episodes = Folgen
item-section-about = Details
item-section-notes = Notizen
item-about-empty = Noch keine Details.
item-ids = Kennungen:
item-links = Anderswo:
game-fact-players = Mitspielende
game-fact-time = Spieldauer
game-fact-age = Alter
game-players = { $count ->
    [one] { $count } Person
   *[other] { $count } Personen
}
game-players-range = { $min }-{ $max } Personen
game-players-from = ab { $min } Personen
game-players-up-to = bis { $max } Personen
game-minutes = { $count } Min.
game-minutes-range = { $min }-{ $max } Min.
game-minutes-from = ab { $min } Min.
game-minutes-up-to = bis { $max } Min.
game-age = ab { $age } Jahren
item-note = Private Notiz
item-note-hint = Nur du siehst diese Notiz.
item-note-save = Notiz speichern
item-note-saved = Notiz gespeichert.

# log sheets and inline loggers of the item page (design.md 5.4)
log-sheet-title = Mit Details eintragen
log-when = Wann
log-today = Heute
log-yesterday = Gestern
log-pick = Datum wählen
log-date = Datum
log-people = Mit
log-with-watch = Gesehen mit
log-with-read = Gelesen mit
log-with-listen = Gehört mit
log-with-play = Gespielt mit
log-with-hint = Leer lassen, wenn niemand dabei war.
log-players = Mitspielende
log-people-add = Person hinzufügen
log-people-create = „{ $name }“ hinzufügen
log-people-none = Niemand mit diesem Namen
log-more = Mehr
log-more-watched = Bewertung und Notiz
log-more-play = Kooperativ, Ort, Notizen
log-more-session = Punkte, kooperativ, Ort, Notizen
log-note = Notiz zu diesem Titel (privat)
log-rewatch = { $count ->
    [one] Schon einmal beendet. Das wird Nummer { $next }.
   *[other] Schon { $count }-mal beendet. Das wird Nummer { $next }.
}
log-rewatch-watch = { $count ->
    [one] Schon einmal gesehen. Das wird Nummer { $next }.
   *[other] Schon { $count }-mal gesehen. Das wird Nummer { $next }.
}
log-rewatch-read = { $count ->
    [one] Schon einmal gelesen. Das wird Nummer { $next }.
   *[other] Schon { $count }-mal gelesen. Das wird Nummer { $next }.
}
log-rewatch-listen = { $count ->
    [one] Schon einmal gehört. Das wird Nummer { $next }.
   *[other] Schon { $count }-mal gehört. Das wird Nummer { $next }.
}
log-rewatch-play = { $count ->
    [one] Schon einmal gespielt. Das wird Nummer { $next }.
   *[other] Schon { $count }-mal gespielt. Das wird Nummer { $next }.
}
log-save = Speichern
log-cancel = Abbrechen
log-discard-question = Änderungen verwerfen?
log-discard = Verwerfen
log-keep = Weiter bearbeiten
log-score = Punkte von { $name }
log-coop = Kooperatives Spiel
log-coop-won = Wir haben gewonnen
log-duration = Dauer
log-duration-hint = h:mm oder Minuten
log-location = Ort
log-notes = Notizen
log-play-step = Partie
log-session-step = Sitzung
log-page = Aktuelle Seite
log-of-pages = { $pages ->
    [one] von { $pages } Seite
   *[other] von { $pages } Seiten
}
log-of-pages-estimate = { $pages ->
    [one] von { $pages } Seite (geschätzt)
   *[other] von { $pages } Seiten (geschätzt)
}
log-pages-change = Ändern
log-pages-change-label = Seitenzahl ändern
log-pages-set = Seitenzahl festlegen
log-pages-total = Seiten dieses Buchs
log-pages-total-hint = Gilt für alle auf dieser Instanz. Bleibt erhalten, wenn die Datenquelle das Buch aktualisiert.
log-pages-total-saved = Seitenzahl auf { $pages } gesetzt.
log-error-pages-total = Gib eine Zahl von 1 bis 100000 ein.
log-update = Aktualisieren
log-plus-pages = +{ $count }
log-finished = Fertig
log-position = Position
log-position-hint = Als h:mm:ss oder in Minuten.
log-of-time = von { $time }
log-plus-minutes = +{ $count } Min.
log-error-date-missing = Wähle ein Datum.
log-error-date-future = Das Datum liegt in der Zukunft.
log-error-date-before-run = Du hast am { $date } nochmal von vorn begonnen. Wähle diesen Tag oder einen späteren.
log-error-score = Punkte sind Zahlen, zum Beispiel 42 oder 12,5.
log-error-duration = Gib die Dauer als Stunden:Minuten (1:30) oder in Minuten (90) ein.
log-error-page = Gib eine Seitenzahl ab 1 ein.
log-error-page-beyond = Das hat { $pages } Seiten.
log-error-position = Gib die Position als h:mm:ss ein, innerhalb der Länge.

# the Episodes tab of the item page (design.md 6.3)
episodes-seasons = Staffeln
episodes-season = Staffel { $season }
episodes-count = { $watched }/{ $total }
episodes-episode = Folge { $episode }
episodes-mark-season = Staffel als gesehen markieren
episodes-mark = S{ $season } E{ $episode } als gesehen markieren
episodes-mark-again = S{ $season } E{ $episode } nochmal sehen
episodes-watched = Gesehen
episodes-mark-season-listen = Staffel als gehört markieren
episodes-mark-show-listen = Podcast als gehört markieren
episodes-mark-listen = S{ $season } E{ $episode } als gehört markieren
episodes-mark-again-listen = S{ $season } E{ $episode } nochmal hören
episodes-watched-listen = Gehört
episodes-again = Nochmal
episodes-next = Als Nächstes
episodes-row-menu = Mehr zu S{ $season } E{ $episode }
episodes-up-to-here = Bis hierhin markieren
episodes-ask = { $count ->
    [one] Auch die frühere Folge als gesehen markieren?
   *[other] Auch die { $count } früheren Folgen als gesehen markieren?
}
episodes-ask-listen = { $count ->
    [one] Auch die frühere Folge als gehört markieren?
   *[other] Auch die { $count } früheren Folgen als gehört markieren?
}
episodes-ask-all = Alle { $count } markieren
episodes-ask-one = Nur S{ $season } E{ $episode }
episodes-remember = Für diese Serie merken
episodes-remember-listen = Für diesen Podcast merken
episodes-logged = { $count ->
    [one] Eine Folge
   *[other] { $count } Folgen
}
episodes-none = Noch keine Folgen bekannt.
episodes-again-ask = { $count ->
    [one] Diese Folge ist schon gesehen. Nochmal als Wiederholung eintragen?
   *[other] Diese { $count } Folgen sind schon gesehen. Nochmal als Wiederholung eintragen?
}
episodes-again-ask-views = { $count ->
    [one] Diese Folge ist schon gesehen. Ein weiteres Mal eintragen?
   *[other] Diese { $count } Folgen sind schon gesehen. Alle ein weiteres Mal eintragen?
}
episodes-again-ask-listen = { $count ->
    [one] Diese Folge ist schon gehört. Nochmal eintragen?
   *[other] Diese { $count } Folgen sind schon gehört. Nochmal eintragen?
}
episodes-again-now = { $count ->
    [one] Sie wird mit dem heutigen Datum eingetragen.
   *[other] Sie werden mit dem heutigen Datum eingetragen.
}
episodes-again-all = { $count ->
    [one] Nochmal eintragen
   *[other] { $count } nochmal eintragen
}
episodes-again-cancel = Abbrechen
episodes-skipped = { $count ->
    [one] Eine schon gesehen, nicht erneut eingetragen
   *[other] { $count } schon gesehen, nicht erneut eingetragen
}
episodes-skipped-listen = { $count ->
    [one] Eine schon gehört, nicht erneut eingetragen
   *[other] { $count } schon gehört, nicht erneut eingetragen
}

search-title = Suche
search-label = Katalog durchsuchen
search-placeholder = Titel, Autorin, Studio...
search-empty = Noch keine Treffer.
search-local = In deiner Instanz
search-library = In deiner Bibliothek
search-waiting = Frage { $sources } an...



sources-title = Datenquellen
sources-lead = Die Metadaten stammen von diesen Diensten. Für ihre Daten gelten deren Bedingungen und Lizenzen.

theme-dark = Zum dunklen Design wechseln
theme-light = Zum hellen Design wechseln
dialog-close = Schließen

auth-invited = Du wurdest eingeladen: Dein Konto wird mit der Person verknüpft, die dich eingeladen hat, und du siehst die Sitzungen, bei denen du dabei warst.
privacy-title = Privatsphäre
privacy-share-sessions = Verknüpfte Mitspieler sehen die Sitzungen, die ich erfasse
privacy-saved = Gespeichert.
privacy-save-failed = Nicht gespeichert. Die Einstellung steht wieder wie vorher; bitte erneut versuchen.
sessions-by = von { $name }
sessions-claim = In mein Protokoll kopieren
sessions-claimed = In dein Protokoll kopiert.
sharing-error-conflict = Das geht bei dieser Person nicht.
sharing-error-not_found = Nicht gefunden. Prüfe den Benutzernamen.
sharing-error-forbidden = Das darfst du nicht.
sharing-error-invalid = Bitte prüfe die Felder.

nav-notifications = Benachrichtigungen
notify-title = Benachrichtigungen
notify-inbox = Posteingang
notify-inbox-empty = Nichts Neues.
notify-mark-read = Als gelesen markieren
notify-channels = Kanäle
notify-channels-lead = Wohin Benachrichtigungen außer in den Posteingang gehen. Jede Karte zeigt, wie die letzte Zustellung lief.
notify-new-channel = Kanal hinzufügen
notify-kind = Art
notify-channel-name = Name
notify-add-channel = Kanal hinzufügen
notify-test = Test senden
notify-delete = Löschen
notify-disabled = Deaktiviert
notify-kind-ntfy = ntfy
notify-kind-gotify = Gotify
notify-kind-discord = Discord
notify-kind-matrix = Matrix
notify-kind-apprise = Apprise API
notify-kind-webhook = Webhook (signiert)
notify-kind-email = E-Mail
notify-kind-web_push = Browser-Push
notify-f-server = Server-URL
notify-f-topic = Thema
notify-f-token-optional = Zugriffstoken (optional)
notify-f-app-token = App-Token
notify-f-webhook-url = Webhook-URL
notify-f-homeserver = Homeserver-URL
notify-f-room = Raum-ID
notify-f-access-token = Zugriffstoken
notify-f-apprise-urls = Apprise-URLs
notify-f-url = URL
notify-f-secret-optional = Signatur-Geheimnis (leer: wird erzeugt)
notify-f-email = E-Mail-Adresse
notify-event = Ereignis
notify-all-events = Alle Ereignisse
notify-group-release = Alle Veröffentlichungen
notify-group-library = Alle Bibliotheksänderungen
notify-group-social = Alle sozialen Ereignisse
notify-group-boardgame = Alle Brettspiel-Ereignisse
notify-group-integration = Alle Integrations-Ereignisse
notify-group-import = Alle Import-Ereignisse
notify-group-admin = Alle Admin-Ereignisse
notify-channel = Kanal
notify-all-channels = Alle Kanäle
notify-mode = Zustellung
notify-mode-immediate = Sofort
notify-mode-digest = Tägliche Zusammenfassung
notify-digest-at = Zusammenfassung um
notify-default = Instanz-Standard
notify-log = Protokoll
notify-log-empty = Noch nichts gesendet.
notify-state-queued = Wartet
notify-state-digest = In Zusammenfassung
notify-state-sending = Wird gesendet
notify-state-sent = Gesendet
notify-state-failed = Fehlgeschlagen
notify-state-suppressed = Unterdrückt
notify-error-invalid = Bitte prüfe die Felder.
notify-error-channel_error = Der Kanal hat die Nachricht nicht angenommen. Prüfe seine Einstellungen.
notify-error-forbidden = Das darfst du nicht.
notify-error-not_found = Nicht gefunden.
notify-error-unavailable = Benachrichtigungen sind auf diesem Server nicht aktiviert.

library-more = Mehr anzeigen

nav-integrations = Integrationen
integrations-title = Integrationen
integrations-lead = Lass deinen Medienserver oder ein Skript festhalten, was du geschaut hast. Jeder Absender bekommt ein eigenes Token, so kannst du einen widerrufen, ohne die anderen anzufassen.
integrations-name = Name
integrations-url = Webhook-URL
integrations-example = Derselbe Aufruf mit curl
integrations-empty = Noch keine Absender.
integrations-revoke = Widerrufen
auth-error-too_many_attempts = Zu viele Fehlversuche. Warte einen Moment und versuch es noch einmal.
auth-error-username_too_short = Der Benutzername braucht mindestens 3 Zeichen.
auth-error-username_too_long = Der Benutzername darf höchstens 32 Zeichen haben.
auth-error-username_space = Der Benutzername enthält ein Leerzeichen. Nimm stattdessen - oder _.
auth-error-username_invalid_chars = Im Benutzernamen gehen nur a-z, 0-9 und _ . -.
auth-error-username_taken = Dieser Benutzername ist schon vergeben.
auth-error-display_name_too_long = Der Anzeigename darf höchstens 100 Zeichen haben.
auth-error-password_too_short = Das Passwort braucht mindestens 10 Zeichen.
auth-error-password_too_long = Das Passwort darf höchstens 1024 Zeichen haben.
auth-error-search_unavailable = Die Suche ist gerade nicht erreichbar. Versuch es gleich noch einmal.

# ---- UX phase 0: app shell (design.md 3.2, 6.15) ----
nav-home = Start
nav-history = Verlauf
nav-people = Personen
nav-you = Du
shell-search = Suchen oder hinzufügen...
shell-ctrl = Strg
shell-add = Hinzufügen
shell-notifications = { $count ->
    [0] Benachrichtigungen
   *[other] Benachrichtigungen, { $count } ungelesen
}
shell-unread = { $count } ungelesen
shell-account = Kontomenü von { $name }
shell-admin = Verwaltung
shell-theme = Design
shell-theme-system = System
shell-theme-light = Hell
shell-theme-dark = Dunkel
shell-shortcuts = Tastenkürzel
shell-help = Hilfe und Support
shell-key-search = Suchen oder hinzufügen
shell-key-add = Etwas hinzufügen
shell-key-help = Tastenkürzel anzeigen
shell-key-close = Dialog oder Menü schliessen
shell-offline = Du bist offline. Änderungen werden gespeichert und später gesendet.
shell-dismiss = Ausblenden
notfound-title = Unter dieser Adresse gibt es nichts
notfound-lead = Der Link ist vielleicht veraltet oder falsch geschrieben.
# ---- end app shell ----

# ---- UX phase 0: form discipline on sign-up and sign-in (design.md 5.2, 2.4) ----
auth-username-hint = Für Links und zum Anmelden. { $min } bis { $max } Zeichen.
auth-username-rule-length = { $min } bis { $max } Zeichen
auth-username-rule-space = Keine Leerzeichen
auth-username-rule-chars = Nur a-z, 0-9 und _ . -
auth-rule-met = Erfüllt
auth-rule-unmet = Nicht erfüllt
auth-username-saved-as = Wird als { $username } gespeichert (Großbuchstaben werden klein)
auth-username-saved-as-trimmed = Wird als { $username } gespeichert (Leerzeichen am Anfang und Ende fallen weg)
auth-username-available = '{ $username }' ist frei.
auth-username-required = Gib einen Benutzernamen ein.
auth-username-too-short = '{ $value }' ist zu kurz: Nimm mindestens { $min } Zeichen.
auth-username-too-long = '{ $value }' hat { $count } Zeichen; erlaubt sind höchstens { $max }.
auth-username-space = '{ $value }' enthält ein Leerzeichen. Versuch '{ $dash }' oder '{ $underscore }'.
auth-username-invalid-chars = '{ $value }' enthält Zeichen, die nicht erlaubt sind: { $chars }. Erlaubt sind nur a-z, 0-9 und _ . -
auth-username-taken = { $count ->
    [0] '{ $username }' ist schon vergeben.
   *[other] '{ $username }' ist schon vergeben. Frei: { $suggestions }.
}
auth-username-suggestions = Vorschläge
auth-username-use = { $username } nehmen
auth-display-name-hint = So sehen dich andere. Leer lassen, um deinen Benutzernamen zu verwenden.
auth-display-name-too-long = Der Anzeigename hat { $count } Zeichen; erlaubt sind höchstens { $max }.
auth-password-hint = Mindestens { $min } Zeichen. Keine weiteren Regeln; ein langer Satz ist am besten. Einfügen ist erlaubt.
auth-password-rule-length = Mindestens { $min } Zeichen (bisher { $count })
auth-password-required = Gib ein Passwort ein.
auth-password-too-short = { $count ->
    [one] Das Passwort hat 1 Zeichen; es braucht mindestens { $min }.
   *[other] Das Passwort hat { $count } Zeichen; es braucht mindestens { $min }.
}
auth-password-too-long = Das Passwort hat { $count } Zeichen; erlaubt sind höchstens { $max }.
auth-show = Anzeigen
auth-error-summary = { $count ->
    [one] Es gibt ein Problem
   *[other] Es gibt { $count } Probleme
}
auth-sign-in-username-required = Gib deinen Benutzernamen ein.
auth-sign-in-password-required = Gib dein Passwort ein.
auth-totp-title = Einmalcode eingeben
auth-totp-lead = Öffne deine Authenticator-App und gib den Code ein, den sie für dieses Konto zeigt.
auth-totp-required = Gib den Einmalcode ein.
auth-back = Zurück
# ---- end form discipline ----

# ---- UX-Phase 0: i18n (DEC-171, DEC-200, DEC-201) ----
shell-language = Sprache
shell-language-auto = Automatisch
shell-key-esc = Esc
# Datum: CLDR mittleres Datum (de: dd.MM.y), kurze Uhrzeit (HH:mm); Zeiten sind UTC (DEC-200).
month-short-1 = Jan.
month-short-2 = Feb.
month-short-3 = März
month-short-4 = Apr.
month-short-5 = Mai
month-short-6 = Juni
month-short-7 = Juli
month-short-8 = Aug.
month-short-9 = Sept.
month-short-10 = Okt.
month-short-11 = Nov.
month-short-12 = Dez.
time-am = AM
time-pm = PM
datetime-utc = { $date }, { $time } UTC
progress-finished = Abgeschlossen
progress-episode = Staffel { $season }, Folge { $episode }
progress-page = Seite { $page }
progress-chapter = Kapitel { $chapter }
progress-volume = Band { $volume }
progress-position = Bei { $time }
progress-session = Sitzung eingetragen
progress-rewatch = Nochmal von vorn begonnen
progress-rewatch-stop = Vorzeitig abgebrochen
notify-mark-read-named = Als gelesen markieren: { $title }
notify-test-named = Test senden an { $name }
notify-delete-named = { $name } löschen
notify-mode-digest-at = Tägliche Zusammenfassung um { $time }
notify-event-release-item_released = Ein Titel ist erschienen
notify-event-release-episode = Neue Folge
notify-event-release-chapter = Neues Kapitel
notify-event-release-date_changed = Erscheinungstermin geändert
notify-event-library-status_changed = Status geändert
notify-event-library-metadata_changed = Details aktualisiert
notify-event-social-link_request = Verknüpfungsanfrage
notify-event-social-link_accepted = Verknüpfung angenommen
notify-event-social-session_shared = Sitzung mit dir geteilt
notify-event-social-list_shared = Liste mit dir geteilt
notify-event-boardgame-loan_due = Verliehenes Spiel bald fällig
notify-event-boardgame-loan_overdue = Verliehenes Spiel überfällig
notify-event-boardgame-wishlist_owned = Spiel von deiner Wunschliste bei jemandem, den du kennst
notify-event-integration-play_recorded = Partie von einem Sender eingetragen
notify-event-integration-failing = Sender funktioniert nicht mehr
notify-event-import-finished = Import abgeschlossen
notify-event-import-failed = Import fehlgeschlagen
notify-event-admin-storage_high = Speicher fast voll (Admin)
notify-event-admin-storage_full = Speicher voll (Admin)
notify-event-admin-connector_down = Datenquelle nicht erreichbar (Admin)
notify-event-admin-signup = Neues Konto (Admin)
notify-event-admin-job_backlog = Aufgaben stauen sich (Admin)
notify-event-admin-source_requested = Datenquelle gewünscht (Admin)
notify-event-admin-local_sign_in = Lokale Anmeldung verwendet (Admin)
# ---- Ende i18n ----

# ---- UX phase 0 review (docs/reviews/2026-11-06-ux0-review.md) ----
auth-lockout-minutes = { $minutes ->
    [one] Zu viele Fehlversuche. Versuch es in 1 Minute noch einmal.
   *[other] Zu viele Fehlversuche. Versuch es in { $minutes } Minuten noch einmal.
}
auth-wait-seconds = { $seconds ->
    [one] Zu viele Fehlversuche. Versuch es in 1 Sekunde noch einmal.
   *[other] Zu viele Fehlversuche. Versuch es in { $seconds } Sekunden noch einmal.
}
auth-check-throttled = Gerade zu viele Prüfungen; du kannst das Konto trotzdem anlegen.
auth-offline = Du bist offline
legal-nav = Rechtliches
legal-imprint = Impressum
legal-privacy = Datenschutz
legal-terms = Nutzungsbedingungen
history-today = Heute
history-yesterday = Gestern
history-day = { $weekday }, { $date }
weekday-monday = Montag
weekday-tuesday = Dienstag
weekday-wednesday = Mittwoch
weekday-thursday = Donnerstag
weekday-friday = Freitag
weekday-saturday = Samstag
weekday-sunday = Sonntag
stats-bucket-month = { $month } { $year }
nav-stats = Statistik
home-greeting = Willkommen zurück, { $name }
home-error = Die Startseite konnte nicht geladen werden.
home-loading = Deine Startseite wird geladen …
home-empty = Noch nichts hier
home-empty-body = Füge das Erste hinzu, das du gerade schaust, liest oder spielst: Suche nach einem Titel und nimm ihn in deine Bibliothek auf.
home-continue = Weitermachen
home-all-in-progress = Alles Begonnene
home-up-next = Als Nächstes
home-all-planned = Alles Geplante
home-reason-next-in-series = Nächster Teil der Reihe
home-reason-planned = Geplant
home-activity = Letzte Aktivität
home-all-history = Ganzer Verlauf
home-stats = Statistik insgesamt
home-all-stats = Alle Statistiken
stats-title = Statistik
stats-groups = Gruppen
stats-bucket = Zeiteinheit
stats-by-month = Nach Monat
stats-by-year = Nach Jahr
stats-error = Die Statistik konnte nicht geladen werden.
stats-totals = Summen insgesamt
stats-empty = Hier ist noch nichts eingetragen
stats-empty-body = Sobald du Fortschritt einträgst, erscheint hier deine Aktivität im Zeitverlauf.
stats-chart-summary = { $measure } in { $group }, { $from } bis { $to }: insgesamt { $total }, am meisten im { $peak }.
stats-chart-summary-empty = { $measure } in { $group }, { $from } bis { $to }: noch nichts.
stats-totals-summary = { $group }: { $parts }
stats-totals-part-planned = { $count } geplant
stats-totals-part-in_progress = { $count } begonnen
stats-totals-part-completed = { $count } abgeschlossen
stats-totals-part-paused = { $count } pausiert
stats-totals-part-dropped = { $count } abgebrochen
chart-value = { $label }: { $value }
stats-show-table = Als Tabelle zeigen
stats-col-month = Monat
stats-col-year = Jahr
stats-measure-events = Aktivität
stats-measure-completions = Abgeschlossene Einträge
stats-measure-finished = Gesehen
stats-measure-episodes = Folgen
stats-measure-pages = Seiten
stats-measure-plays = Partien
stats-measure-minutes = Minuten
stats-count-events = { $count ->
    [one] Ein eingetragener Schritt
   *[other] { $count } eingetragene Schritte
}
stats-count-completions = { $count ->
    [one] Ein Eintrag abgeschlossen
   *[other] { $count } Einträge abgeschlossen
}
stats-count-finished = { $count ->
    [one] Einmal gesehen
   *[other] { $count }-mal gesehen
}
stats-count-episodes = { $count ->
    [one] Eine Folge
   *[other] { $count } Folgen
}
stats-count-pages = { $count ->
    [one] Eine Seite
   *[other] { $count } Seiten
}
stats-count-plays = { $count ->
    [one] Eine Partie
   *[other] { $count } Partien
}
stats-count-minutes = { $count ->
    [one] Eine Minute
   *[other] { $count } Minuten
}
stats-measure-hours = Stunden
stats-count-hours = { $count ->
    [one] { $count } Stunde
   *[other] { $count } Stunden
}
stats-hero-label = Angesparte Zeit
stats-hero-days = { $days ->
    [one] Das ist 1 Tag
   *[other] Das sind { $days } Tage
} und { $hours ->
    [one] 1 Stunde
   *[other] { $hours } Stunden
} mit Filmen, Serien, Hören und Spielen.
stats-hero-under-day = Filme, Serien, Hören und Spiele, zusammengezählt.
stats-hero-none = Noch nichts mit bekannter Länge eingetragen.
stats-tile-movies = Filme
stats-tile-shows = Serien
stats-tile-books = Bücher
stats-tile-games = Spiele
stats-tile-movies-count = { $count ->
    [one] Ein Film gesehen
   *[other] { $count } Filme gesehen
}
stats-tile-books-count = { $count ->
    [one] Ein Buch abgeschlossen
   *[other] { $count } Bücher abgeschlossen
}
stats-also = Außerdem: { $parts }
stats-also-part = { $kind } { $amount }
stats-time-minutes = { $count ->
    [one] { $count } Minute
   *[other] { $count } Minuten
}
stats-hours-under = unter { $hours }
stats-hero-days-only = { $days ->
    [one] Das ist 1 Tag
   *[other] Das sind { $days } Tage
} mit Filmen, Serien, Hören und Spielen.
stats-also-pages = { $count ->
    [one] { $count } Seite
   *[other] { $count } Seiten
}
stats-also-episodes = { $count ->
    [one] { $count } Folge
   *[other] { $count } Folgen
}
stats-also-plays = { $count ->
    [one] { $count } Partie
   *[other] { $count } Partien
}
stats-also-finished = { $count ->
    [one] einmal gesehen
   *[other] { $count }-mal gesehen
}
stats-also-completions = { $count } abgeschlossen
stats-also-events = { $count ->
    [one] { $count } eingetragener Schritt
   *[other] { $count } eingetragene Schritte
}
stats-reading-note = Lesen wird in Seiten gezählt.
stats-finished-note = Abgeschlossene Bücher ohne Seitenzahl zählen keine Seiten.
stats-undated-note = Titel, die ohne einen eingetragenen Schritt auf Abgeschlossen gesetzt wurden, haben kein Datum: Sie zählen in den Summen, aber in keinem Monat, Jahr oder Ziel.
stats-untimed-note = Titel ohne bekannte Länge zählen als 0.
stats-library-heading = Bibliothek
stats-library-by-status = Bibliothek nach Status
stats-totals-part-entries = { $count ->
    [one] { $count } Eintrag
   *[other] { $count } Einträge
}
history-title = Verlauf
history-lead = Alles, was du eingetragen hast, das Neueste zuerst.
history-stream = Aktivität nach Tagen
history-filters = Filter
history-filters-summary = { $count ->
    [0] Art, Person, Typ, mit mir geteilt, Jahr
    [one] Ein Filter gesetzt
   *[other] { $count } Filter gesetzt
}
history-filter-kind = Art
history-filter-person = Person
history-filter-type = Typ
history-filter-year = Jahr
history-filter-shared = Nur mit mir geteilt
history-kind-all = Alle Arten
history-person-all = Mit allen
history-type-all = Alles
history-type-progress = Fortschritt
history-type-session = Sitzungen
history-year-all = Alle Jahre
history-session = Sitzung eingetragen
history-play = Partie eingetragen
history-with = mit { $names }
history-play-you = Du
history-play-won-you = Du hast gewonnen
history-play-won = Gewonnen: { $names }
history-play-coop-won = Gemeinsam gewonnen
history-play-coop-lost = Gemeinsam verloren
history-play-score = Punkte: { $score }
history-play-scores = Punkte: { $scores }
history-play-duration = { $minutes } Min.
history-time-utc = { $time } UTC
history-shared = Geteilt
history-copied = In deinem Protokoll
history-undo-named = Rückgängig: { $title }
history-copy-named = In mein Protokoll kopieren: { $title }
history-error = Der Verlauf konnte nicht geladen werden.
history-loading = Dein Verlauf wird geladen …
history-empty = Noch nichts eingetragen
history-empty-body = Eingetragen wird auf der Seite eines Titels oder mit den Knöpfen für den nächsten Schritt; alles landet hier.
history-no-match = In dieser Ansicht ist nichts eingetragen
history-no-match-body = Keine Aktivität passt zu diesen Filtern.
history-side-title = Auf dieser Seite
history-side-range = { $from } bis { $to }
history-side-rows = Einträge im Verlauf
history-side-finished = Abgeschlossen
history-side-episodes = Folgen
history-side-sessions = Sitzungen
history-side-shared = Mit dir geteilt

# ---- UX phase 2: Server-Codes und Wörter (contracts::admin) ----
auth-error-account_disabled = Dieses Konto ist deaktiviert. Wende dich an einen Admin.
auth-error-totp_locked = Deine Authenticator-App kann gerade nicht geprüft werden. Wende dich an deinen Admin.
auth-error-reauth_required = Bestätige, dass du es bist, um das zu ändern.
admin-error-reauth_required = Bestätige, dass du es bist, um das zu ändern.
admin-error-last_admin = Das ist der letzte Admin. Mach zuerst jemand anderen zum Admin.
admin-error-self_role_change = Deine eigene Rolle kannst du nicht ändern.
admin-error-self_disable = Dein eigenes Konto kannst du nicht deaktivieren.
admin-error-self_delete = Dein eigenes Konto kannst du hier nicht löschen.
admin-error-secure_cookies_need_https = Sichere Cookies brauchen eine öffentliche https-Adresse, sonst könnte sich niemand anmelden.
admin-error-fixed_by_config = Dieser Wert ist in der Konfiguration festgelegt und lässt sich hier nicht ändern.
admin-error-invalid_setting = Dieser Wert ist ungültig.
admin-error-invalid_contact = Gib eine E-Mail-Adresse oder eine Adresse ein, die mit https:// beginnt, höchstens 200 Zeichen, ohne Leerzeichen.
admin-error-unknown_setting = Diese Einstellung gibt es nicht.
admin-error-unknown_source = Diese Datenquelle gibt es nicht.
admin-error-missing_field = Fülle alle Felder aus.
admin-error-unknown_field = Dieses Feld gehört nicht zur Datenquelle.
admin-error-setup_done = Die Einrichtung ist abgeschlossen.
admin-error-forbidden = Das dürfen nur Admins.
admin-error-not_found = Nicht gefunden.
admin-error-conflict = Das geht gerade nicht.
admin-error-internal = Etwas ist schiefgegangen. Bitte versuch es noch einmal.
admin-warning-public_url_not_https = Die öffentliche Adresse nutzt kein https. Sichere Cookies lassen sich nicht einschalten.
admin-warning-secure_cookies_over_http = Sichere Cookies sind an, aber die öffentliche Adresse nutzt kein https.
admin-warning-mail_transport_unusable = Gespeichert, aber mit diesen Einstellungen kann keine E-Mail gesendet werden. Prüfe Server, Port und Sicherheit.
admin-setting-instance_hosting = Wie du hostest
admin-setting-auth_signup = Registrierung
admin-setting-auth_session_days = Sitzungsdauer (Tage)
admin-setting-auth_secure_cookies = Sichere Cookies
admin-setting-storage_max_total_bytes = Speichergrenze
admin-setting-storage_high_watermark = Aufräumen ab
admin-setting-storage_low_watermark = Aufräumen bis
admin-setting-env = Über die Umgebungsvariable { $var } gesetzt
admin-setting-file = In der Konfigurationsdatei gesetzt
admin-setting-shadowed = Gespeicherter Wert wird ignoriert, solange { $var } gesetzt ist
admin-setting-next-session = Gilt ab der nächsten Anmeldung
source-state-working = Funktioniert
source-state-not_set_up = Nicht eingerichtet
source-state-key_rejected = Schlüssel abgelehnt
source-state-key_unavailable = Schlüssel nicht verfügbar
source-state-rate_limited = Anfragelimit erreicht
source-state-circuit_open = Antwortet nicht
source-state-disabled = Ausgeschaltet
source-state-needs_account = Braucht dein Konto
source-state-unavailable = Vorübergehend nicht verfügbar
source-state-slow = Gerade langsam
source-state-retrying = Antwortet nicht, neuer Versuch folgt
source-state-needs_contact = Betreiberkontakt fehlt
source-test-ok = Funktioniert ({ $ms } ms).
source-test-key_rejected = Schlüssel abgelehnt.
source-test-unreachable = Der Dienst war nicht erreichbar.
source-test-rate_limited = Anfragelimit erreicht. Versuch es später noch einmal.
source-test-circuit_open = Nach wiederholten Fehlern pausiert. Versuch es später noch einmal.
source-test-missing_key = Kein Schlüssel gesetzt.
source-test-failed = Der Test ist fehlgeschlagen.
admin-source-field-api_key = API-Schlüssel
admin-source-field-api_secret = API-Geheimnis
admin-source-field-client_id = Client-ID
admin-source-field-client_secret = Client-Geheimnis
admin-source-field-pin = Abonnenten-PIN
admin-health-database-reachable = Erreichbar
admin-health-database-unreachable = Nicht erreichbar
admin-health-search-backend = Läuft mit { $backend }
admin-health-search-fallback = Meilisearch nicht verfügbar, die Datenbank sucht
admin-health-storage-used = { $used } von { $cap } Bytes belegt
admin-health-storage-high = Fast voll: { $used } von { $cap } Bytes
admin-health-storage-full = Voll: { $used } von { $cap } Bytes
admin-health-storage-unreadable = Der Speicherbericht ist fehlgeschlagen
admin-health-storage-off = Nicht eingerichtet
admin-health-jobs-queued = { $queued } warten
admin-health-jobs-failed = { $failed } fehlgeschlagen, { $queued } warten
admin-health-jobs-unreadable = Die Aufgabenliste lässt sich nicht lesen
admin-health-jobs-off = Nicht eingerichtet
admin-health-mail-configured = Eingerichtet
admin-health-mail-off = Nicht eingerichtet
admin-health-data_sources-working = { $working } von { $total } funktionieren
admin-health-data_sources-some_not_working = { $working } von { $total } funktionieren
admin-health-data_sources-none_working = Keine Datenquelle funktioniert
admin-health-identity_provider-configured = Eingerichtet
admin-health-identity_provider-off = Nicht eingerichtet
admin-health-reverse_proxy-fine = Keine weitergeleiteten Anfragen von einem nicht eingetragenen Proxy
admin-health-reverse_proxy-untrusted = Anfragen mit X-Forwarded-For kommen von { $peer }, das nicht in APP__SERVER__TRUSTED_PROXIES steht: alle Besucher zählen als diese Adresse, auch für die Anmeldegrenzen (zuletzt { $at })
admin-health-secrets-active = Schlüssel v{ $version } aktiv
admin-health-secrets-locked = { $count } Geheimnisse lassen sich nicht entschlüsseln
admin-health-secrets-rotation_due = Schlüssel v{ $version } ist älter als zwei Jahre: ein neuer wäre gut
admin-health-secrets-rewrap = { $count } Geheimnisse liegen noch unter einem älteren Schlüssel
setup-checklist-first_item = Füge deinen ersten Eintrag hinzu
setup-checklist-invite = Lade deinen Haushalt ein
setup-checklist-media_server = Verbinde deinen Medienserver
setup-checklist-notification_defaults = Lege die Standards für Benachrichtigungen fest
setup-checklist-backup_key = secrets.key sichern
setup-checklist-identity_provider = Verbinde einen Identitätsanbieter
admin-audit-setting_changed = Einstellung geändert
admin-audit-secret_replaced = Schlüssel ersetzt
admin-audit-secret_cleared = Schlüssel entfernt
admin-audit-secrets_rewrapped = Geheimnisse neu verschlüsselt
admin-audit-secrets_backup_confirmed = Sicherung des Geheimnisschlüssels bestätigt
admin-audit-role_changed = Rolle geändert
admin-audit-user_disabled = Konto deaktiviert
admin-audit-user_enabled = Konto aktiviert
admin-audit-user_deleted = Konto gelöscht
admin-audit-invite_created = Einladung erstellt
admin-audit-invite_revoked = Einladung widerrufen
admin-audit-licence_recorded = Lizenz vermerkt
admin-audit-wizard_completed = Einrichtung abgeschlossen
admin-audit-eviction_run = Speicher aufgeräumt
admin-audit-job_retried = Aufgabe neu gestartet
admin-audit-job_discarded = Aufgabe verworfen

# ---- UX-Phase 2: Einstellungen und Verwaltung (addendum-1 3.4, design.md 6.12, 6.13) ----
nav-settings = Einstellungen
settings-nav-label = Einstellungen
settings-group-you = Du
settings-group-admin = Instanz (Verwaltung)
settings-nav-account = Konto
settings-nav-privacy = Privatsphäre
settings-privacy-lead = Wer sieht, was du einträgst.
settings-account-lead = Dein Name, wie die App für dich aussieht und wie du dich anmeldest.
settings-account-display-name = Anzeigename
settings-account-username = Benutzername
settings-account-username-hint = Für die Anmeldung und in Links. Er lässt sich nicht ändern.
settings-account-email = E-Mail
settings-account-no-email = Nicht hinterlegt
settings-signin = Anmeldung
settings-signin-summary = Passwort und Authenticator
settings-password = Passwort ändern
settings-password-current = Aktuelles Passwort
settings-password-new = Neues Passwort
settings-password-rule = Mindestens 10 Zeichen. Deine anderen Geräte werden abgemeldet.
settings-password-save = Passwort ändern
settings-password-changed = Passwort geändert. Deine anderen Geräte sind abgemeldet.
settings-totp = Authenticator
settings-totp-on = Eine Authenticator-App ist eingerichtet. Die Anmeldung fragt nach ihrem Code.
settings-totp-off = Noch kein Authenticator. Damit fragt die Anmeldung zusätzlich nach einem Code von deinem Handy.
settings-totp-start = Authenticator einrichten
settings-totp-scan = Füge diesen Schlüssel in deiner Authenticator-App hinzu (oder öffne den Link auf dem Handy) und gib dann den angezeigten Code ein.
settings-totp-secret = Schlüssel
settings-totp-link = Link für Authenticator-Apps
settings-totp-code = Code aus der App
settings-totp-confirm = Einschalten
settings-codes-title = Deine Wiederherstellungscodes
settings-codes-lead = Jeder Code meldet dich einmal an, falls du dein Handy verlierst. Bewahre sie sicher auf.
settings-codes-copy = Alle Codes
settings-codes-once = Nur jetzt sichtbar. Gespeichert werden nur Prüfwerte, die Codes lassen sich nicht noch einmal anzeigen.
settings-copy = Kopieren
settings-copied = Kopiert
settings-replace = Ersetzen
settings-sources-lead = Woher Titel, Cover und Folgen kommen, und was du tun kannst, wenn etwas fehlt.
settings-sources-missing = Für manche Arten gibt es noch keine Datenquelle
settings-sources-no-source = { $group }: noch keine Datenquelle
settings-sources-set-up = Einrichten
settings-ask-admin = Admin fragen
settings-ask-admin-named = Admin fragen: { $name }
settings-asked = Dein Admin wurde gefragt.
settings-credential-set = Dein Schlüssel ist gesetzt (seit { $date }).
settings-credential-locked = Dein Schlüssel ist vorübergehend nicht verfügbar.
settings-credential-hint = Verschlüsselt gespeichert, wird nie wieder angezeigt.
settings-credential-save = Speichern
settings-credential-saved = Gespeichert.
settings-credential-clear = Entfernen

reauth-title = Bestätige, dass du es bist
reauth-lead = Diese Änderung betrifft die Anmeldung. Gib dein Passwort ein, um fortzufahren.
reauth-password = Passwort
reauth-code = Authenticator-Code oder Wiederherstellungscode
reauth-code-hint = Die 6 Ziffern aus deiner App oder einer deiner Wiederherstellungscodes.
reauth-confirm = Bestätigen
reauth-cancel = Abbrechen
reauth-confirm-now = Jetzt bestätigen
reauth-window-fresh = { $minutes ->
    [one] Noch 1 Minute bestätigt.
   *[other] Noch { $minutes } Minuten bestätigt.
}
reauth-window-stale = Nicht kürzlich bestätigt. Änderungen an der Anmeldung fragen zuerst nach deinem Passwort.

admin-only-title = Nur für Admins
admin-only-lead = Diese Seite gehört zur Verwaltung der Instanz. Deine eigenen Einstellungen findest du unter Einstellungen.
admin-only-back = Zu deinen Einstellungen
admin-save = Speichern
admin-saved = Gespeichert.
admin-setting-read-only = Nur lesbar
admin-nav-overview = Überblick
admin-nav-users = Personen
admin-nav-instance = Instanz
admin-nav-sso = Anmeldung und SSO
admin-nav-sources = Datenquellen
admin-nav-storage = Speicher und Aufgaben
admin-nav-notifications = Standard-Benachrichtigungen
admin-nav-audit = Protokoll
admin-status-ok = Funktioniert
admin-status-warning = Braucht Aufmerksamkeit
admin-status-error = Funktioniert nicht
admin-status-off = Aus
admin-health-name-database = Datenbank
admin-health-name-search = Suche
admin-health-name-storage = Speicher
admin-health-name-jobs = Hintergrundaufgaben
admin-health-name-mail = E-Mail
admin-health-name-data_sources = Datenquellen
admin-health-name-identity_provider = Identitätsanbieter
admin-health-name-reverse_proxy = Reverse-Proxy
admin-health-name-secrets = Gespeicherte Geheimnisse
admin-overview-lead = Was Mitglieder nicht sehen: die Teile, von denen diese Instanz abhängt.
admin-overview-attention = { $count ->
    [one] Eine Sache braucht Aufmerksamkeit
   *[other] { $count } Sachen brauchen Aufmerksamkeit
}
admin-overview-fix = Öffnen
admin-overview-hosted = Gehosteter Betrieb: Halte für jede Datenquelle die Lizenzvereinbarung fest.
admin-key-backup-title = Geheimnisschlüssel sichern
admin-key-backup-body = Gespeicherte Schlüssel für Datenquellen, Geheimnisse für Benachrichtigungen und Authenticator-Einträge sind mit diesem Schlüssel verschlüsselt. Eine Datenbanksicherung kann sie ohne ihn nicht öffnen, und der Schlüssel lässt sich nicht neu erzeugen. Kopiere ihn dorthin, wo du deine Datenbanksicherungen aufbewahrst.
admin-key-backup-file = Schlüsseldatei auf dem Server: { $path }
admin-key-backup-env = Der Schlüssel wird über die Umgebungsvariable APP__SECRETS__KEYS gesetzt.
admin-key-backup-confirm = Ich habe den Schlüssel gesichert
admin-key-backup-confirmed = Vermerkt. Dieser Hinweis kommt wieder, wenn ein neuer Schlüssel hinzukommt.
admin-overview-health = Zustand
admin-overview-open = Details: { $name }
admin-overview-details = Details
admin-overview-recent = Letzte Verwaltungsaktionen
admin-overview-recent-none = Noch keine Verwaltungsaktionen.
admin-overview-audit = Ganzes Protokoll
admin-overview-signups = Neueste Konten
admin-overview-brand-title = Marke
admin-users-lead = Wer ein Konto hat und wer Admin ist. Einladungen gibt es hier; die Registrierung stellst du unter Instanz ein.
admin-users-signup = Wer ein Konto anlegen kann
admin-signup-open = Alle
admin-signup-open-hint = Alle, die diese Adresse erreichen, können ein Konto anlegen.
admin-signup-invite = Mit Einladung
admin-signup-invite-hint = Man braucht einen Einladungslink von einem Admin oder von der Personen-Seite eines Mitglieds.
admin-signup-closed = Niemand
admin-signup-closed-hint = Keine neuen Konten. Auch Einladungen funktionieren dann nicht.
admin-invites-title = { $count ->
    [one] Offene Einladungen (1)
   *[other] Offene Einladungen ({ $count })
}
admin-invites-summary = Einladungslink erstellen oder zurückziehen
admin-invites-list = Einladungen
admin-invite-email = E-Mail (optional)
admin-invite-note = Notiz (optional)
admin-invite-create = Einladung erstellen
admin-invite-link = Einladungslink
admin-invite-once = Jetzt kopieren: Der Link wird nur einmal angezeigt. Schick ihn der Person, die du einlädst.
admin-invite-anyone = Alle mit dem Link
admin-invite-line = Erstellt am { $created }, gültig bis { $expires }
admin-invite-state-open = Offen
admin-invite-state-used = Benutzt
admin-invite-state-revoked = Zurückgezogen
admin-invite-state-expired = Abgelaufen
admin-invite-revoke = Zurückziehen
admin-invite-revoke-named = Zurückziehen: Einladung für { $name }
admin-users-people = Personen
admin-users-person = Person
admin-users-role = Rolle
admin-users-signs-in = Meldet sich an mit
admin-users-last-seen = Zuletzt gesehen
admin-users-actions = Aktionen
admin-users-admin = Admin
admin-users-member = Mitglied
admin-users-disabled = Deaktiviert
admin-users-you = (du)
admin-users-never = Nie
admin-users-with-password = Passwort
admin-users-with-totp = Authenticator
admin-users-with-oidc = Identitätsanbieter
admin-users-self = Deine eigene Rolle und dein eigenes Konto kannst du hier nicht ändern. Ein anderer Admin kann das.
admin-users-only-admin = Du bist der einzige Admin. Mach zuerst jemand anderen zum Admin, bevor sich an deiner Rolle oder deinem Konto etwas ändern kann.
admin-users-promote = Zum Admin machen
admin-users-promote-named = Zum Admin machen: { $name }
admin-users-demote = Admin entfernen
admin-users-demote-named = Admin entfernen: { $name }
admin-users-disable = Deaktivieren
admin-users-disable-named = Deaktivieren: { $name }
admin-users-enable = Aktivieren
admin-users-enable-named = Aktivieren: { $name }
admin-users-delete = Löschen
admin-users-delete-named = Löschen: { $name }
admin-users-delete-title = { $name } löschen?
admin-users-delete-body = Das Konto und seine Bibliothek werden gelöscht. Gemeinsam eingetragene Sitzungen bleiben im Verlauf der anderen, mit dieser Person als Begleitung.
admin-sources-lead = Woher Titel, Cover und Folgen kommen. Schlüssel werden verschlüsselt gespeichert und nie wieder angezeigt.
admin-contact-label = Betreiberkontakt
admin-contact-hint = Deine E-Mail-Adresse (bevorzugt: die Quellen nennen eine E-Mail-Adresse) oder eine https-Adresse. Wird bei jeder Anfrage an Open Library und Wikidata mitgeschickt, damit sie den Betreiber erreichen können; es ist dein Kontakt, nicht der eines Mitglieds. Ohne ihn wird Wikidata nicht gefragt; mit ihm schickt jede Brettspielsuche die eingegebenen Wörter an Wikidata. Nicht gewünscht? Schalte Wikidata in der Liste unten aus.
admin-contact-cleared = Entfernt. Anfragen tragen jetzt keinen Kontakt mehr.
admin-sources-note-openlibrary = Open Library ist ein gemeinschaftlicher Bibliotheksdienst mit niedrigem Anfragelimit. Ein kommerzieller Dienst mit viel Verkehr darf sich nicht auf diese Quelle stützen.
admin-sources-attention = { $count ->
    [one] Eine Datenquelle braucht Aufmerksamkeit
   *[other] { $count } Datenquellen brauchen Aufmerksamkeit
}
admin-sources-licence-banner = Gehosteter Betrieb: Halte für jede Datenquelle fest, dass die Lizenzvereinbarung vorliegt.
admin-sources-source = Quelle
admin-sources-kinds = Arten
admin-sources-status = Zustand
admin-sources-set-up = Einrichten
admin-sources-set-up-named = Einrichten: { $name }
admin-sources-details = Details
admin-sources-details-named = Details zu { $name }
admin-sources-calls = { $calls } Anfragen in den letzten 24 Stunden, { $limited } durch das Anfragelimit gebremst
admin-sources-last-ok = Letzte Antwort { $when }
admin-sources-last-error = Letzter Fehler { $when }: { $error }
admin-sources-no-key = Diese Quelle braucht keinen Schlüssel.
admin-sources-shadowed = Ein gespeicherter Schlüssel wird ignoriert, solange die Umgebungsvariable gesetzt ist.
admin-sources-key-hint = Verschlüsselt gespeichert, wird nie wieder angezeigt.
admin-sources-set-by = Gesetzt am { $date } von { $name }
admin-sources-set-on = Gesetzt am { $date }
admin-sources-locked = Gespeichert, aber der Schlüssel zum Entschlüsseln ist nicht geladen.
admin-sources-test = Testen
admin-sources-sample = Erstes Ergebnis: { $title }.
admin-sources-clear = Schlüssel entfernen
admin-sources-clear-title = Schlüssel von { $name } entfernen?
admin-sources-clear-body = Die Suche nutzt diese Quelle nicht mehr, bis ein neuer Schlüssel gespeichert ist.
admin-sources-licence = Vereinbarung liegt vor
admin-sources-licence-hint = Nötig, wenn du für andere Leute hostest: Die Bedingungen des Anbieters erlauben diese Nutzung.
admin-sources-attribution-preview = Quellenangabe
admin-sources-terms = Nutzungsbedingungen
admin-sources-attribution = Quellenangaben aller Datenquellen
admin-storage-lead = Bilder und Caches auf der Platte oder in S3 und die Hintergrundarbeit dieser Instanz.
admin-storage-full = Der Speicher ist voll: Neue Bilder werden erst wieder gespeichert, wenn Platz frei wird.
admin-storage-title = Speicher
admin-storage-used = { $used } von { $cap } belegt
admin-storage-backend = Speicherort: { $backend }
admin-storage-pinned = Behalten (Cover in Bibliotheken)
admin-storage-cache = Cache
admin-storage-evictable = Sofort freigebbar
admin-storage-high = Aufräumen beginnt bei
admin-storage-low = Aufräumen endet bei
admin-storage-evict = Jetzt aufräumen
admin-storage-evicted = { $bytes } freigegeben ({ $objects } Dateien).
admin-storage-budget = Budget
admin-storage-limit-hint = Zum Beispiel 20 GiB.
admin-storage-percent-hint = Prozent der Grenze.
admin-storage-invalid = Prüfe den Wert von { $field }.
admin-jobs-title = Hintergrundaufgaben
admin-jobs-lanes = Warteschlangen
admin-jobs-lane = Warteschlange
admin-jobs-queued = Wartend
admin-jobs-running = Laufend
admin-jobs-dead = Fehlgeschlagen
admin-jobs-failed = Fehlgeschlagene Aufgaben
admin-jobs-failed-attention = { $count ->
    [one] Eine Aufgabe ist fehlgeschlagen.
   *[other] { $count } Aufgaben sind fehlgeschlagen.
}
admin-jobs-none-failed = Keine fehlgeschlagenen Aufgaben.
admin-jobs-failed-line = { $attempts } Versuche, zuletzt { $when }
admin-jobs-retry = Erneut versuchen
admin-jobs-retry-named = Erneut versuchen: { $name }
admin-jobs-discard = Verwerfen
admin-jobs-discard-named = Verwerfen: { $name }
admin-jobs-discard-title = { $name } verwerfen?
admin-jobs-discard-body = Die Aufgabe wird entfernt und läuft nicht mehr.
admin-jobs-cron = Geplant
admin-jobs-name = Name
admin-jobs-schedule = Zeitplan
admin-jobs-next = Nächster Lauf
admin-defaults-lead = Regeln für alle, die keine eigenen festgelegt haben. Mitglieder ändern ihre unter Einstellungen, Benachrichtigungen.
admin-defaults-event = Ereignis
admin-defaults-delivery = Zustellung
admin-defaults-none = Es sind keine Standardregeln festgelegt.
admin-defaults-events = Ereignisse dieser Instanz
admin-defaults-events-summary = { $count ->
    [one] 1 Ereignis
   *[other] { $count } Ereignisse
}
admin-defaults-for-admins = Admins
admin-defaults-for-everyone = alle
admin-brand-name = Name
admin-brand-short-name = Kurzname
admin-brand-locale = Standardsprache
admin-brand-dir = Markenverzeichnis
admin-brand-accent = Akzentfarbe
admin-brand-light = Helles Design
admin-brand-dark = Dunkles Design
admin-brand-sample = Aa
admin-brand-ratio = Kontrast { $ratio }:1
admin-brand-unreadable = Kontrast lässt sich nicht prüfen
admin-brand-pass = Erfüllt AA
admin-brand-fail = Unter AA (4,5:1)
admin-brand-edit = Um die Marke zu ändern, bearbeite brand.toml in { $dir } und starte neu.
admin-security-window = Deine Bestätigung
admin-security-window-rule = Änderungen an der Anmeldung fragen erneut nach deinem Passwort, wenn { $slide } Minuten lang keine solche Änderung war, spätestens { $cap } Minuten nach deiner Bestätigung.
admin-security-sessions = Sitzungen
admin-security-days-hint = 1 bis 365 Tage.
admin-security-days-invalid = Gib 1 bis 365 Tage an.
admin-security-cookies-hint = Browser senden das Sitzungscookie nur über https. Gilt ab der nächsten Anmeldung, deine aktuelle Sitzung läuft weiter.
admin-secrets-title = Gespeicherte Geheimnisse
admin-secrets-active = Aktiver Schlüssel
admin-secrets-version = Version { $version }
admin-secrets-source = Geladen aus
admin-secrets-from-file = Der Schlüsseldatei
admin-secrets-from-env = Der Umgebung
admin-secrets-generated = Einer beim ersten Start erzeugten Schlüsseldatei: sichere sie zusammen mit der Datenbank
admin-secrets-ephemeral = Einem vorübergehenden Schlüssel (nur dieser Prozess)
admin-secrets-file = Schlüsseldatei
admin-secrets-old = Geheimnisse mit älterem Schlüssel
admin-secrets-rewrap = Jetzt neu verschlüsseln
admin-secrets-rewrapped = { $count } Geheimnisse neu verschlüsselt; { $locked } ließen sich nicht entschlüsseln.
admin-secrets-versions = Schlüsselversionen
admin-secrets-versions-summary = { $count ->
    [one] 1 Version
   *[other] { $count } Versionen
}
admin-secrets-key = Schlüssel
admin-secrets-values = Geheimnisse
admin-secrets-state = Zustand
admin-secrets-state-active = Aktiv
admin-secrets-state-loaded = Geladen
admin-secrets-state-missing = Nicht geladen
admin-secrets-state-removable = Nicht mehr benutzt, kann entfernt werden
admin-audit-lead = Jede Verwaltungsaktion, wer sie ausgeführt hat und was sich geändert hat. 365 Tage aufbewahrt. Geheime Werte erscheinen nie.
admin-audit-filter-who = Wer
admin-audit-filter-what = Was
admin-audit-anyone = Alle
admin-audit-anything = Jede Aktion
admin-audit-apply = Anwenden
admin-audit-csv = Als CSV exportieren
admin-audit-none = Keine passenden Verwaltungsaktionen.
admin-audit-when = Wann
admin-audit-who = Wer
admin-audit-what = Was
admin-audit-detail = Detail
admin-audit-actor-system = System
admin-audit-actor-cli = Kommandozeile

## Einrichtungsassistent (design.md 5.1)
setup-title = Einrichtung
setup-title-named = { $product } einrichten
setup-sign-in-lead = Die Einrichtung geht weiter, sobald sich die Administration anmeldet.
setup-progress = Fortschritt der Einrichtung
setup-step-done-label = erledigt
setup-step-account = Admin-Konto
setup-step-hosting = Betriebsart
setup-step-sources = Datenquellen
setup-step-done = Fertig
setup-account-title = Admin-Konto anlegen
setup-account-lead = Dieses erste Konto verwaltet die Instanz. Du bleibst angemeldet.
setup-account-submit = Admin-Konto anlegen
setup-hosting-title = Wie betreibst du die Instanz?
setup-hosting-lead = Das ändert Voreinstellungen, keine Funktionen. Du kannst es später unter Admin ändern.
setup-hosting-home = Zuhause: kostenlos, für Familie und Freunde
setup-hosting-home-body = Registrierung per Einladung. Keine Lizenz-Erinnerungen.
setup-hosting-hosted = Gehostet: du betreibst sie für andere
setup-hosting-hosted-body = Lizenz-Erinnerungen an: halte je Datenquelle fest, dass eine Vereinbarung vorliegt. Danach Registrierung schließen oder Leute einladen.
setup-hosting-fixed = Die Betriebsart ist durch die Konfiguration festgelegt.
setup-continue = Weiter
setup-back = Zurück
setup-sources-title = Datenquellen
setup-sources-lead = Trage die Schlüssel ein, die du hast, und teste sie. Alles ist freiwillig: Schlüssel kannst du später unter Admin, Datenquellen ergänzen.
setup-sources-read-only = Alle Schlüssel kommen aus der Umgebung. Diese verwendet die Instanz.
setup-sources-hosted = Die kostenlosen Zugänge vieler Quellen (zum Beispiel TMDB und IGDB) decken keine bezahlten Dienste ab. Halte je Quelle fest, ob eine Vereinbarung vorliegt.
setup-done-title = Fertig
setup-done-lead = Ein paar Dinge, die sich als Nächstes lohnen. Jeder Punkt öffnet seine Seite.
setup-checklist = Nächste Schritte
setup-task-done = Erledigt
setup-task-todo = Offen
setup-key-file = Datei: { $file }
setup-go-home = Zur Startseite
setup-go-users = Zu Benutzer und Registrierung

## Hinweis für Admins auf der Startseite (design.md 6.1)
home-nudge-setup = Die Einrichtung ist noch nicht abgeschlossen.
home-nudge-setup-action = Einrichtung fortsetzen
home-nudge-no-source = { $count ->
    [one] { $groups } hat noch keine Datenquelle, die Suche findet dort nichts.
   *[other] { $groups } haben noch keine Datenquelle, die Suche findet dort nichts.
}
home-nudge-set-up = Datenquellen einrichten
home-nudge-failing = { $count ->
    [one] { $sources } ist eingerichtet, antwortet aber nicht.
   *[other] { $sources } sind eingerichtet, antworten aber nicht.
}
home-nudge-check = Datenquellen prüfen
home-nudge-contact = { $count ->
    [one] { $sources } wird erst gefragt, wenn du den Betreiberkontakt eingetragen hast; bis dahin findet die Suche dort nichts.
   *[other] { $sources } werden erst gefragt, wenn du den Betreiberkontakt eingetragen hast; bis dahin findet die Suche dort nichts.
}
home-nudge-contact-action = Betreiberkontakt eintragen
home-nudge-snooze = 7 Tage ausblenden
home-nudge-decline = Nicht mehr erinnern

## Fußzeile und Keine-Treffer-Karte der Suche (design.md 6.4)
search-searching = Suche läuft...
search-none-title = Nichts passt zu „{ $query }“
search-none-library = Deine Bibliothek wurde durchsucht.
search-none-searched = Deine Bibliothek und { $sources } wurden durchsucht.
search-none-spelling = Prüfe die Schreibweise oder versuche einen anderen Titel.
search-none-group = { $group }: nicht durchsucht, keine Quelle eingerichtet.
search-none-skipped = { $source }: nicht durchsucht ({ $state }).
search-unset-title = Die Suche ist noch nicht eingerichtet
search-unset-admin = Für diese Suche hat die Instanz keine Datenquellen, online lässt sich nichts finden.
search-unset-member = Für diese Suche gibt es noch keine Datenquelle. Bitte deine Administration, eine hinzuzufügen.
search-scope = Suchen in
search-footer-label = Datenquellen
search-footer-source = { $source }: { $state }
search-footer-source-kinds = { $source } ({ $kinds }): { $state }
search-footer-group = { $group }: keine Quelle eingerichtet

# UX phase 2 review, web pages (UX2-W1, W7, W12, W14, W21)
admin-sources-howto = So bekommst du einen Schlüssel
admin-sources-howto-link = Entwicklerseite von { $name } öffnen (öffnet einen neuen Tab)
admin-sources-howto-tmdb-1 = Melde dich bei themoviedb.org an oder lege ein kostenloses Konto an.
admin-sources-howto-tmdb-2 = Öffne in den Kontoeinstellungen „API“ und beantrage einen Schlüssel für ein privates, nicht-kommerzielles Projekt.
admin-sources-howto-tmdb-3 = Kopiere das „API Read Access Token“ (das lange) und füge es hier ein.
admin-sources-howto-igdb-1 = IGDB nutzt Twitch-Konten: Melde dich damit in der Twitch-Entwicklerkonsole an.
admin-sources-howto-igdb-2 = Registriere eine Anwendung; als OAuth-Weiterleitungsadresse genügt http://localhost.
admin-sources-howto-igdb-3 = Kopiere ihre Client-ID, erzeuge ein neues Client-Geheimnis und füge beides hier ein.
admin-sources-howto-tvdb-1 = Lege ein Konto bei thetvdb.com an.
admin-sources-howto-tvdb-2 = Beantrage auf der Seite mit den API-Informationen einen Schlüssel und beschreibe diesen Server als dein Projekt.
admin-sources-howto-tvdb-3 = Füge den API-Schlüssel hier ein. Braucht dein Schlüssel eine Abonnenten-PIN, trägt jedes Mitglied seine eigene unter Einstellungen, Datenquellen ein.
admin-sources-howto-podcastindex-1 = Öffne api.podcastindex.org und registriere ein kostenloses Entwicklerkonto.
admin-sources-howto-podcastindex-2 = Bestätige deine E-Mail-Adresse; API-Schlüssel und API-Geheimnis kommen per E-Mail.
admin-sources-howto-podcastindex-3 = Füge API-Schlüssel und API-Geheimnis hier ein.
admin-sources-attention-line = { $name }: { $state }
admin-sources-attention-open = { $name } öffnen
settings-replace-named = { $field } ersetzen
admin-only-lead-signed-out = Diese Seite gehört zur Verwaltung der Instanz. Melde dich mit einem Admin-Konto an, um sie zu öffnen.
admin-error-unauthorized = Deine Sitzung ist abgelaufen. Melde dich erneut an, um weiterzumachen.
auth-error-unauthorized = Deine Sitzung ist abgelaufen. Melde dich erneut an, um weiterzumachen.
auth-error-forbidden = Das darfst du nicht.

# Phase 3 (WP3-4): Anmeldung mit Identitätsanbietern (addendum-1 4.3-4.5, DEC-326).
signin-continue = Weiter mit { $provider }
signin-password-instead = Stattdessen mit Passwort anmelden
signin-provider-fallback = dem Identitätsanbieter
signin-sso-only = Mitglieder melden sich über { $provider } an.
signin-outage-title = { $provider } antwortet nicht
signin-outage-body = Wir konnten den Anmeldedienst nicht erreichen. Deine Daten sind sicher. Versuch es in einer Minute noch einmal.
signin-outage-capped = Wir versuchen es vorerst nicht weiter, damit diese Seite dich nicht ständig zurückschickt. Versuch es in ein paar Minuten noch einmal.
signin-retry = Noch einmal versuchen
signin-break-glass = Admins: mit einem lokalen Konto anmelden
signin-error-oidc_denied = { $provider } hat diese Anmeldung nicht erlaubt.
signin-error-group_rule = Dein Konto ist hier nicht zugelassen (Gruppenregel).
signin-error-no_account = Für diese Anmeldung gibt es kein Konto. Bitte deinen Admin um eine Einladung.
signin-error-account_disabled = Dieses Konto ist deaktiviert. Frag einen Admin.
signin-error-identity_in_use = Diese Anmeldung ist schon mit einem anderen Konto verknüpft.
signin-error-oidc_failed = Die Anmeldung hat nicht geklappt. Versuch es noch einmal oder melde dich anders an.
signin-session-ended-title = Deine Sitzung ist beendet
signin-session-ended-body = Es ist nichts verloren gegangen. Melde dich wieder an, um dort weiterzumachen, wo du warst.
signin-local-title = Lokale Anmeldung
signin-local-lead = Für Admins, wenn die zentrale Anmeldung nicht verfügbar ist. Mitglieder melden sich über den Identitätsanbieter an.
signin-local-audit = Diese Anmeldung wird im Protokoll festgehalten.
signed-out-title = Du bist abgemeldet
signed-out-lead = Du kannst diesen Tab schließen oder dich wieder anmelden.
signin-link-title = Ist das dein Konto?
signin-link-lead = Dein Konto bei { $provider } nutzt dieselbe Adresse wie das Konto { $username } hier. Melde dich einmal mit dem Passwort dieses Kontos an, um beide zu verknüpfen. Danach meldet dich { $provider } allein an.
signin-link-password = Passwort für { $username }
signin-link-submit = Verknüpfen und anmelden
signin-link-not-mine = Das ist nicht mein Konto
signin-link-no-password = Dieses Konto hat kein Passwort und kann hier nicht verknüpft werden. Bitte deinen Admin, es zu verknüpfen.
signin-link-expired = Diese Verknüpfungsanfrage ist abgelaufen. Melde dich noch einmal an, um neu zu beginnen.
signin-link-back = Zurück zur Anmeldung
signin-reset-title = Wähle ein neues Passwort
signin-reset-lead = Dieser Link funktioniert einmal. Ein neues Passwort meldet alle Sitzungen des Kontos ab.
signin-reset-submit = Neues Passwort festlegen
signin-reset-done = Dein Passwort ist geändert. Melde dich jetzt damit an.
signin-reset-error-reset_link_invalid = Dieser Link gilt nicht mehr. Bitte deinen Admin um einen neuen.
auth-error-sso_only = Melde dich über den Identitätsanbieter an. Admins können die lokale Anmeldung nutzen.
datetime-local = { $date }, { $time }

# Bestätigung über einen Anbieter (DEC-323).
reauth-with-provider = Mit { $provider } bestätigen
reauth-returned-saved = Du hast bestätigt, dass du es bist. Die Änderung ist gespeichert.
reauth-returned-again = Du hast bestätigt, dass du es bist. Mach die Änderung noch einmal; jetzt geht sie durch.
reauth-error-reauth_not_fresh = Der Anbieter hat nicht nach einer neuen Anmeldung gefragt, deshalb zählt das nicht als Bestätigung. Nimm dein Passwort oder frag einen Admin.
reauth-error-oidc_denied = Der Anbieter hat die Bestätigung nicht erlaubt.
reauth-error-oidc_unavailable = Der Anbieter antwortet nicht. Versuch es in einer Minute noch einmal.
reauth-error-oidc_failed = Die Bestätigung hat nicht geklappt. Versuch es noch einmal.
reauth-error-no_linked_provider = Dieses Konto hat keine Anmeldung bei einem Anbieter. Nimm dein Passwort.

# Admin > Instanz (addendum-1 3.4).
admin-instance-lead = Einstellungen, die hier für alle gelten. Werte aus der Umgebung werden angezeigt, lassen sich auf dieser Seite aber nicht ändern.
admin-hosting-title = Wie du hostest
admin-hosting-home = Zuhause: kostenlos, für Familie und Freunde
admin-hosting-home-hint = Offene Voreinstellungen; jeder Schlüssel einer Datenquelle gehört der Instanz.
admin-hosting-hosted = Gehostet: für andere Leute
admin-hosting-hosted-hint = Strengere Voreinstellungen: Lizenz-Erinnerungen für Datenquellen und Schlüssel pro Person, wo eine Quelle sie braucht.
admin-hosting-note = Ändert Voreinstellungen (Registrierung, Lizenz-Erinnerungen, Schlüssel pro Person), nie Funktionen.
admin-public-url = Öffentliche Adresse
admin-public-url-hint = Gesetzt mit { $var }. Wird für Links, Einladungen und die Weiterleitung bei der Anmeldung genutzt.
admin-mail-title = E-Mail
admin-mail-summary = SMTP für Passwort-Zurücksetzen und E-Mail-Benachrichtigungen
admin-mail-lead = Nötig für Passwort-Zurücksetzen und E-Mail-Benachrichtigungen. Sonst hängt nichts davon ab.
admin-mail-password = Passwort
admin-mail-password-hint = Verschlüsselt gespeichert, nie wieder angezeigt.
admin-mail-password-stored = Ein Passwort ist gespeichert.
admin-mail-send-test = Speichern und Test senden
admin-mail-send-test-hint = Sendet eine Nachricht an deine eigene Adresse.
admin-mail-test-ok = Die Testnachricht wurde gesendet.
admin-mail-test-failed = Der Test ist fehlgeschlagen: { $reason }
admin-mail-last-ok = Letzter Test { $at }: hat geklappt.
admin-mail-last-failed = Letzter Test { $at }: { $reason }
admin-mail-reason-not_configured = E-Mail ist nicht eingerichtet.
admin-mail-reason-connection = Der Mailserver war nicht erreichbar. Prüfe Server, Port und Verbindungssicherheit.
admin-mail-reason-auth = Der Mailserver hat Benutzername oder Passwort abgelehnt.
admin-mail-reason-rejected = Der Mailserver hat die Nachricht abgelehnt.
admin-mail-reason-send_failed = Die Nachricht konnte nicht gesendet werden.
admin-mail-reason-private_target = Die Adresse des Mailservers liegt in einem privaten Netz, und diese Instanz verbindet sich nicht dorthin.
admin-mail-security-tls = TLS
admin-mail-security-starttls = STARTTLS
admin-mail-security-none = Keine (nur lokales Relay)
admin-setting-mail_host = Server
admin-setting-mail_port = Port
admin-setting-mail_security = Verbindungssicherheit
admin-setting-mail_username = Benutzername
admin-setting-mail_from = Absenderadresse
admin-setting-auth_sign_in_mode = Anmeldemodus
admin-secrets-summary = Der Schlüssel, der gespeicherte Geheimnisse schützt
admin-advanced-title = Erweitert
admin-advanced-summary = Sitzungsdauer, Cookie-Sicherheit, Bestätigungsfenster
admin-replace = Ersetzen
admin-health-mail-tested = Testnachricht gesendet { $at }
admin-health-mail-test_failed = Der letzte Test ist fehlgeschlagen: { $result }
admin-health-identity_provider-discovered = Discovery zwischengespeichert seit { $at }
admin-health-identity_provider-test_failed = Der letzte Test von { $provider } ist fehlgeschlagen
admin-health-identity_provider-unavailable = { $provider } hat bei der letzten Anmeldung nicht geantwortet
admin-users-via-group = über Gruppe
admin-users-via-group-hint = Von { $provider } bei jeder Anmeldung gesetzt
admin-sources-enabled = Diese Quelle nutzen
admin-sources-enabled-hint = Aus: Niemand kann sie durchsuchen und ihre Einträge werden nicht aktualisiert. Gilt sofort.

# Admin > Anmeldung und SSO (addendum-1 3.4, 4.2-4.5).
admin-sso-lead = Wie Leute hereinkommen. Die Passwort-Anmeldung steht Admins unter /sign-in/local immer zur Verfügung, damit ein kaputter Anbieter niemanden aussperrt.
admin-sso-providers = Identitätsanbieter
admin-sso-provider = Anbieter
admin-sso-issuer = Aussteller
admin-sso-status = Status
admin-sso-actions = Aktionen
admin-sso-details = Details zu { $name }
admin-sso-default = Standard
admin-sso-none = Noch kein Identitätsanbieter. Füge unten einen hinzu; die Passwort-Anmeldung bleibt.
admin-sso-state-off = Aus
admin-sso-state-ok = Funktioniert
admin-sso-state-failed = Test fehlgeschlagen
admin-sso-state-untested = Nicht getestet
admin-sso-display-name = Anzeigename
admin-sso-display-name-hint = Steht auf dem Anmeldeknopf: "Weiter mit ...".
admin-sso-slug = Kurzname
admin-sso-slug-hint = a-z, 0-9 und -, für Adressen. Nach dem Speichern fest.
admin-sso-issuer-url = Aussteller-URL
admin-sso-issuer-hint = Die Adresse, die der Anbieter als Aussteller nennt, etwa https://auth.example.org/application/o/shelf/.
admin-sso-client-id = Client-ID
admin-sso-client-secret = Client-Geheimnis
admin-sso-client-secret-hint = Verschlüsselt gespeichert, nie wieder angezeigt.
admin-sso-secret-stored = Ein Client-Geheimnis ist gespeichert.
admin-sso-secret-locked = Ein Client-Geheimnis ist gespeichert, aber sein Schlüssel ist nicht geladen.
admin-sso-save-secret = Geheimnis speichern
admin-sso-scopes = Scopes
admin-sso-scopes-hint = Durch Leerzeichen getrennt; openid kommt immer dazu.
admin-sso-enabled = Aktiv
admin-sso-is-default = Standardanbieter (den "Direkt weiter" nutzt)
admin-sso-rules = Kontoregeln
admin-sso-auto-create = Bei der ersten Anmeldung ein Konto anlegen, auch wenn die Registrierung nur mit Einladung geht
admin-sso-groups-claim = Gruppen-Claim
admin-sso-groups-claim-hint = Der Claim mit den Gruppen; die meisten Anbieter nennen ihn groups.
admin-sso-allowed-groups = Nur Mitglieder dieser Gruppen dürfen sich anmelden
admin-sso-allowed-groups-hint = Durch Kommas getrennt; leer heißt alle.
admin-sso-admin-groups = Mitglieder dieser Gruppen sind hier Admins
admin-sso-admin-groups-hint = Durch Kommas getrennt.
admin-sso-provider-roles = Rollen aus den Admin-Gruppen übernehmen (bei jeder Anmeldung geprüft)
admin-sso-rules-hint = Du bleibst angemeldet; eine Regel, die dir selbst die Admin-Rolle nehmen würde, wird abgelehnt.
admin-sso-delete = Anbieter entfernen
admin-sso-delete-title = { $name } entfernen?
admin-sso-delete-body = Wer nur mit diesem Anbieter verknüpft ist, kann sich erst wieder anmelden, wenn es einen anderen Weg gibt. Konten und Verlauf bleiben.
admin-sso-test-title = Test
admin-sso-test = Testen
admin-sso-test-ok = Test bestanden.
admin-sso-test-failed = Test fehlgeschlagen.
admin-sso-test-at = ({ $at })
admin-sso-test-discovery = Discovery
admin-sso-test-discovery-ok = funktioniert
admin-sso-test-discovery-failed = fehlgeschlagen: { $reason }
admin-sso-reason-oidc_unavailable = der Anbieter hat nicht geantwortet
admin-sso-reason-oidc_failed = die Antwort des Anbieters war nicht verwendbar; prüfe die Aussteller-Adresse
admin-sso-reason-oidc_blocked_target = seine Adresse liegt in einem privaten Netz, und diese Instanz verbindet sich nicht dorthin
admin-sso-test-end-session = Abmelden beim Anbieter
admin-sso-test-backchannel = Back-Channel-Logout
admin-sso-test-frontchannel = Front-Channel-Logout
admin-sso-test-pkce = PKCE (S256)
admin-sso-test-iss = Aussteller in der Antwort (RFC 9207)
admin-sso-test-groups = Gruppen-Claim
admin-sso-test-groups-seen = bei deiner letzten Anmeldung darüber gesehen
admin-sso-test-groups-missing = fehlte bei deiner letzten Anmeldung darüber
admin-sso-test-groups-unknown = du hast dich noch nicht darüber angemeldet
admin-sso-test-silent = Stille Anmeldung
admin-sso-test-max-age = Fragt bei einer Bestätigung neu (max_age=0)
admin-sso-offered = angeboten
admin-sso-not-offered = nicht angeboten
admin-sso-offered-unused = angeboten, nicht genutzt
admin-sso-supported = ja
admin-sso-not-advertised = nicht angegeben
admin-sso-probe-silent = Stille Anmeldung prüfen
admin-sso-probe-max-age = Bestätigung über den Anbieter prüfen
admin-sso-probe-hint = Beide Prüfungen schicken deinen Browser zum Anbieter und hierher zurück. Direkt zum Anbieter nutzt die stille Anmeldung erst, wenn sie geklappt hat.
admin-sso-probe-not-run = noch nicht geprüft
admin-sso-probe-silent_ok = funktioniert: Wer dort angemeldet ist, kommt ohne Formular zurück
admin-sso-probe-silent_login_required = funktioniert, wenn man dort angemeldet ist; diesmal wollte er eine Anmeldung
admin-sso-probe-silent_unsupported = nicht unterstützt; der Anbieter zeigt sein eigenes Formular
admin-sso-probe-max_age_ok = ja, der Anbieter hat dich neu anmelden lassen
admin-sso-probe-max_age_ignored = nein, der Anbieter ignoriert es; Admins ohne Passwort können damit nicht bestätigen
admin-sso-probe-max_age_error = der Anbieter hat mit einem Fehler geantwortet
admin-sso-probe-error = Die Prüfung wurde nicht fertig ({ $code }).
admin-sso-add = Anbieter hinzufügen
admin-sso-add-summary = Aussteller-URL, Client-ID, Client-Geheimnis (nur schreibbar), dann prüft Testen die Discovery.
admin-sso-add-submit = Anbieter hinzufügen
admin-sso-register = Diese Adressen beim Anbieter eintragen
admin-sso-register-lead = Dieselben drei für jeden Anbieter.
admin-sso-redirect-uri = Weiterleitungs-URI
admin-sso-logout-uri = URI nach dem Abmelden
admin-sso-backchannel-uri = Back-Channel-Logout-URI
admin-sso-mode = Wenn jemand die App abgemeldet öffnet
admin-sso-mode-page = Die Anmeldeseite zeigen
admin-sso-mode-page-hint = Zuerst die Anbieter-Knöpfe, darunter das Passwort-Formular.
admin-sso-mode-auto = Direkt weiter zu { $provider }
admin-sso-mode-auto-hint = Wer dort schon angemeldet ist, sieht unser Formular nie. Mitglieder können keine Passwörter nutzen; Admins weiterhin unter /sign-in/local.
admin-sso-mode-needs = Direkt zum Anbieter braucht einen Standardanbieter, dessen Test in den letzten 24 Stunden bestanden wurde, und einen Admin mit Passwort und Authenticator-App.
admin-sso-front-channel = Front-Channel-Logout wird nicht genutzt (es hängt an Drittanbieter-Cookies, die Safari und Firefox blockieren oder trennen).
admin-error-auto_needs_tested_provider = Teste zuerst den Standardanbieter: Direkt dorthin braucht einen Test, der in den letzten 24 Stunden bestanden wurde.
admin-error-auto_needs_admin_authenticator = Gib zuerst mindestens einem Admin ein Passwort und eine Authenticator-App, damit niemand ausgesperrt wird.
admin-error-last_sign_in_method = Das ist der einzige verbliebene Weg zur Anmeldung. Setz zuerst ein Passwort oder verknüpfe einen anderen Anbieter.
admin-error-invalid_slug = Nimm 1 bis 32 Zeichen a-z, 0-9 und -, beginnend mit einem Buchstaben oder einer Ziffer.
admin-error-slug_taken = Ein anderer Anbieter nutzt diesen Kurznamen.
admin-error-invalid_issuer = Das ist keine Aussteller-Adresse.
admin-error-insecure_issuer = Nimm https (einfaches http nur für einen Anbieter auf diesem Rechner oder solange diese Instanz über http läuft).
admin-error-admin_groups_required = Rollen vom Anbieter brauchen mindestens eine Admin-Gruppe.
admin-error-self_demotion = Diese Regel würde dir selbst die Admin-Rolle nehmen.
admin-error-role_managed_by_provider = Diese Rolle kommt aus der Gruppenregel eines Anbieters. Ändere sie beim Anbieter.
admin-error-identity_in_use = Diese Anmeldung ist schon mit einem anderen Konto verknüpft.
admin-error-no_linked_provider = Dieses Konto hat keine Anmeldung bei einem Anbieter.
admin-error-invalid_time_zone = Das ist kein Zeitzonen-Name. Nimm einen wie Europe/Berlin.

# Einstellungen > Konto: verknüpfte Anmeldungen und die Zeitzone (DEC-306, DEC-312).
settings-linked = Verknüpfte Anmeldungen
settings-linked-lead = Anbieter, die dich bei diesem Konto anmelden können.
settings-linked-none = Kein Anbieter verknüpft.
settings-linked-since = Verknüpft { $date }
settings-linked-only = Das ist dein einziger Weg zur Anmeldung.
settings-unlink = Lösen
settings-unlink-named = { $provider } lösen
settings-unlinked = Gelöst.
settings-link-provider = { $provider } verknüpfen
settings-tz = Zeitzone
settings-tz-current = Zeiten werden in { $zone } angezeigt.
settings-tz-none = Zeiten werden in UTC angezeigt: noch keine Zeitzone gewählt.
settings-tz-use-browser = { $zone } nutzen (dieser Browser)
settings-tz-field = Name der Zeitzone
settings-tz-hint = Ein IANA-Name wie Europe/Berlin. Start, Verlauf und Statistik nutzen ihn.
settings-tz-save = Zeitzone speichern
settings-tz-reset = Zurück zu UTC
admin-mail-to = Test senden an
admin-mail-to-hint = Leer: deine eigene Adresse.
admin-error-no_recipient = Dein Konto hat keine E-Mail-Adresse. Gib eine Adresse für den Test ein.

# ---- UX phase 3, WP3-5: Settings > Notifications and Integrations (design.md 6.8, 6.9) ----
notify-lead = Was dich erreicht und wo. Der Posteingang ist immer an; Kanäle bringen E-Mail, Push oder Chat dazu.
notify-tabs = Bereiche der Benachrichtigungen
notify-rules = Regeln
notify-unread = Ungelesen:
notify-mark-all-read = Alle als gelesen markieren
notify-inbox-empty-body = Hier erscheinen neue Folgen, Freigaben und Import-Ergebnisse.
notify-log-when = Wann
notify-log-state = Status
notify-log-show-error = Fehler anzeigen
notify-channel-gone = Gelöschter Kanal
notify-health-never = Noch nie benutzt
notify-health-working = Funktioniert, letzte Zustellung { $when }
notify-health-waiting = Wartet auf den Versand
notify-health-failing = Letzte Zustellung fehlgeschlagen { $when }
notify-live-failing = { $channel }: Die letzte Zustellung ist fehlgeschlagen
notify-live-working = { $channel }: Die letzte Zustellung kam an
notify-failing-title = Die letzte Zustellung kam nicht an
notify-failing-hint = Benachrichtigungen landen weiter im Posteingang. Prüfe die Einstellungen des Kanals und sende dann einen Test.
notify-test-delivered = Test zugestellt.
notify-test-failed = Der Test wurde nicht zugestellt
notify-secret-label = Signaturgeheimnis
notify-secret-once = Wird nur einmal angezeigt. Kopiere es jetzt in den empfangenden Dienst.
notify-delete-title = { $name } löschen?
notify-delete-body = An diesen Kanal wird nichts mehr gesendet. Dein Posteingang und die Regeln für andere Kanäle bleiben, wie sie sind.
notify-hint-name = Nur du siehst ihn.
notify-hint-url = Beginnt mit https:// (oder http:// im eigenen Netz).
notify-hint-topic = Nur Buchstaben, Ziffern, - und _, keine Leerzeichen. Zum Beispiel "mein-topic".
notify-hint-discord = Die Webhook-URL aus den Kanaleinstellungen in Discord.
notify-hint-room = Sieht aus wie !abc123:example.org.
notify-hint-webhook = Wir signieren jeden Aufruf (Standard Webhooks), damit der Empfänger prüfen kann, dass er von uns kommt.
notify-hint-secret = Wird verschlüsselt gespeichert und nie wieder angezeigt.
notify-check-required = Bitte ausfüllen.
notify-check-url = Gib eine Adresse ein, die mit https:// oder http:// beginnt.
notify-check-email = Gib eine E-Mail-Adresse mit @ ein.
notify-check-topic = Topics dürfen keine Leerzeichen oder Sonderzeichen enthalten. Zum Beispiel "mein-topic".
notify-add-and-test = Hinzufügen und Test senden
notify-rules-lead = Instanz-Standards sind markiert; gesperrte Regeln lassen sich nicht ändern. Alles landet außerdem im Posteingang.
notify-rules-no-channels = Lege zuerst einen Kanal an. Bis dahin landet alles nur im Posteingang.
notify-when = Wann
notify-when-quiet = { $mode }, Ruhezeit { $from }-{ $to }
notify-when-inbox-only = Nur Posteingang
notify-inbox-only = Nur Posteingang
notify-inbox-always = Immer an
notify-on = An
notify-off = Aus
notify-locked = Gesperrt
notify-switch-named = { $event } an { $channel }
notify-offer-title = { $channel } ist neu
notify-offer-body = { $count ->
    [one] Die Instanz schlägt ein weiteres Ereignis vor: { $events }. Auch für { $channel } einschalten?
   *[other] Die Instanz schlägt { $count } weitere Ereignisse vor: { $events }. Auch für { $channel } einschalten?
}
notify-offer-yes = Vorschläge übernehmen
notify-offer-no = Nur dieses
notify-section-release = Veröffentlichungen
notify-section-library = Bibliothek
notify-section-social = Personen und Teilen
notify-section-boardgame = Brettspiele
notify-section-integration = Absender
notify-section-import = Importe
notify-section-admin = Instanz (Admin)
notify-section-patterns = Mehrere Ereignisse auf einmal
notify-filtered = Regeln für eine Liste, einen Titel oder eine Person
notify-filtered-empty = Noch keine.
notify-add-filtered = Regel für eine Liste, einen Titel oder eine Person hinzufügen
notify-filter-for = Für
notify-filter-pick = Liste oder Person wählen
notify-filter-list = Liste: { $name }
notify-filter-person = Person: { $name }
notify-filter-items = { $count ->
    [one] ein Titel
   *[other] { $count } Titel
}
notify-filter-items-later = Einen einzelnen Titel wählst du unten mit der Suche.
notify-filter-required = Wähle eine Liste, eine Person oder einen Titel.
notify-add-rule = Regel hinzufügen
integrations-senders = Absender
integrations-connect = Absender verbinden
integrations-connect-lead = 3 Schritte, etwa 5 Minuten.
integrations-steps = Absender verbinden, Fortschritt
integrations-step-name = Benennen
integrations-step-copy = Token kopieren
integrations-step-wait = Auf den ersten Aufruf warten
integrations-step-done = erledigt
integrations-name-hint = Nur du siehst ihn, z. B. "Medienserver - Wohnzimmer".
integrations-name-invalid = Der Name braucht 1 bis 100 Zeichen.
integrations-create = Token erstellen
integrations-cancel = Abbrechen
integrations-done = Fertig
integrations-token-for = Token für "{ $name }"
integrations-token-once = Wird nur einmal angezeigt. Wenn du es verlierst, lege einen neuen Absender an.
integrations-help-title = Einen Medienserver mit Webhook-Plugin einrichten
integrations-help-1 = Lege im Webhook-Plugin des Servers ein Ziel mit der URL und dem Header Authorization: Bearer gefolgt vom Token an.
integrations-help-2 = Lass es senden, wenn die Wiedergabe stoppt oder ein Titel als gesehen markiert wird.
integrations-help-3 = Nutze eine Vorlage für den Body, die dieses JSON erzeugt (fülle die Felder mit den Variablen des Plugins):
integrations-shape = Body-Form für eine Folge
integrations-waiting = Warte auf den ersten Aufruf...
integrations-waiting-body = Spiel etwas ab und stoppe es. Der erste angenommene Aufruf erscheint hier nach wenigen Sekunden. Wird er abgelehnt, steht hier der Grund.
integrations-quiet = Noch kein Aufruf
integrations-quiet-body = Prüfe Token und URL im Absender. Ein Aufruf mit falschem oder fehlendem Token wird abgelehnt, bevor wir wissen, welcher Absender es ist, deshalb kann er hier nicht erscheinen.
integrations-accepted = Erster Aufruf angekommen
integrations-accepted-body = Er hat { $title } festgehalten. Dieser Absender funktioniert.
integrations-wait-refused = Der Aufruf kam an, wurde aber abgelehnt.
integrations-wait-refused-field = Der Aufruf kam an, wurde aber wegen des Felds { $field } abgelehnt.
integrations-send-test = Testaufruf von hier senden
integrations-test-ok = Testaufruf angenommen (nichts wurde festgehalten)
integrations-test-matched = Er würde { $title } festhalten, das schon im Katalog ist.
integrations-test-new = Er würde { $title } in den Katalog aufnehmen und festhalten.
integrations-test-refused = Der Testaufruf wurde abgelehnt
integrations-never-called = Noch nie aufgerufen
integrations-working = Funktioniert
integrations-refused = Letzter Aufruf abgelehnt
integrations-refused-title = Ein Aufruf wurde abgelehnt
integrations-last-recorded = Letzter Aufruf: { $when } - { $title } festgehalten
integrations-last-call-status = Letzter Aufruf: { $when } - Antwort { $status }
integrations-earlier-recorded = Zuletzt davor festgehalten: { $title }
integrations-created-on = Watched-Webhook - angelegt { $when }
integrations-menu-named = Aktionen für { $name }
integrations-revoke-title = { $name } widerrufen?
integrations-revoke-body = Sein Token funktioniert sofort nicht mehr und seine Aufrufe werden abgelehnt. Was er festgehalten hat, bleibt in deinem Verlauf. Das lässt sich nicht rückgängig machen.
integrations-refusal = Der letzte Aufruf von "{ $name }" wurde abgelehnt.
integrations-refusal-field = Der letzte Aufruf von "{ $name }" wurde wegen des Felds { $field } abgelehnt. Felder werden streng geprüft, damit Tippfehler nicht still ignoriert werden.
integrations-refusal-expected = Erwartete Felder: { $fields }.
integrations-show-payload = Letzten Body anzeigen (Geheimnisse verborgen)
integrations-empty-body = Verbinde deinen Medienserver, und jede fertige Folge landet von selbst in deinem Verlauf.

## Personen (design.md 6.7, 5.11)
companions-title = Personen
companions-lead = Mit wem du schaust, liest und spielst. Nur du siehst diese Liste.
companions-add-open = Person hinzufügen
companions-new = Name
companions-note = Notiz (optional)
companions-note-hint = Nur du siehst sie.
companions-add = Hinzufügen
companions-added = { $name } hinzugefügt.
companions-tabs = Personen
companions-tab-all = Alle { $count }
companions-tab-linked = Verknüpfte Konten { $count }
companions-empty = Noch keine Personen. Füge jemanden hinzu, wenn ihr zum ersten Mal zusammen spielt oder schaut; dann erscheint die Person hier.
companions-empty-linked = Noch ist niemand mit einem Konto verknüpft. Lade jemanden ein oder bitte eine Person mit Konto zu bestätigen, dass sie es ist.
companions-linked = Verknüpft
companions-linked-to = Verknüpft mit @{ $username }
companions-requested = Anfrage gesendet
companions-no-account = Kein Konto
companions-together = Zusammen
companions-plays = Partien
companions-win-rate = Siegquote
companions-percent = { $percent } %
companions-no-rate = Noch kein Sieg erfasst
companions-last = Zuletzt: { $title }, { $when }
companions-most-played = Am meisten gespielt: { $title }, { $plays ->
    [one] eine Partie
   *[other] { $plays } Partien
}
companions-history = Verlauf
companions-history-named = Verlauf mit { $name }
companions-menu-named = Weitere Aktionen für { $name }
companions-invite = Einladen
companions-invite-named = { $name } einladen
companions-link = Mit Konto verknüpfen
companions-unlink = Verknüpfung lösen
companions-merge = Zusammenführen mit...
companions-edit = Bearbeiten
companions-delete = Löschen
companions-requests = Anfragen an dich
companions-request-text = { $from } sagt, der Eintrag "{ $name }" bist du. Wenn du annimmst, sieht { $from } die Partien und Filme, die du teilst, und du siehst die von { $from } mit dir.
companions-accept = Annehmen
companions-decline = Ablehnen
companions-accept-named = Anfrage von { $from } annehmen
companions-decline-named = Anfrage von { $from } ablehnen
companions-accepted = Du bist jetzt mit { $from } verknüpft.
companions-declined = Anfrage abgelehnt.
companions-suggestions = Mögliche Doppelte
companions-suggest = "{ $keep }" und "{ $remove }" scheinen dieselbe Person zu sein.
companions-suggest-merge = { $keep } und { $remove } zusammenführen
companions-invite-title = { $name } einladen
companions-invite-consequence = { $name } bekommt ein Konto auf dieser Instanz, verknüpft mit diesem Eintrag. { $name } sieht { $count ->
    [one] die Sitzung
   *[other] die { $count } Sitzungen
}, die du mit { $name } erfasst hast, und kann sie in den eigenen Verlauf kopieren. In deinem Verlauf ändert sich nichts.
companions-invite-making = Link wird erstellt...
companions-invite-link = Einmaliger Link, 14 Tage gültig
companions-invite-send = Schick ihn auf beliebigem Weg. Hat { $name } hier schon ein Konto, nimm stattdessen "Mit Konto verknüpfen".
companions-link-title = { $name } mit einem Konto verknüpfen
companions-link-lead = Hat die Person hier schon ein Konto, bitte sie zu bestätigen, dass sie es ist. Sie sieht eine Anfrage auf ihrer Seite Personen; nimmt sie an, sieht sie die Sitzungen, die du mit ihr teilst.
companions-link-username = Benutzername
companions-link-submit = Anfrage senden
companions-link-sent = Anfrage gesendet.
companions-unlink-title = Verknüpfung mit { $name } lösen?
companions-unlink-body = { $name } sieht die Sitzungen nicht mehr, die du mit { $name } erfasst, und du siehst die von { $name } nicht mehr. Dein Verlauf bleibt, wie er ist.
companions-unlinked = { $name } ist nicht mehr verknüpft.
companions-merge-title = Doppelte zusammenführen
companions-merge-keep = Behalten
companions-merge-remove = Aufgehen lassen
companions-choose = Auswählen...
companions-merge-fold-count = { $count ->
    [one] eine Sitzung
   *[other] { $count } Sitzungen
}
companions-merge-swap = Tauschen
companions-merge-consequence = Die Sitzungen von { $remove } erscheinen unter { $keep }. Nichts wird gelöscht; auf der Seite von { $keep } lässt sich das rückgängig machen.
companions-merge-submit = Zusammenführen
companions-merged = { $remove } in { $keep } zusammengeführt.
companions-merge-undo = Zusammenführen rückgängig
companions-merge-undo-named = Zusammenführen rückgängig: { $name }
companions-unmerged = { $name } ist wieder ein eigener Eintrag.
companions-edit-title = { $name } bearbeiten
companions-save = Speichern
companions-saved = Gespeichert.
companions-delete-title = { $name } löschen?
companions-delete-history = { $name } verschwindet aus Personen und dem Erfassen-Fenster. Dein Verlauf behält { $count ->
    [one] die Sitzung
   *[other] die { $count } Sitzungen
} mit { $name }.
companions-delete-none = { $name } wird gelöscht.
companions-delete-link = Die Verknüpfung mit @{ $username } endet.
companions-deleted = { $name } wurde gelöscht.
companions-person-back = Alle Personen
companions-person-history = Verlauf mit { $name }
companions-person-all-history = Ganzer Verlauf mit { $name }
companions-person-no-history = Mit { $name } ist noch nichts erfasst.
companions-person-lists = Listen von { $name }
companions-person-no-lists = { $name } teilt keine Liste mit allen hier.
companions-person-lists-unlinked = Listen erscheinen hier, sobald { $name } ein Konto hat, das mit diesem Eintrag verknüpft ist.
companions-person-merged = In diesen Eintrag zusammengeführt
companions-person-merged-row = { $name }, zusammengeführt am { $when }

## Listen (design.md 6.5, 5.10)
lists-title = Listen
lists-lead = Geordnete Sammlungen, die du pflegst. Teile eine per Link oder mit allen auf dieser Instanz.
lists-new-open = Neue Liste
lists-new = Name der Liste
lists-create = Liste anlegen
lists-tabs = Listen
lists-tab-mine = Meine { $count }
lists-tab-shared = Mit mir geteilt { $count }
lists-tab-instance = Auf dieser Instanz { $count }
lists-empty = Du hast noch keine Listen.
lists-empty-shared = Noch nichts mit dir geteilt. Listen, die mit dir verknüpfte Personen mit allen hier teilen, erscheinen hier.
lists-empty-instance = Hier teilt noch niemand eine Liste mit allen.
lists-items = { $count ->
    [one] Ein Eintrag
   *[other] { $count } Einträge
}
lists-items-kinds = { $items } - { $kinds }
lists-updated = Geändert { $when }
lists-vis-private = Privat
lists-vis-instance = Alle hier
lists-vis-link = Per Link geteilt
lists-back = Alle Listen
lists-edit = Titel und Beschreibung bearbeiten
lists-title-label = Titel
lists-description = Beschreibung
lists-save = Speichern
lists-saved = Gespeichert.
lists-delete = Liste löschen
lists-delete-title = "{ $name }" löschen?
lists-delete-body = Die Liste und ihr Link verschwinden. Die Einträge bleiben in deiner Bibliothek.
lists-add = Hinzufügen
lists-add-named = Zu { $name } hinzufügen
lists-add-target = Ein gewähltes Ergebnis kommt in "{ $name }".
lists-added = { $title } zu { $name } hinzugefügt.
lists-empty-items = Diese Liste ist leer. Füge mit "Hinzufügen" etwas hinzu.
lists-in-library = In deiner Bibliothek
lists-move-up = Nach oben
lists-move-down = Nach unten
lists-move-up-named = Nach oben: { $title }
lists-move-down-named = Nach unten: { $title }
lists-moved = { $title } ist jetzt Nummer { $position } von { $count }.
lists-remove = Entfernen
lists-remove-named = { $title } aus der Liste entfernen
lists-removed = { $title } entfernt.
lists-add-all = Alle in meine Bibliothek
lists-added-all = { $added } in deine Bibliothek aufgenommen; { $already } waren schon da.
lists-share = Teilen
lists-share-named = { $name } teilen
lists-share-title = "{ $name }" teilen
lists-share-legend = Wer diese Liste sehen kann
lists-share-private = Privat
lists-share-private-hint = Nur du.
lists-share-instance = Alle auf dieser Instanz
lists-share-instance-hint = Angemeldete Mitglieder finden sie unter Listen.
lists-share-link = Alle mit dem Link
lists-share-link-hint = Kein Konto nötig. Dein Name und die Einträge sind sichtbar, deine Bewertungen und Notizen nicht.
lists-share-the-link = Öffentlicher Link
lists-share-new-link = Neuen Link erstellen
lists-share-new-hint = Ein neuer Link beendet den alten.
lists-share-save = Speichern
lists-share-done-private = Gespeichert: nur du siehst diese Liste.
lists-share-done-instance = Gespeichert: alle auf dieser Instanz sehen diese Liste.
lists-share-done-link = Gespeichert: alle mit dem Link sehen diese Liste. Kopiere den Link unten.
lists-share-kept = Gespeichert: der Link bleibt gleich.
lists-share-replaced = Ein neuer Link ersetzt den alten; der alte Link funktioniert nicht mehr.
lists-share-revoked = Gespeichert: der Link funktioniert nicht mehr.

## Öffentliche Seite einer geteilten Liste
shared-list-missing = Diese Liste ist nicht mehr geteilt
shared-list-missing-lead = Der Link wurde geändert oder die Liste ist jetzt privat. Bitte die Person, die ihn geschickt hat, um einen neuen.
shared-list-cta = Melde dich an, um diese in deine Bibliothek aufzunehmen
shared-list-made-with = Erstellt mit { $product }

## Einstellungen > Importe und Export (WP3-7, design.md 6.11)
settings-nav-imports = Importe und Export
imports-title = Importe und Export
imports-lead = Hol deine Bibliothek aus einer anderen App oder Datei, sieh vor dem Import, was eine Datei enthält, und lade alles herunter, was du erfasst hast.
imports-choose = Datei importieren
imports-latest = Deine Importe
imports-format-generic = CSV oder JSON Lines
imports-format-generic-body = Das eigene Format dieser App, auch das, was der Export schreibt. Eine Datei auf .csv wird als CSV gelesen, jede andere als JSON Lines.
imports-format-legacy = Export des bisherigen Trackers
imports-format-legacy-body = Der JSON-Export des Trackers, den du vorher benutzt hast: Arten, IDs, gesehene Folgen mit Datum, Status und Sammlungen als Listen.
imports-format-trakt = Trakt
imports-format-trakt-body = Dein eigener Trakt-Verlauf, deine Bewertungen oder deine Merkliste als JSON, eine Datei nach der anderen. Von Trakt wird nichts abgerufen.
imports-format-goodreads = Goodreads
imports-format-goodreads-body = Der Bibliotheks-Export von Goodreads (CSV). Er kommt dazu, sobald ein Beispiel-Export vorliegt.
imports-format-bgstats = BG Stats
imports-format-bgstats-body = Die BG-Stats-Sicherung mit deinen Brettspiel-Partien. Sie kommt dazu, sobald ein Beispiel-Export vorliegt.
imports-format-bgg = BoardGameGeek
imports-format-bgg-body = Deine BoardGameGeek-Sammlung und Partien. Sie kommen dazu, sobald ein Beispiel-Export vorliegt.
imports-waiting = Wartet auf ein Beispiel
imports-pick = Datei auswählen
imports-drop = Oder eine Datei hierher ziehen. Sie wird zuerst geprüft; importiert wird erst, wenn du es sagst.
imports-trakt-credit = { $product } nutzt Trakt, wird aber von Trakt weder unterstützt noch zertifiziert.
imports-trakt-link = trakt.tv
imports-none = Noch keine Importe
imports-none-body = Wähle oben eine Datei. Du siehst, was sie enthält, bevor etwas importiert wird.
imports-earlier = Frühere Importe
imports-earlier-row = { $format }, { $when }: { $state }
imports-state-queued = Wartet auf den Start
imports-state-running = Wird importiert
imports-state-done = Import abgeschlossen
imports-state-failed = Import fehlgeschlagen
imports-state-cancelled = Import angehalten
imports-state-previewed = Geprüft, nicht importiert
imports-reading = Datei wird gelesen ...
imports-progress = { $done } von { $total }
imports-progress-label = Fortschritt des Imports
imports-panel-title = { $format }, { $when }
imports-checking = Datei wird geprüft
imports-running = Wird importiert
imports-cancel = Abbrechen
imports-cancelling = Wird angehalten ...
imports-preview = Was diese Datei enthält
imports-preview-summary = { $rows ->
    [one] Eine Zeile
   *[other] { $rows } Zeilen
} mit { $events ->
    [one] einem Verlaufseintrag
   *[other] { $events } Verlaufseinträgen
}. { $unmatched ->
    [0] Jede Zeile hat eine ID zum Zuordnen.
    [one] Eine Zeile hat keine ID zum Zuordnen und wartet auf dich.
   *[other] { $unmatched } Zeilen haben keine ID zum Zuordnen und warten auf dich.
} { $errors ->
    [0] Keine Fehler.
    [one] Eine Zeile ist nicht lesbar.
   *[other] { $errors } Zeilen sind nicht lesbar.
}
imports-run = { $count ->
    [one] 1 Zeile importieren
   *[other] { $count } Zeilen importieren
}
imports-kinds = Zeilen pro Art
imports-kind = Art
imports-rows = Zeilen
imports-lists = Listen: { $lists }
imports-samples = Beispielzeilen
imports-sample = { $kind }, { $status }, { $events ->
    [one] 1 Verlaufseintrag
   *[other] { $events } Verlaufseinträge
}
imports-no-status = kein Status
imports-no-match = Noch nicht zugeordnet
imports-mapping = Was wohin kommt
imports-from = In der Datei
imports-to = Hier
imports-target-kind = Art
imports-target-external_ids = IDs zum Finden des Titels
imports-target-title = Titel
imports-target-history = Verlauf (gesehen, gelesen, gespielt)
imports-target-status = Status
imports-target-rating = Bewertung
imports-target-notes = Notiz
imports-target-lists = Listen
imports-target-same = Dieselben Felder
imports-lost = Was nicht übernommen wird
imports-lost-nothing = Nichts: jedes Feld wird übernommen.
imports-lost-people = { $count } Personen (nicht importiert)
imports-lost-workouts = { $count } Trainings (nicht importiert)
imports-lost-exercises = { $count } Übungen (nicht importiert)
imports-lost-measurements = { $count } Messungen (nicht importiert)
imports-lost-metadata_groups = { $count } Gruppen (nicht importiert)
imports-lost-workout_templates = { $count } Trainingsvorlagen (nicht importiert)
imports-lost-reviews = { $count } Rezensionen (nicht importiert)
imports-lost-skipped_collections = { $count } Einträge in den Sammlungen Reminders und Custom (nicht importiert)
imports-lost-watched_on = Wo du es gesehen hast, bei { $count } Einträgen
imports-lost-partial_progress = { $count } begonnene oder teilweise beendete Einträge: Prozentsatz und Startdatum (der Status bleibt)
imports-lost-views_without_episode = { $count } Ansichten ohne Folgennummer
imports-lost-finish_dates = Das Enddatum von { $count } Büchern oder Spielen (der Status bleibt)
imports-lost-undated_history = { $count } Ansichten ohne Datum
imports-lost-estimated_dates = { $count } Ansichten ohne Datum: erfasst am Tag, an dem der Titel in Completed oder In Progress kam (geschätzt)
imports-lost-start_dates = Das Startdatum von { $count } beendeten Ansichten
imports-lost-action = Wie { $count } Ansichten erfasst wurden (Scrobble, Check-in, Watch)
imports-lost-rated_at = Wann { $count } Bewertungen abgegeben wurden
imports-lost-listed_at = Wann { $count } Titel auf die Merkliste kamen, und ihre Reihenfolge
imports-lost-season_episode_ratings = { $count } Bewertungen einzelner Staffeln oder Folgen
imports-errors = { $count ->
    [one] Eine Zeile wurde übersprungen
   *[other] { $count } Zeilen wurden übersprungen
}
imports-error-line = Zeile { $line }:
imports-warnings = { $count ->
    [one] In einer Zeile wird etwas ausgelassen
   *[other] In { $count } Zeilen wird etwas ausgelassen
}
imports-result = Importiert { $imported }, übersprungen { $skipped }, nicht zugeordnet { $unmatched }.
imports-completed-note = Ein Titel, den die Datei als abgeschlossen führt, gilt am Tag seines neuesten Verlaufseintrags als beendet. Ohne Verlauf hat er kein Datum und zählt nur in den Summen, nicht in einem Monat, Jahr oder Ziel.
imports-stopped-at = Angehalten nach { $done } von { $total } Zeilen. Was schon importiert ist, bleibt.
imports-unmatched = Nicht zugeordnete Zeilen
imports-unmatched-body = Diese Zeilen haben keine ID, die diese App nachschlagen kann. Such jede heraus; sie wird importiert, sobald du den Titel wählst.
imports-again = Erneut versuchen
imports-find = Zuordnen
imports-find-named = { $title } zuordnen
imports-matched = Zugeordnet und importiert.
imports-export = Export
imports-export-lead = Alles in deiner Bibliothek, im eigenen Format dieser App. Beide Dateien lassen sich wieder importieren.
imports-export-jsonl = JSON Lines herunterladen
imports-export-jsonl-body = Jedes Feld und der ganze Verlauf, mit Folgen und Positionen.
imports-export-csv = CSV herunterladen
imports-export-csv-body = Eine Zeile pro Titel für Tabellen; vom Verlauf nur die Tage, an denen du etwas beendet hast.
admin-error-import_finished = Der Import ist schon abgeschlossen.
admin-error-import_dry_run = Das war nur eine Prüfung. Importiere die Datei zuerst.
admin-error-import_kind_mismatch = Dieser Titel ist von einer anderen Art als die Zeile.
admin-error-import_running = Der Import läuft noch. Ordne die Zeile zu, wenn er abgeschlossen ist.
admin-error-import_file_gone = Die Datei dieses Imports wird nicht mehr aufbewahrt. Lade sie erneut hoch.
admin-error-quota = Die Datei ist größer als dein Upload-Kontingent.
history-export = Export
episodes-mark-show = Serie als gesehen markieren
episodes-when = Datum beim Markieren mehrerer Folgen
episodes-when-now = Jetzt
episodes-when-range = Über einen Zeitraum verteilen
episodes-from = Von
episodes-to = Bis
episodes-range-hint = Staffel, Serie und bis hier markieren verteilen die Folgen gleichmäßig vom ersten bis zum letzten Tag.
episodes-range-hint-listen = Staffel, Podcast und bis hier markieren verteilen die Folgen gleichmäßig vom ersten bis zum letzten Tag.
episodes-range-missing = Wähle beide Tage des Zeitraums.
episodes-range-reversed = Der letzte Tag liegt vor dem ersten.
episodes-range-future = Der Zeitraum darf nicht nach heute enden.
episodes-when-release = Am Erscheinungstag
episodes-release-hint = Staffel, Serie und bis hier markieren tragen jede Folge an dem Tag ein, an dem sie erschienen ist.
episodes-release-hint-listen = Staffel, Podcast und bis hier markieren tragen jede Folge an dem Tag ein, an dem sie erschienen ist.
episodes-release-missing = { $count ->
    [one] „Am Erscheinungstag“ wird nicht angeboten: Eine offene Folge dieser Staffel hat bis heute kein Erscheinungsdatum.
   *[other] „Am Erscheinungstag“ wird nicht angeboten: { $count } offene Folgen dieser Staffel haben bis heute kein Erscheinungsdatum.
}
episodes-release-refused = { $count ->
    [one] Eine dieser Folgen hat bis heute kein Erscheinungsdatum, daher wurde nichts markiert.
   *[other] { $count } dieser Folgen haben bis heute kein Erscheinungsdatum, daher wurde nichts markiert.
}
imports-match-target = Ein gewählter Treffer wird der importierten Zeile „{ $title }“ zugeordnet und die Zeile importiert.

# WP4-2 admin-ops: Prüfzeilen, Menü der Personenliste, verknüpfte Konten, Link per E-Mail
admin-status-unknown = Noch nicht geprüft
admin-health-not-checked = Die Prüfung im Hintergrund lief noch nicht.
admin-health-stale = Länger nicht geprüft (zuletzt { $time })
admin-users-more = Weitere Aktionen für { $name }
admin-users-more-short = Mehr
admin-users-reset-totp = Authenticator zurücksetzen
admin-users-reset-totp-title = Authenticator von { $name } zurücksetzen?
admin-users-reset-totp-body = { $name } meldet sich dann nur mit dem Passwort an, bis eine neue Authenticator-App eingerichtet ist. Die Wiederherstellungscodes gelten nicht mehr, und { $name } wird benachrichtigt.
admin-users-unlink = { $provider } trennen
admin-users-unlink-title = { $provider } von { $name } trennen?
admin-users-unlink-body = { $name } kann sich nicht mehr mit { $provider } anmelden und wird benachrichtigt. Konto und Bibliothek bleiben.
admin-users-unlink-confirm = Trennen
admin-error-no_authenticator = Diese Person hat keinen Authenticator.
admin-sso-linked = Verknüpfte Konten
admin-audit-user_totp_reset = Authenticator zurückgesetzt
admin-audit-user_identity_unlinked = Anmeldung getrennt
admin-audit-auth_link_emailed = Anmeldelink gesendet
admin-audit-auth_link_confirmed = Anmeldung über einen gesendeten Link verknüpft
notify-event-account-authenticator_reset = Dein Authenticator wurde entfernt
notify-event-account-sign_in_unlinked = Eine Anmeldung wurde von deinem Konto getrennt
signin-link-email-lead = Dieses Konto hat kein Passwort. Wir können einen einmaligen Link an seine Adresse senden; öffne ihn in diesem Browser, um die Anmeldung zu verknüpfen.
signin-link-email = Link per E-Mail senden
signin-link-emailed = Wir haben einen Link an { $address } gesendet. Öffne ihn innerhalb von { $minutes } Minuten in diesem Browser.
signin-link-emailed-short = Wir haben einen Link gesendet. Öffne ihn in diesem Browser.
signin-link-email-failed = Der Link konnte nicht gesendet werden. Bitte deinen Admin, die Anmeldung zu verknüpfen.
signin-link-code-invalid = Dieser Link funktioniert nicht mehr: Er wurde schon benutzt, ist abgelaufen oder wurde in einem anderen Browser geöffnet. Melde dich erneut an, um neu zu beginnen.
signin-link-linking = Dein Konto wird verknüpft ...

## Mehrfachbearbeitung und Entfernen aus der Bibliothek (WP4-3)
library-select = { $title } auswählen
library-select-all = Alle auf dieser Seite auswählen
library-add-to-list = Zu einer Liste hinzufügen...
library-remove = Aus der Bibliothek entfernen
library-removed = { $title } aus deiner Bibliothek entfernt
library-bulk-label = Aktionen für die Auswahl
library-bulk-count = { $count } ausgewählt
library-bulk-status-menu = Status setzen
library-bulk-tag = Schlagwort hinzufügen
library-bulk-tag-title = Den ausgewählten Einträgen ein Schlagwort geben
library-bulk-list = Zu einer Liste
library-bulk-remove = Entfernen
library-bulk-clear = Auswahl aufheben
library-bulk-remove-title = { $count ->
    [one] Einen Eintrag aus deiner Bibliothek entfernen?
   *[other] { $count } Einträge aus deiner Bibliothek entfernen?
}
library-bulk-remove-body = Status, Bewertung, Notiz und Schlagwörter gehen verloren. Partien, Fortschritt, Rezensionen und Listen bleiben, und Rückgängig stellt alles wieder her.
library-bulk-remove-confirm = Entfernen
library-bulk-status = { $count ->
    [one] Ein Eintrag als { $status } markiert
   *[other] { $count } Einträge als { $status } markiert
}
library-bulk-tagged = { $count ->
    [one] Einem Eintrag das Schlagwort { $tag } gegeben
   *[other] { $count } Einträgen das Schlagwort { $tag } gegeben
}
library-bulk-listed = { $count ->
    [one] Ein Eintrag zu { $list } hinzugefügt
   *[other] { $count } Einträge zu { $list } hinzugefügt
}
library-bulk-removed = { $count ->
    [one] Ein Eintrag aus deiner Bibliothek entfernt
   *[other] { $count } Einträge aus deiner Bibliothek entfernt
}
library-bulk-failed = { $count ->
    [one] Ein Eintrag konnte nicht geändert werden.
   *[other] { $count } Einträge konnten nicht geändert werden.
}
library-list-pick-title = { $count ->
    [one] Zu welcher Liste hinzufügen?
   *[other] { $count } Einträge zu welcher Liste hinzufügen?
}
library-list-none = Du hast noch keine Listen.
library-list-create = Liste anlegen
item-remove = Aus der Bibliothek entfernen
# Tastaturkürzel und Suchpalette (WP4-4)
shortcuts-chord-waiting = G gedrückt: dann H, L, Y, T oder P, um zu einem Bereich zu wechseln.
shortcuts-then = dann
shortcuts-or = oder
shortcuts-everywhere = Überall
shortcuts-home = Start
shortcuts-library = Bibliothek
shortcuts-item = Titelseite
shortcuts-search = Suche
shortcuts-go-home = Zu Start
shortcuts-go-library = Zur Bibliothek
shortcuts-go-history = Zum Verlauf
shortcuts-go-lists = Zu den Listen
shortcuts-go-people = Zu den Personen
shortcuts-home-move = Zwischen den Karten unter „Weitermachen“ wechseln
shortcuts-home-run = Nächsten Schritt der fokussierten Karte ausführen
shortcuts-library-move = Zwischen den Karten wechseln
shortcuts-library-row = Erste oder letzte Karte der Zeile
shortcuts-library-first = Erste Karte überhaupt (Strg+Ende: die letzte)
shortcuts-library-open = Fokussierte Karte öffnen
shortcuts-library-step = Nächster Schritt der fokussierten Karte
shortcuts-library-tags = Schlagwörter der fokussierten Karte
shortcuts-library-list = Fokussierte Karte zu einer Liste hinzufügen
shortcuts-library-filter = Bibliothek filtern
shortcuts-item-step = Nächsten Schritt eintragen
shortcuts-item-rating = Zur eigenen Bewertung springen
shortcuts-item-tags = Schlagwörter bearbeiten
shortcuts-item-list = Zu einer Liste hinzufügen
shortcuts-search-open = Treffer öffnen oder für die Seite auswählen, die die Suche geöffnet hat
shortcuts-search-quick = Als „Geplant“ hinzufügen oder den nächsten Schritt ausführen
shortcuts-key-enter = Eingabe
shortcuts-key-home = Pos1
shortcuts-key-end = Ende
shortcuts-key-arrows = Pfeiltasten
shortcuts-off-note = Kürzel mit einer Taste sind ausgeschaltet (Einstellungen > Konto). Strg+K und die Tasten in Rastern, Menüs und der Suche funktionieren weiter.
search-target-rule = Ein gewählter Treffer wird zum Gegenstand der Regel.
search-target-log-with = Ein gewählter Treffer wird mit { $name } eingetragen.
search-recent = Zuletzt geöffnet
search-suggested = Vorschläge
search-cmd-lists = Zu den Listen
search-cmd-new-list = Neue Liste
search-cmd-log-play = Partie von { $title } eintragen
search-cmd-import = Datei importieren
search-hint-open = Öffnen
search-hint-choose = Auswählen
search-hint-quick = Als „Geplant“ hinzufügen oder nächster Schritt
manual-row = „{ $query }“ von Hand hinzufügen...
manual-group = Nicht dabei?
manual-add-title = Von Hand hinzufügen
manual-add-lead = Für etwas, das keine Quelle kennt. Es kommt in deine Bibliothek, und alle Mitglieder dieser Instanz können es finden.
manual-edit-title = Details bearbeiten
manual-field-title = Titel
manual-field-kind = Art
manual-kind-choose = Art wählen...
manual-field-year = Jahr
manual-field-year-hint = Optional.
manual-field-cover = Bildadresse
manual-field-cover-hint = Optional, beginnt mit https://. Das Bild wird von jedem Browser, der es anzeigt, direkt von dieser Adresse geladen.
manual-submit-add = Zur Bibliothek hinzufügen
manual-submit-edit = Speichern
manual-existing-title = Schon vorhanden
manual-existing-use = Diesen Eintrag verwenden
manual-existing-use-named = Diesen Eintrag verwenden: { $title }
manual-error-title = Gib einen Titel ein.
manual-error-title-long = Der Titel darf höchstens 500 Zeichen haben.
manual-error-kind = Wähle eine Art.
manual-error-year = Gib das Jahr als Zahl ein, etwa 1995, oder lass das Feld leer.
manual-error-cover = Die Bildadresse muss mit https:// beginnen und einen Host nennen, ohne Benutzernamen und ohne Leerzeichen.
manual-error-cover-forbidden = Auf dieser Instanz kann nur ein Admin eine Bildadresse setzen.
manual-error-forbidden = Auf dieser Instanz kann nur ein Admin die Details eines Eintrags ändern.
manual-error-sourced = Für diesen Eintrag ist jetzt eine Datenquelle zuständig; seine Details kommen von dort.
manual-error-duplicate = Ein Eintrag dieser Art mit diesem Titel ist schon vorhanden, siehe unten. Verwende ihn, oder wähle noch einmal „Zur Bibliothek hinzufügen“, um einen zweiten anzulegen.
item-edit-details = Details bearbeiten
companions-log-with = Eintragen mit...
companions-log-with-named = Eintragen mit { $name }
notify-filter-pick-item = Titel auswählen...
notify-filter-item = Titel: { $name }
notify-hint-secret-keep = Leer lassen, um den gespeicherten Wert zu behalten.
notify-edit = Bearbeiten
notify-edit-named = { $name } bearbeiten
notify-channel-enabled = An diesen Kanal senden
notify-channel-save = Speichern
notify-channel-cancel = Abbrechen
settings-shortcuts = Tastaturkürzel
settings-shortcuts-hint = Einzelne Tasten wie G, N, W oder ? wirken außerhalb von Textfeldern als Kürzel. Schalte sie aus, wenn sie Spracheingabe oder Screenreader stören. Wird in diesem Browser gespeichert.
# WP4-5 Offline-Warteschlange und Messung des Eintragens
offline-saved = Offline gespeichert, wird synchronisiert
offline-waiting = { $count ->
    [one] 1 Änderung wartet
   *[other] { $count } Änderungen warten
}
offline-dropped = Nicht gespeichert: { $what } - der Server hat diese offline gemachte Änderung abgelehnt
offline-sheet = Du bist offline. Speichere, wenn du wieder online bist.
offline-bulk = Du bist offline. Sammelbearbeitungen warten, bis du wieder online bist.
privacy-usage-on = Diese Instanz zählt pro Tag und ohne deinen Namen oder Titel, wie viele Klicks und Sekunden das Eintragen braucht (vom Öffnen der Suche, eines Formulars oder des nächsten Schritts bis zum gespeicherten Eintrag), und wie oft diese aufklappbaren Abschnitte gezeigt, geöffnet, innerhalb von 2 Sekunden geöffnet und beim Speichern offen sind: die Filter der Bibliothek; „Mehr“ in den Formularen zum Eintragen und für Partien eines Eintrags; „Ein Ziel setzen“ in der Statistik; „Anmeldung“ in Einstellungen > Konto; und für Admins die Einladungen unter Personen, „E-Mail“, „Gespeicherte Geheimnisse“ und „Erweitert“ unter Instanz sowie „Nutzungsmessung“ unter Speicher und Aufgaben. Die Summen werden 180 Tage aufbewahrt; der Admin kann das abschalten.
privacy-usage-off = Diese Instanz zählt nicht, wie das Eintragen genutzt wird.
admin-setting-usage_funnel = Eintragen messen
admin-usage-title = Nutzungsmessung
admin-usage-summary = Tagessummen ohne Personen, 180 Tage aufbewahrt
admin-usage-what = Pro Tag gezählt: wie ein Eintrag von seinem Beginn (Suche geöffnet, nächster Schritt geklickt, Formular geöffnet, Folge markiert, Zeilen ausgewählt) zu gespeichert oder abgebrochen kam, in Stufen von Klicks (1 bis 5 und mehr) und Sekunden (unter 2, 5, 10, 30, 60 und mehr); und wie oft jeder der zehn gemessenen aufklappbaren Abschnitte (Filter der Bibliothek, die beiden „Mehr“ der Formulare, „Ein Ziel setzen“, „Anmeldung“ in den Einstellungen, die Einladungen, E-Mail, Gespeicherte Geheimnisse, Erweitert und dieser) gezeigt, geöffnet, innerhalb von 2 Sekunden geöffnet und beim Speichern offen war. Keine Person, kein Eintrag, kein Titel wird gespeichert.
admin-usage-on = An
admin-usage-off = Aus
admin-usage-csv = Tagessummen herunterladen (CSV)
admin-usage-caption = Summen der letzten { $days } Tage ({ $retention } Tage aufbewahrt)
admin-usage-none = In den letzten 28 Tagen wurde nichts gezählt.
admin-usage-flow = Ablauf
admin-usage-section = Abschnitt
admin-usage-outcome = Ergebnis
admin-usage-count = Anzahl
# WP4-6 Ziele, Jahresvergleich, häufigste Personen, Spielgruppen
stats-compare-title = { $year } bisher
stats-compare-lead = Vom 1. Januar bis { $through }, neben denselben Tagen im Vorjahr (bis { $last }).
stats-compare-none = In beiden Zeiträumen ist noch nichts eingetragen.
stats-compare-value = { $this } in { $year }, { $last } in { $last_year }
stats-often-with = Am häufigsten mit
stats-often-none = Trage etwas mit einer Person ein, dann erscheint sie hier.
stats-often-together = { $count ->
    [one] einmal zusammen
   *[other] { $count }-mal zusammen
}
goals-heading = Ziele
goals-lead = Freiwillig und nur für dich. Ein Ziel zählt, was du in seinem Jahr einträgst, sonst nichts.
goals-set = Ein Ziel setzen
goals-set-summary = Eine Jahreszahl für eine Gruppe und eine Größe
goals-title = { $group }: { $measure }
goals-progress = { $done } von { $target } in { $year }
goals-bar = { $title } in { $year }
goals-edit = Bearbeiten
goals-edit-named = Ziel { $title } bearbeiten
goals-delete = Löschen
goals-delete-named = Ziel { $title } löschen
goals-delete-title = Das Ziel { $title } löschen?
goals-delete-body = Nur das Ziel geht; alles Eingetragene bleibt.
goals-year = Jahr
goals-group = Gruppe
goals-metric = Größe
goals-target = Zielwert
goals-target-hint = Eine ganze Zahl, 1 oder mehr.
goals-target-invalid = Gib als Zielwert eine ganze Zahl ab 1 ein.
goals-error-conflict = Für dieses Jahr, diese Gruppe und diese Größe gibt es schon ein Ziel. Bearbeite stattdessen dieses.
goals-save = Ziel speichern
companions-tab-groups = Spielgruppen { $count }
groups-list = Spielgruppen
groups-new = Neue Gruppe
groups-new-title = Neue Spielgruppe
groups-edit-title = { $name } bearbeiten
groups-empty = Noch keine Spielgruppen
groups-empty-body = Eine Gruppe sind die Leute, mit denen du oft spielst. Wählst du sie, stehen die Mitspielenden einer Partie in einem Schritt fest.
groups-members = Mit { $names }
groups-no-members = In dieser Gruppe ist niemand mehr.
groups-plays = Partien als Gruppe
groups-last = Zuletzt: { $title }, { $when }
groups-never = Noch keine Partie als Gruppe.
groups-log = Partie eintragen mit...
groups-log-named = Partie eintragen mit { $name }
groups-menu-named = Aktionen für { $name }
groups-edit = Bearbeiten
groups-delete = Löschen
groups-delete-title = { $name } löschen?
groups-delete-body = { $count ->
    [0] Die Gruppe verschwindet aus Personen und dem Partie-Formular.
    [one] Die Gruppe verschwindet aus Personen und dem Partie-Formular. Ihre Partie bleibt in deinem Verlauf.
   *[other] Die Gruppe verschwindet aus Personen und dem Partie-Formular. Ihre { $count } Partien bleiben in deinem Verlauf.
}
groups-deleted = { $name } gelöscht.
groups-name = Name
groups-members-legend = Mitglieder
groups-need-members = Wähle mindestens eine Person.
groups-save = Gruppe speichern
groups-created = { $name } angelegt.
groups-saved = { $name } gespeichert.
log-groups = Gruppen
search-target-log-group = Ein Spiel wählen trägt eine Partie mit { $name } ein.

# UX phase 4 review (web)
catalog-title-unavailable = Titel nicht verfügbar - Quelldaten abgelaufen
offline-what-status = Status { $status }
offline-what-rating = Bewertung
offline-what-notes = Notizen
offline-what-add = zur Bibliothek hinzugefügt
offline-full = Nicht gespeichert: Schon { $count } Änderungen warten, mehr behält dieser Browser nicht. Geh erst wieder online, damit sie gesendet werden.
offline-undo-sending = Nicht rückgängig gemacht: Diese Änderung wird gerade gesendet. Ändere sie zurück, sobald sie synchronisiert ist.
offline-undo-sent = Nicht rückgängig gemacht: Diese Änderung ist schon synchronisiert. Ändere sie auf der Seite des Eintrags oder im Verlauf zurück.
signout-waiting-title = { $count ->
    [one] Eine Änderung ist noch nicht synchronisiert
   *[other] { $count } Änderungen sind noch nicht synchronisiert
}
signout-waiting-body = Sie entstanden, als der Server nicht erreichbar war, und bleiben in diesem Browser, bis er es wieder ist. Wenn du dich jetzt abmeldest, werden sie verworfen.
signout-stay = Angemeldet bleiben
signout-discard = Abmelden und verwerfen
library-undo-failed = Rückgängig machen hat nicht geklappt: { $error }
library-undo-expired = Diese Änderung lässt sich hier nicht mehr rückgängig machen (das geht nur eine Stunde lang).
library-bulk-too-large = Zu viele Einträge, um sie auf einmal mit Rückgängig zu ändern. Wähle weniger aus und versuch es noch einmal.
admin-error-self_change = Das kannst du hier nicht für dein eigenes Konto tun. Nutze stattdessen Einstellungen > Konto.
admin-health-mail-result-not_configured = E-Mail ist nicht eingerichtet
admin-health-mail-result-connection = der Mailserver war nicht erreichbar
admin-health-mail-result-auth = der Mailserver hat die Anmeldung abgelehnt
admin-health-mail-result-rejected = der Mailserver hat die Nachricht abgelehnt
admin-health-mail-result-send_failed = das Senden ist fehlgeschlagen
admin-health-mail-result-private_target = der Mailserver liegt in einem privaten Netz, mit dem sich diese Instanz nicht verbindet
log-coop-result = Ausgang
log-coop-none = Kein Ergebnis
log-coop-lost = Wir haben verloren
log-scores = Punkte
log-scores-player = Person
log-scores-score = Punkte
log-scores-won = Gewonnen
log-scores-won-name = Gewonnen: { $name }
log-winner-reset = Höchste Punktzahl verwenden
log-winner-none = Niemand hat gewonnen
log-winner-hint = Die höchste Punktzahl wird als Sieg markiert. Bei Bedarf anders wählen.
log-winner-live = Gewonnen: { $name } (höchste Punktzahl).
log-winner-live-none = Niemand ist als Sieger markiert.
history-play-coop-none = Gemeinsam gespielt
item-section-plays = Partien
item-times-total = { $line }, insgesamt { $time }
plays-empty = Noch keine Partie eingetragen.
plays-caption = Partien von { $title }, neueste zuerst
plays-col-date = Datum
plays-col-players = Mitspielende
plays-col-scores = Punkte
plays-col-result = Ergebnis
plays-col-duration = Dauer
plays-newest = Aufgelistet sind die neuesten { $shown } von { $count } Partien.
plays-total = Partien
plays-time = Spielzeit
plays-time-none = nicht eingetragen
plays-rate = Gewinnquote
plays-rate-none = noch keine Ergebnisse
plays-rate-value = { $percent } %
plays-rate-detail = { $wins } von { $decided } { $decided ->
    [one] Partie
   *[other] Partien
} mit Ergebnis in deinem Protokoll
plays-bare = { $count ->
    [one] Eine Partie hat keine Details (Mitspielende, Ergebnis, Dauer). Sie zählt als Partie, nicht in der Spielzeit oder der Gewinnquote.
   *[other] { $count } Partien haben keine Details (Mitspielende, Ergebnis, Dauer). Sie zählen als Partien, nicht in der Spielzeit oder der Gewinnquote.
}
plays-duration-h = { $hours } Std.
plays-duration-hm = { $hours } Std. { $minutes } Min.
stats-play-results = Deine Ergebnisse
stats-play-open = { $count ->
    [one] Eine Partie zählt nicht mit (ohne Ergebnis oder ohne dich).
   *[other] { $count } Partien zählen nicht mit (ohne Ergebnis oder ohne dich).
}
stats-tile-untimed-movies = { $count ->
    [one] Länge noch nicht bekannt
   *[other] Längen noch nicht bekannt
}
stats-tile-untimed-shows = Laufzeiten noch nicht bekannt
stats-tile-untimed-books = noch keine Seiten eingetragen
stats-tile-untimed-games = noch keine Spieldauer eingetragen
