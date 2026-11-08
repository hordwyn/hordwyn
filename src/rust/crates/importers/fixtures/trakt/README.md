# Trakt fixtures (synthetic)

**synthetic**: built by hand from the example responses of the Trakt API blueprint
(`https://jsapi.apiary.io/apis/trakt.apib`, sections "Get History", "Get Ratings", "Get
Watchlist", fetched 2026-11-07) as quoted in `docs/connectors/trakt.md` section 2, trimmed and
extended with a second watch of one movie, an episode without its own ids and a movie
without a TMDB, IMDb or TVDB id (to exercise the unmatched list). Small JSON documents of
Trakt example data, used for tests under the Trakt API Use Policy (DEC-297). Not a real
user's data.
