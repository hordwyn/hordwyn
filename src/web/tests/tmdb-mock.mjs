// Serves the TMDB fixtures of the connectors crate over HTTP for the e2e tests: search for any
// query returns the recorded search results; details exist only for recorded ids; images 404.
// Synthetic fixtures from the TMDB API reference, see docs/connectors/tmdb.md.
//
// WP4-7: plus one synthetic show that exists only here (id 990001, "Lantern Coast"), shaped
// like TV Series > Details and TV Seasons > Details, so the release-date tests seed their own
// show (DEC-445): season 1 aired in full, season 2 has one episode still to come.
import { createServer } from "node:http";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const dir = new URL("../../rust/crates/connectors/fixtures/tmdb/", import.meta.url).pathname; // ESM file (.mjs)
const byPath = new Map();
for (const f of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
  const fx = JSON.parse(readFileSync(join(dir, f), "utf8"));
  byPath.set(new URL(fx.request.url).pathname, fx);
}

const episode = (season, n, air_date) => ({ air_date, episode_number: n, episode_type: "standard", runtime: 44, season_number: season });
const synthetic = (body) => ({ status: 200, body });
byPath.set(
  "/3/tv/990001",
  synthetic({
    id: 990001,
    name: "Lantern Coast",
    original_name: "Lantern Coast",
    first_air_date: "2024-01-05",
    overview: "Synthetic show of the e2e tests.",
    genres: [{ id: 18, name: "Drama" }],
    networks: [{ id: 1, name: "Test Network" }],
    status: "Returning Series",
    episode_run_time: [44],
    original_language: "en",
    poster_path: null,
    backdrop_path: null,
    seasons: [
      { air_date: "2024-01-05", episode_count: 3, season_number: 1 },
      { air_date: "2025-02-07", episode_count: 2, season_number: 2 },
    ],
  }),
);
byPath.set(
  "/3/tv/990001/season/1",
  synthetic({
    air_date: "2024-01-05",
    season_number: 1,
    episodes: [episode(1, 1, "2024-01-05"), episode(1, 2, "2024-01-12"), episode(1, 3, "2024-01-19")],
  }),
);
byPath.set(
  "/3/tv/990001/season/2",
  synthetic({
    air_date: "2025-02-07",
    season_number: 2,
    episodes: [episode(2, 1, "2025-02-07"), episode(2, 2, "2099-01-01")],
  }),
);

const port = Number(process.env.TMDB_MOCK_PORT ?? 3011);

createServer((req, res) => {
  const path = new URL(req.url, "http://localhost").pathname;
  if (path === "/health") {
    res.writeHead(200).end("ok");
    return;
  }
  const fx = byPath.get(path);
  if (!fx) {
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ success: false, status_code: 34 }));
    return;
  }
  res.writeHead(fx.status, { "content-type": "application/json" });
  res.end(JSON.stringify(fx.body));
}).listen(port, "127.0.0.1", () => console.log(`tmdb mock on ${port}`));
