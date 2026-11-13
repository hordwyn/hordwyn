// Catalogue mock for the website captures: answers the TMDB-shaped requests of the server with
// invented movies and shows (no real work, no real image). Every title has no poster and no
// backdrop, and any other path (images included) is a 404.
//
//   CATALOGUE_MOCK_PORT=3511 node site-tests/capture/catalogue-mock.mjs
//
// The capture script (capture.mjs) imports MOVIES and SHOWS for its sample library.
import { createServer } from "node:http";
import { pathToFileURL } from "node:url";

const GENRE_IDS = { Drama: 18, Comedy: 35, Mystery: 9648, Thriller: 53, Adventure: 12, Romance: 10749, Family: 10751, Documentary: 99, "Science Fiction": 878, Crime: 80 };

// id, title, release date, runtime (minutes), genres, overview
export const MOVIES = [
  [910001, "The Salt Meridian", "2019-10-03", 118, ["Drama", "Adventure"], "A ferry pilot charts one last crossing along a coast the maps no longer agree on."],
  [910002, "Paper Lanterns for Ilse", "2022-03-17", 102, ["Drama", "Family"], "Three siblings rebuild their grandmother's lantern workshop in a single summer."],
  [910003, "Northbound Quiet", "2021-11-11", 96, ["Mystery", "Thriller"], "A night train stops between two stations, and one passenger is missing by morning."],
  [910004, "A Field of Small Engines", "2017-06-22", 109, ["Comedy", "Drama"], "A village lawnmower race becomes the event of the decade."],
  [910005, "The Clockmaker's Tide", "2023-09-07", 127, ["Adventure", "Family"], "An apprentice follows a drowned clock tower's chimes out to sea."],
  [910006, "Vellum Sky", "2024-02-01", 134, ["Science Fiction", "Drama"], "The last archivist of a floating library decides which books may land."],
  [910007, "Last Tram to Elsterby", "2018-12-06", 91, ["Comedy", "Romance"], "Two strangers miss the same tram every evening for a week."],
  [910008, "The Orchard Protocol", "2025-04-24", 112, ["Thriller", "Science Fiction"], "A seed vault engineer finds a tree that should not exist."],
  [910009, "Copper Winter", "2020-01-16", 105, ["Crime", "Drama"], "A scrap dealer and a detective share one very cold case."],
  [910010, "Midnight at the Lido Royale", "2016-07-14", 99, ["Comedy", "Mystery"], "The night shift of a seaside pool solves a theft before opening time."],
  [910011, "Glasswing Summer", "2026-05-21", 108, ["Romance", "Drama"], "A butterfly counter and a bus driver spend a season on the same mountain road."],
  [910012, "How the Harbour Sleeps", "2024-10-10", 84, ["Documentary"], "One year in a small working port, told by the people of the night shift."],
];

// id, title, first air date, network, status, episode runtime, genres, overview, episodes per season
export const SHOWS = [
  [920001, "Hollow Tide Station", "2023-02-09", "Northlight", "Returning Series", 47, ["Drama", "Mystery"], "The crew of a remote tidal research station records signals that the sea should not send.", [8, 8, 6]],
  [920002, "The Brackwater Files", "2021-09-14", "Canal One", "Ended", 52, ["Crime", "Drama"], "A river police unit of four works the cases nobody upstream wants.", [6, 6]],
  [920003, "Kestrel Lane", "2024-04-02", "Parlour", "Returning Series", 28, ["Comedy"], "Six neighbours share one street, one allotment and no secrets.", [10, 10]],
  [920004, "Low Orbit Diner", "2025-01-18", "Northlight", "Returning Series", 24, ["Comedy", "Science Fiction"], "The only restaurant on a cargo ring serves breakfast around the clock.", [12]],
];

const genres = (names) => names.map((name) => ({ id: GENRE_IDS[name] ?? 1, name }));

function addDays(date, days) {
  const d = new Date(`${date}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

/** First air date of season `n` (1-based): one season a year. */
function seasonStart(first, n) {
  return `${Number(first.slice(0, 4)) + n - 1}${first.slice(4)}`;
}

const byPath = new Map();
const movieHits = [];
const showHits = [];

for (const [id, title, date, runtime, g, overview] of MOVIES) {
  byPath.set(`/3/movie/${id}`, {
    adult: false,
    backdrop_path: null,
    genres: genres(g),
    id,
    imdb_id: null,
    original_language: "en",
    original_title: title,
    overview,
    poster_path: null,
    release_date: date,
    runtime,
    status: "Released",
    title,
    vote_average: 0,
    vote_count: 0,
  });
  movieHits.push({ adult: false, backdrop_path: null, genre_ids: [], id, original_language: "en", original_title: title, overview, popularity: 1, poster_path: null, release_date: date, title, vote_average: 0, vote_count: 0 });
}

for (const [id, name, first, network, status, runtime, g, overview, counts] of SHOWS) {
  byPath.set(`/3/tv/${id}`, {
    id,
    name,
    original_name: name,
    first_air_date: first,
    overview,
    genres: genres(g),
    networks: [{ id: 1, name: network }],
    status,
    episode_run_time: [runtime],
    original_language: "en",
    poster_path: null,
    backdrop_path: null,
    vote_count: 0,
    seasons: counts.map((episode_count, i) => ({ air_date: seasonStart(first, i + 1), episode_count, season_number: i + 1 })),
  });
  counts.forEach((count, i) => {
    const start = seasonStart(first, i + 1);
    byPath.set(`/3/tv/${id}/season/${i + 1}`, {
      air_date: start,
      season_number: i + 1,
      episodes: Array.from({ length: count }, (_, e) => ({
        air_date: addDays(start, e * 7),
        episode_number: e + 1,
        episode_type: "standard",
        name: `Episode ${e + 1}`,
        runtime,
        season_number: i + 1,
      })),
    });
  });
  showHits.push({ adult: false, backdrop_path: null, genre_ids: [], id, origin_country: [], original_language: "en", original_name: name, overview, popularity: 1, poster_path: null, first_air_date: first, name, vote_average: 0, vote_count: 0 });
}

function search(hits, query) {
  const q = (query ?? "").toLowerCase();
  const results = hits.filter((h) => (h.title ?? h.name).toLowerCase().includes(q));
  return { page: 1, results, total_pages: 1, total_results: results.length };
}

function serve(port) {
  createServer((req, res) => {
    const url = new URL(req.url, "http://localhost");
    const path = url.pathname;
    if (path === "/health") {
      res.writeHead(200).end("ok");
      return;
    }
    let body = byPath.get(path);
    if (path === "/3/search/movie") {
      body = search(movieHits, url.searchParams.get("query"));
    } else if (path === "/3/search/tv") {
      body = search(showHits, url.searchParams.get("query"));
    }
    if (!body) {
      // unknown ids and every image request
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ success: false, status_code: 34 }));
      return;
    }
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(body));
  }).listen(port, "127.0.0.1", () => console.log(`catalogue mock on ${port}`));
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  serve(Number(process.env.CATALOGUE_MOCK_PORT ?? 3511));
}
