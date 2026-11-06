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
library-empty = Deine Bibliothek ist leer. Mit Strg+K findest du etwas zum Hinzufügen.
library-count = { $count ->
    [one] Ein Eintrag
   *[other] { $count } Einträge
}

status-planned = Geplant
status-in_progress = Begonnen
status-completed = Abgeschlossen
status-paused = Pausiert
status-dropped = Abgebrochen

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

item-add = Zur Bibliothek hinzufügen
item-in-library = In deiner Bibliothek
item-mark-done = Als abgeschlossen markieren
item-mark-done-with = Als abgeschlossen markieren mit
item-done-logged = Eingetragen.
item-rating = Deine Bewertung
item-rate = Mit { $stars } von 5 bewerten
item-history = Verlauf
item-history-empty = Noch nichts eingetragen.
item-loading-details = Details werden geladen...
item-source = Quelle: { $name }
item-cover-alt = Cover von { $title }
item-lists = Zu Liste hinzufügen

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
companions-you-short = du
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
sessions-title = Verlauf
sessions-shared = Mit dir geteilt
sessions-shared-empty = Noch hat niemand eine Sitzung mit dir geteilt.
sessions-mine = Deine Sitzungen
sessions-mine-empty = Noch keine Sitzungen erfasst.
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
sessions-winner = { $name } (gewonnen)
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
