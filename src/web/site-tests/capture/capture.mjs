// Captures of the running application for the project website: seeds an invented household
// (one user, five people, about 35 titles with views, reading, sessions and plays) and writes
// PNG files of six views in two themes and two languages: `home` (recent activity and the
// all-time stats of Home), `home-top` (greeting, Continue, Up next), `item` (head of a partly
// watched show), `episodes` (its episode list), `library` (the grid) and `log` (the play
// dialog of a board game with players and scores).
//
// Usage (the application must be built first: `cargo leptos build` in src/rust, as for the
// e2e tests; Postgres of `cargo xtask services up` must be reachable):
//
//   cd src/web
//   nix develop ../.. -c node site-tests/capture/capture.mjs
//
// The script creates the database `ls_shots`, starts the catalogue mock (catalogue-mock.mjs)
// and the server, seeds, captures, stops both and drops the database again.
//
// Environment:
//   CAPTURE_PORT            port of the server (default 3510)
//   CATALOGUE_MOCK_PORT     port of the catalogue mock (default 3511)
//   CAPTURE_PG              maintenance URL of Postgres (default postgres://app@127.0.0.1:55432/postgres)
//   CAPTURE_DB              name of the database, must start with `ls_` (default ls_shots)
//   CAPTURE_BRAND_DIR       brand directory (default: a temporary copy of config/brand/default
//                           under the product name of the website's site.toml)
//   CAPTURE_SERVER_CMD      server binary, relative to src/rust (default target/debug/server)
//   CAPTURE_OUT             output directory (default src/rust/apps/website/content/shots)
//   CAPTURE_LOCALES         default "en,de"
//   CAPTURE_THEMES          default "light,dark"
//   CAPTURE_VIEWS           default "home,home-top,item,episodes,library,log"
//   CAPTURE_BASE_URL        use a server that already runs (with its mock and database);
//                           nothing is started, stopped, created or dropped then
//   CAPTURE_KEEP=1          leave the mock, the server and the database after the run
//
// The mock serves no image, so every cover is the placeholder of the application.
import { execFileSync, spawn } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { MOVIES, SHOWS } from "./catalogue-mock.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../../../..");
const port = Number(process.env.CAPTURE_PORT ?? 3510);
const mockPort = Number(process.env.CATALOGUE_MOCK_PORT ?? 3511);
const pgAdmin = process.env.CAPTURE_PG ?? "postgres://app@127.0.0.1:55432/postgres";
const dbName = process.env.CAPTURE_DB ?? "ls_shots";
const external = process.env.CAPTURE_BASE_URL;

// The pictures must show the product under the name the website gives it: a copy of the
// default brand whose name is the one of the website's site.toml (CAPTURE_BRAND_DIR: use
// that brand directory as it is).
function brandDir() {
  if (process.env.CAPTURE_BRAND_DIR) return process.env.CAPTURE_BRAND_DIR;
  const site = readFileSync(`${repo}/src/rust/apps/website/content/site.toml`, "utf8");
  const name = /^name = "([^"]+)"/m.exec(site)?.[1];
  if (!name) throw new Error("site.toml of the website has no name");
  const dir = mkdtempSync(`${tmpdir()}/capture-brand-`);
  cpSync(`${repo}/config/brand/default`, dir, { recursive: true });
  const file = `${dir}/brand.toml`;
  const brand = readFileSync(file, "utf8")
    .replace(/^name = ".*"$/m, `name = "${name}"`)
    .replace(/^short_name = ".*"$/m, `short_name = "${name}"`);
  writeFileSync(file, brand);
  return dir;
}
const base = external ?? `http://127.0.0.1:${port}`;
const out = resolve(process.env.CAPTURE_OUT ?? `${repo}/src/rust/apps/website/content/shots`);
const list = (name, all) => (process.env[name] ?? all).split(",").map((s) => s.trim()).filter(Boolean);
const LOCALES = list("CAPTURE_LOCALES", "en,de");
const THEMES = list("CAPTURE_THEMES", "light,dark");
const VIEWS = list("CAPTURE_VIEWS", "home,home-top,item,episodes,library,log");

const PASSWORD = "correct horse battery";
const ADMIN = "admin";
const USER = "mara";
const DISPLAY = "Mara";
const PEOPLE = ["Jonas", "Elif", "Ben", "Greta", "Noor"];
const SHOW = "Hollow Tide Station";
const BOARD_GAME = "Amber Caravan";

const DESKTOP = { viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 };
const PHONE = { viewport: { width: 390, height: 780 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true };

// ---------------------------------------------------------------- services

const children = [];

function psql(statement) {
  execFileSync("psql", [pgAdmin, "-q", "-v", "ON_ERROR_STOP=1", "-c", statement], { stdio: ["ignore", "ignore", "inherit"] });
}

async function waitFor(url, what, ms = 120_000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    try {
      if ((await fetch(url)).ok) {
        return;
      }
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`${what} did not answer at ${url}`);
}

async function startServices() {
  if (!/^ls_[a-z0-9_]+$/.test(dbName)) {
    throw new Error("CAPTURE_DB must start with ls_");
  }
  psql(`DROP DATABASE IF EXISTS ${dbName} WITH (FORCE)`);
  psql(`CREATE DATABASE ${dbName}`);
  const mock = spawn(process.execPath, [resolve(here, "catalogue-mock.mjs")], {
    env: { ...process.env, CATALOGUE_MOCK_PORT: String(mockPort) },
    stdio: "inherit",
  });
  children.push(mock);
  await waitFor(`http://127.0.0.1:${mockPort}/health`, "catalogue mock", 20_000);
  // no setting of the caller's environment leaks in: no connector but the mock is configured
  const inherited = Object.fromEntries(Object.entries(process.env).filter(([k]) => !/^(APP__|KK_|LEPTOS_)/.test(k)));
  const env = {
    ...inherited,
    LEPTOS_SITE_ROOT: "target/site",
    LEPTOS_OUTPUT_NAME: "app",
    LEPTOS_SITE_ADDR: `127.0.0.1:${port}`,
    LEPTOS_ENV: "DEV",
    APP__SERVER__BIND: `127.0.0.1:${port}`,
    APP__SERVER__PUBLIC_URL: base,
    APP__DATABASE__URL: pgAdmin.replace(/\/[^/]*$/, `/${dbName}`),
    APP__AUTH__SIGNUP: "open",
    APP__AUTH__SECURE_COOKIES: "false",
    APP__SEARCH__BACKEND: "postgres",
    APP__STORAGE__BACKEND: "memory",
    APP__SECRETS__EPHEMERAL: "true",
    APP__BRAND__DIR: brandDir(),
    APP__CONNECTORS__TMDB__BASE_URL: `http://127.0.0.1:${mockPort}`,
    APP__CONNECTORS__TMDB__API_KEY: "capture-not-a-real-key",
  };
  const server = spawn(process.env.CAPTURE_SERVER_CMD ?? "target/debug/server", [], {
    cwd: `${repo}/src/rust`,
    env,
    stdio: ["ignore", "ignore", "inherit"],
  });
  children.push(server);
  await waitFor(`${base}/healthz`, "server");
}

function stopServices() {
  for (const c of children) {
    c.kill("SIGTERM");
  }
  if (children.length > 0 && !process.env.CAPTURE_KEEP) {
    try {
      psql(`DROP DATABASE IF EXISTS ${dbName} WITH (FORCE)`);
    } catch (e) {
      console.error(`could not drop ${dbName}: ${e.message}`);
    }
  }
}

// ---------------------------------------------------------------- sample data

const BOOKS = [
  ["The Cartographer of Lost Rivers", 2019],
  ["Ninefold Harbour", 2022],
  ["Ash and Marzipan", 2016],
  ["The Quiet Arithmetic", 2024],
  ["A Lighthouse for Wolves", 2021],
  ["Sundial Street", 2025],
];
const PODCASTS = [
  ["Static and Starlight", 2022],
  ["The Long Table", 2020],
  ["Rust Belt Radio Hour", 2023],
];
const VIDEO_GAMES = [
  ["Emberfall Tactics", 2023],
  ["Lumen Drift", 2025],
  ["Tiny Harbour Tycoon", 2021],
  ["Moss Kingdom", 2024],
  ["Signal Runner 2089", 2026],
];
const BOARD_GAMES = [
  [BOARD_GAME, 2020],
  ["Fjordlings", 2023],
  ["Clockwork Orchard", 2018],
  ["Nine Bridges", 2022],
  ["Spice Route Rivals", 2025],
];

/** Up next, then Continue, each from the last card to the first (see `rows`). */
const LEAD = [
  "The Orchard Protocol",
  "Spice Route Rivals",
  "Signal Runner 2089",
  "Sundial Street",
  "Low Orbit Diner",
  "Glasswing Summer",
  "Kestrel Lane",
  "Lumen Drift",
  "Static and Starlight",
  BOARD_GAME,
  "The Quiet Arithmetic",
  SHOW,
];

/** Ratings (0..100) by title; the rest stays unrated. */
const RATINGS = {
  "The Salt Meridian": 90,
  "Paper Lanterns for Ilse": 80,
  "Northbound Quiet": 70,
  "A Field of Small Engines": 80,
  "The Clockmaker's Tide": 100,
  "Vellum Sky": 90,
  "Last Tram to Elsterby": 60,
  "Copper Winter": 80,
  "How the Harbour Sleeps": 70,
  "The Brackwater Files": 90,
  "Hollow Tide Station": 90,
  "The Cartographer of Lost Rivers": 100,
  "Ninefold Harbour": 80,
  "Ash and Marzipan": 70,
  "Emberfall Tactics": 80,
  "Tiny Harbour Tycoon": 70,
  "Amber Caravan": 100,
  Fjordlings: 80,
  "Clockwork Orchard": 80,
};

function rows() {
  const all = [];
  const add = (kind, title, year, external_ids) =>
    all.push({
      source_id: `${kind}-${all.length + 1}`,
      kind,
      title,
      year,
      status: "planned",
      ...(external_ids ? { external_ids } : {}),
      ...(RATINGS[title] ? { rating: RATINGS[title] } : {}),
    });
  for (const [id, title, date] of MOVIES) {
    add("movie", title, Number(date.slice(0, 4)), { tmdb: String(id) });
  }
  for (const [id, title, date] of SHOWS) {
    add("show", title, Number(date.slice(0, 4)), { tmdb: String(id) });
  }
  for (const [title, year] of BOOKS) add("book", title, year);
  for (const [title, year] of PODCASTS) add("podcast", title, year);
  for (const [title, year] of VIDEO_GAMES) add("video_game", title, year);
  for (const [title, year] of BOARD_GAMES) add("board_game", title, year);
  // Home orders Continue and Up next by the newest of "added" and "last logged", and every
  // entry is added now: the rows the two sections should lead with are imported last
  const rank = (row) => LEAD.indexOf(row.title);
  return all.sort((a, b) => rank(a) - rank(b));
}

/** A moment `days` days ago at `hour` o'clock (UTC), as RFC 3339. */
function ago(days, hour = 19) {
  const d = new Date();
  d.setUTCDate(d.getUTCDate() - days);
  d.setUTCHours(hour, 15, 0, 0);
  return d.toISOString();
}

async function call(api, method, path, data) {
  const r = await api[method](path, data === undefined ? undefined : { data });
  if (!r.ok()) {
    throw new Error(`${method.toUpperCase()} ${path}: ${r.status()} ${await r.text()}`);
  }
  const text = await r.text();
  return text ? JSON.parse(text) : null;
}

async function library(api) {
  const lib = await call(api, "get", "/api/v1/library?limit=500");
  return Object.fromEntries(lib.items.map((i) => [i.item.title, i.item.id]));
}

async function poll(what, check, ms = 180_000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await check()) {
      return;
    }
    await new Promise((r) => setTimeout(r, 1000));
  }
  throw new Error(`timed out: ${what}`);
}

async function seed(api) {
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...rows().map((r) => JSON.stringify(r))].join("\n");
  const started = await api.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  if (started.status() !== 202) {
    throw new Error(`import: ${started.status()} ${await started.text()}`);
  }
  const importId = (await started.json()).id;
  await poll("import", async () => {
    const state = (await call(api, "get", `/api/v1/imports/${importId}`)).state;
    if (state === "failed" || state === "cancelled") {
      throw new Error(`import ${state}`);
    }
    return state === "done";
  });
  const ids = await library(api);
  // the background jobs fetch the details and the episodes from the mock
  for (const [, title, , , , , , , counts] of SHOWS) {
    const total = counts.reduce((a, b) => a + b, 0);
    await poll(`episodes of ${title}`, async () => ((await call(api, "get", `/api/v1/items/${ids[title]}/page`)).episodes ?? []).length === total);
  }
  for (const [, title] of MOVIES) {
    await poll(`details of ${title}`, async () => Boolean((await call(api, "get", `/api/v1/items/${ids[title]}/page`)).item.attributes.runtime_minutes));
  }

  const me = (await call(api, "get", "/api/v1/auth/me")).self_companion;
  const who = {};
  for (const name of PEOPLE) {
    who[name] = (await call(api, "post", "/api/v1/companions", { name })).id;
  }
  const log = (title, progress, at, companions = []) =>
    call(api, "post", `/api/v1/items/${ids[title]}/progress`, { progress, occurred_at: at, with: companions.map((n) => who[n]) });
  const status = (title, value) => call(api, "put", `/api/v1/library/${ids[title]}/status`, { status: value });
  const play = (title, days, minutes, players = [], extra = {}) =>
    call(api, "post", `/api/v1/items/${ids[title]}/plays`, {
      players: players.map(([name, score, winner]) => ({
        companion_id: name === DISPLAY ? me : who[name],
        ...(score === undefined ? {} : { score, winner: Boolean(winner) }),
      })),
      duration_minutes: minutes,
      started_at: ago(days, 20),
      ...extra,
    });

  // movies: nine seen (one of them twice), three planned
  const finished = { type: "finished" };
  await log("A Field of Small Engines", finished, ago(310), ["Jonas"]);
  await log("Midnight at the Lido Royale", finished, ago(268));
  await log("Copper Winter", finished, ago(231), ["Jonas"]);
  await log("The Salt Meridian", finished, ago(190), ["Jonas", "Elif"]);
  await log("Last Tram to Elsterby", finished, ago(154), ["Elif"]);
  await log("Paper Lanterns for Ilse", finished, ago(118), ["Greta", "Noor"]);
  await log("Northbound Quiet", finished, ago(83));
  await log("The Clockmaker's Tide", finished, ago(47), ["Jonas", "Greta", "Noor"]);
  await log("How the Harbour Sleeps", finished, ago(26), ["Ben"]);
  await log("The Salt Meridian", finished, ago(12), ["Ben"]);
  await log("Vellum Sky", finished, ago(3), ["Jonas"]);

  // shows: one finished, one partly watched (season 1 and three of season 2), one started
  const episodes = async (title, season, from, to, firstDay, step, companions = []) => {
    for (let e = from; e <= to; e += 1) {
      await log(title, { type: "episode", season, episode: e }, ago(firstDay - (e - from) * step, 21), companions);
    }
  };
  await episodes("The Brackwater Files", 1, 1, 6, 300, 3);
  await episodes("The Brackwater Files", 2, 1, 6, 270, 3);
  await status("The Brackwater Files", "completed");
  await episodes(SHOW, 1, 1, 8, 75, 5, ["Jonas"]);
  await episodes(SHOW, 2, 1, 3, 9, 4, ["Jonas"]);
  await episodes("Kestrel Lane", 1, 1, 4, 40, 6, ["Elif"]);

  // books: three read, one in progress, two planned (reading counts pages)
  const read = async (title, pages, firstDay, days) => {
    const steps = 4;
    for (let s = 1; s <= steps; s += 1) {
      await log(title, { type: "paged", page: Math.round((pages * s) / steps) }, ago(firstDay - Math.round((days * (s - 1)) / (steps - 1)), 22));
    }
    await status(title, "completed");
  };
  await read("Ash and Marzipan", 288, 290, 21);
  await read("The Cartographer of Lost Rivers", 412, 200, 30);
  await read("Ninefold Harbour", 356, 95, 24);
  await log("The Quiet Arithmetic", { type: "paged", page: 64 }, ago(15, 22));
  await log("The Quiet Arithmetic", { type: "paged", page: 187 }, ago(2, 22));

  // podcasts: episodes heard (no source is set up, so they have no length)
  await episodes("The Long Table", 1, 1, 3, 180, 14);
  await episodes("Static and Starlight", 1, 1, 6, 62, 7);

  // video games: sessions with a length
  await play("Tiny Harbour Tycoon", 250, 95, [[DISPLAY]]);
  await play("Tiny Harbour Tycoon", 243, 140, [[DISPLAY]]);
  await play("Tiny Harbour Tycoon", 236, 75, [[DISPLAY]]);
  await status("Tiny Harbour Tycoon", "completed");
  await play("Emberfall Tactics", 140, 120, [[DISPLAY]]);
  await play("Emberfall Tactics", 131, 180, [[DISPLAY], ["Ben"]]);
  await play("Emberfall Tactics", 122, 90, [[DISPLAY]]);
  await play("Emberfall Tactics", 60, 150, [[DISPLAY], ["Ben"]]);
  await play("Lumen Drift", 20, 110, [[DISPLAY]]);
  await play("Lumen Drift", 6, 85, [[DISPLAY]]);

  // board games: plays with players, scores and a winner
  await play("Clockwork Orchard", 280, 60, [[DISPLAY, 41], ["Jonas", 44, true]]);
  await play("Clockwork Orchard", 210, 55, [[DISPLAY, 52, true], ["Jonas", 39], ["Elif", 47]]);
  await play("Fjordlings", 160, 40, [[DISPLAY, 23], ["Greta", 27, true], ["Noor", 19]]);
  await play("Fjordlings", 97, 45, [[DISPLAY, 31, true], ["Jonas", 28], ["Greta", 22], ["Noor", 25]]);
  await play("Nine Bridges", 70, 30, [[DISPLAY], ["Jonas"], ["Ben"]], { coop: "won" });
  await play(BOARD_GAME, 133, 80, [[DISPLAY, 58], ["Jonas", 63, true], ["Elif", 51]]);
  await play(BOARD_GAME, 90, 75, [[DISPLAY, 66, true], ["Jonas", 61], ["Ben", 49]]);
  await play(BOARD_GAME, 54, 90, [[DISPLAY, 57], ["Jonas", 54], ["Elif", 71, true], ["Ben", 48]]);
  await play(BOARD_GAME, 19, 85, [[DISPLAY, 72, true], ["Jonas", 64], ["Elif", 69]]);
  await play("Fjordlings", 10, 50, [[DISPLAY, 29], ["Jonas", 33, true], ["Greta", 30]]);
  // the newest play: the group the log dialog opens with
  await play(BOARD_GAME, 5, 80, [[DISPLAY, 61], ["Jonas", 68, true], ["Elif", 55], ["Ben", 59]]);
  return ids;
}

// ---------------------------------------------------------------- accounts

async function hydrated(page) {
  await page.locator("body[data-hydrated]").waitFor({ state: "attached", timeout: 30_000 });
}

/** The first account of a fresh database is the instance admin: walk the setup wizard. */
async function ensureAdmin(browser) {
  const context = await browser.newContext({ ...DESKTOP, baseURL: base, locale: "en-US" });
  const page = await context.newPage();
  await page.goto("/library");
  await hydrated(page);
  if (page.url().endsWith("/setup")) {
    await page.getByLabel("Username").fill(ADMIN);
    await page.getByLabel("Display name").fill("Admin");
    await page.getByLabel("Password").fill(PASSWORD);
    await page.getByRole("button", { name: "Create admin account" }).click();
    await page.getByRole("heading", { level: 2, name: "How do you host?" }).waitFor();
    await hydrated(page);
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByRole("heading", { level: 2, name: "Data sources" }).waitFor();
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByRole("heading", { level: 2, name: "Ready" }).waitFor();
    await page.getByRole("button", { name: "Go to Home" }).click();
    await page.waitForURL(/\/$/);
  }
  await context.close();
}

/** Sign the sample user in (made on the first run); answers the title -> item id map. */
async function ensureUser(browser) {
  const context = await browser.newContext({ baseURL: base, locale: "en-US" });
  const api = context.request;
  const made = await api.post("/api/v1/auth/sign-up", { data: { username: USER, display_name: DISPLAY, password: PASSWORD } });
  if (!made.ok()) {
    await call(api, "post", "/api/v1/auth/sign-in", { username: USER, password: PASSWORD });
  }
  let ids = await library(api);
  if (Object.keys(ids).length === 0) {
    console.log("seeding the sample library");
    ids = await seed(api);
  }
  await context.close();
  return ids;
}

// ---------------------------------------------------------------- captures

/** A signed-in page in the given language and theme. */
async function open(browser, device, locale, theme) {
  const context = await browser.newContext({
    ...device,
    baseURL: base,
    locale: locale === "de" ? "de-DE" : "en-US",
    timezoneId: "Europe/Berlin",
    colorScheme: theme,
    reducedMotion: "reduce",
  });
  await context.addInitScript((t) => localStorage.setItem("app-theme", t), theme);
  await call(context.request, "post", "/api/v1/auth/sign-in", { username: USER, password: PASSWORD });
  return context.newPage();
}

async function ready(page, locale, theme) {
  await hydrated(page);
  const lang = await page.locator("html").getAttribute("lang");
  if (lang !== locale) {
    throw new Error(`page is in "${lang}", wanted "${locale}"`);
  }
  const dark = await page.evaluate(() => document.documentElement.classList.contains("dark"));
  if (dark !== (theme === "dark")) {
    throw new Error(`theme is not ${theme}`);
  }
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready);
}

async function shoot(page, view, locale, theme) {
  // no pointer hover; nothing here saves, so no toast is open
  await page.mouse.move(0, 0);
  await page.waitForTimeout(400);
  const path = `${out}/${view}-${locale}-${theme}.png`;
  await page.screenshot({ path, fullPage: false, animations: "disabled", caret: "hide" });
  console.log(`wrote ${path}`);
}

/** Scroll the window so that the top of `selector` sits `gap` px under the sticky top bar. */
async function scrollUnderBar(page, selector, gap = 16) {
  await page.evaluate(
    ([sel, g]) => {
      const bar = document.querySelector("body header");
      const top = document.querySelector(sel).getBoundingClientRect().top + window.scrollY;
      window.scrollTo(0, Math.max(0, top - (bar ? bar.getBoundingClientRect().height : 0) - g));
    },
    [selector, gap],
  );
}

/** One capture: open `path` on `device`, let `prepare` set the scene, write the file. */
async function capture(browser, device, view, locale, theme, path, prepare = async () => {}) {
  const page = await open(browser, device, locale, theme);
  await page.goto(path);
  await ready(page, locale, theme);
  await prepare(page);
  await shoot(page, view, locale, theme);
  await page.context().close();
}

const SCORES = { [DISPLAY]: "64", Jonas: "58", Elif: "71", Ben: "52" };

const VIEW = {
  home: (browser, ids, locale, theme) => capture(browser, DESKTOP, "home", locale, theme, "/", (page) => scrollUnderBar(page, "#hm-activity-title", 28)),
  "home-top": (browser, ids, locale, theme) => capture(browser, DESKTOP, "home-top", locale, theme, "/"),
  item: (browser, ids, locale, theme) =>
    capture(browser, PHONE, "item", locale, theme, `/items/${ids[SHOW]}`, (page) => scrollUnderBar(page, "[data-testid=ip-kind]")),
  episodes: (browser, ids, locale, theme) =>
    capture(browser, PHONE, "episodes", locale, theme, `/items/${ids[SHOW]}`, async (page) => {
      await page.getByTestId("episode-row").first().waitFor();
      await scrollUnderBar(page, "[role=tablist]", 12);
    }),
  library: (browser, ids, locale, theme) =>
    capture(browser, PHONE, "library", locale, theme, "/library", async (page) => {
      await page.locator("#lib-results a").first().waitFor();
      await scrollUnderBar(page, "#lib-status-all", 16);
    }),
  log: (browser, ids, locale, theme) =>
    capture(browser, PHONE, "log", locale, theme, `/items/${ids[BOARD_GAME]}`, async (page) => {
      const de = locale === "de";
      await page
        .getByTestId("action-bar")
        .getByRole("button", { name: de ? "Partie eintragen" : "Log a play" })
        .click();
      const sheet = page.getByRole("dialog");
      await sheet.waitFor();
      // the sheet opens with the group and the duration of the last play; the scores are new
      for (const [name, score] of Object.entries(SCORES)) {
        await sheet.getByLabel(`${de ? "Punkte von" : "Score of"} ${name}`, { exact: true }).fill(score);
      }
      await page.evaluate(() => document.activeElement?.blur());
      await sheet.getByRole("group", { name: de ? "Mitspielende" : "Players" }).evaluate((el) => el.scrollIntoView({ block: "start" }));
    }),
};

async function main() {
  mkdirSync(out, { recursive: true });
  if (!external) {
    await startServices();
  }
  const browser = await chromium.launch();
  try {
    await ensureAdmin(browser);
    const ids = await ensureUser(browser);
    for (const view of VIEWS) {
      for (const locale of LOCALES) {
        for (const theme of THEMES) {
          await VIEW[view](browser, ids, locale, theme);
        }
      }
    }
  } finally {
    await browser.close();
    stopServices();
  }
}

main().catch((e) => {
  console.error(e);
  stopServices();
  process.exit(1);
});
