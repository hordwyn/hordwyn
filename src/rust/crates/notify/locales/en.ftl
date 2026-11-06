# Notification texts (D-17). One title and body per event of the catalogue; a channel can
# get its own text with `<event>-<channel>-title` / `-body`. `$product` is the brand name.

digest-title = { $count ->
    [one] One notification
   *[other] { $count } notifications
}

release-item_released-title = { $title } is out
release-item_released-body = { $title } has been released.
release-episode-title = New episode of { $title }
release-episode-body = Season { $season }, episode { $episode } of { $title } is out.
release-chapter-title = New chapter of { $title }
release-chapter-body = Chapter { $number } of { $title } is out.
release-date_changed-title = New release date for { $title }
release-date_changed-body = { $title } now comes out on { $date }.

library-status_changed-title = { $title } changed
library-status_changed-body = { $title } in { $list } is now: { $status }.
library-metadata_changed-title = { $title } was updated
library-metadata_changed-body = The details of { $title } changed.

social-link_request-title = { $from } says you are "{ $companion }"
social-link_request-body = Confirm on the People page that { $from }'s companion "{ $companion }" is you.
social-link_accepted-title = { $user } accepted your link
social-link_accepted-body = Your companion "{ $companion }" is now linked to { $user }.
social-session_shared-title = { $owner } logged { $title } with you
social-session_shared-body = See it in your history and copy it to your log if you like.
social-list_shared-title = { $owner } shared a list
social-list_shared-body = { $owner } shared the list "{ $list }" with you.

boardgame-loan_due-title = { $title } is due back
boardgame-loan_due-body = { $companion } should return { $title } on { $date }.
boardgame-loan_overdue-title = { $title } is overdue
boardgame-loan_overdue-body = { $companion } was to return { $title } on { $date }.
boardgame-wishlist_owned-title = { $user } owns { $title }
boardgame-wishlist_owned-body = { $title } from your wishlist is in { $user }'s collection.

integration-play_recorded-title = Recorded: { $title }
integration-play_recorded-body = { $source } reported that you finished { $title }.
integration-failing-title = { $name } is failing
integration-failing-body = The integration { $name } stopped working: { $error }

import-finished-title = Import finished
import-finished-body = { $imported ->
    [one] One entry imported
   *[other] { $imported } entries imported
}, { $errors ->
    [one] one row with problems.
   *[other] { $errors } rows with problems.
}
import-failed-title = Import failed
import-failed-body = The import stopped: { $error }

admin-storage_high-title = Storage almost full
admin-storage_high-body = { $used } of { $cap } used.
admin-storage_full-title = Storage full
admin-storage_full-body = { $used } of { $cap } used; new images are not stored.
admin-connector_down-title = { $connector } is unavailable
admin-connector_down-body = Requests to { $connector } are paused after repeated failures.
admin-signup-title = New account: { $username }
admin-signup-body = { $name } ({ $username }) created an account on { $product }.
admin-job_backlog-title = Jobs are piling up
admin-job_backlog-body = { $count ->
    [one] One job is waiting in the { $lane } lane.
   *[other] { $count } jobs are waiting in the { $lane } lane.
}

test-title = Test from { $product }
test-body = The channel "{ $name }" works.
