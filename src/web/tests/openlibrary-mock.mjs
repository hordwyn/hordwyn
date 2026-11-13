// Serves the Open Library fixtures of the connectors crate over HTTP for the e2e tests
// (recorded 2026-11-12, see docs/connectors/openlibrary.md). The works are those of the
// recorded search answers. A search finds them by title, by `key:` and by several keys at
// once; `/works/{id}.json` answers the recorded work, or for a work that was only recorded
// as a search row a synthetic one built from that row; `/isbn/` and `/books/` answer the
// recorded edition. Covers are one small picture generated here: no provider image is kept.
import { createServer } from "node:http";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { deflateSync } from "node:zlib";

const dir = new URL("../../rust/crates/connectors/fixtures/openlibrary/", import.meta.url).pathname;
const docs = new Map(); // work id -> search row
const byPath = new Map(); // /works/.., /isbn/.., /books/.. -> fixture
for (const f of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
  const fx = JSON.parse(readFileSync(join(dir, f), "utf8"));
  const url = new URL(fx.request.url);
  if (url.pathname === "/search.json") {
    for (const d of fx.body.docs ?? []) {
      docs.set(d.key.replace("/works/", ""), d);
    }
  } else if (fx.status === 200) {
    byPath.set(url.pathname, fx.body);
  }
}

const idOf = (d) => d.key.replace("/works/", "");
const page = (found, q) => ({ numFound: found.length, start: 0, numFoundExact: true, num_found: found.length, q, offset: null, docs: found });

function search(params) {
  const q = params.get("q") ?? "";
  const limit = Number(params.get("limit") ?? 20);
  const keys = [...q.matchAll(/\/works\/(OL\d+W)/g)].map((m) => m[1]);
  let found;
  if (q.startsWith("key:")) {
    found = keys.map((k) => docs.get(k)).filter(Boolean);
  } else {
    const words = (params.get("title") ?? q).toLowerCase().split(/\s+/).filter(Boolean);
    found = [...docs.values()].filter((d) => words.every((w) => d.title.toLowerCase().includes(w)));
    found.sort((a, b) => (b.edition_count ?? 0) - (a.edition_count ?? 0));
  }
  return page(found.slice(0, limit), q);
}

// A work that was only recorded as a search row: shaped like the recorded works.
function work(id) {
  const recorded = byPath.get(`/works/${id}.json`);
  if (recorded) return recorded;
  const d = docs.get(id);
  if (!d) return null;
  return {
    key: d.key,
    title: d.title,
    description: "Synthetic description of the e2e tests.",
    subjects: ["Fiction", "Synthetic subject"],
    covers: d.cover_i ? [d.cover_i] : [],
    type: { key: "/type/work" },
  };
}

// One 60 x 90 picture in a single colour (an own PNG, not a provider image).
function png(width, height, [r, g, b]) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (buf) => {
    let c = 0xffffffff;
    for (const x of buf) c = crcTable[(c ^ x) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type, data) => {
    const head = Buffer.alloc(4);
    head.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const tail = Buffer.alloc(4);
    tail.writeUInt32BE(crc(body));
    return Buffer.concat([head, body, tail]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr.set([8, 2, 0, 0, 0], 8); // 8 bit, RGB
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(Array.from({ length: width }, () => [r, g, b]).flat())]);
  const raw = Buffer.concat(Array.from({ length: height }, () => row));
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}
const cover = png(60, 90, [72, 61, 139]);

const port = Number(process.env.OPENLIBRARY_MOCK_PORT ?? 3015);
const json = (res, status, body) => {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(body));
};

createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  const path = url.pathname;
  if (path === "/health") {
    res.writeHead(200).end("ok");
    return;
  }
  if (path === "/search.json") {
    json(res, 200, search(url.searchParams));
    return;
  }
  const w = path.match(/^\/works\/(OL\d+W)\.json$/);
  const body = w ? work(w[1]) : byPath.get(path);
  if (body) {
    json(res, 200, body);
    return;
  }
  if (/^\/b\/id\/\d+-[SML]\.jpg$/.test(path)) {
    res.writeHead(200, { "content-type": "image/png", "content-length": cover.length });
    res.end(cover);
    return;
  }
  json(res, 404, { error: "notfound", key: path.replace(/\.json$/, "") });
}).listen(port, "127.0.0.1", () => console.log(`openlibrary mock on ${port}`));
