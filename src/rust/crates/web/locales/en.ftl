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
