# Benachrichtigungstexte (D-17), siehe en.ftl.

digest-title = { $count ->
    [one] Eine Benachrichtigung
   *[other] { $count } Benachrichtigungen
}

release-item_released-title = { $title } ist erschienen
release-item_released-body = { $title } ist jetzt erschienen.
release-episode-title = Neue Folge von { $title }
release-episode-body = Staffel { $season }, Folge { $episode } von { $title } ist da.
release-chapter-title = Neues Kapitel von { $title }
release-chapter-body = Kapitel { $number } von { $title } ist da.
release-date_changed-title = Neuer Termin für { $title }
release-date_changed-body = { $title } erscheint jetzt am { $date }.

library-status_changed-title = { $title } hat sich geändert
library-status_changed-body = { $title } in { $list } ist jetzt: { $status }.
library-metadata_changed-title = { $title } wurde aktualisiert
library-metadata_changed-body = Die Angaben zu { $title } haben sich geändert.

social-link_request-title = { $from } sagt, du bist "{ $companion }"
social-link_request-body = Bestätige auf der Seite Personen, dass der Begleiter "{ $companion }" von { $from } du bist.
social-link_accepted-title = { $user } hat die Verknüpfung angenommen
social-link_accepted-body = Dein Begleiter "{ $companion }" ist jetzt mit { $user } verknüpft.
social-session_shared-title = { $owner } hat { $title } mit dir erfasst
social-session_shared-body = Du findest es in deinem Verlauf und kannst es in dein Protokoll kopieren.
social-list_shared-title = { $owner } hat eine Liste geteilt
social-list_shared-body = { $owner } hat die Liste "{ $list }" mit dir geteilt.

boardgame-loan_due-title = { $title } kommt bald zurück
boardgame-loan_due-body = { $companion } soll { $title } am { $date } zurückgeben.
boardgame-loan_overdue-title = { $title } ist überfällig
boardgame-loan_overdue-body = { $companion } sollte { $title } am { $date } zurückgeben.
boardgame-wishlist_owned-title = { $user } besitzt { $title }
boardgame-wishlist_owned-body = { $title } von deiner Wunschliste ist in der Sammlung von { $user }.

integration-play_recorded-title = Erfasst: { $title }
integration-play_recorded-body = { $source } hat gemeldet, dass du { $title } beendet hast.
integration-failing-title = { $name } funktioniert nicht
integration-failing-body = Die Integration { $name } funktioniert nicht mehr: { $error }

import-finished-title = Import abgeschlossen
import-finished-body = { $imported ->
    [one] Ein Eintrag importiert
   *[other] { $imported } Einträge importiert
}, { $errors ->
    [one] eine Zeile mit Problemen.
   *[other] { $errors } Zeilen mit Problemen.
}
import-failed-title = Import fehlgeschlagen
import-failed-body = Der Import wurde abgebrochen: { $error }

admin-storage_high-title = Speicher fast voll
admin-storage_high-body = { $used } von { $cap } belegt.
admin-storage_full-title = Speicher voll
admin-storage_full-body = { $used } von { $cap } belegt; neue Bilder werden nicht gespeichert.
admin-connector_down-title = { $connector } ist nicht erreichbar
admin-connector_down-body = Anfragen an { $connector } pausieren nach wiederholten Fehlern.
admin-signup-title = Neues Konto: { $username }
admin-signup-body = { $name } ({ $username }) hat ein Konto bei { $product } angelegt.
admin-job_backlog-title = Aufgaben stauen sich
admin-job_backlog-body = { $count ->
    [one] Eine Aufgabe wartet in der Spur { $lane }.
   *[other] { $count } Aufgaben warten in der Spur { $lane }.
}

test-title = Test von { $product }
test-body = Der Kanal "{ $name }" funktioniert.
