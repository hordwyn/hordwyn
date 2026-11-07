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
next-step-watched = Gesehen
next-step-watched-again = Nochmal gesehen
next-step-finished = Beendet
next-step-finished-again = Nochmal beendet
next-step-episode = S{ $season } E{ $episode } gesehen
next-step-episode-listened = S{ $season } E{ $episode } gehört
next-step-caught-up = Auf dem neuesten Stand
next-step-update-page = Seite eintragen
next-step-update-position = Position eintragen
next-step-log-play = Partie eintragen
next-step-log-session = Sitzung eintragen

item-add = Zur Bibliothek hinzufügen
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
item-rewatches = Wiederholungen
item-plays-count = { $count ->
    [one] Eine Partie
   *[other] { $count } Partien
}
item-actions = Weitere Aktionen für { $title }
item-action-bar = Nächster Schritt
item-with = Mit...
item-with-named = Mit Details eintragen: { $title }
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
log-players = Mitspielende
log-people-add = Person hinzufügen
log-people-create = „{ $name }“ hinzufügen
log-people-none = Niemand mit diesem Namen
log-more = Mehr
log-more-watched = Bewertung und Notiz
log-more-play = Kooperativ, Dauer, Ort, Notizen
log-note = Notiz zu diesem Titel (privat)
log-rewatch = { $count ->
    [one] Schon einmal abgeschlossen: Das zählt als Wiederholung.
   *[other] Schon { $count }-mal abgeschlossen: Das zählt als Wiederholung.
}
log-save = Speichern
log-discard-question = Änderungen verwerfen?
log-discard = Verwerfen
log-keep = Weiter bearbeiten
log-score = Punkte von { $name }
log-winner = Gewonnen hat
log-winner-auto = Höchste Punktzahl
log-coop = Kooperatives Spiel
log-coop-won = Wir haben gewonnen
log-duration = Dauer in Minuten
log-location = Ort
log-notes = Notizen
log-play-step = Partie
log-page = Aktuelle Seite
log-of-pages = von { $pages }
log-update = Aktualisieren
log-plus-pages = +{ $count }
log-finished = Fertig
log-position = Position
log-position-hint = Als h:mm:ss oder in Minuten.
log-of-time = von { $time }
log-plus-minutes = +{ $count } Min.
log-error-date-missing = Wähle ein Datum.
log-error-date-future = Das Datum liegt in der Zukunft.
log-error-score = Punkte sind Zahlen, zum Beispiel 42 oder 12,5.
log-error-duration = Die Dauer ist eine ganze Zahl von Minuten.
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
episodes-again = Nochmal
episodes-next = Als Nächstes
episodes-row-menu = Mehr zu S{ $season } E{ $episode }
episodes-up-to-here = Bis hierhin markieren
episodes-with = Gesehen mit...
episodes-ask = { $count ->
    [one] Auch die frühere Folge als gesehen markieren?
   *[other] Auch die { $count } früheren Folgen als gesehen markieren?
}
episodes-ask-all = Alle { $count } markieren
episodes-ask-one = Nur S{ $season } E{ $episode }
episodes-remember = Für diese Serie merken
episodes-logged = { $count ->
    [one] Eine Folge
   *[other] { $count } Folgen
}
episodes-none = Noch keine Folgen bekannt.

search-title = Suche
search-label = Katalog durchsuchen
search-placeholder = Titel, Autorin, Studio...
search-empty = Noch keine Treffer.
search-local = In deiner Instanz
search-waiting = Frage { $sources } an...

companions-title = Personen
companions-lead = Menschen, mit denen du schaust, liest und spielst. Nur du siehst diese Liste.
companions-new = Name
companions-add = Person hinzufügen

lists-title = Listen
lists-new = Name der Liste
lists-create = Liste anlegen
lists-empty = Noch keine Listen.
lists-items = { $count ->
    [one] Ein Eintrag
   *[other] { $count } Einträge
}

sources-title = Datenquellen
sources-lead = Die Metadaten stammen von diesen Diensten. Für ihre Daten gelten deren Bedingungen und Lizenzen.

theme-dark = Zum dunklen Design wechseln
theme-light = Zum hellen Design wechseln
dialog-close = Schließen

auth-invited = Du wurdest eingeladen: Dein Konto wird mit der Person verknüpft, die dich eingeladen hat, und du siehst die Sitzungen, bei denen du dabei warst.
companions-choose = Auswählen...
companions-companion = Person
companions-linked = Verknüpft
companions-invite = Einladen
companions-invite-link = Schicke diesen einmaligen Link:
companions-unlink = Verknüpfung lösen
companions-requests = Anfragen an dich
companions-request-text = { $from } sagt, "{ $name }" auf der Seite Personen bist du.
companions-accept = Annehmen
companions-decline = Ablehnen
companions-link-title = Mit einem Konto verknüpfen
companions-link-lead = Hat jemand hier schon ein Konto, bitte die Person zu bestätigen, dass sie es ist. Sie sieht dann die Sitzungen, die du mit ihr teilst.
companions-link-submit = Anfrage senden
companions-link-sent = Anfrage gesendet.
companions-merge-title = Doppelte zusammenführen
companions-merge-lead = Der doppelte Eintrag geht in der Person auf, die du behältst. Erfasste Sitzungen bleiben unverändert.
companions-merge-keep = Behalten
companions-merge-remove = Doppelter Eintrag
companions-merge-submit = Zusammenführen
privacy-title = Privatsphäre
privacy-share-sessions = Verknüpfte Mitspieler sehen die Sitzungen, die ich erfasse
sessions-by = von { $name }
sessions-claim = In mein Protokoll kopieren
sessions-claimed = In dein Protokoll kopiert.
lists-share = Per Link teilen
lists-share-link = Alle mit diesem Link sehen die Liste:
shared-list-missing = Diese Liste ist nicht (mehr) geteilt.
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
notify-channels-lead = Wohin Benachrichtigungen außer in diesen Posteingang gehen. Mit "Testen" prüfst du einen Kanal.
notify-new-channel = Kanal hinzufügen
notify-kind = Art
notify-channel-name = Name
notify-add-channel = Kanal hinzufügen
notify-channel-added = Kanal hinzugefügt.
notify-secret-created = Kanal hinzugefügt. Das Signatur-Geheimnis wird nur einmal angezeigt: { $secret }
notify-test = Testen
notify-test-sent = Testnachricht gesendet.
notify-delete = Löschen
notify-deleted = Gelöscht.
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
notify-subscriptions = Abonnements
notify-subscriptions-empty = Keine Abonnements: Benachrichtigungen landen nur im Posteingang.
notify-new-subscription = Abonnieren
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
notify-quiet-from = Ruhe ab
notify-quiet-to = Ruhe bis
notify-timezone = Zeitzone
notify-subscribe = Abonnieren
notify-subscribed = Abonniert.
notify-default = Standard
notify-muted = Stumm
notify-log = Zustellprotokoll
notify-log-empty = Noch nichts gesendet.
notify-state-queued = Wartet
notify-state-digest = In Zusammenfassung
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
integrations-lead = Medienserver und Skripte können über einen Webhook melden, was du geschaut hast. Lege für jeden Absender ein Token an; die Dokumentation ("Integrations") beschreibt das Format und die Einrichtung mit Jellyfin.
integrations-name = Name, z. B. Jellyfin
integrations-add = Token anlegen
integrations-created = Neues Token: jetzt kopieren, es wird nicht wieder angezeigt.
integrations-url = Webhook-URL:
integrations-token = Token:
integrations-example = Beispiel:
integrations-empty = Noch keine Integrationen.
integrations-never = nie
integrations-error = Letzter Aufruf fehlgeschlagen
integrations-revoke = Widerrufen
auth-error-too_many_attempts = Zu viele fehlgeschlagene Anmeldungen. Bitte warte ein paar Minuten.
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
lists-share-named = { $name } per Link teilen
companions-name-self = { $name } (du)
companions-invite-named = { $name } einladen
companions-unlink-named = Verknüpfung lösen: { $name }
integrations-last-call = Letzter Aufruf: { $when }
integrations-revoke-named = { $name } widerrufen
notify-mark-read-named = Als gelesen markieren: { $title }
notify-test-named = { $name } testen
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
# ---- Ende i18n ----

# ---- UX phase 0 review (docs/reviews/2026-11-06-ux0-review.md) ----
auth-lockout-minutes = { $minutes ->
    [one] Zu viele fehlgeschlagene Anmeldungen. Versuch es in 1 Minute noch einmal.
   *[other] Zu viele fehlgeschlagene Anmeldungen. Versuch es in { $minutes } Minuten noch einmal.
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
stats-entries = Einträge
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
history-with = mit { $names }
history-time-utc = { $time } UTC
history-shared = Geteilt
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
source-state-disabled = In der Konfiguration abgeschaltet
source-state-needs_account = Braucht dein Konto
source-state-unavailable = Vorübergehend nicht verfügbar
source-state-slow = Gerade langsam
source-state-retrying = Antwortet nicht, neuer Versuch folgt
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
admin-health-secrets-active = Schlüssel v{ $version } aktiv
admin-health-secrets-locked = { $count } Geheimnisse lassen sich nicht entschlüsseln
admin-health-secrets-rotation_due = Schlüssel v{ $version } ist älter als zwei Jahre: ein neuer wäre gut
admin-health-secrets-rewrap = { $count } Geheimnisse liegen noch unter einem älteren Schlüssel
setup-checklist-first_item = Füge deinen ersten Eintrag hinzu
setup-checklist-invite = Lade deinen Haushalt ein
setup-checklist-media_server = Verbinde deinen Medienserver
setup-checklist-notification_defaults = Lege die Standards für Benachrichtigungen fest
setup-checklist-backup_key = Sichere den Geheimnis-Schlüssel zusammen mit der Datenbank
setup-checklist-identity_provider = Verbinde einen Identitätsanbieter
admin-audit-setting_changed = Einstellung geändert
admin-audit-secret_replaced = Schlüssel ersetzt
admin-audit-secret_cleared = Schlüssel entfernt
admin-audit-secrets_rewrapped = Geheimnisse neu verschlüsselt
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
admin-nav-users = Personen und Registrierung
admin-nav-sources = Datenquellen
admin-nav-storage = Speicher und Aufgaben
admin-nav-notifications = Standard-Benachrichtigungen
admin-nav-brand = Marke
admin-nav-security = Sicherheit und Sitzungen
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
admin-health-name-secrets = Gespeicherte Geheimnisse
admin-overview-lead = Was Mitglieder nicht sehen: die Teile, von denen diese Instanz abhängt.
admin-overview-attention = { $count ->
    [one] Eine Sache braucht Aufmerksamkeit
   *[other] { $count } Sachen brauchen Aufmerksamkeit
}
admin-overview-fix = Öffnen
admin-overview-hosted = Gehosteter Betrieb: Halte für jede Datenquelle die Lizenzvereinbarung fest.
admin-overview-health = Zustand
admin-overview-open = Details: { $name }
admin-overview-details = Details
admin-overview-recent = Letzte Verwaltungsaktionen
admin-overview-recent-none = Noch keine Verwaltungsaktionen.
admin-overview-audit = Ganzes Protokoll
admin-overview-signups = Neueste Konten
admin-overview-brand = { $name }, Standardsprache { $locale }. Nur lesbar.
admin-overview-brand-link = Marke und Kontrast
admin-users-lead = Wer ein Konto hat, wer Admin ist und wie neue Leute dazukommen.
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
admin-brand-lead = Name, Sprache und Farben dieser Instanz. Hier nur lesbar.
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
admin-security-lead = Wie lange Sitzungen dauern, wie Cookies gesendet werden und der Schlüssel, der gespeicherte Geheimnisse schützt.
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
home-nudge-snooze = 7 Tage ausblenden

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
