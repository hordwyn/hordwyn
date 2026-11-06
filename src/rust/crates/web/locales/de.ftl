# Deutsche Texte (Fluent). Marken können jeden Schlüssel in brand.toml unter [strings.de]
# überschreiben.

nav-library = Bibliothek
nav-companions = Begleiter
nav-lists = Listen
nav-sources = Datenquellen
nav-search = Suche
nav-sign-in = Anmelden
nav-sign-up = Konto erstellen
nav-sign-out = Abmelden
nav-main = Hauptnavigation
skip-to-content = Zum Inhalt springen

home-title = Halte fest, was du schaust, liest und spielst
home-lead = Filme, Serien, Bücher, Spiele und mehr, zusammen mit den Menschen, mit denen du sie teilst.

auth-username = Benutzername
auth-display-name = Anzeigename
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
auth-error-weak_password = Das Passwort braucht mindestens 10 Zeichen.
auth-error-conflict = Dieser Benutzername ist vergeben.
auth-error-invalid = Bitte prüfe die Felder.
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
item-mark-done = Als erledigt markieren
item-mark-done-with = Als erledigt markieren mit
item-done-logged = Eingetragen.
item-rating = Deine Bewertung
item-rate = Mit { $stars } von 5 bewerten
item-history = Verlauf
item-history-empty = Noch nichts eingetragen.
item-loading-details = Details werden geladen...
item-source = Quelle: { $name }
item-cover-alt = Cover von { $title }
item-year = Jahr
item-lists = Zu Liste hinzufügen

search-title = Suche
search-label = Katalog durchsuchen
search-placeholder = Titel, Autorin, Studio...
search-empty = Noch keine Treffer.
search-local = In deiner Instanz
search-waiting = Frage { $sources } an...
search-hint = Strg+K

companions-title = Begleiter
companions-lead = Menschen, mit denen du schaust, liest und spielst. Nur du siehst diese Liste.
companions-new = Name
companions-add = Begleiter hinzufügen
companions-you = (du)
companions-empty = Noch keine Begleiter.

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

nav-sessions = Verlauf
auth-invited = Du wurdest eingeladen: Dein Konto wird mit dem Begleiter verknüpft, der dich eingeladen hat, und du siehst die Sitzungen, bei denen du dabei warst.
companions-choose = Auswählen...
companions-companion = Begleiter
companions-you-short = du
companions-linked = Verknüpft
companions-invite = Einladen
companions-invite-link = Schicke diesen einmaligen Link:
companions-unlink = Verknüpfung lösen
companions-requests = Anfragen an dich
companions-request-text = { $from } sagt, der Begleiter "{ $name }" bist du.
companions-accept = Annehmen
companions-decline = Ablehnen
companions-link-title = Mit einem Konto verknüpfen
companions-link-lead = Hat ein Begleiter hier schon ein Konto, bitte ihn zu bestätigen, dass er es ist. Er sieht dann die Sitzungen, die du mit ihm teilst.
companions-link-submit = Anfrage senden
companions-link-sent = Anfrage gesendet.
companions-merge-title = Doppelte zusammenführen
companions-merge-lead = Der doppelte Eintrag geht in dem Begleiter auf, den du behältst. Erfasste Sitzungen bleiben unverändert.
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
sharing-error-conflict = Das geht bei diesem Begleiter nicht.
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
