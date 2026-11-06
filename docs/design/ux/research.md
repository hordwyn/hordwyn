# UX research: how people use media trackers

Status: v1, researched 2026-11-06. Input for `design.md`; findings are referenced there as
`R-nn`.

## Method and honesty note

Sources were fetched on 2026-11-06: app store listings, vendor help centres and roadmaps,
forum and community threads (mostly through snapshot mirrors, since reddit.com and most
mirrors were blocked from this host), independent blogs, two student usability studies,
NN/g, Baymard, GOV.UK, W3C, Apple, Google and design-system documentation, and the providers'
own API terms. Where a page could not be opened and only a search-result snippet was seen,
the entry says "search snippet". Numbers are quoted only when they appear on the cited
page. Nothing here is from a GPL/AGPL/LGPL project's code or documentation (KICKOFF rule 6);
UI patterns are described in our own words (DESIGN D-18), never copied.

There are **no public heatmaps, funnels, retention or A/B data for any tracker app**
(Letterboxd, Trakt, Simkl, TV Time, Serializd, Goodreads, StoryGraph, Hardcover, Backloggd,
HowLongToBeat, Grouvee, BG Stats, BGG, Last.fm, Pocket Casts, Discogs). The closest real
evidence is: two student studies with participants and metrics (one with click heatmaps from
a custom browser extension), vendor usability notes from 2008, public roadmap vote counts,
app-store rating counts and review text, and general research from NN/g and Baymard. Section
J lists everything that was looked for and not found.

Source quality labels: [primary] vendor page, help centre, roadmap, official blog, standard;
[study] usability study or academic work; [community] app store review, forum, snapshot of a
community thread; [blog] individual or media blog; [vendor-roundup] written by someone
selling a competing product (weighted down); [opinion] my own judgement.

---

## A. Why people track, and what keeps them tracking

**R-01. Board gamers log plays to remember what friends played and liked, to see their own
scores improve, to find unplayed games, to keep a diary of sessions, and for stats such as
the H-index.** The counter-view (logging delays teardown and pulls attention from the
table) exists too. Older polls add cost-per-play and "what gets played most and least" as
motives, and time plus gaps at conventions as the frustrations.
Evidence: Meeple Mountain, "Top 6 reasons why I log my plays", 2019,
https://www.meeplemountain.com/top-six/top-6-reasons-why-i-log-my-plays [blog];
Meeple Mountain counter-piece, 2019, https://www.meeplemountain.com/?p=12809 [blog];
Opinionated Gamers poll, 2011,
https://opinionatedgamers.com/2011/01/26/poll-do-you-keep-a-log-of-the-games-that-you-play/
[community].
Implication: per-person history and plays-per-game are the core value; logging must be
fast enough to do while packing up.

**R-02. Rating granularity: half stars were the longest-running Goodreads request, and
"no rating" must be a legitimate state.** A 2013 Goodreads poll shows 4,820 votes with
61.1% choosing "some books fall between whole stars"; Goodreads shipped half and quarter
stars in October 2026 after "more than a decade". A minority objected to more granularity
(fear of re-rating, slippery slope) and asked for a neutral option and a way to mark DNF
without a low rating. Scales differ by medium: Backloggd and Hardcover 5 stars in half
steps, StoryGraph quarter steps, BGG 1-10; a Chrome extension converting Backloggd to 0-10
has about 12 users (search snippet), suggesting little demand for a bare 10-point input.
Evidence: https://www.goodreads.com/poll/show/81672-do-you-think-goodreads-should-implement-half-stars-into-its-rating-syste
[community, 2013];
https://www.goodreads.com/topic/show/1168498-any-way-to-rate-a-book-with-1-2-stars
[community, 2013]; Book Riot, 2026-10-06, https://bookriot.com/goodreads-half-star-ratings/
[blog]; https://www.twoaveragegamers.com/backloggd-review/ [vendor-roundup].
Implication: store 0-100 (already DEC-018), render 5 stars with half steps, make whole-star
taps still work, and show "Not rated" as a first-class state; never force re-rating.

**R-03. Data ownership is a stated reason people keep spreadsheets or leave services.**
TV Time (26.4M lifetime installs per Appfigures, cited by TechCrunch) shut down on
2026-07-15; users scrambled for exports and a competitor shipped a migration tool. TV Time
was criticised for having no import/export. Simkl's FAQ lists fear of losing history,
fixed fields and wariness of social/cloud as reasons people stay on spreadsheets.
Evidence: TechCrunch, 2026-07-02,
https://techcrunch.com/2026/07/02/popular-tv-tracking-app-tv-time-is-shutting-down-as-company-focuses-on-ai
[media]; 9to5Mac, 2026-07-09,
https://9to5mac.com/2026/07/09/as-tv-time-prepares-to-shut-down-justwatch-launches-migration-tool/
[media]; AlternativeTo reviews of TV Time, https://alternativeto.net/software/tvshow-time/about
[community]; Simkl docs,
https://docs.simkl.org/how-to-use-simkl/faq/frequently-asked-questions/why-do-people-still-track-media-on-spreadsheets-notes-or-keep
[primary, vendor].
Implication: free export in a documented format and importers for the common sources are
trust features; a private-by-default stance is part of the pitch.

**R-04. Yearly reading challenges distort behaviour and cause anxiety.** An r/goodreads
thread titled "Goodreads taking the fun out of reading" (28 points, 60 comments) had the top
advice "Put a goal of 12. Problem solved." (116 points) and "stop caring so much about the
challenges and the numbers" (64 points); bloggers describe choosing shorter books, skipping
rereads and omitting comics and audiobooks because others might think it is cheating.
Evidence: snapshot of r/goodreads,
https://reddit.sentinel-team.org/posts/1sej4r8/snapshots/2026-04-07T05%3A23%3A43.840897Z
[community]; Book Riot, 2019, https://bookriot.com/i-quit-my-goodreads-challenge/ [blog];
Substack, 2025-01-11, https://offmike.substack.com/p/why-my-goodreads-reading-challenge [blog].
Implication: goals are opt-in and private, never shown as "behind schedule" on the home
page; stats compare with the same period last year instead.

**R-05. Logging that takes a phone detour or many steps kills the habit within months.** A
six-month Backloggd user reports about 45 seconds to log via the mobile site versus about
10 seconds in a native app and says the missing app "eventually broke my habit"; Grouvee
users built a "gaming diary" workaround and called it tedious ("about ten steps per entry").
Evidence: https://www.twoaveragegamers.com/backloggd-review/ [vendor-roundup, 2026];
https://discuss.grouvee.com/t/feature-suggestion-gaming-diary/947 [community, 2017-2018].
Implication: the log action must be reachable in a few taps on a phone (PWA/home-screen at
least), with everything prefilled.

---

## B. Logging friction

**R-06. Mark-watched must be one tap with zero mandatory follow-up.** TV Time users
complained that marking an episode watched triggered prompts to pick a favourite character
and rate every episode ("super annoying"), that episodes had to be marked one by one after
a redesign, and that remaining-episode counts were hidden. Letterboxd reviews praise the
logging loop and complain about ads and crashes "after logging a film as watched". Trakt's
2025 redesign drew "too many taps to accomplish simple tasks".
Evidence: MacRumors comments, 2026-07-02,
https://macrumors.com/2026/07/02/show-tracking-app-tv-time-shutting-down [community];
https://justuseapp.com/en/app/431065232/tv-time-track-shows-movies/reviews [community
aggregator]; Letterboxd App Store, https://apps.apple.com/us/app/letterboxd/id1054271011
[community]; https://justuseapp.com/en/app/1514873602/trakt/reviews [community aggregator].
Implication: one primary action per item, auto-advance to the next episode, rating as an
optional secondary control, and "N left" always visible.

**R-07. "Mark all previous episodes" is the most repeated request across TV trackers; the
best implementations ask once per show.** Simkl prompts "mark all previous episodes?" with
Yes / No / Never for this show and offers mark season / mark all seasons / unmark all; an
older app marked earlier episodes when the last episode of a season was ticked, with the
prompt switchable off; Serializd reviewers ask for it and praise it where present; a media
player's users could not find bulk marking because it hid behind a long-press.
Evidence: Simkl docs,
https://docs.simkl.org/how-to-use-simkl/core-features/content-tracking/tracking-content/progress-tracking
[primary]; The Sweet Setup on Television Time, 2016, https://thesweetsetup.com/?p=6400
[blog]; Serializd AU App Store, https://apps.apple.com/au/app/serializd/id1581244120
[community]; Firecore community (player, not a GPL project doc),
https://community.firecore.com/t/marking-a-season-as-watched-nothing-happens-when-you-long-press-it-direct-mode/48427
[community].
Implication: ship "mark this and all previous", "mark season" and "mark show" as visible
menu items with a per-show "never ask again".

**R-08. Default to "now" with a one-tap date change; make undo symmetric.** Trakt requires
choosing a date for each history entry; its roadmap carries "Date watched entry is slower in
the new version" (7 votes), reviewers found a season marked watched "hard to undo", and
long-press was needed to reveal more options. Letterboxd auto-removes a logged film from
the watchlist, and un-marking requires deleting diary entries first (search snippet).
Evidence: Trakt roadmap, https://roadmap.trakt.tv/ [primary];
https://justuseapp.com/en/app/1514873602/trakt/reviews [community aggregator]; Woodstock
Film Festival blog on Letterboxd watchlist behaviour,
https://woodstockfilmfestival.org/blog/2023/8/4/5-reasons-you-should-be-on-letterboxd [blog].
Implication: "now" preselected, "Yesterday" and "Pick a date" one tap away, bulk actions in
a visible menu, Undo on every toast.

**R-09. People backfill in two tiers (watched, date unknown; dated diary entry) and need
range-based dates for shows.** Letterboxd distinguishes an undated "watched" flag from dated
diary entries (undated films are missing from diary and year counts, per FAQ snippets);
users mark everything remembered as watched and only date what they know. Simkl's bulk
episode editor offers release date / now / custom / date range (spreading episodes evenly in
the evening) with a review screen and warns that "now" gives every episode the same
timestamp; a Serializd user asks to log a show over a date range.
Evidence: Letterboxd FAQ (search snippets; fetch 403) https://letterboxd.com/faq/ ;
Simkl docs,
https://docs.simkl.org/how-to-use-simkl/core-features/content-tracking/tracking-content/bulk-episode-editor
[primary]; https://justuseapp.com/en/app/1581244120/serializd/reviews [community aggregator];
r/Letterboxd thread (search snippet)
https://lr.in.psf.lt/r/Letterboxd/comments/1ftwv1x/did_you_logged_movies_that_you_watched_before_you .
Implication: "date unknown" is a legal value that still counts; offer release-date and
date-range backfill; stats tolerate unknown dates.

**R-10. Rewatches and rereads are under-served everywhere and must be first-class.**
Serializd shows only the latest log per episode and users forget the rewatch toggle; Simkl
models rewatches as sessions but behind a paid tier; Trakt users want one-click season
rewatch. StoryGraph stores each reread as its own row (intentional, some users see it as a
bug because it inflates counts); Hardcover supports multiple dates and formats per read
(search snippet); Goodreads' CSV carries only the latest finish date.
Evidence: https://justuseapp.com/en/app/1581244120/serializd/reviews [community aggregator];
Simkl API rewatch guide, https://api.simkl.org/guides/rewatches [primary]; Trakt roadmap
https://roadmap.trakt.tv/ [primary]; StoryGraph help,
https://thestorygraph.freshdesk.com/support/solutions/articles/79000141973-tracking-rereads-on-the-storygraph
[primary]; StoryGraph roadmap bug,
https://roadmap.thestorygraph.com/bugs/posts/reread-book-counts-as-separate-book [primary].
Implication: pre-tick "rewatch" when an item is already Done; keep every play; show
distinct titles and total plays as separate counters; never paywall it.

**R-11. A dense log dialog and an ambiguous "Log" verb cost users.** Letterboxd's log
dialog carries search, diary checkbox, date, rewatch, spoiler, tags, stars, like, privacy
and draft. A 27-participant study found that many users did not understand the "Log"
button (its click heatmap showed almost no clicks during the review task), confused rating
with logging, found "Watch" vs "Watchlist" ambiguous, and were unsure a saved list had
succeeded because they were not taken to it.
Evidence: Letterboxd about/FAQ page for the dialog fields,
https://letterboxd.com/about/faq/ [primary]; Telecom Paris Quant UX study, 2026-02-27,
https://quantux.telecom-paris.fr/?p=641 [study].
Implication: label the action with the verb people use ("Watched"), keep the quick path to
one tap, disclose date/rewatch/review progressively, and confirm saves visibly.

**R-12. Fast board-game logging comes from prefilling: game, date, location, players,
scoring type per game, and "log another like the last one".** BG Stats' flow pre-fills
from a game, location, player or previous play; per-game scoring type (high/low/none/co-op/
team); App Store 4.9 from 458 ratings; a reviewer praises "the ease and speed at which I can
record my plays". Gaps users cite: campaigns, filtering by expansion, and that a player
added to a play does not get their own record.
Evidence: Meeple Mountain review of BG Stats, 2017,
https://www.meeplemountain.com/articles/board-game-stats-app-review-recording-your-plays-is-simple-and-easy
[blog]; App Store, https://apps.apple.com/us/app/id892542000 [community].
Implication: prefill the play sheet from the last play of that game; expansions and
campaign grouping as first-class fields; a linked player sees the play in their own history
(which the companion model already does).

**R-13. Video-game trackers lack per-session logging, so users leave for spreadsheets.**
Grouvee users proposed a diary (date, play time, co-players) and stressed "easy and quick to
do"; a 2026 blogger left Backloggd because it does not track hours or spend and wanted
cost-per-hour and completion by genre; HowLongToBeat's tracking is described as "bolted on".
Evidence: https://discuss.grouvee.com/t/feature-suggestion-gaming-diary/947 [community];
https://sheetflux.pages.dev/blog/backloggd-alternative-spreadsheet [vendor-roundup, 2026];
https://www.twoaveragegamers.com/howlongtobeat-review-still-the-best-free-answer-to-how-long-is-this-game/
[vendor-roundup].
Implication: a light session entry (date, minutes, note, co-players) besides status; derive
hours and cost-per-hour.

**R-14. Progress units differ: pages, percent, chapter, minutes; people also want pace and
projections.** StoryGraph's stats request list asks for pages per day, projections and
spreading pages across start/finish dates when no daily log exists; users ask for chapter
logging (search snippet) and e-reader sync.
Evidence: StoryGraph roadmap "Stats v5", Dec 2025,
https://roadmap.thestorygraph.com/features/posts/stats-v5 [primary]; StoryGraph App Store,
https://apps.apple.com/us/app/-/id1570489264 [community].
Implication: accept pages, percent, chapter or minutes and convert; derive pace from dates
when there is no intermediate log.

**R-15. Bulk editing is the long-tail pain in big libraries.** StoryGraph's "batch edit"
request has 287 votes across six merged posts, was promised in May 2024, retracted in
December 2024 and is still open in September 2026; commenters have 900+ books to tag and
propose a table view with multi-select.
Evidence: https://roadmap.thestorygraph.com/requests-ideas/posts/batch-edit-book-tags
[primary, with community comments].
Implication: a list/table view with multi-select and bulk status/tag/list/owned actions,
early.

**R-16. A private note on every item, regardless of status, removes the main reason tags
sprawl.** The top-voted StoryGraph idea seen (370 votes, open since 2021) is notes on
to-read items: why it was added, who recommended it, where to get it; users abuse tags for
this. Trakt's redesign removed private notes and users complained. Discogs users rely on
the notes field heavily.
Evidence: https://roadmap.thestorygraph.com/requests-ideas/posts/add-notes-field-to-to-read-list
[primary]; Trakt App Store reviews, https://apps.apple.com/us/app/trakt/id1514873602
[community]; Discogs App Store, https://apps.apple.com/us/app/id1036449551 [community].
Implication: free-text private note and a "recommended by" line on every library entry;
notes searchable.

**R-17. Silent state changes destroy trust; every automatic mutation needs a visible
history and undo.** Apple Podcasts users report episodes marked played before being
started; Pocket Casts users report their queue cleared and stats vanishing (threads
unresolved for over a year); Last.fm app reviews complain of missing, doubled and
misattributed scrobbles while praising the stats; Serializd users report accidental double
logging skewing year stats.
Evidence: https://discussions.apple.com/thread/255619497 [community, 2024];
https://forums.pocketcasts.com/forums/topic/up-next-gets-cleared-automatically/ [community];
https://worldsapps.com/reviews-last-fm [community aggregator];
https://apps.apple.com/us/app/serializd/id1581244120?l=en-us [community].
Implication: webhook and import events are shown with their source in history, duplicates
are detected (DEC-159 already does this for webhooks), and everything can be edited or
undone.

---

## C. Status, shelves, tags, lists

**R-18. Users do not understand exclusive shelves unless the control makes the
distinction visible.** Goodreads' own 2008 usability test found members did not grasp that a
book must be on exactly one of three default shelves, many did not know custom shelves
existed, and adding to "to-read" took several clicks although it was the most popular
action; the fix was a dropdown that separates exclusive status from other shelves and a
one-click hover for the three statuses.
Evidence: Goodreads blog, 2008-06-11,
https://www.goodreads.com/blog/show/96-new-way-to-shelf-your-books [primary].
Implication: status is a single-select control; tags and lists are visibly different
controls; the most common action (plan it) is one click.

**R-19. Non-exclusive, non-exhaustive statuses make users invent their own.** A Grouvee
newcomer could not tell when to move a game from Playing to Played or what to do with a
paused game; the developer confirmed "played" meant any capacity and considered splitting
it; users created "Started" and "Abandoned" shelves. Trakt lacked Dropped/On hold for years
and added Up to date / In progress / Dropped drill-downs only in 2026.
Evidence: https://discuss.grouvee.com/t/am-i-using-this-site-correctly/6766 [community,
2020]; Trakt changelog, https://roadmap.trakt.tv/en/changelog [primary].
Implication: Planned, In progress, Done, Paused, Dropped as the complete, exclusive set
(the kinds crate already has them), plus kind-specific extras.

**R-20. "Did not finish" must be built in, record progress at abandonment, and be
private by default.** Goodreads launched a default DNF shelf in March 2026 as "one of your
most requested features", auto-converting custom shelves named dnf/abandoned/dropped and
recording how far the reader got; an author argues public DNF labels are "cruel and
unnecessary" because reasons are often personal.
Evidence: Goodreads blog, 2026-03-12, https://www.goodreads.com/blog/show/3092 [primary];
https://rhodabaxter.com/2026/03/22/why-i-wont-be-using-the-goodreads-dnf-shelf/ [blog].
Implication: Dropped keeps the progress event, allows no rating, is private by default and
is mapped from common custom shelf names on import.

**R-21. Ownership is orthogonal to play status in board games.** BGG collection flags are
Owned, Previously Owned, For Trade, Want in Trade, Want to Buy, Want to Play, Preordered,
plus a wishlist with priorities 1-5 (search snippets; BGG pages returned 403).
Evidence: https://boardgamegeek.com/wiki/page/My_Geek [primary, search snippet only].
Implication: ownership flags on the entry (already in D-9a), shown as chips and filters,
never mixed into the status control.

**R-22. Single-folder organisation frustrates collectors; many-to-many tags plus notes
work better.** A Discogs collector with folders like "death metal", "dutch", "coloured
vinyl" calls one-folder-per-item "too restrictive" and proposes private tags with a tag
cloud; Discogs added search by notes and custom fields only in 2025.
Evidence: https://vinylanddata.blogspot.com/2019/06/maintaining-collection-with-discogs.html
[blog]; https://apps.apple.com/us/app/id1036449551 [community].
Implication: flat user tags, multi-valued, filterable and searchable; no folders.

**R-23. A status that depends on provider metadata breaks expectations.** Simkl refuses
"Completed" for a show whose provider status is still "Returning" even when every aired
episode is watched; a Trakt switcher complained a show "can't be marked as completed".
Evidence: https://docs.simkl.org/how-to-use-simkl/faq/frequently-asked-questions/why-cant-i-add-a-show-that-has-not-ended-to-completed
[primary]; Trakt App Store, https://apps.apple.com/us/app/trakt/id1514873602 [community].
Implication: users set any status; "Up to date" is a derived badge, never a gate.

---

## D. Navigation and information architecture

**R-24. The biggest book tracker fails on IA: a 2020 remote study gave Goodreads a SUS of
45/100 and blamed convoluted IA and content overload; a 2025 cursor-tracking study found
Explore vs Lists redundant, the Community section weakest, and new users clicking an empty
"My Books".** Praised items were the rating control and the search bar. Student redesigns
converge on three top-level areas.
Evidence: https://nicolat8071.github.io/web-portfolio/project-3.html [study, 2020, 6 live +
14 tree-test participants]; https://quantux.telecom-paris.fr/?p=313 [study, 2025, 16
participants]; https://johnsonzsong.github.io/goodreads [portfolio case study, no testing].
Implication: few top-level sections; never land a new user on an empty library without an
add path.

**R-25. Search-first can be the slow path; profile-owned things need a hub; labels must
match users' words.** In the Letterboxd study, users adding a favourite via search had 40%
success and about 112 s versus 100% and about 64 s via the profile; the task averaged 55
extra clicks; "Avatar" as a label for the profile picture caused errors; the Diary and its
date filters had low visibility.
Evidence: https://quantux.telecom-paris.fr/?p=641 [study, 27 participants, click and
scroll heatmaps from a custom extension].
Implication: a user's own things (library, history, lists, people) are top-level and
reachable from the item; plain labels; the palette is an accelerator, not the only path.

**R-26. About five top-level sections; do not give each media type its own tab.** A Pratt
card-sort and tree-test study of IMDb found users describing many sections as cluttered
("so many sections, it feels like Wikipedia"), proposed combining Movies and TV, and scored
70% success on the new tree. NN/g: tab bars work only with few options; "the fewer tabs,
the better"; Material 3 navigation bars are for three to five destinations; Apple suggests
five or fewer and warns that overflow "More" hides content; TV Time users hated having
their shows split across tabs.
Evidence: Pratt IXD, 2024-12-12,
https://ixd.prattsi.org/2024/12/ui-ux-case-study-imdb-web-mobile-revamp-youve-all-been-waiting-for/
[study]; NN/g tabs, https://www.nngroup.com/articles/tabs-used-right/ [primary]; NN/g mobile
navigation patterns, https://www.nngroup.com/articles/mobile-navigation-patterns/ [primary];
Android developers restating M3, https://developer.android.com/develop/ui/compose/components/navigation-bar
[primary]; Apple HIG tab bars (JSON endpoint),
https://developer.apple.com/tutorials/data/design/human-interface-guidelines/tab-bars.json
[primary]; https://justuseapp.com/en/app/431065232/tv-time-track-shows-movies/reviews
[community].
Implication: five sections (Home, Library, History, Lists, People); kinds are a filter inside
Library, not navigation.

**R-27. Mixing media types is wanted by a minority and feared by the core; a persistent
type filter resolves it.** About 30% of mined Letterboxd reviews asked for TV tracking
(secondary synthesis), yet users demanded a toggle so TV logs do not clutter film diaries,
and the 2026 announcement of TV tracking drew a largely negative reaction. All-in-one search
returns a film, a novel and a game with the same title in one list (search snippet).
Evidence: Contra concept study, 2026-09-17,
https://contra.com/p/6ZPPWlRX-uiux-redesign-for-letterboxd-app [blog, concept];
https://www.netflixjunkie.com/hollywood-news-fans-boo-letterboxds-decision-to-add-tv-show-tracking-to-the-beloved-movie-only-platform/
[media, 2026-07-02]; Substack synthesis, 2025-02-22,
https://punchamoorthee.substack.com/p/a-deep-dive-into-letterboxd [blog].
Implication: one library with a group/kind filter that persists across library, history,
stats and feeds; search results grouped by kind; a remembered default group.

**R-28. On an item page the tracking state is the hero; history needs day grouping; "Up
next" is for started shows only.** Trakt's 2025 redesign put metadata ahead of progress,
lost day grouping in history, mixed unstarted watchlist items into Up next, and was called
"cluttered and confusing"; roadmap items ask to restore the vertical episode list and hide
the social feed; the changelog later separated "Up to date" from ended shows.
Evidence: AlternativeTo, 2025-11-02,
https://alternativeto.net/news/2025/11/trakt-unveils-a-full-redesign-of-its-movie-and-tv-tracking-platform-for-web-and-mobile/
[media]; https://apps.apple.com/us/app/trakt/id1514873602 and
https://apps.apple.com/ca/app/trakt/id1514873602 [community]; Trakt roadmap and changelog,
https://roadmap.trakt.tv/ and https://roadmap.trakt.tv/en/changelog [primary].
Implication: item page order = progress + next step + actions, then history, then
metadata; vertical episode lists; day-grouped history; "Continue" only for in-progress.

**R-29. Hidden navigation hurts; vertical nav scales; mobile bottom bars take 3-5 items;
no hamburger.** NN/g's 179-participant study: hidden nav used in 27% of desktop cases vs
48-50% visible, discoverability down more than 20%, desktop tasks at least 39% slower. Left
rails suit broad IAs; top bars suit few categories. Hub-and-spoke works for task-based apps
at the cost of one extra step per hub return. Apple: never hide a tab because its section
is empty.
Evidence: NN/g hamburger menus, 2016, https://www.nngroup.com/articles/hamburger-menus/ ;
NN/g vertical nav, 2021, https://www.nngroup.com/articles/vertical-nav/ ; NN/g menu design,
2024, https://www.nngroup.com/articles/menu-design/ ; NN/g mobile patterns (R-26); Apple tab
bars (R-26) [all primary].
Implication: visible five-item top bar on desktop, five-item bottom bar with a "You" hub on
mobile, settings in a side nav, no hamburger.

**R-30. Cards for browsing, lists for comparing; default sort must show breadth; rating
sort must weight count.** NN/g: cards suit heterogeneous browsing, lists suit scanning one
attribute and comparing; cards are "not a modern substitute for lists". Baymard: a default
order that hides breadth made users think catalogues were narrower (24% of sites fail);
users expect rating sort to weight the number of ratings (64% of sites do not) and trusted
averages only from about 5 ratings.
Evidence: NN/g cards, 2016, https://www.nngroup.com/articles/cards-component/ ; Baymard
default sort, 2021, https://baymard.com/research-articles/default-sort-type ; Baymard rating
sort, 2024, https://baymard.com/blog/sort-by-customer-ratings [all primary].
Implication: grid and list views; default sort "last activity"; weighted community rating
if ever shown.

**R-31. Search box visible on every page; suggestions capped and grouped by scope; full
keyboard support; palettes are a practitioner pattern, not a studied one.** NN/g: a type-in
field on every page, top of page; switching from a link to a box raised search use 91%;
suggestions show scope in a lighter line. Baymard: at most 10 suggestions on desktop, 4-8
on mobile; arrows move, Return submits, copy the focused suggestion into the field. The
Ctrl+K palette guidance (same shortcut closes, fuzzy match, show why a result matched,
icons per command) comes from a product blog; no NN/g or Baymard article on command
palettes exists.
Evidence: NN/g search visible, 2001, https://www.nngroup.com/articles/search-visible-and-simple/ ;
NN/g suggestions, 2018, https://www.nngroup.com/articles/site-search-suggestions/ ;
Baymard autocomplete, 2022, https://baymard.com/blog/autocomplete-design ; Superhuman blog,
2021, https://blog.superhuman.com/how-to-build-a-remarkable-command-palette/ [blog].
Implication: keep the palette, add a visible search pill in the bar on every page, group
results by source and kind, cap groups at 8 with "more", complete keyboard handling.

---

## E. Search results, empty states, first run, imports

**R-32. A no-results state must be unmistakable, keep the query, and offer a way
forward.** Users often miss "nothing found"; the page should state it, keep the query in
the box, suggest spelling fixes or related queries, and never joke. Response times: up to
1 s needs no feedback, beyond 1 s show that the system is working, beyond about 10 s show
progress.
Evidence: NN/g no-results SERP, 2014, https://www.nngroup.com/articles/search-no-results-serp/ ;
NN/g response times, https://www.nngroup.com/articles/response-times-3-important-limits/ ;
NN/g progress indicators, 2014, https://www.nngroup.com/articles/progress-indicators/ [all
primary].
Implication: the palette shows "Asking X..." after the first second, a no-results card
with the query, a spelling suggestion, "add by hand" and which sources were skipped.

**R-33. Empty states show system status, teach what the area is for, and link straight to
the task; "configuration required" is its own type.** NN/g: totally empty states cause
confusion about whether the system works; distinguish "nothing matches", "loading" and
"error". Carbon: types no-data / user-action / error-management (permissions, configuration
required with the first step, systems issue with a log pointer); one primary action per
screen when several empty states show. GitLab Pajamas: a "Configuration required" state with
a Configure button for people with rights and "invite someone to configure" for others.
Atlassian: heading required, imperative CTAs, tone matched to the cause, no negative
illustrations.
Evidence: NN/g empty states, 2021, https://www.nngroup.com/articles/empty-state-interface-design/ ;
Carbon, https://carbondesignsystem.com/patterns/empty-states-pattern/ ; GitLab Pajamas,
https://design.gitlab.com/patterns/empty-states ; Atlassian,
https://atlassian.design/components/empty-state/usage [all primary].
Implication: every empty library, history, inbox and list screen names the action and
links to it; "no data source" is a configuration-required state with an admin action and a
member "ask admin" action, distinct from "provider failing".

**R-34. First-run tutorials do not help; wizards are right for setup; checklists let
people finish in any order.** NN/g found no task-performance improvement from a card-deck
tutorial and recommends self-explanatory UI and brief, skippable, contextual hints. Wizards
suit novices and rare tasks such as configuration: show all steps, descriptive buttons,
allow exit and resume, self-sufficient steps. GOV.UK's task list lets users complete tasks
in any order across sessions with Completed/Incomplete states, the whole row as the link.
Practitioner guidance: 3 to 7 steps, skip options for optional data, minimal and advanced
paths, validate immediately after setup, avoid external calls that block.
Evidence: NN/g onboarding, 2020, https://www.nngroup.com/articles/mobile-app-onboarding/ ;
NN/g wizards, 2017, https://www.nngroup.com/articles/wizards/ ; GOV.UK task list,
https://design-system.service.gov.uk/components/task-list/ ; Eleken wizard pattern,
https://eleken.co/blog-posts/wizard-ui-pattern-explained [blog]; hoop.dev on self-hosted
onboarding, https://hoop.dev/blog/optimizing-onboarding-for-self-hosted-software/ [blog].
Implication: a four-step admin wizard with skippable key entry and inline tests, a
"get set up" checklist afterwards, and no welcome carousel for members.

**R-35. Import is the universal onboarding path and is opaque: show progress, a persistent
unmatched list, and what was lost.** Goodreads' export can take up to 24 hours and users ask
"idek if it's working"; Goodreads fails to import non-Latin titles even with ISBN (snapshot);
Goodreads CSV carries only the latest finish date, custom shelves map to tags; a reviewer
whose import "transferred perfectly" gave five stars; Steam imports fail silently on private
profiles; BGG-backed play logging is valued because it pre-populates collection and plays.
Evidence: https://www.goodreads.com/topic/show/22422873-pls-help-how-do-u-export-to-storygraph?page=1
[community, 2023];
https://reddit.sentinel-team.org/subreddits/goodreads/snapshots/2026-03-19T03%3A32%3A38.834608Z
[community]; StoryGraph App Store, https://apps.apple.com/us/app/-/id1570489264 [community];
https://discuss.grouvee.com/t/steam-games-not-importing/2374 [community]; Meeple Mountain
BG Stats review (R-12).
Implication: dry-run preview that says what maps to what and what is lost, progress with
counts and cancel, a persistent unmatched list with manual match, test with non-Latin
titles.

---

## F. Forms, errors, status messages

**R-36. Inline validation on blur is measurably better; validating while typing is worse.**
A 22-user study of six registration-form variants: the best inline variant raised success
22%, cut errors 22%, raised satisfaction 31%, cut completion time 42% and eye fixations
47%; validating after leaving a field beat validating during typing, which produced the
most errors. Baymard: validate on blur, re-validate on every keystroke only after an error
is shown and clear it the instant the input is valid; never validate untouched fields;
positive checkmarks reassure; inline validation does not replace submit-time handling.
Evidence: A List Apart, Wroblewski, 2009,
https://alistapart.com/article/inline-validation-in-web-forms/ [study]; Baymard, 2024,
https://baymard.com/blog/inline-form-validation [primary].
Implication: hints before input, validation on blur, live re-validation after an error, a
green line for "available"; the username normalisation preview is a hint, not a validation.

**R-37. Error messages next to the field, specific, with a fix; an error summary on
submit; one plain password rule.** GOV.UK: red text and border, hidden "Error:" prefix,
specific wording per failure, avoid "please", "sorry", "invalid", "oops" and "This field is
required"; never clear fields; use a summary at the top headed "There is a problem" with
links to each field, focus moved to it. GOV.UK passwords and NIST SP 800-63B: minimum 8
(we use 10), no composition rules, allow paste, offer show-password, check against
breached lists, 5-10 attempts before lockout, do not reveal which of username/password was
wrong.
Evidence: GOV.UK error message, https://design-system.service.gov.uk/components/error-message/ ;
error summary, https://design-system.service.gov.uk/components/error-summary/ ; passwords,
https://design-system.service.gov.uk/patterns/passwords/ ; NIST SP 800-63B 5.1.1.2,
https://pages.nist.gov/800-63-3/sp800-63b.html (page notes it was superseded 2025-08-01;
rules quoted are from the fetched page) [all primary].
Implication: the sign-up form in `design.md` 5.2; "Please check the fields" is removed.

**R-38. Describe the problem, suggest the fix, keep the data; never use toasts for
errors; expose status messages to assistive tech.** NN/g error guidelines: message next to
the cause, icon plus colour, plain language, no codes, no blame, no humour, keep entered
data. NN/g indicators vs validations vs notifications: transient toasts for errors get
missed; a dead integration needs a persistent indicator and an inline notification. GOV.UK
"problem with the service" page: one page for unexpected errors, say what happened to the
data and when to try again, no "500". WCAG 2.2 SC 4.1.3: success/progress via
`role=status`, errors via `role=alert`, no assertive live regions for non-urgent content.
Evidence: NN/g, 2023, https://www.nngroup.com/articles/error-message-guidelines/ ; NN/g,
2024, https://www.nngroup.com/articles/indicators-validations-notifications/ ; GOV.UK,
https://design-system.service.gov.uk/patterns/problem-with-the-service-pages/ ; W3C,
https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html [all primary].
Implication: inline persistent alerts for failing channels, senders and sources; toasts
only for success with Undo; a single friendly service-problem page.

**R-39. Toasts auto-dismiss only for low-stakes confirmations; skeletons for page loads,
spinners for one module, progress bars beyond 10 s.** Atlassian flags: never auto-dismiss
critical warnings or errors; a banner for loss of functionality, inline for required
action. Carbon: toasts about two lines, may auto-dismiss after five seconds, information
must be findable elsewhere; inline notifications do not auto-dismiss. NN/g skeletons: under
1 s show nothing; 2-10 s spinner for a module or skeleton for a page; over 10 s progress
bar; avoid frame-only skeletons; shimmer can distract.
Evidence: Atlassian flag, https://atlassian.design/components/flag/usage ; Carbon
notification, https://carbondesignsystem.com/components/notification/usage/ ; NN/g skeleton
screens, 2023, https://www.nngroup.com/articles/skeleton-screens/ [all primary].
Implication: 10 s undoable toasts in a `role=status` region; shaped skeletons on library
and item pages; progress bars for imports.

---

## G. Mobile and accessibility

**R-40. Touch: design for one-handed use, targets 44-48 px for primary controls, 24 px
WCAG floor, content and actions in the centre, destructive controls away from frequent
targets; bottom sheets for scoped tasks.** Hoober's 2013 observations: 49% one-handed,
36% cradled, 15% two-handed (and a 2017 note withdrawing the old reach diagrams); 2017
research: people prefer and are most accurate in the centre; targets about 7 mm centre,
12 mm at corners. WCAG 2.2 SC 2.5.8: 24 x 24 CSS px minimum. Apple: 44 x 44 pt; Android:
48 x 48 dp. Apple sheets: one at a time, swipe-to-dismiss with a confirm when there are
unsaved changes, grabber, medium detent for progressive disclosure.
Evidence: https://www.uxmatters.com/mt/archives/2013/02/how-do-users-really-hold-mobile-devices.php ;
https://www.uxmatters.com/mt/archives/2017/03/design-for-fingers-touch-and-people-part-1.php ;
https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html ;
https://developer.apple.com/design/tips/ ;
https://developer.android.com/guide/topics/ui/accessibility/apps ;
https://developer.apple.com/tutorials/data/design/human-interface-guidelines/sheets.json
[all primary].
Implication: 44 px buttons on cards and in sheets, the five-item bar at 48 px targets,
log sheets as bottom sheets with a grabber and a dirty-state confirm.

**R-41. WCAG 2.2 AA requirements that shape the token and component choices.** Contrast
4.5:1 for text and 3:1 for large text, UI boundaries and state indicators (1.4.3, 1.4.11;
4.499 fails); focus visible and never removed (2.4.7); focused elements not hidden under
sticky bars (2.4.11, fix with `scroll-padding`); interaction-triggered motion disableable,
`prefers-reduced-motion` maps to OS settings and should replace transforms with fades
(2.3.3); all functionality keyboard-operable, drag needs an alternative (2.1.1); APG
patterns for combobox (`aria-activedescendant`, Escape closes), listbox (type-ahead above
seven options), modal dialog (focus in, trap, return, visible close), tabs (arrow keys,
manual activation when panels load lazily).
Evidence: https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html ;
https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html ;
https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html ;
https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html ;
https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html ;
https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion ;
https://www.w3.org/WAI/WCAG22/Understanding/keyboard.html ;
https://www.w3.org/WAI/ARIA/apg/patterns/combobox/ ; https://www.w3.org/WAI/ARIA/apg/patterns/listbox/ ;
https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/ ; https://www.w3.org/WAI/ARIA/apg/patterns/tabs/
[all primary].
Implication: the `--input` border change (the current zinc-300 on white is 1.48:1), the
ring token, reduced-motion CSS, keyboard alternatives for list ordering, and the palette
staying an APG combobox.

**R-42. Dark mode: light stays the default for most people, offer a toggle, honour the OS
setting, keep text sizes generous in dark.** NN/g summarises Piepenbrock et al. 2013 (better
acuity and proofreading in light mode for normal vision, more so at small sizes) and Legge
et al. 1985 (some low-vision readers faster in dark mode).
Evidence: NN/g dark mode, 2020, https://www.nngroup.com/articles/dark-mode/ [primary].
Implication: system/light/dark tri-state (the app already honours `prefers-color-scheme`),
no body text under 0.85 rem in dark.

---

## H. Data-source credentials, deployment modes, self-hosted patterns

**R-43. TMDB: application key, non-commercial without a written agreement, attribution and
logo required, cache at most 6 months, limits by IP.** The API terms call the licence
non-transferable and non-sublicensable, list "charging users a fee for your application" as
commercial use needing a written agreement, require the TMDB logo and the non-endorsement
notice, forbid caching longer than 6 months and require purging on termination. The FAQ:
free for non-commercial use with attribution. Rate limiting: the 40-per-10-s limit was
disabled in 2019, roughly 40 requests per second remain, respect 429. A 2013 staff forum
answer says limiting is by IP, not key, and that routing all users through one server IP
would trip limits, recommending local caching.
Evidence: https://www.themoviedb.org/api-terms-of-use ; https://developer.themoviedb.org/docs/faq ;
https://developer.themoviedb.org/docs/rate-limiting ;
https://www.themoviedb.org/talk/512403ec760ee37257037fdb (staff, 2013);
https://www.themoviedb.org/about/logos-attribution [all primary].
Implication: instance key; per-user keys buy nothing on one server; the HTTP cache TTL must
stay under 6 months; hosted-for-pay operators need an agreement; the logo slot is an
operator-supplied brand asset.

**R-44. IGDB: application credential by design (Twitch client id and secret, app access
token), 4 requests per second per client, free for non-commercial use.** Twitch's client
credentials flow returns only an app token; secrets are to be safeguarded like passwords.
Evidence: https://api-docs.igdb.com/ (fetched with curl) ;
https://dev.twitch.tv/docs/authentication/ ;
http://discuss.dev.twitch.com/t/new-developer-agreement-and-terms-of-service/7212 (staff
confirming client id public, secret private) [all primary].
Implication: instance credential only, server-side; a shared 4 req/s budget means the
interactive lane needs fair sharing in hosted mode.

**R-45. Podcast Index: free for any purpose including commercial, but no public proxying
of their API, keys confidential and server-side, a specific User-Agent required, unpublished
rate limits, caching encouraged.** Registrations from free email providers are refused.
Evidence: https://api.podcastindex.org/tos_v2.0.html (v2.0, effective 2026-10-04) ;
https://api.podcastindex.org/developer_docs [primary].
Implication: instance key; the instance sends its brand-derived User-Agent (already
derived from the brand); document fair use for large hosted instances.

**R-46. TheTVDB v4 institutionalises per-user credentials: project key plus a subscriber
PIN per end user in the user-supported model, or a negotiated licence; attribution with a
direct link is mandatory.** "v4 API keys are now per-project, and not for use by individual
users"; "all API keys are required to have either a commercial license, or have
subscriptions enabled for end users"; the login call takes `apikey` and optional `pin`;
subscription about $12 a year; FOSS projects get discounts.
Evidence: https://support.thetvdb.com/kb/faq.php?id=62 ; https://github.com/thetvdb/v4-api ;
https://thetvdb.github.io/v4-api/swagger.yml ; https://thetvdb.com/subscribe ;
https://thetvdb.com/api-information [all primary].
Implication: a per-user "TheTVDB PIN" field under Settings > Data sources whenever the
instance key is user-supported; an admin licence-mode switch per source.

**R-47. Open Library and MusicBrainz need no key but strict identification and per-IP
limits.** Open Library: User-Agent with app name and email, 1 req/s unidentified, 3 req/s
identified, no bulk downloads via API. MusicBrainz: no key, meaningful User-Agent, at most
1 call per second per IP or the IP is blocked, non-commercial free.
Evidence: https://openlibrary.org/developers/api ; https://musicbrainz.org/doc/MusicBrainz_API ;
https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting [all primary].
Implication: a contact email field for the instance; the governed client's per-connector
token bucket at 1 req/s; nothing per user. (These services' software is GPL-family; only
their public API policy pages were read, which DEC-106 permits reconsidering with the owner.)

**R-48. Hardcover: personal access tokens belong to one user and must not be pasted into
third-party apps; OAuth is the intended path; queries must run server-side; rate limits are
per token.** "Never ask a user to paste a PAT into your app"; "for anything a user signs
into, use OAuth"; user-owned data only on that user's behalf.
Evidence: https://docs.hardcover.app/api/getting-started/ (fetched with curl) [primary].
Implication: Hardcover is a per-user source by definition: "Connect your Hardcover account"
under the member's Data sources, never an instance key.

**R-49. BoardGameGeek: a bearer token is required (unauthenticated requests return 401
pointing to the policy page), but the policy page itself is behind a Cloudflare challenge
and could not be read.** Per-application vs per-user, rate limits and attribution wording
are unverified (consistent with DEC-107).
Evidence: live request to https://boardgamegeek.com/xmlapi2/thing?id=13 returned 401 with a
pointer to https://boardgamegeek.com/using_the_xml_api (403 from this host).
Implication: treat as an instance token with a "key rejected" state; verify the policy
before shipping the connector.

**R-50. Patterns from MIT/Apache self-hosted apps (licences checked on the repo pages):
global default plus per-user override for preferences, admin-only credentials, test before
save, encrypted server-side secrets, a run-once installer that locks itself, headless bypass
via config, inline error badges for broken integrations.** Seerr: admin API key with
regenerate and env preset, global discover settings that users may override, blocklists
that they may not, documented "feature X needs setting Y" sentences, SMTP configured once.
Homarr: onboarding at `/init` until finished, optional sections skippable, integration
credentials encrypted and server-only, "does not save a new integration until the test
succeeds", per-user/group permissions on an integration. Gitea: web installer writes the
config, `INSTALL_LOCK=true` disables the page, mailer test from the admin summary page.
Rocket.Chat: wizard opens automatically, stays until an admin completes it, runs once,
offline path for network steps. Dashy: secrets via environment variables only, substituted
server-side so they never reach the browser; broken widgets highlighted with a message.
Evidence: https://docs.seerr.dev/getting-started/ , http://docs.seerr.dev/using-seerr/settings/general/ ,
http://docs.seerr.dev/using-seerr/settings/services/ , https://docs.seerr.dev/using-seerr/settings/users/
(Seerr/Jellyseerr, MIT, https://github.com/fallenbagel/jellyseerr);
https://homarr.dev/docs/getting-started/after-the-installation/ ,
https://homarr.dev/docs/management/integrations/ (Homarr, Apache-2.0,
https://github.com/homarr-labs/homarr); https://docs.gitea.com/installation/install-with-docker ,
https://docs.gitea.com/administration/config-cheat-sheet , https://docs.gitea.com/administration/email-setup
(Gitea, MIT); https://docs.rocket.chat/docs/setup-your-workspace (Rocket.Chat, MIT);
https://raw.githubusercontent.com/lissy93/dashy/master/docs/widgets.md (Dashy, MIT) [all
primary].
Implication: the wizard and lock, environment keys shown as read-only, Test before Save on
source cards, encrypted `connector_credentials`, inline status on every integration row.

**R-51. "Configuration required" is a recognised empty-state type with a configure action
for admins and an "ask someone with rights" action for others.** (GitLab Pajamas and Carbon
as in R-33; NN/g wizard guidance as in R-34.)
Implication: the member-facing "Ask admin" button and the admin-facing setup link are the
same state rendered for two roles.

**R-52. Instance-wide vs per-user keys is a per-provider decision; some providers forbid
storing user keys server-side.** A published ADR tabulates the trade-off (operator pays and
shares one rate limit vs per-user quota and one-time key entry); OpenAI's forum states users
may not share keys through bring-your-own-key apps and that apps may not store end-user keys
server-side, the same stance Hardcover takes for PATs. No Hacker News or r/selfhosted thread
specifically on metadata keys was found.
Evidence: https://sartor87.github.io/ai-engineer-interview-concept/adrs/0005-user-supplied-api-key.html
[blog, ADR]; https://community.openai.com/t/openais-bring-your-own-key-policy/14538
[community, staff answer].
Implication: per-user credentials are offered only for sources whose terms define them
(TheTVDB PIN, Hardcover account), never as a generic "use your own key" override.

---

## I. Fonts

**R-53. Verified font licences and the system stack.** Inter: SIL OFL 1.1
(https://github.com/rsms/inter/blob/master/LICENSE.txt). Source Sans 3: OFL 1.1, reserved
name "Source" (https://github.com/adobe-fonts/source-sans/blob/release/LICENSE.md). IBM Plex
Sans: OFL 1.1, reserved name "Plex" (https://github.com/IBM/plex/blob/master/LICENSE.txt).
Geist: OFL 1.1 (https://github.com/vercel/geist-font , https://vercel.com/font). Atkinson
Hyperlegible Next: OFL 1.1 in the Google Fonts repository
(https://github.com/googlefonts/atkinson-hyperlegible-next); the Braille Institute's own
download page (https://www.brailleinstitute.org/freefont/) uses a separate EULA, so take
the googlefonts files. System stacks: GitHub Primer primitives v11.1.0
(https://unpkg.com/@primer/primitives@11.1.0/dist/css/functional/typography/typography.css ,
MIT) and https://systemfontstack.com/ ; Tailwind v4's `--font-sans` is
`ui-sans-serif, system-ui, sans-serif, ...` (https://unpkg.com/tailwindcss@4.1.0/theme.css).
Implication: default to the system stack (no download, no licence); any of the five fonts
may be self-hosted if a brand wants one; never load from a CDN.

---

## J. Where evidence was missing

Looked for and not found, or not reachable from this host:

- Public analytics, funnels, retention, A/B results or vendor heatmaps for any tracker
  (film/TV, books, games, board games, music, podcasts). The only heatmaps are from the
  Telecom Paris student study of Letterboxd (R-25) using a custom extension; the only other
  quantitative behavioural data are the two Goodreads studies (R-24), roadmap vote counts,
  and app-store rating counts. **Design consequence:** the implementation plan includes
  first-party instrumentation of the logging funnel (R-54).
- Reddit: reddit.com, old.reddit.com, web.archive.org and most mirrors were blocked; only a
  few snapshot pages (r/goodreads, r/books, r/GirlGamers, r/jellyfin community thread) were
  readable. No r/52book, r/patientgamers, r/boardgames, r/television or r/selfhosted thread
  on these questions was retrievable.
- Trakt's forum closed in 2026; its threads are search snippets only. Trakt's help-centre
  articles on check-in vs scrobble vs history were not found.
- Letterboxd's welcome/onboarding page and FAQ returned 403; the "watched vs logged"
  wording is from snippets. Letterboxd's TV tracking status after July 2026 is unconfirmed.
- Hardcover's blog, FAQ and CSV format pages returned 403; claims about Hardcover's status
  model and import rest on search snippets, except the API documentation (R-48), which
  was fetched.
- BGG: policy page, wiki and forums behind Cloudflare (R-21, R-49).
- Serializd has no public help centre; its Play Store page returned 404; TV Time's store
  pages are gone, so its review evidence is from aggregators and news comments.
- No usability study with SUS or task times exists for Trakt, Simkl, TV Time, Serializd,
  StoryGraph, Hardcover, Backloggd, HowLongToBeat, Grouvee, BG Stats, Last.fm, Pocket
  Casts or Discogs.
- No study on logging a board-game play with scores, or on half-star vs 10-point rating
  accuracy in hobby trackers.
- No NN/g, Baymard, Smashing or design-system article on command palettes; the only source
  is a product blog (R-31). No NN/g article on toasts (Atlassian and Carbon used instead).
- Material 3 guideline pages (navigation bar, bottom sheets) and Apple HIG HTML pages
  render only with JavaScript; Google's Android developer docs and Apple's JSON endpoints
  were used. Material's standard vs modal bottom-sheet rules were not verified.
- Baymard's error-message guidelines are paywalled; GOV.UK and NN/g used instead.
- The Twitch Developer Services Agreement text is JavaScript-rendered; its "Keys" clause is
  known only from a forum quote confirmed by staff (R-44).
- Whether TMDB's current limits are per IP or per key is stated only in a 2013 staff forum
  post (R-43).
- Podcast Index's newer docs site rendered empty; the ToS and developer docs pages were
  used.
- Shopify Polaris empty-state guidance redirects to an index page.
- The 5-10% unmatched-import figure quoted by a third-party blog is unverified and not
  used.

**R-54. Instrument the logging funnel ourselves.** Because no comparable data exists, the
plan measures taps and time from intent (palette open or next-step click) to "logged", and
sheet abandonment, as aggregated daily `tracing` events without per-user profiles, and
reviews them after four weeks. [opinion, grounded in J]
