# Legacy export fixture

`export.json` is an anonymised extract of the owner's own export of their current library
(Q4; supplied 2026-11-06). The legacy importer is derived only from this file (KICKOFF rule 6).

Extract (owner's choice: anonymised subset):

- 21 of 135 items, covering every kind and source in the export (anime from AniList and
  MyAnimeList, books from Google Books and Open Library, movies and shows from TMDB, video
  games from IGDB) and every history state (completed, dropped, in_progress, on_a_hold).
- Shows keep their first 12 history entries; all other items keep their full history.
- All 12 collection definitions.
- Anonymised: the account name is "Owner Example", every user id is `usr_owner_example`,
  two personal collection names are renamed ("WatchWithPartner", "Read to Kid").

The full export stays outside the repository (git-ignored `owner-inbox/legacy/export.json`)
for a local, ignored full-import test.
