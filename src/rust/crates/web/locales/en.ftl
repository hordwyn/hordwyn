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
