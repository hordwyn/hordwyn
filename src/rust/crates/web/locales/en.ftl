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
auth-redirect-title = Signing in
auth-redirect-status = Taking you to { $provider }...
auth-redirect-continue = Continue to { $provider }
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
next-step-watched = Mark as watched
next-step-watched-again = Log a rewatch
next-step-finished = Mark as finished
next-step-finished-again = Log it again
next-step-episode = Mark S{ $season } E{ $episode } watched
next-step-episode-listened = Mark S{ $season } E{ $episode } listened
next-step-caught-up = Up to date
next-step-update-page = Update page
next-step-update-position = Update position
next-step-log-play = Log a play
next-step-log-session = Log a session
logged-step-watched = Watched
logged-step-watched-again = Watched again
logged-step-finished = Finished
logged-step-finished-again = Finished again
logged-step-episode = Watched S{ $season } E{ $episode }
logged-step-episode-listened = Listened to S{ $season } E{ $episode }
next-step-episode-short = S{ $season } E{ $episode }

item-add = Add to library
item-want-watch = Want to watch
item-want-read = Want to read
item-want-listen = Want to listen
item-want-play = Want to play
item-added-planned = Added to your library as Planned
item-logged-added = { $title }, added to your library
item-added-kept = Kept in your library: it has changed since you added it.
item-added-on = Added on { $date }
item-times-watch = { $count ->
    [one] Watched once
   *[other] Watched { $count } times
}
item-times-read = { $count ->
    [one] Read once
   *[other] Read { $count } times
}
item-times-listen = { $count ->
    [one] Listened once
   *[other] Listened { $count } times
}
item-times-play = { $count ->
    [one] Played once
   *[other] Played { $count } times
}
item-times = { $count ->
    [one] Finished once
   *[other] Finished { $count } times
}
item-times-on = { $times }, on { $date }
item-times-last-on = { $times }, last on { $date }
item-times-finished-on = { $times }, finished on { $date }
item-fact-up-to-date = Up to date: { $progress }
item-rewatch-start = Start a rewatch
item-rewatch-start-listen = Start listening again
item-rewatch-stop = Stop the rewatch
item-rewatch-stop-listen = Stop this listen-through
item-rewatch-started = Rewatch started
item-rewatch-started-listen = Started listening again
item-rewatch-stopped-toast = Rewatch stopped
item-rewatch-stopped-toast-listen = Stopped listening again
item-rewatch-finished = Rewatch finished: { $title }
item-rewatch-finished-listen = Finished listening again: { $title }
item-rewatch-before = { $count ->
    [one] Watched once before.
   *[other] Watched { $count } times before.
}
item-rewatch-before-listen = { $count ->
    [one] Listened once before.
   *[other] Listened { $count } times before.
}
item-rewatch-line = Rewatch: { $done } of { $total } episodes. { $before }
item-rewatch-line-listen = Listening again: { $done } of { $total } episodes. { $before }
item-rewatch-line-open = { $done ->
    [one] Rewatch: one episode. { $before }
   *[other] Rewatch: { $done } episodes. { $before }
}
item-rewatch-line-open-listen = { $done ->
    [one] Listening again: one episode. { $before }
   *[other] Listening again: { $done } episodes. { $before }
}
item-rewatch-stopped = { $times }; rewatch stopped after { $done } of { $total } episodes
item-rewatch-stopped-listen = { $times }; stopped listening again after { $done } of { $total } episodes
item-rewatch-stopped-open = { $done ->
    [one] { $times }; rewatch stopped after one episode
   *[other] { $times }; rewatch stopped after { $done } episodes
}
item-rewatch-stopped-open-listen = { $done ->
    [one] { $times }; stopped listening again after one episode
   *[other] { $times }; stopped listening again after { $done } episodes
}

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
item-plays-count = { $count ->
    [one] One play
   *[other] { $count } plays
}
item-actions = More actions for { $title }
item-action-bar = Next step
item-with = With...
item-with-watch = Watched with...
item-with-read = Read with...
item-with-listen = Listened with...
item-with-play = Played with...
item-with-named = { $with }: { $title }
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
log-with-watch = Watched with
log-with-read = Read with
log-with-listen = Listened with
log-with-play = Played with
log-with-hint = Leave empty if nobody was with you.
log-players = Players
log-people-add = Add a person
log-people-create = Add "{ $name }"
log-people-none = Nobody with that name
log-more = More
log-more-watched = Rating and note
log-more-play = Co-op, place, notes
log-more-session = Scores, co-op, place, notes
log-note = Note on this title (private)
log-rewatch = { $count ->
    [one] Finished once before. This will be number { $next }.
   *[other] Finished { $count } times before. This will be number { $next }.
}
log-rewatch-watch = { $count ->
    [one] Watched once before. This will be watch number { $next }.
   *[other] Watched { $count } times before. This will be watch number { $next }.
}
log-rewatch-read = { $count ->
    [one] Read once before. This will be number { $next }.
   *[other] Read { $count } times before. This will be number { $next }.
}
log-rewatch-listen = { $count ->
    [one] Listened once before. This will be number { $next }.
   *[other] Listened { $count } times before. This will be number { $next }.
}
log-rewatch-play = { $count ->
    [one] Played once before. This will be number { $next }.
   *[other] Played { $count } times before. This will be number { $next }.
}
log-save = Save
log-cancel = Cancel
log-discard-question = Discard your changes?
log-discard = Discard
log-keep = Keep editing
log-score = Score of { $name }
log-coop = Co-op game
log-coop-won = We won
log-duration = Duration
log-duration-hint = h:mm or minutes
log-location = Place
log-notes = Notes
log-play-step = Play
log-session-step = Session
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
log-error-date-before-run = You started again on { $date }. Pick that day or a later one.
log-error-score = Scores are numbers, for example 42 or 12.5.
log-error-duration = Enter the duration as hours:minutes (1:30) or as minutes (90).
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
episodes-mark-season-listen = Mark season listened
episodes-mark-show-listen = Mark podcast listened
episodes-mark-listen = Mark S{ $season } E{ $episode } listened
episodes-mark-again-listen = Listen to S{ $season } E{ $episode } again
episodes-watched-listen = Listened
episodes-again = Again
episodes-next = Next up
episodes-row-menu = More for S{ $season } E{ $episode }
episodes-up-to-here = Mark up to here
episodes-ask = { $count ->
    [one] Also mark the earlier episode as watched?
   *[other] Also mark the { $count } earlier episodes as watched?
}
episodes-ask-listen = { $count ->
    [one] Also mark the earlier episode as listened?
   *[other] Also mark the { $count } earlier episodes as listened?
}
episodes-ask-all = Mark all { $count }
episodes-ask-one = Only S{ $season } E{ $episode }
episodes-remember = Remember for this show
episodes-remember-listen = Remember for this podcast
episodes-logged = { $count ->
    [one] One episode
   *[other] { $count } episodes
}
episodes-none = No episodes known yet.
episodes-again-ask = { $count ->
    [one] This episode is already watched. Log it again as a rewatch?
   *[other] These { $count } episodes are already watched. Log them again as a rewatch?
}
episodes-again-ask-views = { $count ->
    [one] This episode is already watched. Log it again as a further view?
   *[other] These { $count } episodes are already watched. Log them again as further views?
}
episodes-again-ask-listen = { $count ->
    [one] This episode is already listened to. Log it again?
   *[other] These { $count } episodes are already listened to. Log them again?
}
episodes-again-now = { $count ->
    [one] It is logged with today's date.
   *[other] They are logged with today's date.
}
episodes-again-all = { $count ->
    [one] Log it again
   *[other] Log { $count } again
}
episodes-again-cancel = Cancel
episodes-skipped = { $count ->
    [one] One already watched, not logged again
   *[other] { $count } already watched, not logged again
}
episodes-skipped-listen = { $count ->
    [one] One already listened to, not logged again
   *[other] { $count } already listened to, not logged again
}

search-title = Search
search-label = Search the catalog
search-placeholder = Title, author, studio...
search-empty = No results yet.
search-local = In your instance
search-library = In your library
search-waiting = Asking { $sources }...



sources-title = Data sources
sources-lead = Metadata comes from these services. Their terms and licences apply to their data.

theme-dark = Switch to dark theme
theme-light = Switch to light theme
dialog-close = Close

auth-invited = You were invited: your account is linked to the person who invited you, and you see the sessions you were part of.
privacy-title = Privacy
privacy-share-sessions = Linked participants see the sessions I log
sessions-by = by { $name }
sessions-claim = Copy to my log
sessions-claimed = Copied to your log.
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
notify-channels-lead = Where notifications go besides the inbox. Each card shows how its last delivery went.
notify-new-channel = Add a channel
notify-kind = Type
notify-channel-name = Name
notify-add-channel = Add a channel
notify-test = Send test
notify-delete = Delete
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
notify-default = Instance default
notify-log = Log
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
integrations-lead = Let your media server or a script record what you watched. Each sender gets its own token, so you can revoke one without touching the others.
integrations-name = Name
integrations-url = Webhook URL
integrations-example = The same call with curl
integrations-empty = No senders yet.
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
progress-rewatch = Started again from the beginning
progress-rewatch-stop = Stopped before the end
notify-mark-read-named = Mark as read: { $title }
notify-test-named = Send test to { $name }
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
notify-event-admin-local_sign_in = Local sign-in used (admin)
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
stats-measure-hours = Hours
stats-count-hours = { $count ->
    [one] { $count } hour
   *[other] { $count } hours
}
stats-hero-label = Time banked
stats-hero-days = That is { $days ->
    [one] 1 day
   *[other] { $days } days
} and { $hours ->
    [one] 1 hour
   *[other] { $hours } hours
} of movies, shows, listening and play.
stats-hero-under-day = Movies, shows, listening and play, added up.
stats-hero-none = Nothing with a known length logged yet.
stats-tile-movies = Movies
stats-tile-shows = Shows
stats-tile-books = Books
stats-tile-games = Games
stats-tile-movies-count = { $count ->
    [one] One movie watched
   *[other] { $count } movies watched
}
stats-tile-books-count = { $count ->
    [one] One book finished
   *[other] { $count } books finished
}
stats-also = Also: { $parts }
stats-also-part = { $kind } { $amount }
stats-time-minutes = { $count ->
    [one] { $count } minute
   *[other] { $count } minutes
}
stats-hours-under = under { $hours }
stats-hero-days-only = That is { $days ->
    [one] 1 day
   *[other] { $days } days
} of movies, shows, listening and play.
stats-also-pages = { $count ->
    [one] { $count } page
   *[other] { $count } pages
}
stats-also-episodes = { $count ->
    [one] { $count } episode
   *[other] { $count } episodes
}
stats-also-plays = { $count ->
    [one] { $count } play
   *[other] { $count } plays
}
stats-also-finished = { $count ->
    [one] watched once
   *[other] watched { $count } times
}
stats-also-completions = { $count } completed
stats-also-events = { $count ->
    [one] { $count } logged step
   *[other] { $count } logged steps
}
stats-reading-note = Reading is counted in pages.
stats-untimed-note = Titles without a known length count as 0.
stats-library-heading = Library
stats-library-by-status = Library by status
stats-totals-part-entries = { $count ->
    [one] { $count } entry
   *[other] { $count } entries
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
history-play = Play logged
history-with = with { $names }
history-play-you = You
history-play-won-you = You won
history-play-won = { $names } won
history-play-coop-won = Won together
history-play-coop-lost = Lost together
history-play-score = Score: { $score }
history-play-scores = Scores: { $scores }
history-play-duration = { $minutes } min
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
admin-warning-mail_transport_unusable = Saved, but mail cannot be sent with these settings. Check the server, port and security.
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
source-state-disabled = Switched off
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
setup-checklist-backup_key = Back up secrets.key
setup-checklist-identity_provider = Connect an identity provider
# Audit log actions (`admin-audit-<action with . replaced by _>`).
admin-audit-setting_changed = Setting changed
admin-audit-secret_replaced = Key replaced
admin-audit-secret_cleared = Key removed
admin-audit-secrets_rewrapped = Secrets re-encrypted
admin-audit-secrets_backup_confirmed = Secrets key backup confirmed
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
admin-nav-users = Users
admin-nav-instance = Instance
admin-nav-sso = Sign-in and SSO
admin-nav-sources = Data sources
admin-nav-storage = Storage and jobs
admin-nav-notifications = Notification defaults
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
admin-key-backup-title = Back up the secrets key
admin-key-backup-body = Saved data-source keys, notification secrets and authenticators are encrypted with this key. A database backup cannot open them without it, and the key cannot be recreated. Copy it to where you keep your database backups.
admin-key-backup-file = Key file on the server: { $path }
admin-key-backup-env = The key is set through the environment variable APP__SECRETS__KEYS.
admin-key-backup-confirm = I have backed up the key
admin-key-backup-confirmed = Recorded. This reminder returns when a new key is added.
admin-overview-health = Health
admin-overview-open = Details: { $name }
admin-overview-details = Details
admin-overview-recent = Recent admin actions
admin-overview-recent-none = No admin actions yet.
admin-overview-audit = Full audit log
admin-overview-signups = Recent sign-ups
admin-overview-brand-title = Brand
admin-users-lead = Who has an account and who is admin. Invites live here; the sign-up mode is under Instance.
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

# Phase 3 (WP3-4): sign-in with identity providers (addendum-1 4.3-4.5, DEC-326).
signin-continue = Continue with { $provider }
signin-password-instead = Sign in with a password instead
signin-provider-fallback = the identity provider
signin-sso-only = Members sign in through { $provider }.
signin-outage-title = { $provider } is not answering
signin-outage-body = We could not reach the sign-in service. Your data is safe. Try again in a minute.
signin-outage-capped = We stopped trying for now, so this page does not keep sending you back. Try again in a few minutes.
signin-retry = Try again
signin-break-glass = Admins: sign in with a local account
signin-error-oidc_denied = { $provider } did not allow this sign-in.
signin-error-group_rule = Your account is not allowed here (group rule).
signin-error-no_account = No account for this sign-in. Ask your admin for an invite.
signin-error-account_disabled = This account is disabled. Ask an admin.
signin-error-identity_in_use = This sign-in is already linked to another account.
signin-error-oidc_failed = The sign-in did not work. Try again, or sign in another way.
signin-session-ended-title = Your session ended
signin-session-ended-body = Nothing was lost. Sign in again to continue where you were.
signin-local-title = Local sign-in
signin-local-lead = For admins when single sign-on is unavailable. Members sign in through the identity provider.
signin-local-audit = This sign-in is recorded in the audit log.
signed-out-title = You are signed out
signed-out-lead = You can close this tab, or sign in again.
signin-link-title = Is this your account?
signin-link-lead = Your { $provider } account uses the same address as the account { $username } here. Sign in with that account's password once to link them. After that, { $provider } alone signs you in.
signin-link-password = Password for { $username }
signin-link-submit = Link and sign in
signin-link-not-mine = This is not my account
signin-link-no-password = That account has no password, so it cannot be linked here. Ask your admin to link it.
signin-link-expired = This link request has expired. Sign in again to start over.
signin-link-back = Back to sign-in
signin-reset-title = Choose a new password
signin-reset-lead = This link works once. Setting a password signs out every session of the account.
signin-reset-submit = Set the new password
signin-reset-done = Your password is changed. Sign in with it now.
signin-reset-error-reset_link_invalid = This link is not valid any more. Ask your admin for a new one.
auth-error-sso_only = Sign in through the identity provider. Admins can use the local sign-in.
datetime-local = { $date }, { $time }

# Re-authentication with a provider (DEC-323).
reauth-with-provider = Confirm with { $provider }
reauth-returned-saved = You confirmed it is you. The change is saved.
reauth-returned-again = You confirmed it is you. Make the change again; it will go through now.
reauth-error-reauth_not_fresh = The provider did not ask you to sign in again, so this does not count as a confirmation. Use your password, or ask an admin.
reauth-error-oidc_denied = The provider did not allow the confirmation.
reauth-error-oidc_unavailable = The provider is not answering. Try again in a minute.
reauth-error-oidc_failed = The confirmation did not work. Try again.
reauth-error-no_linked_provider = This account has no sign-in at a provider. Use your password.

# Admin > Instance (addendum-1 3.4).
admin-instance-lead = Settings that apply to everyone here. Values set in the environment are shown but cannot be changed from this page.
admin-hosting-title = How you host
admin-hosting-home = Home: free, for family and friends
admin-hosting-home-hint = Open defaults; each data source key is the instance's own.
admin-hosting-hosted = Hosted: for other people
admin-hosting-hosted-hint = Stricter defaults: licence reminders for data sources and keys per person where a source needs them.
admin-hosting-note = Changes defaults (sign-up, licence reminders, per-user keys), never features.
admin-public-url = Public address
admin-public-url-hint = Set with { $var }. Used in links, invites and the sign-in redirect.
admin-mail-title = Mail
admin-mail-summary = SMTP for password resets and email notifications
admin-mail-lead = Needed for password resets and email notifications. Nothing else depends on it.
admin-mail-password = Password
admin-mail-password-hint = Stored encrypted, never shown again.
admin-mail-password-stored = A password is stored.
admin-mail-send-test = Save and send a test
admin-mail-send-test-hint = Sends one message to your own address.
admin-mail-test-ok = The test message was sent.
admin-mail-test-failed = The test failed: { $reason }
admin-mail-last-ok = Last test { $at }: it worked.
admin-mail-last-failed = Last test { $at }: { $reason }
admin-mail-security-tls = TLS
admin-mail-security-starttls = STARTTLS
admin-mail-security-none = None (local relay only)
admin-setting-mail_host = Server
admin-setting-mail_port = Port
admin-setting-mail_security = Connection security
admin-setting-mail_username = Username
admin-setting-mail_from = From address
admin-setting-auth_sign_in_mode = Sign-in mode
admin-secrets-summary = The key that protects stored secrets
admin-advanced-title = Advanced
admin-advanced-summary = Session length, cookie security, confirmation window
admin-replace = Replace
admin-health-mail-tested = Test message sent { $at }
admin-health-mail-test_failed = The last test failed: { $result }
admin-health-identity_provider-discovered = Discovery cached since { $at }
admin-health-identity_provider-test_failed = The last Test of { $provider } failed
admin-health-identity_provider-unavailable = { $provider } did not answer at the last sign-in
admin-users-via-group = via group
admin-users-via-group-hint = Set by { $provider } at every sign-in
admin-sources-enabled = Use this source
admin-sources-enabled-hint = Off: nobody can search it and its items are not refreshed. Applies at once.

# Admin > Sign-in and SSO (addendum-1 3.4, 4.2-4.5).
admin-sso-lead = How people get in. Password sign-in is always available to admins at /sign-in/local, so a broken provider cannot lock you out.
admin-sso-providers = Identity providers
admin-sso-provider = Provider
admin-sso-issuer = Issuer
admin-sso-status = Status
admin-sso-actions = Actions
admin-sso-details = Details of { $name }
admin-sso-default = default
admin-sso-none = No identity provider yet. Add one below; the password sign-in stays.
admin-sso-state-off = Off
admin-sso-state-ok = Working
admin-sso-state-failed = Test failed
admin-sso-state-untested = Not tested
admin-sso-display-name = Display name
admin-sso-display-name-hint = Shown on the sign-in button: "Continue with ...".
admin-sso-slug = Short name
admin-sso-slug-hint = a-z, 0-9 and -, used in addresses. Fixed once saved.
admin-sso-issuer-url = Issuer URL
admin-sso-issuer-hint = The address the provider names as its issuer, such as https://auth.example.org/application/o/shelf/.
admin-sso-client-id = Client ID
admin-sso-client-secret = Client secret
admin-sso-client-secret-hint = Stored encrypted, never shown again.
admin-sso-secret-stored = A client secret is stored.
admin-sso-secret-locked = A client secret is stored, but its key is not loaded.
admin-sso-save-secret = Save the secret
admin-sso-scopes = Scopes
admin-sso-scopes-hint = Separated by spaces; openid is always added.
admin-sso-enabled = Enabled
admin-sso-is-default = Default provider (the one "Go straight to" uses)
admin-sso-rules = Account rules
admin-sso-auto-create = Create an account at the first sign-in, even when sign-up is invite-only
admin-sso-groups-claim = Groups claim
admin-sso-groups-claim-hint = The claim that lists the groups; most providers call it groups.
admin-sso-allowed-groups = Only members of these groups may sign in
admin-sso-allowed-groups-hint = Separated by commas; empty means everyone.
admin-sso-admin-groups = Members of these groups are admins here
admin-sso-admin-groups-hint = Separated by commas.
admin-sso-provider-roles = Take roles from the admin groups (checked at every sign-in)
admin-sso-rules-hint = You stay signed in; a rule that would remove your own admin role is refused.
admin-sso-delete = Remove the provider
admin-sso-delete-title = Remove { $name }?
admin-sso-delete-body = People linked only to this provider cannot sign in until they get another way in. Their accounts and history stay.
admin-sso-test-title = Test
admin-sso-test = Test
admin-sso-test-ok = Test passed.
admin-sso-test-failed = Test failed.
admin-sso-test-at = ({ $at })
admin-sso-test-discovery = Discovery
admin-sso-test-discovery-ok = works
admin-sso-test-discovery-failed = failed: { $reason }
admin-sso-test-end-session = Sign-out at the provider
admin-sso-test-backchannel = Back-channel logout
admin-sso-test-frontchannel = Front-channel logout
admin-sso-test-pkce = PKCE (S256)
admin-sso-test-iss = Issuer in the answer (RFC 9207)
admin-sso-test-groups = Groups claim
admin-sso-test-groups-seen = seen at your last sign-in through it
admin-sso-test-groups-missing = missing at your last sign-in through it
admin-sso-test-groups-unknown = you have not signed in through it yet
admin-sso-test-silent = Silent sign-in
admin-sso-test-max-age = Asks again for a confirmation (max_age=0)
admin-sso-offered = offered
admin-sso-not-offered = not offered
admin-sso-offered-unused = offered, not used
admin-sso-supported = yes
admin-sso-not-advertised = not advertised
admin-sso-probe-silent = Check silent sign-in
admin-sso-probe-max-age = Check the confirmation round trip
admin-sso-probe-hint = Both checks send your browser to the provider and back here. Going straight to the provider uses the silent sign-in only after it worked.
admin-sso-probe-not-run = not checked yet
admin-sso-probe-silent_ok = works: people with a session there come back without a form
admin-sso-probe-silent_login_required = works when signed in there; it asked for a sign-in this time
admin-sso-probe-silent_unsupported = not supported; the provider shows its own form
admin-sso-probe-max_age_ok = yes, the provider asked you to sign in again
admin-sso-probe-max_age_ignored = no, the provider ignores it; password-less admins cannot confirm with it
admin-sso-probe-max_age_error = the provider answered with an error
admin-sso-probe-error = The check did not finish ({ $code }).
admin-sso-add = Add a provider
admin-sso-add-summary = Issuer URL, client ID, client secret (write-only), then Test runs discovery.
admin-sso-add-submit = Add the provider
admin-sso-register = Register these addresses at your provider
admin-sso-register-lead = The same three for every provider.
admin-sso-redirect-uri = Redirect URI
admin-sso-logout-uri = Post-logout redirect URI
admin-sso-backchannel-uri = Back-channel logout URI
admin-sso-mode = When someone opens the app signed out
admin-sso-mode-page = Show the sign-in page
admin-sso-mode-page-hint = Provider buttons first, the password form below them.
admin-sso-mode-auto = Go straight to { $provider }
admin-sso-mode-auto-hint = People already signed in there never see our form. Members cannot use passwords; admins still can at /sign-in/local.
admin-sso-mode-needs = Going straight to the provider needs a default provider whose Test passed in the last 24 hours, and an admin with a password and an authenticator app.
admin-sso-front-channel = Front-channel logout is not used (it depends on third-party cookies, which Safari and Firefox block or partition).
admin-error-auto_needs_tested_provider = Test the default provider first: going straight to it needs a Test that passed in the last 24 hours.
admin-error-auto_needs_admin_authenticator = First give at least one admin a password and an authenticator app, so nobody gets locked out.
admin-error-last_sign_in_method = This is the only way left to sign in. Set a password or link another provider first.
admin-error-invalid_slug = Use 1 to 32 characters a-z, 0-9 and -, starting with a letter or digit.
admin-error-slug_taken = Another provider uses this short name.
admin-error-invalid_issuer = This is not an issuer address.
admin-error-insecure_issuer = Use https (plain http only for a provider on this machine, or while this instance runs on http).
admin-error-admin_groups_required = Roles from the provider need at least one admin group.
admin-error-self_demotion = This rule would remove your own admin role.
admin-error-role_managed_by_provider = This role comes from a provider's group rule. Change it at the provider.
admin-error-identity_in_use = This sign-in is already linked to another account.
admin-error-no_linked_provider = This account has no sign-in at a provider.
admin-error-invalid_time_zone = This is not a time zone name. Use one like Europe/Berlin.

# Settings > Account: linked sign-ins and the time zone (DEC-306, DEC-312).
settings-linked = Linked sign-ins
settings-linked-lead = Providers that can sign you in to this account.
settings-linked-none = No provider linked.
settings-linked-since = Linked { $date }
settings-linked-only = This is your only way to sign in.
settings-unlink = Unlink
settings-unlink-named = Unlink { $provider }
settings-unlinked = Unlinked.
settings-link-provider = Link { $provider }
settings-tz = Time zone
settings-tz-current = Times are shown in { $zone }.
settings-tz-none = Times are shown in UTC: no time zone chosen yet.
settings-tz-use-browser = Use { $zone } (this browser)
settings-tz-field = Time zone name
settings-tz-hint = An IANA name such as Europe/Berlin. Home, History and Stats use it.
settings-tz-save = Save the time zone
settings-tz-reset = Back to UTC
admin-mail-to = Send the test to
admin-mail-to-hint = Empty: your own address.
admin-error-no_recipient = Your account has no email address. Enter an address to send the test to.

# ---- UX phase 3, WP3-5: Settings > Notifications and Integrations (design.md 6.8, 6.9) ----
notify-lead = What reaches you, and where. The inbox is always on; channels add email, push or chat.
notify-tabs = Notification sections
notify-rules = Rules
notify-unread = Unread:
notify-mark-all-read = Mark all read
notify-inbox-empty-body = You will see new episodes, shares and import results here.
notify-log-when = When
notify-log-state = State
notify-log-show-error = Show the error
notify-channel-gone = Deleted channel
notify-health-never = Never used
notify-health-working = Working, last delivery { $when }
notify-health-waiting = Waiting to send
notify-health-failing = Last delivery failed { $when }
notify-live-failing = { $channel }: the last delivery failed
notify-live-working = { $channel }: the last delivery got through
notify-failing-title = The last delivery did not get through
notify-failing-hint = Notifications still reach your inbox. Check the channel's settings, then send a test.
notify-test-delivered = Test delivered.
notify-test-failed = The test was not delivered
notify-secret-label = Signing secret
notify-secret-once = Shown once. Copy it into the receiving service now.
notify-delete-title = Delete { $name }?
notify-delete-body = Nothing more is sent to this channel. Your inbox and the rules for other channels stay as they are.
notify-hint-name = Only you see it.
notify-hint-url = Starts with https:// (or http:// on your own network).
notify-hint-topic = Letters, digits, - and _ only, no spaces. Try "my-topic".
notify-hint-discord = The webhook URL from the channel's settings in Discord.
notify-hint-room = Looks like !abc123:example.org.
notify-hint-webhook = We sign every call (Standard Webhooks), so the receiver can check it came from us.
notify-hint-secret = Stored encrypted, never shown again.
notify-check-required = Fill this in.
notify-check-url = Enter an address that starts with https:// or http://.
notify-check-email = Enter an email address with an @.
notify-check-topic = Topics cannot contain spaces or special characters. Try "my-topic".
notify-add-and-test = Add and send a test
notify-rules-lead = Instance defaults are marked; locked rules cannot be changed. Everything also lands in the inbox.
notify-rules-no-channels = Add a channel first. Until then everything reaches the inbox only.
notify-when = When
notify-when-quiet = { $mode }, quiet { $from }-{ $to }
notify-when-inbox-only = Inbox only
notify-inbox-only = Inbox only
notify-inbox-always = Always on
notify-on = On
notify-off = Off
notify-locked = Locked
notify-switch-named = { $event } to { $channel }
notify-offer-title = { $channel } is new
notify-offer-body = { $count ->
    [one] The instance suggests one more event: { $events }. Turn it on for { $channel } too?
   *[other] The instance suggests { $count } more events: { $events }. Turn them on for { $channel } too?
}
notify-offer-yes = Use the suggestions
notify-offer-no = Only this one
notify-section-release = Releases
notify-section-library = Library
notify-section-social = People and sharing
notify-section-boardgame = Board games
notify-section-integration = Senders
notify-section-import = Imports
notify-section-admin = Instance (admin)
notify-section-patterns = Several events at once
notify-filtered = Rules for one list, item or person
notify-filtered-empty = None yet.
notify-add-filtered = Add a rule for one list, item or person
notify-filter-for = For
notify-filter-pick = Choose a list or a person
notify-filter-list = List: { $name }
notify-filter-person = Person: { $name }
notify-filter-items = { $count ->
    [one] one item
   *[other] { $count } items
}
notify-filter-items-later = For one item, choose it with the search below.
notify-filter-required = Choose a list, a person or an item.
notify-add-rule = Add rule
integrations-senders = Senders
integrations-connect = Connect a sender
integrations-connect-lead = 3 steps, about 5 minutes.
integrations-steps = Connect a sender, progress
integrations-step-name = Name it
integrations-step-copy = Copy the token
integrations-step-wait = Wait for the first call
integrations-step-done = done
integrations-name-hint = Only you see it, e.g. "Media server - living room".
integrations-name-invalid = Use 1 to 100 characters for the name.
integrations-create = Create token
integrations-cancel = Cancel
integrations-done = Done
integrations-token-for = Token for "{ $name }"
integrations-token-once = Shown once. If you lose it, create a new sender.
integrations-help-title = Setting up a media server with a webhook plugin
integrations-help-1 = In the server's webhook plugin, add a destination with the URL and the request header Authorization: Bearer followed by the token.
integrations-help-2 = Send it when playback stops or an item is marked as played.
integrations-help-3 = Use a body template that produces this JSON (fill the fields from the plugin's own variables):
integrations-shape = Body shape for an episode
integrations-waiting = Waiting for the first call...
integrations-waiting-body = Play something and stop it. The first accepted call appears here within seconds. If it is refused, the reason is shown here.
integrations-quiet = No call yet
integrations-quiet-body = Check the token and the URL in the sender. A call with a wrong or missing token is refused before we know which sender it is, so it cannot show up here.
integrations-accepted = First call received
integrations-accepted-body = It recorded { $title }. This sender is working.
integrations-wait-refused = The call came in but was refused.
integrations-wait-refused-field = The call came in but was refused because of the field { $field }.
integrations-send-test = Send a test call from here
integrations-test-ok = Test call accepted (nothing was recorded)
integrations-test-matched = It would record { $title }, which is already in the catalogue.
integrations-test-new = It would add { $title } to the catalogue and record it.
integrations-test-refused = The test call was refused
integrations-never-called = Never called
integrations-working = Working
integrations-refused = Last call refused
integrations-refused-title = A call was refused
integrations-last-recorded = Last call: { $when } - recorded { $title }
integrations-last-call-status = Last call: { $when } - answered { $status }
integrations-earlier-recorded = Last recorded before that: { $title }
integrations-created-on = Watched webhook - created { $when }
integrations-menu-named = Actions for { $name }
integrations-revoke-title = Revoke { $name }?
integrations-revoke-body = Its token stops working at once and its calls are refused. What it recorded stays in your history. This cannot be undone.
integrations-refusal = The last call from "{ $name }" was refused.
integrations-refusal-field = The last call from "{ $name }" was refused because of the field { $field }. Fields are checked strictly, so typos do not get silently ignored.
integrations-refusal-expected = Expected fields: { $fields }.
integrations-show-payload = Show the last body (secrets hidden)
integrations-empty-body = Connect your media server and every finished episode lands in your history by itself.

## People (design.md 6.7, 5.11)
companions-title = People
companions-lead = Who you watch, read and play with. Only you see this list.
companions-add-open = Add a person
companions-new = Name
companions-note = Note (optional)
companions-note-hint = Only you see it.
companions-add = Add person
companions-added = Added { $name }.
companions-tabs = People
companions-tab-all = All { $count }
companions-tab-linked = Linked accounts { $count }
companions-empty = No people yet. Add someone the first time you log a play or a watch together; they appear here.
companions-empty-linked = Nobody here is linked to an account yet. Invite someone, or ask a person with an account to confirm it is them.
companions-linked = Linked
companions-linked-to = Linked to @{ $username }
companions-requested = Link request sent
companions-no-account = No account
companions-together = Together
companions-plays = Plays
companions-win-rate = Win rate
companions-percent = { $percent }%
companions-no-rate = No winner yet
companions-last = Last: { $title }, { $when }
companions-most-played = Most played: { $title }, { $plays ->
    [one] one play
   *[other] { $plays } plays
}
companions-history = History
companions-history-named = History with { $name }
companions-menu-named = More actions for { $name }
companions-invite = Invite
companions-invite-named = Invite { $name }
companions-link = Link to account
companions-unlink = Unlink
companions-merge = Merge with...
companions-edit = Edit
companions-delete = Delete
companions-requests = Requests for you
companions-request-text = { $from } says their entry "{ $name }" is you. If you accept, { $from } sees the plays and watches you share, and you see theirs with you.
companions-accept = Accept
companions-decline = Decline
companions-accept-named = Accept the request from { $from }
companions-decline-named = Decline the request from { $from }
companions-accepted = You are now linked with { $from }.
companions-declined = Request declined.
companions-suggestions = Possible duplicates
companions-suggest = "{ $keep }" and "{ $remove }" look like the same person.
companions-suggest-merge = Merge { $keep } and { $remove }
companions-invite-title = Invite { $name }
companions-invite-consequence = { $name } gets an account on this instance, linked to this entry. They will see the { $count ->
    [one] session
   *[other] { $count } sessions
} you logged with them and can copy them to their own history. Nothing in your log changes.
companions-invite-making = Making the link...
companions-invite-link = Single-use link, valid 14 days
companions-invite-send = Send it any way you like. If { $name } already has an account here, use "Link to account" instead.
companions-link-title = Link { $name } to an account
companions-link-lead = If they already have an account here, ask them to confirm it is them. They see a request on their People page; once they accept, they see the sessions you share with them.
companions-link-username = Their username
companions-link-submit = Send link request
companions-link-sent = Link request sent.
companions-unlink-title = Unlink { $name }?
companions-unlink-body = { $name } no longer sees the sessions you log with them, and you no longer see theirs. Your history stays as it is.
companions-unlinked = { $name } is no longer linked.
companions-merge-title = Merge duplicates
companions-merge-keep = Keep
companions-merge-remove = Fold in
companions-choose = Choose...
companions-merge-fold-count = { $count ->
    [one] one session
   *[other] { $count } sessions
}
companions-merge-swap = Swap
companions-merge-consequence = { $remove }'s sessions will show under { $keep }. Nothing is deleted; this can be undone from { $keep }'s page.
companions-merge-submit = Merge
companions-merged = Merged { $remove } into { $keep }.
companions-merge-undo = Undo merge
companions-merge-undo-named = Undo merge: { $name }
companions-unmerged = { $name } is a separate entry again.
companions-edit-title = Edit { $name }
companions-save = Save
companions-saved = Saved.
companions-delete-title = Delete { $name }?
companions-delete-history = { $name } disappears from People and the log sheet. Your history keeps the { $count ->
    [one] session
   *[other] { $count } sessions
} with them.
companions-delete-none = { $name } is deleted.
companions-delete-link = The link to @{ $username } ends.
companions-deleted = { $name } was deleted.
companions-person-back = All people
companions-person-history = History with { $name }
companions-person-all-history = All history with { $name }
companions-person-no-history = Nothing logged with { $name } yet.
companions-person-lists = Lists shared by { $name }
companions-person-no-lists = { $name } shares no list with everyone here.
companions-person-lists-unlinked = Lists show here once { $name } has an account linked to this entry.
companions-person-merged = Merged into this entry
companions-person-merged-row = { $name }, merged { $when }

## Lists (design.md 6.5, 5.10)
lists-title = Lists
lists-lead = Ordered collections you curate. Share one with a link or with everyone on this instance.
lists-new-open = New list
lists-new = List name
lists-create = Create list
lists-tabs = Lists
lists-tab-mine = Mine { $count }
lists-tab-shared = Shared with me { $count }
lists-tab-instance = On this instance { $count }
lists-empty = You have no lists yet.
lists-empty-shared = Nothing shared with you yet. Lists that people linked with you share with everyone here appear here.
lists-empty-instance = Nobody here shares a list with everyone yet.
lists-items = { $count ->
    [one] One item
   *[other] { $count } items
}
lists-items-kinds = { $items } - { $kinds }
lists-updated = Updated { $when }
lists-vis-private = Private
lists-vis-instance = Everyone here
lists-vis-link = Shared by link
lists-back = All lists
lists-edit = Edit title and description
lists-title-label = Title
lists-description = Description
lists-save = Save
lists-saved = Saved.
lists-delete = Delete list
lists-delete-title = Delete "{ $name }"?
lists-delete-body = The list and its link go away. The items stay in your library.
lists-add = Add
lists-add-named = Add to { $name }
lists-add-target = Choosing a result adds it to "{ $name }".
lists-added = Added { $title } to { $name }.
lists-empty-items = This list is empty. Add something with "Add".
lists-in-library = In your library
lists-move-up = Move up
lists-move-down = Move down
lists-move-up-named = Move up: { $title }
lists-move-down-named = Move down: { $title }
lists-moved = { $title } is now number { $position } of { $count }.
lists-remove = Remove
lists-remove-named = Remove { $title } from the list
lists-removed = Removed { $title }.
lists-add-all = Add all to my library
lists-added-all = Added { $added } to your library; { $already } were there already.
lists-share = Share
lists-share-named = Share { $name }
lists-share-title = Share "{ $name }"
lists-share-legend = Who can see this list
lists-share-private = Private
lists-share-private-hint = Only you.
lists-share-instance = Everyone on this instance
lists-share-instance-hint = Signed-in members can find it under Lists.
lists-share-link = Anyone with the link
lists-share-link-hint = No account needed. Your name and the items are visible; your ratings and notes are not.
lists-share-the-link = Public link
lists-share-new-link = Create a new link
lists-share-new-hint = Creating a new link stops the old one.
lists-share-save = Save
lists-share-done-private = Saved: only you see this list.
lists-share-done-instance = Saved: everyone on this instance can see this list.
lists-share-done-link = Saved: anyone with the link can see this list. Copy the link below.
lists-share-kept = Saved: the link stays the same.
lists-share-replaced = A new link replaces the old one; the old link no longer works.
lists-share-revoked = Saved: the link no longer works.

## The public page of a shared list
shared-list-missing = This list is not shared any more
shared-list-missing-lead = The link was changed or the list was made private. Ask the person who sent it for a new link.
shared-list-cta = Sign in to add these to your library
shared-list-made-with = Made with { $product }

## Settings > Imports and export (WP3-7, design.md 6.11)
settings-nav-imports = Imports and export
imports-title = Imports and export
imports-lead = Bring your library from another app or file, see what a file holds before anything is written, and download everything you logged.
imports-choose = Import a file
imports-latest = Your imports
imports-format-generic = CSV or JSON Lines
imports-format-generic-body = This app's own format, also what the export writes. A file ending in .csv is read as CSV, any other as JSON Lines.
imports-format-legacy = Previous tracker export
imports-format-legacy-body = The JSON export of the tracker you used before: kinds, ids, watched episodes and dates, statuses and collections as lists.
imports-format-trakt = Trakt
imports-format-trakt-body = Your own Trakt history, ratings or watchlist as JSON, one file at a time. Nothing is fetched from Trakt.
imports-format-goodreads = Goodreads
imports-format-goodreads-body = The Goodreads library export (CSV). It can be added once a sample export is available.
imports-format-bgstats = BG Stats
imports-format-bgstats-body = The BG Stats backup with your board game plays. It can be added once a sample export is available.
imports-format-bgg = BoardGameGeek
imports-format-bgg-body = Your BoardGameGeek collection and plays. It can be added once a sample export is available.
imports-waiting = Waiting for a sample
imports-pick = Choose a file
imports-drop = Or drop a file here. It is checked first; nothing is imported until you say so.
imports-trakt-credit = { $product } uses Trakt but is not endorsed or certified by Trakt.
imports-trakt-link = trakt.tv
imports-none = No imports yet
imports-none-body = Choose a file above. You will see what it holds before anything is imported.
imports-earlier = Earlier imports
imports-earlier-row = { $format }, { $when }: { $state }
imports-state-queued = Waiting to start
imports-state-running = Importing
imports-state-done = Import finished
imports-state-failed = Import failed
imports-state-cancelled = Import stopped
imports-state-previewed = Checked, not imported
imports-reading = Reading the file...
imports-progress = { $done } of { $total }
imports-progress-label = Import progress
imports-panel-title = { $format }, { $when }
imports-checking = Checking the file
imports-running = Importing
imports-cancel = Cancel
imports-cancelling = Stopping...
imports-preview = What this file holds
imports-preview-summary = { $rows ->
    [one] One row
   *[other] { $rows } rows
} with { $events ->
    [one] one history entry
   *[other] { $events } history entries
}. { $unmatched ->
    [0] Every row has an id to match.
    [one] One row has no id to match and will wait for you.
   *[other] { $unmatched } rows have no id to match and will wait for you.
} { $errors ->
    [0] No errors.
    [one] One row cannot be read.
   *[other] { $errors } rows cannot be read.
}
imports-run = { $count ->
    [one] Import 1 row
   *[other] Import { $count } rows
}
imports-kinds = Rows per kind
imports-kind = Kind
imports-rows = Rows
imports-lists = Lists: { $lists }
imports-samples = Sample rows
imports-sample = { $kind }, { $status }, { $events ->
    [one] 1 history entry
   *[other] { $events } history entries
}
imports-no-status = no status
imports-no-match = No match yet
imports-mapping = What maps to what
imports-from = In the file
imports-to = Here
imports-target-kind = Kind
imports-target-external_ids = Ids used to find the item
imports-target-title = Title
imports-target-history = History (watched, read, played)
imports-target-status = Status
imports-target-rating = Rating
imports-target-notes = Note
imports-target-lists = Lists
imports-target-same = The same fields
imports-lost = What is not kept
imports-lost-nothing = Nothing: every field is kept.
imports-lost-people = { $count } people (not imported)
imports-lost-workouts = { $count } workouts (not imported)
imports-lost-exercises = { $count } exercises (not imported)
imports-lost-measurements = { $count } measurements (not imported)
imports-lost-metadata_groups = { $count } groups (not imported)
imports-lost-workout_templates = { $count } workout templates (not imported)
imports-lost-reviews = { $count } reviews (not imported)
imports-lost-skipped_collections = { $count } entries in the Reminders and Custom collections (not imported)
imports-lost-watched_on = Where you watched it, on { $count } entries
imports-lost-partial_progress = { $count } started or partly finished entries: their percentage and start date (the status is kept)
imports-lost-views_without_episode = { $count } views without an episode number
imports-lost-finish_dates = The finish date of { $count } books or games (the status is kept)
imports-lost-undated_history = { $count } views without a date
imports-lost-estimated_dates = { $count } views without a date: logged on the day the item entered Completed or In Progress (an estimate)
imports-lost-start_dates = The start date of { $count } finished views
imports-lost-action = How { $count } views were logged (scrobble, check-in, watch)
imports-lost-rated_at = When { $count } ratings were given
imports-lost-listed_at = When { $count } items were added to the watchlist, and their order
imports-lost-season_episode_ratings = { $count } ratings of single seasons or episodes
imports-errors = { $count ->
    [one] One row was skipped
   *[other] { $count } rows were skipped
}
imports-error-line = Row { $line }:
imports-result = Imported { $imported }, skipped { $skipped }, without a match { $unmatched }.
imports-stopped-at = Stopped after { $done } of { $total } rows. Rows imported so far stay.
imports-unmatched = Rows without a match
imports-unmatched-body = These rows had no id this app can look up. Find each one; it is imported as soon as you choose the item.
imports-again = Try again
imports-find = Find a match
imports-find-named = Find a match for { $title }
imports-matched = Matched and imported.
imports-export = Export
imports-export-lead = Everything in your library, in this app's own format. Both files can be imported again.
imports-export-jsonl = Download JSON Lines
imports-export-jsonl-body = Every field and the whole history, episodes and positions included.
imports-export-csv = Download CSV
imports-export-csv-body = One row per item for spreadsheets; of the history only the dates you finished something.
admin-error-import_finished = The import has already finished.
admin-error-import_dry_run = That was only a check. Import the file first.
admin-error-import_kind_mismatch = That item is of another kind than the row.
admin-error-import_running = The import is still running. Match the row when it has finished.
admin-error-import_file_gone = The file of this import is no longer kept. Upload it again.
admin-error-quota = The file is larger than your upload allowance.
history-export = Export
episodes-mark-show = Mark show watched
episodes-when = Dates for marking several
episodes-when-now = Now
episodes-when-range = Spread over a date range
episodes-from = From
episodes-to = To
episodes-range-hint = Mark season, Mark show and Mark up to here spread the episodes evenly from the first to the last day.
episodes-range-hint-listen = Mark season, Mark podcast and Mark up to here spread the episodes evenly from the first to the last day.
episodes-range-missing = Pick both days of the range.
episodes-range-reversed = The last day is before the first.
episodes-range-future = The range cannot end after today.
episodes-when-release = On release dates
episodes-release-hint = Mark season, Mark show and Mark up to here log each episode on the day it was released.
episodes-release-hint-listen = Mark season, Mark podcast and Mark up to here log each episode on the day it was released.
episodes-release-missing = { $count ->
    [one] "On release dates" is not offered: one open episode of this season has no release date up to today.
   *[other] "On release dates" is not offered: { $count } open episodes of this season have no release date up to today.
}
episodes-release-refused = { $count ->
    [one] One of these episodes has no release date up to today, so nothing was marked.
   *[other] { $count } of these episodes have no release date up to today, so nothing was marked.
}
imports-match-target = Choosing a result matches it with the imported row "{ $title }" and imports the row.

# WP4-2 admin-ops: probe rows, the Users row menu, linked accounts, the emailed link
admin-status-unknown = Not checked yet
admin-health-not-checked = The background check has not run yet.
admin-health-stale = Not checked recently (last check { $time })
admin-users-more = More actions for { $name }
admin-users-more-short = More
admin-users-reset-totp = Reset authenticator
admin-users-reset-totp-title = Reset the authenticator of { $name }?
admin-users-reset-totp-body = { $name } then signs in with the password alone until they set up a new authenticator app. Their recovery codes stop working, and they get a notification.
admin-users-unlink = Unlink { $provider }
admin-users-unlink-title = Unlink { $provider } from { $name }?
admin-users-unlink-body = { $name } can no longer sign in with { $provider } and gets a notification. The account and its library stay.
admin-users-unlink-confirm = Unlink
admin-error-no_authenticator = This person has no authenticator.
admin-sso-linked = Linked accounts
admin-audit-user_totp_reset = Authenticator reset
admin-audit-user_identity_unlinked = Sign-in unlinked
admin-audit-auth_link_emailed = Sign-in link emailed
admin-audit-auth_link_confirmed = Sign-in linked through an emailed link
notify-event-account-authenticator_reset = Your authenticator was removed
notify-event-account-sign_in_unlinked = A sign-in was unlinked from your account
signin-link-email-lead = That account has no password. We can email a one-time link to its address; open it in this browser to link the sign-in.
signin-link-email = Email me a link
signin-link-emailed = We sent a link to { $address }. Open it in this browser within { $minutes } minutes.
signin-link-emailed-short = We sent a link. Open it in this browser.
signin-link-email-failed = The link could not be sent. Ask your admin to link it.
signin-link-code-invalid = This link no longer works: it was used, it expired, or it was opened in another browser. Sign in again to start over.
signin-link-linking = Linking your account...

## Bulk edits and removing from the library (WP4-3)
library-select = Select { $title }
library-select-all = Select all on this page
library-add-to-list = Add to list...
library-remove = Remove from library
library-removed = Removed { $title } from your library
library-bulk-label = Bulk actions
library-bulk-count = { $count } selected
library-bulk-status-menu = Set status
library-bulk-tag = Add tag
library-bulk-tag-title = Add a tag to the selected entries
library-bulk-list = Add to list
library-bulk-remove = Remove
library-bulk-clear = Clear selection
library-bulk-remove-title = { $count ->
    [one] Remove one entry from your library?
   *[other] Remove { $count } entries from your library?
}
library-bulk-remove-body = Their status, rating, note and tags go. Plays, progress, reviews and lists keep them, and Undo brings everything back.
library-bulk-remove-confirm = Remove
library-bulk-status = { $count ->
    [one] One entry marked as { $status }
   *[other] { $count } entries marked as { $status }
}
library-bulk-tagged = { $count ->
    [one] Tagged one entry with { $tag }
   *[other] Tagged { $count } entries with { $tag }
}
library-bulk-listed = { $count ->
    [one] Added one entry to { $list }
   *[other] Added { $count } entries to { $list }
}
library-bulk-removed = { $count ->
    [one] Removed one entry from your library
   *[other] Removed { $count } entries from your library
}
library-bulk-failed = { $count ->
    [one] One entry could not be changed.
   *[other] { $count } entries could not be changed.
}
library-list-pick-title = { $count ->
    [one] Add to which list?
   *[other] Add { $count } entries to which list?
}
library-list-none = You have no lists yet.
library-list-create = Create a list
item-remove = Remove from library
# keyboard map and palette (WP4-4)
shortcuts-chord-waiting = G pressed: then H, L, Y, T or P to go to a section.
shortcuts-then = then
shortcuts-or = or
shortcuts-everywhere = Everywhere
shortcuts-home = Home
shortcuts-library = Library
shortcuts-item = Item page
shortcuts-search = Search
shortcuts-go-home = Go to Home
shortcuts-go-library = Go to Library
shortcuts-go-history = Go to History
shortcuts-go-lists = Go to Lists
shortcuts-go-people = Go to People
shortcuts-home-move = Move between Continue cards
shortcuts-home-run = Run the next step of the focused card
shortcuts-library-move = Move between cards
shortcuts-library-row = First or last card of the row
shortcuts-library-first = First card of all (Ctrl+End: the last)
shortcuts-library-open = Open the focused card
shortcuts-library-step = Next step of the focused card
shortcuts-library-tags = Tags of the focused card
shortcuts-library-list = Add the focused card to a list
shortcuts-library-filter = Filter the library
shortcuts-item-step = Log the next step
shortcuts-item-rating = Focus your rating
shortcuts-item-tags = Edit the tags
shortcuts-item-list = Add to a list
shortcuts-search-open = Open the result, or choose it for the page that opened the search
shortcuts-search-quick = Add as Planned, or run the next step
shortcuts-key-enter = Enter
shortcuts-key-home = Home
shortcuts-key-end = End
shortcuts-key-arrows = Arrow keys
shortcuts-off-note = Single-key shortcuts are turned off (Settings > Account). Ctrl+K and the keys inside grids, menus and the search still work.
search-target-rule = Choosing a result makes it the subject of the rule.
search-target-log-with = Choosing a result logs it with { $name }.
search-recent = Recent
search-suggested = Suggestions
search-cmd-lists = Go to Lists
search-cmd-new-list = New list
search-cmd-log-play = Log a play of { $title }
search-cmd-import = Import a file
search-hint-open = Open
search-hint-choose = Choose
search-hint-quick = Add as Planned or next step
companions-log-with = Log with...
companions-log-with-named = Log with { $name }
notify-filter-pick-item = Choose an item...
notify-filter-item = Item: { $name }
notify-hint-secret-keep = Leave it blank to keep the stored value.
notify-edit = Edit
notify-edit-named = Edit { $name }
notify-channel-enabled = Send to this channel
notify-channel-save = Save
notify-channel-cancel = Cancel
settings-shortcuts = Keyboard shortcuts
settings-shortcuts-hint = Single keys such as G, N, W or ? work as shortcuts outside text fields. Turn them off if they get in the way of speech input or a screen reader. Remembered in this browser.
# WP4-5 offline queue and logging funnel
offline-saved = Saved offline, will sync
offline-waiting = { $count ->
    [one] 1 change waiting
   *[other] { $count } changes waiting
}
offline-dropped = Not saved: { $what } - the server refused this change made offline
offline-sheet = You are offline. Save when you are back.
offline-bulk = You are offline. Bulk edits wait until you are back.
privacy-usage-on = This instance counts, per day and without your name or any titles, how many taps and seconds logging takes (from opening search, a sheet or a next step to the saved entry), and how often these collapsible sections are shown, opened, opened within 2 seconds and open when you save: the Library filters; "More" in the log and play sheets of an item; "Set a goal" on Stats; "Sign-in" in Settings > Account; and, for admins, the invites on Users, "Mail", "Secrets at rest" and "Advanced" on Instance, and "Usage measurement" on Storage and jobs. Totals are kept 180 days; the admin can turn this off.
privacy-usage-off = This instance does not count how logging is used.
admin-setting-usage_funnel = Count the logging funnel
admin-usage-title = Usage measurement
admin-usage-summary = Daily totals without users, kept 180 days
admin-usage-what = Counted per day: how a log went from its start (search opened, next step clicked, sheet opened, episode marked, rows selected) to saved or abandoned, in buckets of taps (1 to 5 and more) and seconds (under 2, 5, 10, 30, 60 and over); and how often each of the ten measured collapsible sections (Library filters, the two "More" of the item sheets, "Set a goal", "Sign-in" in Settings, the invites, Mail, Secrets at rest, Advanced and this one) is shown, opened, opened within 2 seconds and open at a save. No user, no item, no title is stored.
admin-usage-on = On
admin-usage-off = Off
admin-usage-csv = Download the daily totals (CSV)
admin-usage-caption = Totals of the last { $days } days (kept { $retention } days)
admin-usage-none = Nothing counted in the last 28 days.
admin-usage-flow = Flow
admin-usage-section = Section
admin-usage-outcome = Outcome
admin-usage-count = Count
# WP4-6 goals, year comparison, people seen most, game groups
stats-compare-title = { $year } so far
stats-compare-lead = From 1 January to { $through }, next to the same days last year (to { $last }).
stats-compare-none = Nothing logged in either period yet.
stats-compare-value = { $this } in { $year }, { $last } in { $last_year }
stats-often-with = Most often with
stats-often-none = Log something with a person and they show up here.
stats-often-together = { $count ->
    [one] once together
   *[other] { $count } times together
}
goals-heading = Goals
goals-lead = Optional and only for you. A goal counts what you log in its year, nothing more.
goals-set = Set a goal
goals-set-summary = A yearly number for a group and a measure
goals-title = { $group }: { $measure }
goals-progress = { $done } of { $target } in { $year }
goals-bar = { $title } in { $year }
goals-edit = Edit
goals-edit-named = Edit goal { $title }
goals-delete = Delete
goals-delete-named = Delete goal { $title }
goals-delete-title = Delete the goal { $title }?
goals-delete-body = Only the goal goes; everything you logged stays.
goals-year = Year
goals-group = Group
goals-metric = Measure
goals-target = Target
goals-target-hint = A whole number, 1 or more.
goals-target-invalid = Enter a whole number of 1 or more as the target.
goals-error-conflict = There is a goal for this year, group and measure already. Edit that one instead.
goals-save = Save goal
companions-tab-groups = Game groups { $count }
groups-list = Game groups
groups-new = New group
groups-new-title = New game group
groups-edit-title = Edit { $name }
groups-empty = No game groups yet
groups-empty-body = A group is the people you play with often. Choosing it fills in the players of a play in one step.
groups-members = With { $names }
groups-no-members = Nobody in this group any more.
groups-plays = Plays as this group
groups-last = Last: { $title }, { $when }
groups-never = Not played as a group yet.
groups-log = Log a play with...
groups-log-named = Log a play with { $name }
groups-menu-named = Actions for { $name }
groups-edit = Edit
groups-delete = Delete
groups-delete-title = Delete { $name }?
groups-delete-body = { $count ->
    [0] The group goes from People and the play sheet.
    [one] The group goes from People and the play sheet. Its play stays in your history.
   *[other] The group goes from People and the play sheet. Its { $count } plays stay in your history.
}
groups-deleted = { $name } deleted.
groups-name = Name
groups-members-legend = Members
groups-need-members = Choose at least one person.
groups-save = Save group
groups-created = { $name } created.
groups-saved = { $name } saved.
log-groups = Groups
search-target-log-group = Choosing a game logs a play with { $name }.

# UX phase 4 review (web)
catalog-title-unavailable = Title unavailable - source data expired
offline-what-status = status { $status }
offline-what-rating = rating
offline-what-notes = notes
offline-what-add = added to the library
offline-full = Not saved: { $count } changes are already waiting, the most this browser keeps. Go back online to send them first.
offline-undo-sending = Not undone: this change is being sent right now. Change it back once it is synced.
offline-undo-sent = Not undone: this change was already synced. Change it back on the item page or in History.
signout-waiting-title = { $count ->
    [one] One change is not synced yet
   *[other] { $count } changes are not synced yet
}
signout-waiting-body = They were made while the server could not be reached and are kept in this browser until it can be. Signing out now discards them.
signout-stay = Stay signed in
signout-discard = Sign out and discard
library-undo-failed = Undo did not work: { $error }
library-undo-expired = This change can no longer be undone here (the undo expired after an hour).
library-bulk-too-large = Too many entries to change at once with Undo. Select fewer and try again.
admin-error-self_change = You cannot do this to your own account here. Use Settings > Account instead.
admin-health-mail-result-not_configured = mail is not set up
admin-health-mail-result-connection = the mail server could not be reached
admin-health-mail-result-auth = the mail server refused the sign-in
admin-health-mail-result-rejected = the mail server refused the message
admin-health-mail-result-send_failed = sending failed
log-coop-result = How it ended
log-coop-none = No result
log-coop-lost = We lost
log-scores = Scores
log-scores-player = Player
log-scores-score = Score
log-scores-won = Won
log-scores-won-name = { $name } won
log-winner-reset = Use the highest score
log-winner-none = No winner
log-winner-hint = The highest score is marked as the winner. Choose another if needed.
log-winner-live = { $name } is marked as the winner (highest score).
log-winner-live-none = No winner is marked.
history-play-coop-none = Played together
item-section-plays = Plays
item-times-total = { $line }, { $time } in total
plays-empty = No plays logged yet.
plays-caption = Plays of { $title }, newest first
plays-col-date = Date
plays-col-players = Players
plays-col-scores = Scores
plays-col-result = Result
plays-col-duration = Duration
plays-newest = The newest { $shown } of { $count } plays are listed.
plays-total = Plays
plays-time = Time played
plays-time-none = not logged
plays-rate = Win rate
plays-rate-none = no results yet
plays-rate-value = { $percent }%
plays-rate-detail = { $wins } of { $decided } { $decided ->
    [one] play
   *[other] plays
} with a result in your own log
plays-bare = { $count ->
    [one] One play has no details (players, result, duration). It counts as a play, not in the time or the win rate.
   *[other] { $count } plays have no details (players, result, duration). They count as plays, not in the time or the win rate.
}
plays-duration-h = { $hours } h
plays-duration-hm = { $hours } h { $minutes } min
stats-play-results = Your results
stats-play-open = { $count ->
    [one] One play is not counted (no result, or without you).
   *[other] { $count } plays are not counted (no result, or without you).
}
stats-tile-untimed-movies = { $count ->
    [one] length not known yet
   *[other] lengths not known yet
}
stats-tile-untimed-shows = runtimes not known yet
stats-tile-untimed-books = no pages logged yet
stats-tile-untimed-games = no play time logged yet
