// Serves the TMDB fixtures of the connectors crate over HTTP for the e2e tests: search for any
// query returns the recorded search results; details exist only for recorded ids; images 404.
// Synthetic fixtures from the TMDB API reference, see docs/connectors/tmdb.md.
import { createServer } from "node:http";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const dir = new URL("../../rust/crates/connectors/fixtures/tmdb/", import.meta.url).pathname; // ESM file (.mjs)
const byPath = new Map();
for (const f of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
  const fx = JSON.parse(readFileSync(join(dir, f), "utf8"));
  byPath.set(new URL(fx.request.url).pathname, fx);
}
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
