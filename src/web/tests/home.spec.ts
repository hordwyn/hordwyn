import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations, hydrated, signUp, importDone } from "./helpers";

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
async function seed(page: Page): Promise<Record<string, string>> {
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...ROWS.map((r) => JSON.stringify(r))].join("\n");
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
  // the charts are images with a name, the numbers are text too
  await expect(page.getByRole("img", { name: /^Watch: 2 planned, 1 in progress/ })).toBeVisible();
  await expect(page.getByTestId("totals-list-read")).toContainText("40 pages");

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
