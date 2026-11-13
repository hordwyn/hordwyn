// Serves the Wikidata fixtures of the connectors crate over HTTP for the e2e tests (recorded
// 2026-11-13 from the Action API, data CC0; see docs/connectors/wikidata.md). Only
// `/w/api.php` with the two actions the connector uses:
// - `action=query&list=search`: the typed words (the part of `srsearch` before the class
//   filter) are matched against every label of the recorded entities; a search without the
//   class filter is answered with an error, so a test notices when it is not sent;
// - `action=wbgetentities`: the recorded entities (and the recorded labels of linked items),
//   cut to the languages and props asked; an unknown id is `missing`.
// Every request is noted (path, action, typed words, ids, User-Agent) and listed at `/requests`. No image is served:
// the connector reads none (DEC-642).
import { createServer } from "node:http";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const dir = new URL("../../rust/crates/connectors/fixtures/wikidata/", import.meta.url).pathname;
const entities = new Map(); // id -> the fullest recorded entity
for (const f of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
  const fx = JSON.parse(readFileSync(join(dir, f), "utf8"));
  for (const [id, e] of Object.entries(fx.body?.entities ?? {})) {
    if (e.missing !== undefined) continue;
    const known = entities.get(id);
    const size = (x) => Object.keys(x?.claims ?? {}).length + Object.keys(x?.labels ?? {}).length;
    if (!known || size(e) > size(known)) {
      entities.set(id, e);
    }
  }
}

const requests = [];
const port = Number(process.env.WIKIDATA_MOCK_PORT ?? 3016);
const json = (res, status, body) => {
  res.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  res.end(JSON.stringify(body));
};

function pick(map, languages) {
  if (!map || Array.isArray(map)) return {};
  return Object.fromEntries(Object.entries(map).filter(([l]) => languages.includes(l)));
}

function search(params) {
  const text = params.get("srsearch") ?? "";
  const at = text.indexOf(" haswbstatement:P31=Q131436|");
  if (at < 0) {
    return { error: { code: "e2e-no-class-filter", info: "the class filter is missing" } };
  }
  const words = text.slice(0, at).toLowerCase().split(/\s+/).filter(Boolean);
  const limit = Number(params.get("srlimit") ?? 10);
  const found = [...entities.values()]
    .filter((e) => e.claims)
    .filter((e) => {
      const labels = Object.values(e.labels ?? {}).map((l) => l.value.toLowerCase());
      return words.length > 0 && words.every((w) => labels.some((l) => l.includes(w)));
    })
    .sort((a, b) => Object.keys(b.sitelinks ?? {}).length - Object.keys(a.sitelinks ?? {}).length)
    .slice(0, limit);
  return {
    batchcomplete: true,
    query: { searchinfo: { totalhits: found.length }, search: found.map((e) => ({ ns: 0, title: e.id, pageid: 1 })) },
  };
}

function getEntities(params) {
  const languages = (params.get("languages") ?? "en").split("|");
  const props = (params.get("props") ?? "").split("|");
  const out = {};
  for (const id of (params.get("ids") ?? "").split("|").filter(Boolean)) {
    const e = entities.get(id);
    if (!e) {
      out[id] = { id, missing: "" };
      continue;
    }
    const cut = { type: "item", id };
    if (props.includes("labels")) cut.labels = pick(e.labels, languages);
    if (props.includes("descriptions")) cut.descriptions = pick(e.descriptions, languages);
    if (props.includes("claims")) cut.claims = e.claims ?? {};
    if (props.includes("sitelinks")) cut.sitelinks = e.sitelinks ?? {};
    out[id] = cut;
  }
  return { entities: out, success: 1 };
}

createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  if (url.pathname === "/health") {
    res.writeHead(200).end("ok");
    return;
  }
  if (url.pathname === "/requests") {
    json(res, 200, requests);
    return;
  }
  // the typed words of a search (before the class filter) and the ids asked, so that a spec
  // can pick out its own requests from those of the specs running beside it
  const srsearch = url.searchParams.get("srsearch") ?? "";
  requests.push({
    path: url.pathname,
    action: url.searchParams.get("action"),
    words: srsearch.split(" haswbstatement:")[0],
    ids: (url.searchParams.get("ids") ?? "").split("|").filter(Boolean),
    userAgent: req.headers["user-agent"] ?? "",
  });
  if (url.pathname !== "/w/api.php") {
    json(res, 404, { error: { code: "e2e-not-found", info: url.pathname } });
    return;
  }
  const action = url.searchParams.get("action");
  if (action === "query" && url.searchParams.get("list") === "search") {
    json(res, 200, search(url.searchParams));
  } else if (action === "wbgetentities") {
    json(res, 200, getEntities(url.searchParams));
  } else {
    json(res, 200, { error: { code: "badvalue", info: `action ${action}` } });
  }
}).listen(port, "127.0.0.1", () => console.log(`wikidata mock on ${port}`));
