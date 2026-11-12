import { test, expect, type Locator, type Page } from "@playwright/test";
import { brandValue, contrast, countEntityEvents, entityEvents, expectNoA11yViolations, expectReflow, hydrated, signUp, importDone, token } from "./helpers";

// UX phase 1 Home, Stats and History (design.md 6.1, 6.6; addendum-1 1.3): Home sections and
// the activity link, the next step with Undo, Stats tabs with the table equivalent, History
// filters in the URL, "Show more" paging and Undo, and axe in both themes at 1280 and 360 px.

const ROWS = [
  { source_id: "1", kind: "movie", title: "Harbour Lights", year: 2021, status: "planned" },
  {
    source_id: "2",
    kind: "show",
    title: "Downstream",
    year: 2020,
    status: "in_progress",
    history: [{ date: "2026-02-03", progress: { type: "episode", season: 1, episode: 1 } }],
  },
  {
    source_id: "3",
    kind: "book",
    title: "The Glass Ferry",
    year: 2018,
    status: "in_progress",
    history: [{ date: "2026-03-04", progress: { type: "paged", page: 40 } }],
  },
  { source_id: "4", kind: "movie", title: "Milk Teeth", year: 2019, status: "planned" },
];

/** Import the rows through the API (the worker runs the job); answers title -> item id. */
async function seed(page: Page, rows: object[] = ROWS): Promise<Record<string, string>> {
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...rows.map((r) => JSON.stringify(r))].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  expect(started.status()).toBe(202);
  const { id } = await started.json();
  await importDone(page, id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  return Object.fromEntries(lib.items.map((i: { item: { title: string; id: string } }) => [i.item.title, i.item.id]));
}

async function log(page: Page, item: string, progress: object, at?: string) {
  const r = await page.request.post(`/api/v1/items/${item}/progress`, {
    data: { progress, occurred_at: at ?? null, with: [] },
  });
  expect(r.ok()).toBeTruthy();
}

async function open(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

const rows = (page: Page) => page.getByTestId("activity-row");

/** Two frames: long enough for the effects of an event or an answer to have run. */
const frames = (page: Page) => page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));

test("home shows its sections, links to history and runs the next step with undo", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `home${Date.now()}`, "Hana");
  await open(page, "/");
  await expect(page.getByText("Nothing here yet")).toBeVisible();
  await seed(page);

  await open(page, "/");
  await expect(page.getByRole("heading", { level: 1, name: "Welcome back, Hana" })).toBeVisible();
  for (const name of ["Continue", "Up next", "Recent activity", "All-time stats"]) {
    await expect(page.getByRole("heading", { level: 2, name })).toBeVisible();
  }
  await expect(rows(page)).toHaveCount(2);
  await expect(page.getByTestId("up-next-card")).toHaveCount(2);
  // OT-17: the stats block is text: the time tiles lead, the library follows as a line per group
  await expect(page.getByTestId("banked-tile-books")).toContainText("40 pages");
  await expect(page.getByTestId("totals-line-watch")).toHaveText("Watch: 3 entries, 1 in progress, 2 planned");

  // the next step logs in one click; Undo in the toast takes it back
  const card = page.getByTestId("continue-card").filter({ hasText: "Downstream" });
  await card.getByRole("button", { name: "Mark S1 E2 watched: Downstream" }).click();
  await expect(page.getByText("Logged: Watched S1 E2")).toBeVisible();
  await expect(card.getByRole("button", { name: "Mark S1 E3 watched: Downstream" })).toBeVisible();
  await expect(rows(page)).toHaveCount(3);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.getByText("Undone", { exact: true })).toBeVisible();
  await expect(card.getByRole("button", { name: "Mark S1 E2 watched: Downstream" })).toBeVisible();
  await expect(rows(page)).toHaveCount(2);
  // after each refetch the new card's button works: two more clicks log E2, then E3 (UX1-2)
  await card.getByRole("button", { name: "Mark S1 E2 watched: Downstream" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Watched S1 E2" })).toBeVisible();
  await card.getByRole("button", { name: "Mark S1 E3 watched: Downstream" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Watched S1 E3" })).toBeVisible();
  await expect(card.getByRole("button", { name: "Mark S1 E4 watched: Downstream" })).toBeVisible();
  await expect(rows(page)).toHaveCount(4);

  await page.getByRole("link", { name: "All history" }).click();
  await expect(page).toHaveURL(/\/history$/);
  await expect(page.getByRole("heading", { level: 1, name: "History" })).toBeVisible();
  await page.goto("/");
  await page.getByRole("link", { name: "All stats" }).click();
  await expect(page).toHaveURL(/\/stats$/);
});

// OT-17 (DEC-542): Home's stats are led by time. A hero tile with the total hours, then four
// tiles (movies, shows, books, games) with time or pages first and the count second, then the
// library as one line per group. Everything is text, the icon is decoration, nothing talks
// about pace (R-04).
test("home leads its stats with time: the hero tile, four tiles in order, all of it text", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `time${Date.now()}`, "Tess");
  const ids = await seed(page, [
    ...ROWS,
    { source_id: "5", kind: "board_game", title: "Tidewater", year: 2017, status: "in_progress" },
    { source_id: "6", kind: "audiobook", title: "Night Ferry", year: 2022, status: "in_progress" },
    { source_id: "7", kind: "visual_novel", title: "Lantern Route", year: 2016, status: "in_progress" },
  ]);
  // a play of 90 minutes and two hours of listening: 3.5 hours
  const play = await page.request.post(`/api/v1/items/${ids["Tidewater"]}/plays`, { data: { duration_minutes: 90, players: [] } });
  expect(play.ok(), `play: ${play.status()}`).toBeTruthy();
  await log(page, ids["Night Ferry"], { type: "position", seconds: 7200 });
  // a session without a length: no time and no pages, but it was logged (review OT2C-2)
  const session = await page.request.post(`/api/v1/items/${ids["Lantern Route"]}/plays`, { data: { players: [] } });
  expect(session.ok(), `session: ${session.status()}`).toBeTruthy();

  await open(page, "/");
  const stats = page.getByRole("region", { name: "All-time stats" });
  const hero = stats.getByRole("region", { name: "Time banked" });
  await expect(hero.getByRole("heading", { level: 3, name: "Time banked" })).toBeVisible();
  await expect(hero.getByTestId("banked-total")).toHaveText("3.5 hours");
  await expect(hero).toContainText("Movies, shows, listening and play, added up.");
  await expect(hero.locator("svg[data-icon='piggy-bank']")).toHaveAttribute("aria-hidden", "true");
  // four tiles in a fixed order, each a term with its two descriptions
  const tiles = stats.locator("dl > div");
  await expect(tiles).toHaveCount(4);
  await expect(tiles.locator("dt")).toHaveText(["Movies", "Shows", "Books", "Games"]);
  // OT-20: the show has an episode but no known runtime: the tile leads with the count and
  // says what is missing, never "0 hours" beside "One episode"
  await expect(tiles.locator("dd:nth-of-type(1)")).toHaveText(["0 hours", "One episode", "40 pages", "1.5 hours"]);
  await expect(tiles.locator("dd:nth-of-type(2)")).toHaveText(["0 movies watched", "runtimes not known yet", "0 books finished", "One play"]);

  // OT-19: the hero and the tiles share one grid that counts its columns by the space of the
  // block, not of the window: four tiles in a row here, the hero over two of them with the
  // same edges; in a narrow column of a wide window two per row
  const edge = async (id: string) => {
    const b = (await page.getByTestId(id).boundingBox())!;
    return { left: Math.round(b.x), right: Math.round(b.x + b.width), top: Math.round(b.y) };
  };
  const [heroBox, movies, shows, books, games] = [await edge("banked-hero"), await edge("banked-tile-movies"), await edge("banked-tile-shows"), await edge("banked-tile-books"), await edge("banked-tile-games")];
  expect(new Set([movies.top, shows.top, books.top, games.top]).size, "one row of four").toBe(1);
  expect(heroBox.left).toBe(movies.left);
  expect(heroBox.right, "the hero ends where the second tile ends").toBe(shows.right);
  const block = (await page.getByTestId("banked").boundingBox())!;
  expect(games.right).toBe(Math.round(block.x + block.width));
  await page.getByTestId("banked").evaluate((el) => ((el.parentElement as HTMLElement).style.width = "30rem"));
  const narrow = [await edge("banked-hero"), await edge("banked-tile-movies"), await edge("banked-tile-shows"), await edge("banked-tile-books"), await edge("banked-tile-games")];
  expect(narrow[1].top).toBe(narrow[2].top);
  expect(narrow[3].top, "two rows of two").toBeGreaterThan(narrow[1].top);
  expect(narrow[3].top).toBe(narrow[4].top);
  expect(narrow[0].right).toBe(narrow[2].right);
  expect(narrow[4].right).toBe(narrow[2].right);
  expect(narrow[4].right, "no tile leaves the block").toBeLessThanOrEqual(narrow[0].left + 480);
  await page.getByTestId("banked").evaluate((el) => ((el.parentElement as HTMLElement).style.width = ""));
  // at the width the owner named nothing runs off to the right: between 760 px, where the
  // top bar shows its links, and about 790 px the bar was 10 px wider than the window
  // (review OT2F-10: up to 832 px, where the search button shows its key hint)
  for (const width of [760, 770, 790, 810, 831, 832]) {
    await page.setViewportSize({ width, height: 900 });
    await expectReflow(page, `home ${width}`);
    const last = (await page.getByTestId("banked-tile-games").boundingBox())!;
    expect(last.x + last.width, `tiles at ${width}`).toBeLessThanOrEqual(width - 16 + 0.5);
  }
  await page.setViewportSize({ width: 1280, height: 720 });
  // other kinds: time or pages, or the count when there is neither, lower case with the number
  await expect(page.getByTestId("banked-also")).toHaveText("Also: Audiobook 2 hours, Visual novel 1 play");
  await expect(page.getByTestId("banked-notes")).toHaveText("Reading is counted in pages. Titles without a known length count as 0.");
  // the status counts come after the tiles, as text; Home has no chart
  await expect(stats.getByRole("heading", { level: 3, name: "Library" })).toBeVisible();
  await expect(page.getByTestId("totals-line-watch")).toHaveText("Watch: 3 entries, 1 in progress, 2 planned");
  await expect(page.getByTestId("totals-line-play")).toHaveText("Play: 2 entries, 2 in progress");
  await expect(stats.getByRole("img")).toHaveCount(0);
  const order = await stats.evaluate((el) =>
    [...el.querySelectorAll("[data-testid]")].map((n) => n.getAttribute("data-testid") ?? "").filter((id) => /^(banked-hero|banked-tile-|totals-line-)/.test(id)),
  );
  expect(order).toEqual([
    "banked-hero",
    "banked-tile-movies",
    "banked-tile-shows",
    "banked-tile-books",
    "banked-tile-games",
    "totals-line-watch",
    "totals-line-read",
    "totals-line-listen",
    "totals-line-play",
  ]);
  await expect(stats).not.toContainText(/per week|per day|behind|ahead|goal|pace/i);

  // the hero is half as wide as the block from 768 px and as wide as the block on a phone
  const widths = async () => {
    const [h, s] = [await hero.boundingBox(), await stats.boundingBox()];
    return h!.width / s!.width;
  };
  expect(await widths()).toBeCloseTo(0.5, 1);
  await page.setViewportSize({ width: 360, height: 740 });
  expect(await widths()).toBeCloseTo(1, 1);
  await expectReflow(page, "home stats 360");
  await expectNoA11yViolations(page, "home stats 360");

});

// WP4-5 logging funnel (design.md 9, R-54): the next step on Home is one flow from the click
// to "logged"; the browser sends it in one beacon with buckets only, never an item id or a title
test("the next step on Home is counted in one usage beacon without item ids", async ({ page }) => {
  test.setTimeout(120_000);
  const beacons: { events: Record<string, string>[] }[] = [];
  await page.route("**/api/v1/usage/funnel", async (route) => {
    beacons.push(route.request().postDataJSON());
    await route.fulfill({ status: 204 });
  });
  await signUp(page, `funnel${Date.now()}`, "Fenna");
  const ids = await seed(page);
  await open(page, "/");
  const card = page.getByTestId("continue-card").filter({ hasText: "Downstream" });
  await card.getByRole("button", { name: "Mark S1 E2 watched: Downstream" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Watched S1 E2" })).toBeVisible();
  await expect.poll(() => beacons.length, { timeout: 20_000 }).toBeGreaterThan(0);
  const events = beacons.flatMap((b) => b.events);
  expect(events).toContainEqual(expect.objectContaining({ flow: "next_step", outcome: "logged", taps: "1" }));
  for (const e of events) {
    expect(Object.keys(e).sort().join(",")).toMatch(/^(flow,outcome,taps,time|flow,outcome,subject)$/);
  }
  const sent = JSON.stringify(beacons);
  for (const [title, id] of Object.entries(ids)) {
    expect(sent).not.toContain(id);
    expect(sent).not.toContain(title);
  }
});

test("stats switch tabs and show each chart as a table", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `stats${Date.now()}`, "Sten");
  await seed(page);
  await open(page, "/stats");
  const all = page.getByRole("tab", { name: "All" });
  await expect(all).toHaveAttribute("aria-selected", "true");
  await page.getByRole("tab", { name: "Read" }).click();
  await expect(page).toHaveURL(/\/stats\?group=read$/);
  await expect(page.getByRole("tab", { name: "Read" })).toHaveAttribute("aria-selected", "true");
  const pages = page.getByRole("figure", { name: /^Pages/ });
  await expect(pages.getByRole("img", { name: /^Pages in Read/ })).toBeVisible();
  await pages.getByText("Show as a table").click();
  const table = pages.getByRole("table");
  await expect(table).toBeVisible();
  await expect(table.getByRole("row").filter({ hasText: "Mar 2026" })).toContainText("40");

  await page.getByRole("radio", { name: "By year" }).click();
  await expect(page).toHaveURL(/\/stats\?group=read&bucket=year$/);
  await page.goBack();
  await expect(page).toHaveURL(/\/stats\?group=read$/);
  await expect(page.getByRole("radio", { name: "By month" })).toHaveAttribute("aria-checked", "true");
});

test("history keeps its filters in the URL, pages and undoes", async ({ page }) => {
  test.setTimeout(180_000);
  await signUp(page, `hist${Date.now()}`, "Hugo");
  const ids = await seed(page);
  // 61 views in June and July 2025: three pages of history with the 4 other rows
  for (let d = 0; d < 61; d++) {
    const day = new Date(Date.UTC(2025, 5, 1 + d, 12)).toISOString();
    await log(page, ids["Milk Teeth"], { type: "finished" }, day);
  }
  await log(page, ids["Downstream"], { type: "episode", season: 1, episode: 2 });
  await log(page, ids["Downstream"], { type: "episode", season: 1, episode: 3 });

  await open(page, "/history");
  await expect(page.getByRole("heading", { level: 3, name: "Today" })).toBeVisible();
  await expect(rows(page)).toHaveCount(30);
  // "Show more" appends the next page and keeps the keyboard focus (UX3-3)
  const more = page.getByRole("button", { name: "Show more" });
  await more.focus();
  await page.keyboard.press("Enter");
  await expect(rows(page)).toHaveCount(60);
  await expect(more).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(rows(page)).toHaveCount(65);
  await expect(more).toHaveCount(0);

  // undo the two episodes logged today, one after the other: the second click lands on the
  // row rendered after the first refetch (UX1-2)
  const today = page.getByRole("region", { name: "Today" });
  await expect(today.getByTestId("activity-row")).toHaveCount(2);
  await today.getByRole("button", { name: "Undo: Downstream" }).first().click();
  await expect(page.getByText("Undone", { exact: true })).toBeVisible();
  await expect(today.getByTestId("activity-row")).toHaveCount(1);
  await today.getByRole("button", { name: "Undo: Downstream" }).click();
  await expect(page.getByRole("heading", { level: 3, name: "Today" })).toHaveCount(0);

  // the group switcher and the filters live in the URL
  await page.getByRole("radio", { name: "Watch" }).click();
  await expect(page).toHaveURL(/\/history\?group=watch$/);
  await page.getByRole("button", { name: /^Filters/ }).click();
  await page.getByLabel("Year").selectOption("2025");
  await expect(page).toHaveURL(/\/history\?group=watch&year=2025$/);
  await expect(rows(page)).toHaveCount(30);
  await page.getByLabel("Kind").selectOption({ label: "Show" });
  await expect(page).toHaveURL(/group=watch&kind=show&year=2025$/);
  await expect(page.getByText("Nothing logged in this view")).toBeVisible();
  await page.reload();
  await hydrated(page);
  await expect(page.getByRole("radio", { name: "Watch" })).toHaveAttribute("aria-checked", "true");
  await expect(page.getByLabel("Kind")).toHaveValue("show");
  await expect(page.getByRole("button", { name: /^Filters/ })).toHaveAttribute("aria-expanded", "true");
  await page.goBack();
  await expect(page).toHaveURL(/\/history\?group=watch&year=2025$/);
  await page.getByRole("button", { name: "Clear filters" }).click();
  await expect(page).toHaveURL(/\/history$/);
  await expect(rows(page)).toHaveCount(30);

  // the type filter through the panel (UX1-11)
  if (!(await page.getByLabel("Type").isVisible())) {
    await page.getByRole("button", { name: /^Filters/ }).click();
  }
  await page.getByLabel("Type").selectOption({ label: "Sessions" });
  await expect(page).toHaveURL(/\/history\?type=session$/);
  await expect(page.getByText("Nothing logged in this view")).toBeVisible();

  // junk in a shared URL is ignored, never an error with a useless Retry (UX1-7)
  await open(page, "/history?group=foo&person=junk");
  await expect(rows(page)).toHaveCount(30);
});

test("history keeps the pages 'Show more' loaded and the focus when an event refreshes it", async ({ page }) => {
  // DEC-490: an event refetched only the first page and dropped the pages loaded by "Show
  // more" (the "history keeps its filters" test lost them in full runs, DEC-456, DEC-488)
  test.setTimeout(180_000);
  await signUp(page, `hmore${Date.now()}`, "Mona");
  const ids = await seed(page);
  for (let d = 0; d < 61; d++) {
    const day = new Date(Date.UTC(2025, 5, 1 + d, 12)).toISOString();
    await log(page, ids["Milk Teeth"], { type: "finished" }, day);
  }
  await log(page, ids["Downstream"], { type: "episode", season: 1, episode: 2 });
  await log(page, ids["Downstream"], { type: "episode", season: 1, episode: 3 });
  await open(page, "/history");
  await expect(rows(page)).toHaveCount(30);
  const more = page.getByRole("button", { name: "Show more" });
  await more.focus();
  await page.keyboard.press("Enter");
  await expect(rows(page)).toHaveCount(60);
  await expect(more).toBeFocused();

  // an event for one of the user's items that changes no row (a tag): the rows and the focus
  // stay
  const refetched = page.waitForResponse((r) => r.url().includes("/_fn/activity_feed"));
  expect((await page.request.post(`/api/v1/library/${ids["Harbour Lights"]}/tags`, { data: { tag: "rainy" } })).ok()).toBeTruthy();
  await refetched;
  await frames(page);
  await expect(rows(page)).toHaveCount(60);
  await expect(more).toBeFocused();

  // a new row on the first page: the loaded range is fetched again, one more row today and
  // still 60 rows, the focus stays
  const today = page.getByRole("region", { name: "Today" });
  await log(page, ids["Downstream"], { type: "episode", season: 1, episode: 4 });
  await expect(today.getByTestId("activity-row")).toHaveCount(3);
  await frames(page);
  await expect(rows(page)).toHaveCount(60);
  await expect(more).toBeFocused();

  // and "Show more" goes on from there
  await page.keyboard.press("Enter");
  await expect(rows(page)).toHaveCount(66);
  await expect(more).toHaveCount(0);
});

test("home, stats and history have no axe violations in both themes at 1280 and 360 px", async ({ page }) => {
  test.setTimeout(240_000);
  await signUp(page, `haxe${Date.now()}`, "Axel");
  await seed(page);
  for (const width of [1280, 360]) {
    await page.setViewportSize({ width, height: width === 360 ? 740 : 800 });
    for (const theme of ["light", "dark"] as const) {
      await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
      for (const path of ["/", "/stats", "/history"]) {
        await open(page, path);
        if (theme === "dark") {
          await expect(page.locator("html")).toHaveClass(/dark/);
        }
        if (path === "/stats") {
          await page.getByText("Show as a table").first().click();
        }
        await expectNoA11yViolations(page, `${path} ${theme} ${width}`);
      }
    }
  }
});

test("colour: the accent and the group tints of the brand reach the tiles, the kind badge, the history icon and the charts in both themes", async ({ page }) => {
  // OT-13 (DEC-540): the values come from the brand file, the page only names tokens. A tint
  // never stands alone (the tile has its icon and name), and the pairs on screen keep
  // 4.5:1 for text and 3:1 for icons and bars (WCAG 1.4.3, 1.4.11).
  test.setTimeout(180_000);
  await signUp(page, `tint${Date.now()}`, "Tina");
  const ids = await seed(page);
  // the brand file the server runs with, never a pinned value (review OT2D-9)
  const brand = {
    light: { primary: brandValue("theme", "primary"), watch: brandValue("theme", "group-watch") },
    dark: { primary: brandValue("theme_dark", "primary"), watch: brandValue("theme_dark", "group-watch") },
  };
  const look = (target: ReturnType<Page["locator"]>) =>
    target.evaluate((el) => {
      const s = getComputedStyle(el);
      return { background: s.backgroundColor, colour: s.color, fill: s.fill };
    });
  for (const theme of ["light", "dark"] as const) {
    await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
    await open(page, "/");
    await (theme === "dark" ? expect(page.locator("html")).toHaveClass(/dark/) : expect(page.locator("html")).not.toHaveClass(/dark/));
    const root = (name: string) => page.evaluate((n) => getComputedStyle(document.documentElement).getPropertyValue(`--${n}`).trim(), name);
    // a value the brand sets reaches the page as written; one it leaves out stays a fallback
    for (const [name, value] of [["primary", brand[theme].primary], ["group-watch", brand[theme].watch]] as const) {
      if (value !== undefined) {
        expect(await root(name), `${theme} ${name}`).toBe(value);
      }
    }

    // the hero tile sits on the accent's soft surface, its icon is in the accent
    const hero = page.getByTestId("banked-hero");
    expect((await look(hero)).background).toBe(await token(page, "primary-soft"));
    expect((await look(hero.locator("svg").first())).colour).toBe(await token(page, "primary"));
    // the four tiles: soft surface and icon of their group, text in the normal colours
    for (const [tile, group] of [["movies", "watch"], ["shows", "watch"], ["books", "read"], ["games", "play"]]) {
      const box = page.getByTestId(`banked-tile-${tile}`);
      const surface = (await look(box)).background;
      expect(surface, `${theme} ${tile} surface`).toBe(await token(page, `group-${group}-soft`));
      const icon = box.locator("dt svg");
      await expect(icon).toHaveAttribute("aria-hidden", "true");
      expect((await look(icon)).colour, `${theme} ${tile} icon`).toBe(await token(page, `group-${group}`));
      expect(await contrast(page, (await look(icon)).colour, surface), `${theme} ${tile} icon contrast`).toBeGreaterThanOrEqual(3);
      for (const part of ["dt", "dd >> nth=0", "dd >> nth=1"]) {
        expect(await contrast(page, (await look(box.locator(part))).colour, surface), `${theme} ${tile} ${part}`).toBeGreaterThanOrEqual(4.5);
      }
    }
    // a text link on the page takes the accent and keeps its underline (DEC-584)
    const allHistory = page.getByRole("main").getByRole("link", { name: "All history" });
    const link = await allHistory.evaluate((a) => ({ colour: getComputedStyle(a).color, line: getComputedStyle(a).textDecorationLine }));
    expect(link.colour, `${theme} link`).toBe(await token(page, "primary"));
    expect(link.line).toBe("underline");
    expect(await contrast(page, link.colour, await token(page, "background"))).toBeGreaterThanOrEqual(4.5);
    // the active navigation item takes the accent on its soft surface
    const home = page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Home" });
    const nav = await look(home);
    expect(nav.colour).toBe(await token(page, "primary"));
    expect(nav.background).toBe(await token(page, "primary-soft"));
    expect(await contrast(page, nav.colour, nav.background)).toBeGreaterThanOrEqual(4.5);

    // the item page: the kind badge on its group's soft surface, normal text, tinted icon
    await open(page, `/items/${ids["Harbour Lights"]}`);
    const badge = page.getByTestId("ip-kind");
    await expect(badge).toHaveText(/Movie/);
    const b = await look(badge);
    expect(b.background, `${theme} kind badge`).toBe(await token(page, "group-watch-soft"));
    expect(b.colour).toBe(await token(page, "foreground"));
    expect((await look(badge.locator("svg"))).colour).toBe(await token(page, "group-watch"));

    // History: the row's kind icon is in its group's tint
    await open(page, "/history");
    const bookRow = rows(page).filter({ hasText: "The Glass Ferry" }).first();
    expect((await look(bookRow.locator("svg").first())).colour, `${theme} history icon`).toBe(await token(page, "group-read"));

    // Stats: all groups together are drawn in the accent, a group's tab in its tint, and the
    // group tabs carry their marker
    await open(page, "/stats");
    const column = page.locator('#st-panel figure svg[role="img"] rect:not(.fill-transparent)').first();
    await expect(column).toBeVisible();
    expect((await look(column)).fill, `${theme} all groups`).toBe(await token(page, "primary"));
    const readTab = page.getByRole("tab", { name: "Read" });
    expect((await look(readTab.locator("span[aria-hidden]"))).background).toBe(await token(page, "group-read"));
    await expect(page.getByRole("tab", { name: "All" }).locator("span[aria-hidden]")).toHaveCount(0);
    await readTab.click();
    await expect(page).toHaveURL(/group=read/);
    await expect(readTab).toHaveAttribute("aria-selected", "true");
    await expect.poll(async () => (await look(column)).fill, { message: `${theme} read tab` }).toBe(await token(page, "group-read"));
    expect(await contrast(page, (await look(column)).fill, await token(page, "card"))).toBeGreaterThanOrEqual(3);
  }
});

test("stats at 390 px: the month labels under the columns do not touch", async ({ page }) => {
  // review OT2D-13: "Feb 2026Apr 2026Jun 2026" ran together on a phone
  test.setTimeout(90_000);
  await page.setViewportSize({ width: 390, height: 800 });
  await signUp(page, `axis${Date.now()}`, "Axa");
  const ids = await seed(page);
  // twelve months on the axis: every second one is named, six labels
  const now = new Date();
  const month = (back: number) => new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - back, 2, 12)).toISOString();
  await log(page, ids["Milk Teeth"], { type: "finished" }, month(11));
  await log(page, ids["Harbour Lights"], { type: "finished" }, month(0));
  await open(page, "/stats");
  const chart = page.locator('#st-panel figure svg[role="img"]').first();
  await expect(chart).toBeVisible();
  await expect(chart.locator("g")).toHaveCount(12);
  const boxes = await chart.locator("text").evaluateAll((labels) =>
    labels
      .map((l) => l.getBoundingClientRect())
      .filter((r) => r.width > 0)
      .map((r) => ({ left: r.left, right: r.right }))
      .sort((a, b) => a.left - b.left),
  );
  expect(boxes.length).toBeGreaterThanOrEqual(3);
  for (let i = 1; i < boxes.length; i += 1) {
    expect(boxes[i].left - boxes[i - 1].right, `gap before label ${i + 1}`).toBeGreaterThanOrEqual(4);
  }
  // on a wide screen every second month is named again
  await page.setViewportSize({ width: 1280, height: 800 });
  const wide = await chart.locator("text").evaluateAll((labels) => labels.filter((l) => l.getBoundingClientRect().width > 0).length);
  expect(wide).toBe(6);
});

// OT-21: while an import ran in the background the covers flashed between the picture and its
// alternative text. Every `entity.updated` (one per imported item) refetched the page, and
// every answer rendered all cards afresh, so every cover was a new `img` several times a
// second. Now a card and its cover stay while the entry is on the page, the refetches are
// paced, and a cover never paints its alternative text.
test("covers stay while an import runs: cards keep their pictures, refetches are paced", async ({ page }) => {
  test.setTimeout(240_000);
  await signUp(page, `flash${Date.now()}`, "Flo");
  const ids = await seed(page);
  // a planned film with a cover from the source (an `img`); the seeded rows have none
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  await palette.getByRole("combobox").fill("fight club");
  const option = palette.getByRole("option", { name: /^Fight Club \(1999\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.hover();
  await page.keyboard.press("Control+Enter");
  const library = async () => (await (await page.request.get("/api/v1/library?limit=100")).json()).items as { item: { id: string; title: string } }[];
  await expect.poll(async () => (await library()).some((r) => r.item.title === "Fight Club")).toBe(true);
  const film = (await library()).find((r) => r.item.title === "Fight Club")!.item.id;

  await countEntityEvents(page);
  let homeCalls = 0;
  page.on("request", (r) => {
    if (r.url().includes("/_fn/home")) homeCalls += 1;
  });
  await open(page, "/");
  const upNext = page.getByTestId("up-next-card").filter({ hasText: "Fight Club" });
  const poster = upNext.locator("img");
  await expect(poster).toHaveCount(1);
  // the alternative text is never painted in the box (it stays the picture's name)
  expect(await poster.evaluate((el) => getComputedStyle(el).color)).toBe("rgba(0, 0, 0, 0)");
  await expect(poster).toHaveAttribute("alt", "Cover of Fight Club");
  const show = page.getByTestId("continue-card").filter({ hasText: "Downstream" });
  await expect(show.getByRole("button", { name: "Mark S1 E2 watched: Downstream" })).toBeVisible();
  const keep = (l: Locator) => l.evaluate((el) => ((el as HTMLElement).dataset.kept = "1"));
  const kept = (l: Locator) => l.evaluate((el) => (el as HTMLElement).dataset.kept ?? "replaced");
  const showCover = show.locator("[role=img]");
  for (const l of [poster, upNext, show, showCover]) {
    await keep(l);
  }

  // an import of 40 films runs; meanwhile the show's own row changes
  const calls = homeCalls;
  const events = await entityEvents(page);
  const started = Date.now();
  const rows = Array.from({ length: 40 }, (_, n) => ({ source_id: `f${n}`, kind: "movie", title: `Flash Film ${n}`, year: 1990 + n, status: "completed" }));
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...rows.map((r) => JSON.stringify(r))].join("\n");
  const run = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(run.status()).toBe(202);
  await log(page, ids["Downstream"], { type: "episode", season: 1, episode: 2 });
  await importDone(page, (await run.json()).id);
  // Home follows: the show's next step and the library line
  await expect(show.getByRole("button", { name: "Mark S1 E3 watched: Downstream" })).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("totals-line-watch")).toContainText("44 entries", { timeout: 15_000 });
  const elapsed = Date.now() - started;
  for (const [l, what] of [
    [poster, "the film's picture"],
    [upNext, "the film's card"],
    [show, "the show's card"],
    [showCover, "the cover of the show, whose row changed"],
  ] as const) {
    expect(await kept(l), what).toBe("1");
  }
  // one refetch at once, then one per two seconds while the events keep coming
  const heard = (await entityEvents(page)) - events;
  expect(heard, "the import announced its items").toBeGreaterThanOrEqual(20);
  expect(homeCalls - calls, `${heard} events in ${elapsed} ms`).toBeLessThanOrEqual(2 + Math.ceil(elapsed / 2000));
  // the counter counts: Home did refetch for the import (a renamed server function would
  // leave it at 0 and the bound above would hold for any pacing, OT1B-1)
  expect(homeCalls - calls, "Home refetched").toBeGreaterThanOrEqual(1);

  // the library grid: a card whose row changes keeps its picture too
  await open(page, "/library");
  const card = page.getByTestId("library-card").filter({ hasText: "Fight Club" });
  const picture = card.locator("img");
  await expect(picture).toHaveCount(1);
  await keep(picture);
  await keep(card);
  expect((await page.request.post(`/api/v1/library/${film}/tags`, { data: { tag: "rainy" } })).ok()).toBeTruthy();
  await expect(card.getByText("rainy")).toBeVisible({ timeout: 15_000 });
  expect(await kept(picture), "the picture of a card whose row changed").toBe("1");
  expect(await kept(card)).toBe("1");
  // and the card's menu acts on the new row (handlers are rendered afresh, UX1-2): the tag
  // dialog it opens starts from the row's tags
  await page.getByRole("button", { name: "Actions for Fight Club" }).click();
  await page.getByRole("menuitem", { name: "Edit tags" }).click();
  const tags = page.getByRole("dialog", { name: "Tags for Fight Club" });
  await expect(tags.getByRole("button", { name: "Remove tag rainy" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(tags).toHaveCount(0);
  expect(await kept(picture), "the picture after the menu was used").toBe("1");
  await expectNoA11yViolations(page, "library after a background change");
});
