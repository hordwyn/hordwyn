# English messages (Fluent). Keys are stable; brands can override any key in brand.toml
# under [strings.en].

nav-library = Library
nav-companions = Companions
nav-lists = Lists
nav-sources = Data sources
nav-search = Search
nav-sign-in = Sign in
nav-sign-up = Create account
nav-sign-out = Sign out
nav-main = Main navigation
skip-to-content = Skip to content

home-title = Track what you watch, read and play
home-lead = Movies, shows, books, games and more, with the people you share them with.

auth-username = Username
auth-display-name = Display name
auth-password = Password
auth-email = Email (optional)
auth-totp = One-time code
auth-sign-in-title = Sign in
auth-sign-up-title = Create an account
auth-sign-in-submit = Sign in
auth-sign-up-submit = Create account
auth-have-account = Already have an account?
auth-no-account = No account yet?
auth-error-invalid_credentials = Wrong username or password.
auth-error-totp_required = Enter the one-time code from your authenticator app.
auth-error-invalid_totp = That one-time code is not valid.
auth-error-signup_closed = Sign-up is not open on this instance.
auth-error-invalid_invite = This invite is not valid any more.
auth-error-weak_password = Use at least 10 characters for the password.
auth-error-conflict = That username is taken.
auth-error-invalid = Please check the fields.
error-generic = Something went wrong. Please try again.

library-title = Library
library-empty = Your library is empty. Press Ctrl+K to search for something to add.
library-count = { $count ->
    [one] One item
   *[other] { $count } items
}

status-planned = Planned
status-in_progress = In progress
status-completed = Completed
status-paused = Paused
status-dropped = Dropped

kind-movie = Movie
kind-show = Show
kind-anime = Anime
kind-manga = Manga
kind-book = Book
kind-audiobook = Audiobook
kind-podcast = Podcast
kind-music = Music
kind-video_game = Video game
kind-visual_novel = Visual novel
kind-comic = Comic
kind-board_game = Board game

item-add = Add to library
item-in-library = In your library
item-mark-done = Mark as done
item-mark-done-with = Mark as done with
item-done-logged = Logged.
item-rating = Your rating
item-rate = Rate { $stars } of 5
item-history = History
item-history-empty = Nothing logged yet.
item-loading-details = Fetching details...
item-source = Source: { $name }
item-cover-alt = Cover of { $title }
item-year = Year
item-lists = Add to list

search-title = Search
search-label = Search the catalog
search-placeholder = Title, author, studio...
search-empty = No results yet.
search-local = In your instance
search-waiting = Asking { $sources }...
search-hint = Ctrl+K

companions-title = Companions
companions-lead = People you watch, read and play with. Only you see this list.
companions-new = Name
companions-add = Add companion
companions-you = (you)
companions-empty = No companions yet.

lists-title = Lists
lists-new = List name
lists-create = Create list
lists-empty = No lists yet.
lists-items = { $count ->
    [one] One item
   *[other] { $count } items
}

sources-title = Data sources
sources-lead = Metadata comes from these services. Their terms and licences apply to their data.

theme-dark = Switch to dark theme
theme-light = Switch to light theme
dialog-close = Close

nav-sessions = History
auth-invited = You were invited: your account is linked to the companion who invited you, and you see the sessions you were part of.
companions-choose = Choose...
companions-companion = Companion
companions-you-short = you
companions-linked = Linked
companions-invite = Invite
companions-invite-link = Send this single-use link:
companions-unlink = Unlink
companions-requests = Requests for you
companions-request-text = { $from } says their companion "{ $name }" is you.
companions-accept = Accept
companions-decline = Decline
companions-link-title = Link to an account
companions-link-lead = If a companion already has an account here, ask them to confirm it is them. They see the sessions you share with them.
companions-link-submit = Send link request
companions-link-sent = Link request sent.
companions-merge-title = Merge duplicates
companions-merge-lead = The duplicate is folded into the companion you keep. Logged sessions stay as they are.
companions-merge-keep = Keep
companions-merge-remove = Duplicate
companions-merge-submit = Merge
privacy-title = Privacy
privacy-share-sessions = Linked participants see the sessions I log
sessions-title = History
sessions-shared = Shared with you
sessions-shared-empty = Nobody has shared a session with you yet.
sessions-mine = Your sessions
sessions-mine-empty = No sessions logged yet.
sessions-by = by { $name }
sessions-claim = Copy to my log
sessions-claimed = Copied to your log.
lists-share = Share by link
lists-share-link = Anyone with this link can see the list:
shared-list-missing = This list is not shared (any more).
sharing-error-conflict = That is not possible for this companion.
sharing-error-not_found = Not found. Check the username.
sharing-error-forbidden = You may not do that.
sharing-error-invalid = Please check the fields.

nav-notifications = Notifications
notify-title = Notifications
notify-inbox = Inbox
notify-inbox-empty = Nothing new.
notify-mark-read = Mark as read
notify-channels = Channels
notify-channels-lead = Where notifications go besides this inbox. Use "Test" to check a channel.
notify-new-channel = Add a channel
notify-kind = Type
notify-channel-name = Name
notify-add-channel = Add channel
notify-channel-added = Channel added.
notify-secret-created = Channel added. Its signing secret is shown only once: { $secret }
notify-test = Test
notify-test-sent = Test message sent.
notify-delete = Delete
notify-deleted = Deleted.
notify-disabled = Disabled
notify-kind-ntfy = ntfy
notify-kind-gotify = Gotify
notify-kind-discord = Discord
notify-kind-matrix = Matrix
notify-kind-apprise = Apprise API
notify-kind-webhook = Webhook (signed)
notify-kind-email = Email
notify-kind-web_push = Browser push
notify-f-server = Server URL
notify-f-topic = Topic
notify-f-token-optional = Access token (optional)
notify-f-app-token = App token
notify-f-webhook-url = Webhook URL
notify-f-homeserver = Homeserver URL
notify-f-room = Room ID
notify-f-access-token = Access token
notify-f-apprise-urls = Apprise URLs
notify-f-url = URL
notify-f-secret-optional = Signing secret (empty: generate one)
notify-f-email = Email address
notify-subscriptions = Subscriptions
notify-subscriptions-empty = No subscriptions: notifications only reach the inbox.
notify-new-subscription = Subscribe
notify-event = Event
notify-all-events = All events
notify-group-release = All releases
notify-group-library = All library changes
notify-group-social = All social events
notify-group-boardgame = All board game events
notify-group-integration = All integration events
notify-group-import = All import events
notify-group-admin = All admin events
notify-channel = Channel
notify-all-channels = All channels
notify-mode = Delivery
notify-mode-immediate = Right away
notify-mode-digest = Daily digest
notify-digest-at = Digest at
notify-quiet-from = Quiet from
notify-quiet-to = Quiet until
notify-timezone = Timezone
notify-subscribe = Subscribe
notify-subscribed = Subscribed.
notify-default = Default
notify-muted = Muted
notify-log = Delivery log
notify-log-empty = Nothing was sent yet.
notify-state-queued = Queued
notify-state-digest = In digest
notify-state-sent = Sent
notify-state-failed = Failed
notify-state-suppressed = Suppressed
notify-error-invalid = Please check the fields.
notify-error-channel_error = The channel did not accept the message. Check its settings.
notify-error-forbidden = You may not do that.
notify-error-not_found = Not found.
notify-error-unavailable = Notifications are not enabled on this server.
