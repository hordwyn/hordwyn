# English messages (Fluent). Keys are stable; brands can override any key in brand.toml
# under [strings.en].

nav-library = Library
nav-lists = Lists
nav-sources = Data sources
nav-sign-in = Sign in
nav-sign-up = Create account
nav-sign-out = Sign out
nav-main = Main navigation
skip-to-content = Skip to content

home-title = Track what you watch, read and play
home-lead = Movies, shows, books, games and more, with the people you share them with.

auth-username = Username
auth-display-name = Display name (optional)
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
error-generic = Something went wrong. Please try again.

library-title = Library
library-empty = Your library is empty
library-empty-body = Search for a movie, show, book or game and add it. Ctrl+K opens the search on every page.
library-find = Find something
library-add = Add
library-group = Group
library-kinds = Kinds
library-kinds-all = All kinds
library-status = Status
library-status-all = All
library-view = View
library-view-grid = Grid
library-view-list = List
library-sort = Sort
library-sort-last-activity = Last activity
library-sort-title = Title
library-sort-added = Added
library-sort-year = Year
library-sort-rating = Your rating
library-sort-plays = Plays
library-sort-reverse = Reverse the order
library-filter = Filter by title or note
library-filter-placeholder = Filter...
library-filters = Filters
library-filters-active = { $count ->
    [one] one active
   *[other] { $count } active
}
library-tags = Tags
library-tags-none = No tags yet. Add them from the menu on a card.
library-facets = Filter { $kind } by
library-facets-pick = Choose one kind above to filter by genre and other details.
library-facet-any = Any
library-active = Active filters
library-remove-filter = Remove filter { $filter }
library-filter-tag = Tag: { $tag }
library-filter-facet = { $name }: { $value }
library-filter-text = Text: { $text }
library-clear = Clear filters
library-no-match = Nothing matches these filters
library-no-match-body = Active: { $filters }. Clear the filters or look in another status.
library-show-status = Show { $status } instead
library-results = Entries
library-showing = Showing { $shown } of { $total }
library-error = Your library could not be loaded.
library-retry = Try again
library-loading = Loading your library...
library-actions = Actions for { $title }
library-open = Open
library-mark = Mark as { $status }
library-edit-tags = Edit tags
library-tags-title = Tags for { $title }
library-tag-new = New tag
library-tag-add = Add tag
library-tag-remove = Remove tag { $tag }
library-col-title = Title
library-col-kind = Kind
library-col-progress = Progress
library-col-rating = Rating
library-col-tags = Tags
library-col-activity = Last activity
library-col-next = Next step
library-rating-value = { $stars } of 5
library-no-rating = Not rated
library-status-changed = Marked as { $status }
library-next-step-named = { $step }: { $title }
library-logged = Logged: { $step }
library-undo = Undo
library-undone = Undone
library-step-failed = That did not work: { $error }
library-progress = Progress
library-progress-episodes = { $done } of { $total } episodes
library-progress-pages = Page { $page } of { $pages }
library-progress-percent = { $percent }% done
library-facet-genres = Genre
library-facet-authors = Author
library-facet-narrators = Narrator
library-facet-language = Language
library-facet-languages = Language
library-facet-original_language = Original language
library-facet-categories = Category
library-facet-artists = Artist
library-facet-release_type = Release type
library-facet-platforms = Platform
library-facet-game_modes = Game mode
library-facet-tags = Source tag
library-facet-subjects = Subject
library-facet-publishing_status = Publishing status
library-facet-publisher = Publisher
library-facet-series = Series
library-facet-network = Network
library-facet-airing_status = Airing status
library-facet-format = Format
library-facet-studios = Studio
library-facet-players_min = Minimum players
library-facet-players_max = Maximum players
library-facet-players_best = Best player count
library-facet-mechanics = Mechanic
library-facet-weight = Weight
library-count = { $count ->
    [one] One item
   *[other] { $count } items
}

status-planned = Planned
status-in_progress = In progress
status-completed = Completed
status-paused = Paused
status-dropped = Dropped
status-endless = Endless

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

# activity groups (design.md 3.1)
kind-group-all = All
kind-group-watch = Watch
kind-group-read = Read
kind-group-listen = Listen
kind-group-play = Play
kind-group-other = Other

# next step buttons (design.md 3.4); the server names the key
next-step-watched = Watched
next-step-watched-again = Watched again
next-step-finished = Finished
next-step-finished-again = Finished again
next-step-episode = Watched S{ $season } E{ $episode }
next-step-episode-listened = Listened to S{ $season } E{ $episode }
next-step-caught-up = Up to date
next-step-update-page = Update page
next-step-update-position = Update position
next-step-log-play = Log a play
next-step-log-session = Log a session

item-add = Add to library
item-rating = Your rating
item-rate = Rate { $stars } of 5
item-history = History
item-history-empty = Nothing logged yet.
item-loading-details = Fetching details...
item-source = Source: { $name }
item-cover-alt = Cover of { $title }
item-lists = Add to list
item-original-title = Original title: { $title }
item-facts = Entry facts
item-status = Status
item-progress = Progress
item-not-started = Not started
item-not-rated = Not rated
item-rated = { $stars } of 5
item-more-facts = More facts
item-more-facts-summary = Added, started, last logged
item-added = Added
item-started = Started
item-last = Last logged
item-plays = Plays
item-rewatches = Rewatches
item-plays-count = { $count ->
    [one] One play
   *[other] { $count } plays
}
item-actions = More actions for { $title }
item-action-bar = Next step
item-with = With...
item-with-named = Log with details: { $title }
item-tags = Tags
item-edit-tags = Edit tags
item-lists-title = Lists
item-added-to-list = Added to { $list }.
item-sections = Sections
item-section-episodes = Episodes
item-section-about = About
item-section-notes = Notes
item-about-empty = No details yet.
item-ids = Identifiers:
item-note = Private note
item-note-hint = Only you see this note.
item-note-save = Save note
item-note-saved = Note saved.

# log sheets and inline loggers of the item page (design.md 5.4)
log-sheet-title = Log with details
log-when = When
log-today = Today
log-yesterday = Yesterday
log-pick = Pick a date
log-date = Date
log-people = With
log-players = Players
log-people-add = Add a person
log-people-create = Add "{ $name }"
log-people-none = Nobody with that name
log-more = More
log-more-watched = Rating and note
log-more-play = Co-op, duration, place, notes
log-note = Note on this title (private)
log-rewatch = { $count ->
    [one] Finished once before: this counts as a rewatch.
   *[other] Finished { $count } times before: this counts as a rewatch.
}
log-save = Save
log-discard-question = Discard your changes?
log-discard = Discard
log-keep = Keep editing
log-score = Score of { $name }
log-winner = Winner
log-winner-auto = Highest score
log-coop = Co-op game
log-coop-won = We won
log-duration = Duration in minutes
log-location = Place
log-notes = Notes
log-play-step = Play
log-page = Current page
log-of-pages = of { $pages }
log-update = Update
log-plus-pages = +{ $count }
log-finished = Finished
log-position = Position
log-position-hint = As h:mm:ss, or minutes.
log-of-time = of { $time }
log-plus-minutes = +{ $count } min
log-error-date-missing = Pick a date.
log-error-date-future = The date is in the future.
log-error-score = Scores are numbers, for example 42 or 12.5.
log-error-duration = The duration is a whole number of minutes.
log-error-page = Enter a page number from 1 on.
log-error-page-beyond = This has { $pages } pages.
log-error-position = Enter a position as h:mm:ss, within the length.

# the Episodes tab of the item page (design.md 6.3)
episodes-seasons = Seasons
episodes-season = Season { $season }
episodes-count = { $watched }/{ $total }
episodes-episode = Episode { $episode }
episodes-mark-season = Mark season watched
episodes-mark = Mark S{ $season } E{ $episode } watched
episodes-mark-again = Watch S{ $season } E{ $episode } again
episodes-watched = Watched
episodes-again = Again
episodes-next = Next up
episodes-row-menu = More for S{ $season } E{ $episode }
episodes-up-to-here = Mark up to here
episodes-with = Watched with...
episodes-ask = { $count ->
    [one] Also mark the earlier episode as watched?
   *[other] Also mark the { $count } earlier episodes as watched?
}
episodes-ask-all = Mark all { $count }
episodes-ask-one = Only S{ $season } E{ $episode }
episodes-remember = Remember for this show
episodes-logged = { $count ->
    [one] One episode
   *[other] { $count } episodes
}
episodes-none = No episodes known yet.

search-title = Search
search-label = Search the catalog
search-placeholder = Title, author, studio...
search-empty = No results yet.
search-local = In your instance
search-waiting = Asking { $sources }...

companions-title = People
companions-lead = People you watch, read and play with. Only you see this list.
companions-new = Name
companions-add = Add person

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

auth-invited = You were invited: your account is linked to the person who invited you, and you see the sessions you were part of.
companions-choose = Choose...
companions-companion = Person
companions-linked = Linked
companions-invite = Invite
companions-invite-link = Send this single-use link:
companions-unlink = Unlink
companions-requests = Requests for you
companions-request-text = { $from } says "{ $name }" on their People page is you.
companions-accept = Accept
companions-decline = Decline
companions-link-title = Link to an account
companions-link-lead = If someone already has an account here, ask them to confirm it is them. They see the sessions you share with them.
companions-link-submit = Send link request
companions-link-sent = Link request sent.
companions-merge-title = Merge duplicates
companions-merge-lead = The duplicate is folded into the person you keep. Logged sessions stay as they are.
companions-merge-keep = Keep
companions-merge-remove = Duplicate
companions-merge-submit = Merge
privacy-title = Privacy
privacy-share-sessions = Linked participants see the sessions I log
sessions-by = by { $name }
sessions-claim = Copy to my log
sessions-claimed = Copied to your log.
lists-share = Share by link
lists-share-link = Anyone with this link can see the list:
shared-list-missing = This list is not shared (any more).
sharing-error-conflict = That is not possible for this person.
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

library-more = Show more

nav-integrations = Integrations
integrations-title = Integrations
integrations-lead = Media servers and scripts can record what you watched by posting to a webhook. Create a token for each sender; the docs ("Integrations") describe the format and a Jellyfin setup.
integrations-name = Name, e.g. Jellyfin
integrations-add = Create token
integrations-created = New token: copy it now, it is not shown again.
integrations-url = Webhook URL:
integrations-token = Token:
integrations-example = Example:
integrations-empty = No integrations yet.
integrations-never = never
integrations-error = Last call failed
integrations-revoke = Revoke
auth-error-too_many_attempts = Too many failed sign-ins. Please wait a few minutes.
auth-error-username_too_short = Use at least 3 characters for the username.
auth-error-username_too_long = Use at most 32 characters for the username.
auth-error-username_space = The username has a space. Use - or _ instead.
auth-error-username_invalid_chars = Use only a-z, 0-9 and _ . - in the username.
auth-error-username_taken = That username is already taken.
auth-error-display_name_too_long = Use at most 100 characters for the display name.
auth-error-password_too_short = Use at least 10 characters for the password.
auth-error-password_too_long = Use at most 1024 characters for the password.
auth-error-search_unavailable = Search is not available right now. Try again in a moment.

# ---- UX phase 0: app shell (design.md 3.2, 6.15) ----
nav-home = Home
nav-history = History
nav-people = People
nav-you = You
shell-search = Search or add...
shell-ctrl = Ctrl
shell-add = Add
shell-notifications = { $count ->
    [0] Notifications
   *[other] Notifications, { $count } unread
}
shell-unread = { $count } unread
shell-account = Account menu for { $name }
shell-admin = Admin
shell-theme = Theme
shell-theme-system = System
shell-theme-light = Light
shell-theme-dark = Dark
shell-shortcuts = Keyboard shortcuts
shell-help = Help and support
shell-key-search = Search or add
shell-key-add = Add something
shell-key-help = Show the keyboard shortcuts
shell-key-close = Close a dialog or menu
shell-offline = You are offline. Changes are kept and sent when you are back.
shell-dismiss = Dismiss
notfound-title = There is nothing at this address
notfound-lead = The link may be old or mistyped.
# ---- end app shell ----

# ---- UX phase 0: form discipline on sign-up and sign-in (design.md 5.2, 2.4) ----
auth-username-hint = Used in links and for sign-in. { $min } to { $max } characters.
auth-username-rule-length = { $min } to { $max } characters
auth-username-rule-space = No spaces
auth-username-rule-chars = Only a-z, 0-9 and _ . -
auth-rule-met = Met
auth-rule-unmet = Not met
auth-username-saved-as = Will be saved as { $username } (capitals become lowercase)
auth-username-saved-as-trimmed = Will be saved as { $username } (spaces at the start and end are removed)
auth-username-available = '{ $username }' is available.
auth-username-required = Enter a username.
auth-username-too-short = '{ $value }' is too short: use at least { $min } characters.
auth-username-too-long = '{ $value }' has { $count } characters; use at most { $max }.
auth-username-space = '{ $value }' has a space. Try '{ $dash }' or '{ $underscore }'.
auth-username-invalid-chars = '{ $value }' contains characters that are not allowed: { $chars }. Use only a-z, 0-9 and _ . -
auth-username-taken = { $count ->
    [0] '{ $username }' is already taken.
   *[other] '{ $username }' is already taken. Free: { $suggestions }.
}
auth-username-suggestions = Suggestions
auth-username-use = Use { $username }
auth-display-name-hint = How others see you. Leave it empty to use your username.
auth-display-name-too-long = The display name has { $count } characters; use at most { $max }.
auth-password-hint = At least { $min } characters. No other rules; a long phrase is best. Pasting is fine.
auth-password-rule-length = At least { $min } characters ({ $count } so far)
auth-password-required = Enter a password.
auth-password-too-short = { $count ->
    [one] The password has 1 character; it needs at least { $min }.
   *[other] The password has { $count } characters; it needs at least { $min }.
}
auth-password-too-long = The password has { $count } characters; use at most { $max }.
auth-show = Show
auth-error-summary = { $count ->
    [one] There is one problem
   *[other] There are { $count } problems
}
auth-sign-in-username-required = Enter your username.
auth-sign-in-password-required = Enter your password.
auth-totp-title = Enter your one-time code
auth-totp-lead = Open your authenticator app and type the code it shows for this account.
auth-totp-required = Enter the one-time code.
auth-back = Back
# ---- end form discipline ----

# ---- UX phase 0: i18n (DEC-171, DEC-200, DEC-201) ----
shell-language = Language
shell-language-auto = Automatic
shell-key-esc = Esc
# Dates: CLDR medium date (en: MMM d, y), short time (h:mm a); times are UTC (DEC-200).
month-short-1 = Jan
month-short-2 = Feb
month-short-3 = Mar
month-short-4 = Apr
month-short-5 = May
month-short-6 = Jun
month-short-7 = Jul
month-short-8 = Aug
month-short-9 = Sep
month-short-10 = Oct
month-short-11 = Nov
month-short-12 = Dec
time-am = AM
time-pm = PM
datetime-utc = { $date }, { $time } UTC
progress-finished = Completed
progress-episode = Season { $season }, episode { $episode }
progress-page = Page { $page }
progress-chapter = Chapter { $chapter }
progress-volume = Volume { $volume }
progress-position = At { $time }
progress-session = Session logged
lists-share-named = Share by link: { $name }
companions-name-self = { $name } (you)
companions-invite-named = Invite { $name }
companions-unlink-named = Unlink { $name }
integrations-last-call = Last call: { $when }
integrations-revoke-named = Revoke { $name }
notify-mark-read-named = Mark as read: { $title }
notify-test-named = Test { $name }
notify-delete-named = Delete { $name }
notify-mode-digest-at = Daily digest at { $time }
notify-event-release-item_released = A title is released
notify-event-release-episode = New episode
notify-event-release-chapter = New chapter
notify-event-release-date_changed = Release date changed
notify-event-library-status_changed = Status changed
notify-event-library-metadata_changed = Details updated
notify-event-social-link_request = Link request
notify-event-social-link_accepted = Link accepted
notify-event-social-session_shared = Session shared with you
notify-event-social-list_shared = List shared with you
notify-event-boardgame-loan_due = Lent game due back
notify-event-boardgame-loan_overdue = Lent game overdue
notify-event-boardgame-wishlist_owned = Wishlist game owned by someone you know
notify-event-integration-play_recorded = Play recorded by a sender
notify-event-integration-failing = Sender stopped working
notify-event-import-finished = Import finished
notify-event-import-failed = Import failed
notify-event-admin-storage_high = Storage almost full (admin)
notify-event-admin-storage_full = Storage full (admin)
notify-event-admin-connector_down = Data source unavailable (admin)
notify-event-admin-signup = New account (admin)
notify-event-admin-job_backlog = Jobs piling up (admin)
notify-event-admin-source_requested = Data source requested (admin)
# ---- end i18n ----

# ---- UX phase 0 review (docs/reviews/2026-11-06-ux0-review.md) ----
auth-lockout-minutes = { $minutes ->
    [one] Too many failed sign-ins. Try again in 1 minute.
   *[other] Too many failed sign-ins. Try again in { $minutes } minutes.
}
auth-check-throttled = Too many checks right now; you can still create the account.
auth-offline = You are offline
legal-nav = Legal
legal-imprint = Imprint
legal-privacy = Privacy
legal-terms = Terms
history-today = Today
history-yesterday = Yesterday
history-day = { $weekday }, { $date }
weekday-monday = Monday
weekday-tuesday = Tuesday
weekday-wednesday = Wednesday
weekday-thursday = Thursday
weekday-friday = Friday
weekday-saturday = Saturday
weekday-sunday = Sunday
stats-bucket-month = { $month } { $year }
nav-stats = Stats
home-greeting = Welcome back, { $name }
home-error = Home could not be loaded.
home-loading = Loading your home page...
home-empty = Nothing here yet
home-empty-body = Add the first thing you are watching, reading or playing: search for a title and add it to your library.
home-continue = Continue
home-all-in-progress = All in progress
home-up-next = Up next
home-all-planned = All planned
home-reason-next-in-series = Next in the series
home-reason-planned = Planned
home-activity = Recent activity
home-all-history = All history
home-stats = All-time stats
home-all-stats = All stats
stats-title = Stats
stats-groups = Groups
stats-bucket = Time unit
stats-by-month = By month
stats-by-year = By year
stats-error = The stats could not be loaded.
stats-totals = All-time totals
stats-entries = Entries
stats-empty = Nothing logged here yet
stats-empty-body = Once you log progress, your activity over time shows up here.
stats-chart-summary = { $measure } in { $group }, { $from } to { $to }: { $total } in total, most in { $peak }.
stats-chart-summary-empty = { $measure } in { $group }, { $from } to { $to }: none yet.
stats-totals-summary = { $group }: { $parts }
stats-totals-part-planned = { $count } planned
stats-totals-part-in_progress = { $count } in progress
stats-totals-part-completed = { $count } completed
stats-totals-part-paused = { $count } paused
stats-totals-part-dropped = { $count } dropped
chart-value = { $label }: { $value }
stats-show-table = Show as a table
stats-col-month = Month
stats-col-year = Year
stats-measure-events = Activity
stats-measure-completions = Completed entries
stats-measure-finished = Watched
stats-measure-episodes = Episodes
stats-measure-pages = Pages
stats-measure-plays = Plays
stats-measure-minutes = Minutes
stats-count-events = { $count ->
    [one] One logged step
   *[other] { $count } logged steps
}
stats-count-completions = { $count ->
    [one] One entry completed
   *[other] { $count } entries completed
}
stats-count-finished = { $count ->
    [one] Watched once
   *[other] Watched { $count } times
}
stats-count-episodes = { $count ->
    [one] One episode
   *[other] { $count } episodes
}
stats-count-pages = { $count ->
    [one] One page
   *[other] { $count } pages
}
stats-count-plays = { $count ->
    [one] One play
   *[other] { $count } plays
}
stats-count-minutes = { $count ->
    [one] One minute
   *[other] { $count } minutes
}
history-title = History
history-lead = Everything you logged, newest first.
history-stream = Activity by day
history-filters = Filters
history-filters-summary = { $count ->
    [0] Kind, person, type, shared with me, year
    [one] One filter set
   *[other] { $count } filters set
}
history-filter-kind = Kind
history-filter-person = Person
history-filter-type = Type
history-filter-year = Year
history-filter-shared = Only shared with me
history-kind-all = All kinds
history-person-all = With anyone
history-type-all = Everything
history-type-progress = Progress
history-type-session = Sessions
history-year-all = All years
history-session = Session logged
history-with = with { $names }
history-time-utc = { $time } UTC
history-shared = Shared
history-undo-named = Undo: { $title }
history-copy-named = Copy to my log: { $title }
history-error = The history could not be loaded.
history-loading = Loading your history...
history-empty = Nothing logged yet
history-empty-body = Logging happens from an item page or with the next-step buttons; everything lands here.
history-no-match = Nothing logged in this view
history-no-match-body = No activity matches these filters.
history-side-title = On this page
history-side-range = { $from } to { $to }
history-side-rows = Entries in the stream
history-side-finished = Completed
history-side-episodes = Episodes
history-side-sessions = Sessions
history-side-shared = Shared with you

# ---- UX phase 2: server codes and words (contracts::admin) ----
# Error codes of the admin area, the wizard and data sources (`admin-error-<code>`).
auth-error-account_disabled = This account is disabled. Ask an admin.
auth-error-totp_locked = Your authenticator cannot be checked right now. Ask your admin.
auth-error-reauth_required = Confirm it is you to make this change.
admin-error-reauth_required = Confirm it is you to make this change.
admin-error-last_admin = This is the last admin. Make someone else admin first.
admin-error-self_role_change = You cannot change your own role.
admin-error-self_disable = You cannot disable your own account.
admin-error-self_delete = You cannot delete your own account here.
admin-error-secure_cookies_need_https = Secure cookies need an https public address, or nobody could sign in.
admin-error-fixed_by_config = This value is set by the configuration and cannot be changed here.
admin-error-invalid_setting = This value is not valid.
admin-error-unknown_setting = This setting does not exist.
admin-error-unknown_source = This data source does not exist.
admin-error-missing_field = Fill in every field.
admin-error-unknown_field = This field does not belong to the data source.
admin-error-setup_done = The setup is complete.
admin-error-forbidden = Only admins can do this.
admin-error-not_found = Not found.
admin-error-conflict = That is not possible right now.
admin-error-internal = Something went wrong. Please try again.
# Warnings of the settings pages (`admin-warning-<id>`).
admin-warning-public_url_not_https = The public address does not use https. Secure cookies cannot be turned on.
admin-warning-secure_cookies_over_http = Secure cookies are on, but the public address does not use https.
# Setting labels (`admin-setting-<key with . replaced by _>`).
admin-setting-instance_hosting = How you host
admin-setting-auth_signup = Sign-up
admin-setting-auth_session_days = Session length (days)
admin-setting-auth_secure_cookies = Secure cookies
admin-setting-storage_max_total_bytes = Storage limit
admin-setting-storage_high_watermark = Start clean-up at
admin-setting-storage_low_watermark = Clean up down to
admin-setting-env = Set by environment variable { $var }
admin-setting-file = Set in the configuration file
admin-setting-shadowed = Stored value ignored while { $var } is set
admin-setting-next-session = Applies from the next sign-in
# States of data sources (`source-state-<state>`, design.md 4.4).
source-state-working = Working
source-state-not_set_up = Not set up
source-state-key_rejected = Key rejected
source-state-key_unavailable = Key unavailable
source-state-rate_limited = Rate limited
source-state-circuit_open = Not answering
source-state-disabled = Disabled by configuration
source-state-needs_account = Needs your account
source-state-unavailable = Temporarily unavailable
source-state-slow = Slow right now
source-state-retrying = Not answering, retrying
# Results of Test (`source-test-<result>`).
source-test-ok = Works ({ $ms } ms).
source-test-key_rejected = Key rejected.
source-test-unreachable = Could not reach the service.
source-test-rate_limited = Rate limited. Try again later.
source-test-circuit_open = Paused after repeated failures. Try again later.
source-test-missing_key = No key set.
source-test-failed = The test failed.
# Credential fields (`admin-source-field-<name>`).
admin-source-field-api_key = API key
admin-source-field-api_secret = API secret
admin-source-field-client_id = Client ID
admin-source-field-client_secret = Client secret
admin-source-field-pin = Subscriber PIN
# Health rows of Admin > Overview (`admin-health-<id>-<detail>`).
admin-health-database-reachable = Reachable
admin-health-database-unreachable = Not reachable
admin-health-search-backend = Running on { $backend }
admin-health-search-fallback = Meilisearch unavailable, using the database
admin-health-storage-used = { $used } of { $cap } bytes used
admin-health-storage-high = Almost full: { $used } of { $cap } bytes
admin-health-storage-full = Full: { $used } of { $cap } bytes
admin-health-storage-unreadable = The storage report failed
admin-health-storage-off = Not configured
admin-health-jobs-queued = { $queued } waiting
admin-health-jobs-failed = { $failed } failed, { $queued } waiting
admin-health-jobs-unreadable = The job queue cannot be read
admin-health-jobs-off = Not configured
admin-health-mail-configured = Configured
admin-health-mail-off = Not configured
admin-health-data_sources-working = { $working } of { $total } working
admin-health-data_sources-some_not_working = { $working } of { $total } working
admin-health-data_sources-none_working = No data source works
admin-health-identity_provider-configured = Configured
admin-health-identity_provider-off = Not configured
admin-health-secrets-active = Key v{ $version } active
admin-health-secrets-locked = { $count } secrets cannot be decrypted
admin-health-secrets-rotation_due = Key v{ $version } is older than two years: consider a new one
admin-health-secrets-rewrap = { $count } secrets still on an older key
# Done step of the setup wizard (`setup-checklist-<id>`).
setup-checklist-first_item = Add your first item
setup-checklist-invite = Invite your household
setup-checklist-media_server = Connect your media server
setup-checklist-notification_defaults = Set notification defaults
setup-checklist-backup_key = Back up the secrets key together with the database
setup-checklist-identity_provider = Connect an identity provider
# Audit log actions (`admin-audit-<action with . replaced by _>`).
admin-audit-setting_changed = Setting changed
admin-audit-secret_replaced = Key replaced
admin-audit-secret_cleared = Key removed
admin-audit-secrets_rewrapped = Secrets re-encrypted
admin-audit-role_changed = Role changed
admin-audit-user_disabled = Account disabled
admin-audit-user_enabled = Account enabled
admin-audit-user_deleted = Account deleted
admin-audit-invite_created = Invite created
admin-audit-invite_revoked = Invite revoked
admin-audit-licence_recorded = Licence recorded
admin-audit-wizard_completed = Setup completed
admin-audit-eviction_run = Storage cleaned up
admin-audit-job_retried = Job retried
admin-audit-job_discarded = Job discarded

# ---- UX phase 2: settings area and admin area (addendum-1 3.4, design.md 6.12, 6.13) ----
nav-settings = Settings
settings-nav-label = Settings
settings-group-you = You
settings-group-admin = Instance (admin)
settings-nav-account = Account
settings-nav-privacy = Privacy
settings-privacy-lead = Who sees what you log.
settings-account-lead = Your name, how this app looks for you, and how you sign in.
settings-account-display-name = Display name
settings-account-username = Username
settings-account-username-hint = Used for sign-in and in links. It cannot be changed.
settings-account-email = Email
settings-account-no-email = Not set
settings-signin = Sign-in
settings-signin-summary = Password and authenticator
settings-password = Change password
settings-password-current = Current password
settings-password-new = New password
settings-password-rule = At least 10 characters. Your other devices are signed out.
settings-password-save = Change password
settings-password-changed = Password changed. Your other devices are signed out.
settings-totp = Authenticator
settings-totp-on = An authenticator app is set up. Sign-in asks for its code.
settings-totp-off = No authenticator yet. With one, sign-in also asks for a code from your phone.
settings-totp-start = Set up an authenticator
settings-totp-scan = Add this key to your authenticator app (or open the link on your phone), then enter the code it shows.
settings-totp-secret = Key
settings-totp-link = Link for authenticator apps
settings-totp-code = Code from the app
settings-totp-confirm = Turn on
settings-codes-title = Your recovery codes
settings-codes-lead = Each code signs you in once if you lose your phone. Keep them somewhere safe.
settings-codes-copy = All codes
settings-codes-once = Shown only now. They are stored as fingerprints and cannot be shown again.
settings-copy = Copy
settings-copied = Copied
settings-replace = Replace
settings-sources-lead = Where titles, covers and episodes come from, and what you can do when one is missing.
settings-sources-missing = Some kinds have no data source yet
settings-sources-no-source = { $group }: no data source yet
settings-sources-set-up = Set up
settings-ask-admin = Ask admin
settings-ask-admin-named = Ask admin about { $name }
settings-asked = Your admin has been asked.
settings-credential-set = Your key is set (since { $date }).
settings-credential-locked = Your key is temporarily unavailable.
settings-credential-hint = Stored encrypted, never shown again.
settings-credential-save = Save
settings-credential-saved = Saved.
settings-credential-clear = Remove

reauth-title = Confirm it is you
reauth-lead = This change affects how people sign in. Enter your password to continue.
reauth-password = Password
reauth-code = Authenticator code or recovery code
reauth-code-hint = The 6 digits from your app, or one of your recovery codes.
reauth-confirm = Confirm
reauth-cancel = Cancel
reauth-confirm-now = Confirm it is you
reauth-window-fresh = { $minutes ->
    [one] Confirmed for 1 more minute.
   *[other] Confirmed for { $minutes } more minutes.
}
reauth-window-stale = Not confirmed recently. Changes to sign-in settings ask for your password first.

admin-only-title = Admins only
admin-only-lead = This page belongs to the instance admin area. Your own settings are under Settings.
admin-only-back = Go to your settings
admin-save = Save
admin-saved = Saved.
admin-setting-read-only = Read-only
admin-nav-overview = Overview
admin-nav-users = Users and sign-up
admin-nav-sources = Data sources
admin-nav-storage = Storage and jobs
admin-nav-notifications = Notification defaults
admin-nav-brand = Branding
admin-nav-security = Security and sessions
admin-nav-audit = Audit log
admin-status-ok = Working
admin-status-warning = Needs attention
admin-status-error = Not working
admin-status-off = Off
admin-health-name-database = Database
admin-health-name-search = Search
admin-health-name-storage = Storage
admin-health-name-jobs = Background jobs
admin-health-name-mail = Mail
admin-health-name-data_sources = Data sources
admin-health-name-identity_provider = Identity provider
admin-health-name-secrets = Secrets at rest
admin-overview-lead = What members cannot see: the parts this instance depends on.
admin-overview-attention = { $count ->
    [one] One thing needs attention
   *[other] { $count } things need attention
}
admin-overview-fix = Open
admin-overview-hosted = Hosted mode: record the licence agreement of each data source.
admin-overview-health = Health
admin-overview-open = Details: { $name }
admin-overview-details = Details
admin-overview-recent = Recent admin actions
admin-overview-recent-none = No admin actions yet.
admin-overview-audit = Full audit log
admin-overview-signups = Recent sign-ups
admin-overview-brand = { $name }, default language { $locale }. Read-only.
admin-overview-brand-link = Branding and contrast
admin-users-lead = Who has an account, who is admin, and how new people get in.
admin-users-signup = Who can create an account
admin-signup-open = Anyone
admin-signup-open-hint = Anyone who can reach this address can create an account.
admin-signup-invite = With an invite
admin-signup-invite-hint = People need an invite link from an admin or from a member's People page.
admin-signup-closed = Nobody
admin-signup-closed-hint = No new accounts. Invites stop working too.
admin-invites-title = { $count ->
    [one] Open invites (1)
   *[other] Open invites ({ $count })
}
admin-invites-summary = Create an invite link, revoke one
admin-invites-list = Invites
admin-invite-email = Email (optional)
admin-invite-note = Note (optional)
admin-invite-create = Create invite
admin-invite-link = Invite link
admin-invite-once = Copy it now: the link is shown only once. Send it to the person you invite.
admin-invite-anyone = Anyone with the link
admin-invite-line = Created { $created }, valid until { $expires }
admin-invite-state-open = Open
admin-invite-state-used = Used
admin-invite-state-revoked = Revoked
admin-invite-state-expired = Expired
admin-invite-revoke = Revoke
admin-invite-revoke-named = Revoke the invite for { $name }
admin-users-people = People
admin-users-person = Person
admin-users-role = Role
admin-users-signs-in = Signs in with
admin-users-last-seen = Last seen
admin-users-actions = Actions
admin-users-admin = Admin
admin-users-member = Member
admin-users-disabled = Disabled
admin-users-you = (you)
admin-users-never = Never
admin-users-with-password = Password
admin-users-with-totp = Authenticator
admin-users-with-oidc = Identity provider
admin-users-self = You cannot change your own role or account here. Another admin can.
admin-users-only-admin = You are the only admin. Make someone else admin before anything can change your role or account.
admin-users-promote = Make admin
admin-users-promote-named = Make admin: { $name }
admin-users-demote = Remove admin
admin-users-demote-named = Remove admin: { $name }
admin-users-disable = Disable
admin-users-disable-named = Disable: { $name }
admin-users-enable = Enable
admin-users-enable-named = Enable: { $name }
admin-users-delete = Delete
admin-users-delete-named = Delete: { $name }
admin-users-delete-title = Delete { $name }?
admin-users-delete-body = The account and its library are deleted. Sessions logged with others stay in their history, with this person as a companion.
admin-sources-lead = Where titles, covers and episodes come from. Keys are stored encrypted and never shown again.
admin-sources-attention = { $count ->
    [one] One data source needs attention
   *[other] { $count } data sources need attention
}
admin-sources-licence-banner = Hosted mode: record for each data source that its licence agreement is on file.
admin-sources-source = Source
admin-sources-kinds = Kinds
admin-sources-status = Status
admin-sources-set-up = Set up
admin-sources-set-up-named = Set up { $name }
admin-sources-details = Details
admin-sources-details-named = Details of { $name }
admin-sources-calls = { $calls } calls in the last 24 hours, { $limited } rate limited
admin-sources-last-ok = Last answer { $when }
admin-sources-last-error = Last error { $when }: { $error }
admin-sources-no-key = This source needs no key.
admin-sources-shadowed = A stored key is ignored while the environment variable is set.
admin-sources-key-hint = Stored encrypted, never shown again.
admin-sources-set-by = Set { $date } by { $name }
admin-sources-set-on = Set { $date }
admin-sources-locked = Stored, but the key to decrypt it is not loaded.
admin-sources-test = Test
admin-sources-sample = First result: { $title }.
admin-sources-clear = Remove key
admin-sources-clear-title = Remove the key of { $name }?
admin-sources-clear-body = Searches stop using this source until a new key is saved.
admin-sources-licence = Agreement on file
admin-sources-licence-hint = Needed when you host for other people: the provider's terms allow this use.
admin-sources-attribution-preview = Attribution
admin-sources-terms = Terms of use
admin-sources-attribution = Attribution list of all data sources
admin-storage-lead = Images and caches on disk or S3, and the background work of this instance.
admin-storage-full = Storage is full: new images are not stored until space is freed.
admin-storage-title = Storage
admin-storage-used = { $used } of { $cap } used
admin-storage-backend = Backend: { $backend }
admin-storage-pinned = Kept (covers in libraries)
admin-storage-cache = Cache
admin-storage-evictable = Can be freed now
admin-storage-high = Clean-up starts at
admin-storage-low = Clean-up stops at
admin-storage-evict = Run eviction now
admin-storage-evicted = Freed { $bytes } ({ $objects } files).
admin-storage-budget = Budget
admin-storage-limit-hint = For example 20 GiB.
admin-storage-percent-hint = Percent of the limit.
admin-storage-invalid = Check the value of { $field }.
admin-jobs-title = Background jobs
admin-jobs-lanes = Queues
admin-jobs-lane = Queue
admin-jobs-queued = Waiting
admin-jobs-running = Running
admin-jobs-dead = Failed
admin-jobs-failed = Failed jobs
admin-jobs-failed-attention = { $count ->
    [one] One job failed.
   *[other] { $count } jobs failed.
}
admin-jobs-none-failed = No failed jobs.
admin-jobs-failed-line = { $attempts } attempts, last { $when }
admin-jobs-retry = Retry
admin-jobs-retry-named = Retry { $name }
admin-jobs-discard = Discard
admin-jobs-discard-named = Discard { $name }
admin-jobs-discard-title = Discard { $name }?
admin-jobs-discard-body = The job is removed and will not run again.
admin-jobs-cron = Scheduled
admin-jobs-name = Name
admin-jobs-schedule = Schedule
admin-jobs-next = Next run
admin-defaults-lead = Rules that apply to everyone who has not set their own. Members change theirs under Settings, Notifications.
admin-defaults-event = Event
admin-defaults-delivery = Delivery
admin-defaults-none = No instance defaults are set.
admin-defaults-events = Events on this instance
admin-defaults-events-summary = { $count ->
    [one] 1 event
   *[other] { $count } events
}
admin-defaults-for-admins = admins
admin-defaults-for-everyone = everyone
admin-brand-lead = Name, language and colours of this instance. Read-only here.
admin-brand-name = Name
admin-brand-short-name = Short name
admin-brand-locale = Default language
admin-brand-dir = Brand directory
admin-brand-accent = Accent colour
admin-brand-light = Light theme
admin-brand-dark = Dark theme
admin-brand-sample = Aa
admin-brand-ratio = Contrast { $ratio }:1
admin-brand-unreadable = Contrast cannot be checked
admin-brand-pass = Meets AA
admin-brand-fail = Below AA (4.5:1)
admin-brand-edit = To change the brand, edit brand.toml in { $dir } and restart.
admin-security-lead = How long sessions last, how cookies are sent, and the key that protects stored secrets.
admin-security-window = Your confirmation
admin-security-window-rule = Sign-in related changes ask for your password again after { $slide } minutes without such a change, at the latest { $cap } minutes after you confirmed.
admin-security-sessions = Sessions
admin-security-days-hint = 1 to 365 days.
admin-security-days-invalid = Use 1 to 365 days.
admin-security-cookies-hint = Browsers send the session cookie over https only. Applies from the next sign-in, so your current session keeps working.
admin-secrets-title = Secrets at rest
admin-secrets-active = Active key
admin-secrets-version = Version { $version }
admin-secrets-source = Loaded from
admin-secrets-from-file = The key file
admin-secrets-from-env = The environment
admin-secrets-generated = A key file generated at the first start: back it up with the database
admin-secrets-ephemeral = A temporary key (this process only)
admin-secrets-file = Key file
admin-secrets-old = Secrets on an older key
admin-secrets-rewrap = Re-encrypt now
admin-secrets-rewrapped = Re-encrypted { $count } secrets; { $locked } could not be decrypted.
admin-secrets-versions = Key versions
admin-secrets-versions-summary = { $count ->
    [one] 1 version
   *[other] { $count } versions
}
admin-secrets-key = Key
admin-secrets-values = Secrets
admin-secrets-state = State
admin-secrets-state-active = Active
admin-secrets-state-loaded = Loaded
admin-secrets-state-missing = Not loaded
admin-secrets-state-removable = No longer used, can be removed
admin-audit-lead = Every admin action, who did it and what changed. Kept for 365 days. Secret values never appear.
admin-audit-filter-who = Who
admin-audit-filter-what = What
admin-audit-anyone = Anyone
admin-audit-anything = Any action
admin-audit-apply = Apply
admin-audit-csv = Export CSV
admin-audit-none = No admin actions match.
admin-audit-when = When
admin-audit-who = Who
admin-audit-what = What
admin-audit-detail = Detail
admin-audit-actor-system = system
admin-audit-actor-cli = command line

## Setup wizard (design.md 5.1)
setup-title = Setup
setup-title-named = Set up { $product }
setup-sign-in-lead = The setup continues once the admin signs in.
setup-progress = Setup progress
setup-step-done-label = completed
setup-step-account = Admin account
setup-step-hosting = How you host
setup-step-sources = Data sources
setup-step-done = Done
setup-account-title = Create the admin account
setup-account-lead = This first account runs the instance. You stay signed in.
setup-account-submit = Create admin account
setup-hosting-title = How do you host?
setup-hosting-lead = This changes defaults, not features. You can change it later under Admin.
setup-hosting-home = Home: free, for family and friends
setup-hosting-home-body = Sign-up by invite. No licence reminders.
setup-hosting-hosted = Hosted: you run this for other people
setup-hosting-hosted-body = Licence reminders on: record per data source that an agreement is on file. Close sign-up or invite people next.
setup-hosting-fixed = The hosting mode is set by the configuration.
setup-continue = Continue
setup-back = Back
setup-sources-title = Data sources
setup-sources-lead = Add the keys you have and test them. Everything is optional: you can add keys later under Admin, Data sources.
setup-sources-read-only = Every key comes from the environment. This is what the instance will use.
setup-sources-hosted = Free tiers of many sources (for example TMDB and IGDB) do not cover paid services. Record for each source whether an agreement is on file.
setup-done-title = Ready
setup-done-lead = A few things worth doing next. Each one opens its page.
setup-checklist = Next steps
setup-task-done = Done
setup-task-todo = To do
setup-key-file = File: { $file }
setup-go-home = Go to Home
setup-go-users = Go to Users and sign-up

## Admin nudge on Home (design.md 6.1)
home-nudge-setup = The setup is not finished yet.
home-nudge-setup-action = Continue the setup
home-nudge-no-source = { $count ->
    [one] { $groups } has no data source yet, so search finds nothing there.
   *[other] { $groups } have no data source yet, so search finds nothing there.
}
home-nudge-set-up = Set up data sources
home-nudge-failing = { $count ->
    [one] { $sources } is set up but does not answer.
   *[other] { $sources } are set up but do not answer.
}
home-nudge-check = Check data sources
home-nudge-snooze = Snooze for 7 days

## Search palette footer and no-results card (design.md 6.4)
search-searching = Searching...
search-none-title = Nothing matches "{ $query }"
search-none-library = Searched your library.
search-none-searched = Searched your library and { $sources }.
search-none-spelling = Check the spelling or try another title.
search-none-group = { $group }: not searched, no source is set up.
search-none-skipped = { $source }: not searched ({ $state }).
search-unset-title = Search is not set up yet
search-unset-admin = This instance has no data sources for this search, so nothing can be found online.
search-unset-member = There is no data source for this search yet. Ask your admin to add one.
search-scope = Search in
search-footer-label = Data sources
search-footer-source = { $source }: { $state }
search-footer-group = { $group }: no source set up

# UX phase 2 review, web pages (UX2-W1, W7, W12, W14, W21)
admin-sources-howto = How to get a key
admin-sources-howto-link = Open the developer page of { $name } (opens in a new tab)
admin-sources-howto-tmdb-1 = Sign in at themoviedb.org, or create a free account.
admin-sources-howto-tmdb-2 = In your account settings, open API and request a key for a personal, non-commercial project.
admin-sources-howto-tmdb-3 = Copy the API Read Access Token (the long one) and paste it here.
admin-sources-howto-igdb-1 = IGDB uses Twitch accounts: sign in to the Twitch developer console with one.
admin-sources-howto-igdb-2 = Register an application; http://localhost will do as its OAuth redirect URL.
admin-sources-howto-igdb-3 = Copy its Client ID, create a new Client secret, and paste both here.
admin-sources-howto-tvdb-1 = Create an account at thetvdb.com.
admin-sources-howto-tvdb-2 = On the API information page, apply for a key and describe this server as your project.
admin-sources-howto-tvdb-3 = Paste the API key here. If your key needs a subscriber PIN, each member adds their own under Settings, Data sources.
admin-sources-howto-podcastindex-1 = Open api.podcastindex.org and sign up for a free developer account.
admin-sources-howto-podcastindex-2 = Confirm your email address; the API key and the API secret arrive by email.
admin-sources-howto-podcastindex-3 = Paste the API key and the API secret here.
admin-sources-attention-line = { $name }: { $state }
admin-sources-attention-open = Open { $name }
settings-replace-named = Replace { $field }
admin-only-lead-signed-out = This page belongs to the instance admin area. Sign in with an admin account to open it.
admin-error-unauthorized = Your session has ended. Sign in again to continue.
auth-error-unauthorized = Your session has ended. Sign in again to continue.
auth-error-forbidden = You are not allowed to do this.
