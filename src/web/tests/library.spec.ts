import { test, expect, type Page } from "./fixtures";
import {
  expectNoA11yViolations,
  expectReflow,
  gateEntityEvents,
  heldEvents,
  holdEvents,
  hydrated,
  importDone,
  requestStarts,
  signUp,
  twoFrames,
} from "./helpers";

// UX phase 1 Library page (design.md 6.2, 3.5; addendum-1 1.3): group switcher, kind chips,
// status tabs, the one Filters button with the tag filter, sort, grid/list, the URL as the
// state (back button), the next step with Undo, the quick menu, and axe in both themes at
// 1280 and 360 px.

const ROWS = [
  { source_id: "1", kind: "movie", title: "Harbour Lights", year: 2021, status: "planned" },
  {
    source_id: "2",
    kind: "movie",
    title: "Ninety Winters",
    year: 2019,
    status: "completed",
    history: [{ date: "2026-01-02" }],
  },
  {
    source_id: "3",
    kind: "show",
    title: "Downstream",
    year: 2020,
    status: "in_progress",
    history: [{ date: "2026-02-03", progress: { type: "episode", season: 1, episode: 1 } }],
  },
  { source_id: "4", kind: "book", title: "The Glass Ferry", year: 2018, status: "planned" },
  { source_id: "5", kind: "board_game", title: "Meeple Harbour", year: 2017, status: "planned" },
];

/** Import rows through the API (the worker runs the job); answers title -> item id. */
async function importRows(page: Page, rows: object[]): Promise<Record<string, string>> {
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...rows.map((r) => JSON.stringify(r))].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  expect(started.status()).toBe(202);
  const { id } = await started.json();
  await importDone(page, id);
  const lib = await (await page.request.get("/api/v1/library?limit=500")).json();
  return Object.fromEntries(lib.items.map((i: { item: { title: string; id: string } }) => [i.item.title, i.item.id]));
}

/** The five rows, two of them tagged "cosy". */
async function seed(page: Page) {
  const ids = await importRows(page, ROWS);
  for (const title of ["Harbour Lights", "Downstream"]) {
    const r = await page.request.post(`/api/v1/library/${ids[title]}/tags`, { data: { tag: "cosy" } });
    expect(r.ok()).toBeTruthy();
  }
}

async function open(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

const cards = (page: Page) => page.getByTestId("library-card");

test("groups, chips, tabs, tags, sort and view live in the URL", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `lib${Date.now()}`, "Lib Tester");
  await seed(page);
  await open(page, "/library");
  await expect(cards(page)).toHaveCount(5);

  // junk in a shared URL: an unknown group is ignored, a facet the kind lacks names the
  // filters with "Clear filters" instead of an error with a useless Retry (UX1-7)
  await open(page, "/library?group=foo");
  await expect(cards(page)).toHaveCount(5);
  await open(page, "/library?kind=movie&facet=nonsense:x");
  await expect(page.getByRole("heading", { name: "Nothing matches these filters" })).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await open(page, "/library");

  await page.getByRole("radio", { name: "Watch" }).click();
  await expect(page).toHaveURL(/\/library\?group=watch$/);
  await expect(cards(page)).toHaveCount(3);

  const movie = page.getByRole("button", { name: /^Movie/ });
  await movie.click();
  await expect(page).toHaveURL(/group=watch&kind=movie$/);
  await expect(movie).toHaveAttribute("aria-pressed", "true");
  await expect(cards(page)).toHaveCount(2);

  await page.getByRole("tab", { name: /^Planned/ }).click();
  await expect(page).toHaveURL(/kind=movie&status=planned$/);
  await expect(cards(page)).toHaveCount(1);
  await expect(cards(page).first()).toContainText("Harbour Lights");

  // the back button undoes one change at a time
  await page.goBack();
  await expect(page).toHaveURL(/group=watch&kind=movie$/);
  await expect(cards(page)).toHaveCount(2);
  await expect(page.getByRole("tab", { name: /^All/ })).toHaveAttribute("aria-selected", "true");
  await page.goBack();
  await expect(page).toHaveURL(/\/library\?group=watch$/);
  await expect(cards(page)).toHaveCount(3);

  // one Filters button opens the tag filter
  const filters = page.getByRole("button", { name: /^Filters/ });
  await expect(filters).toHaveAttribute("aria-expanded", "false");
  await filters.click();
  await expect(filters).toHaveAttribute("aria-expanded", "true");
  await page.locator("#lib-filters").getByRole("button", { name: /^cosy/ }).click();
  await expect(page).toHaveURL(/group=watch&tag=cosy$/);
  await expect(cards(page)).toHaveCount(2);
  await expect(page.getByRole("button", { name: "Remove filter Tag: cosy" })).toBeVisible();

  await page.getByLabel("Sort").selectOption("title");
  await expect(page).toHaveURL(/tag=cosy&sort=title$/);
  await expect(cards(page).first()).toContainText("Downstream");

  await page.getByRole("radio", { name: "List", exact: true }).click();
  await expect(page).toHaveURL(/sort=title&view=list$/);
  await expect(page.getByRole("table")).toBeVisible();
  await expect(page.getByRole("table").getByRole("row")).toHaveCount(3);

  // the URL alone renders the same view (shareable), with the panel state remembered
  await page.reload();
  await hydrated(page);
  await expect(page.getByRole("table").getByRole("row")).toHaveCount(3);
  await expect(page.getByRole("radio", { name: "List", exact: true })).toHaveAttribute("aria-checked", "true");
  await expect(page.getByLabel("Sort")).toHaveValue("title");
  await expect(filters).toHaveAttribute("aria-expanded", "true");

  // the group is remembered: a bare /library opens the last group
  await open(page, "/library");
  await expect(page).toHaveURL(/\/library\?group=watch$/);

  // clearing the filters keeps the group
  await open(page, "/library?group=watch&tag=cosy&status=completed");
  await expect(page.getByRole("heading", { name: "Nothing matches these filters" })).toBeVisible();
  await page.getByRole("button", { name: "Clear filters" }).last().click();
  await expect(page).toHaveURL(/\/library\?group=watch$/);
});

test("a facet value, the / key with the debounced text filter, and Load more", async ({ page }) => {
  // UX1-11
  test.setTimeout(180_000);
  await signUp(page, `facet${Date.now()}`, "Facet Tester");
  const fillers = Array.from({ length: 50 }, (_, i) => ({
    source_id: `f${i}`,
    kind: "movie",
    title: `Filler ${String(i).padStart(2, "0")}`,
    year: 2000,
    status: "planned",
  }));
  // the TMDB mock knows movie 11: the catalog refresh brings its genres
  const sw = { source_id: "sw", kind: "movie", title: "Star Wars", year: 1977, status: "planned", external_ids: { tmdb: "11" } };
  const ids = await importRows(page, [sw, ...fillers]);
  await expect
    .poll(
      async () => {
        const p = await (await page.request.get(`/api/v1/items/${ids["Star Wars"]}/page`)).json();
        return p.item.attributes.genres ?? [];
      },
      { timeout: 30_000 },
    )
    .toContain("Action");

  await open(page, "/library?kind=movie");
  await expect(cards(page)).toHaveCount(48);
  await expect(page.getByText("Showing 48 of 51")).toBeVisible();
  await page.getByRole("button", { name: "Show more" }).click();
  await expect(cards(page)).toHaveCount(51);
  await expect(page.getByText("Showing 51 of 51")).toBeVisible();
  await expect(page.getByRole("button", { name: "Show more" })).toHaveCount(0);

  await page.getByRole("button", { name: /^Filters/ }).click();
  const genre = page.getByLabel("Genre");
  await genre.fill("Action");
  await genre.press("Tab");
  await expect(page).toHaveURL(/facet=genres%3AAction/);
  await expect(cards(page)).toHaveCount(1);
  await expect(cards(page).first()).toContainText("Star Wars");
  await expect(page.getByRole("button", { name: "Remove filter Genre: Action" })).toBeVisible();
  await page.getByRole("button", { name: "Remove filter Genre: Action" }).click();
  // back on the same view, which keeps its "Show more"
  await expect(cards(page)).toHaveCount(51);

  // "/" focuses the filter box; the text filters after a short pause
  await page.locator("body").click({ position: { x: 1, y: 1 } });
  await page.keyboard.press("/");
  const filter = page.getByLabel("Filter by title or note");
  await expect(filter).toBeFocused();
  await page.keyboard.type("star wars");
  await expect(page).toHaveURL(/q=star(%20|\+)wars/);
  await expect(cards(page)).toHaveCount(1, { timeout: 15_000 });
  await expect(cards(page).first()).toContainText("Star Wars");
});

test("the next step logs in one click and Undo takes it back; the quick menu sets status and tags", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `step${Date.now()}`, "Step Tester");
  await seed(page);
  await open(page, "/library?kind=show");
  const step = page.getByRole("button", { name: "Mark S1 E2 watched: Downstream" });
  await step.click();
  const toast = page.getByRole("status").filter({ hasText: "Logged: Watched S1 E2" });
  await expect(toast).toBeVisible();
  await expect(page.getByRole("button", { name: "Mark S1 E3 watched: Downstream" })).toBeVisible();
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Undone")).toBeVisible();
  await expect(page.getByRole("button", { name: "Mark S1 E2 watched: Downstream" })).toBeVisible();
  // after each refetch the new card's button works: two more clicks log E2, then E3 (UX1-2)
  await page.getByRole("button", { name: "Mark S1 E2 watched: Downstream" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Watched S1 E2" })).toBeVisible();
  await page.getByRole("button", { name: "Mark S1 E3 watched: Downstream" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Watched S1 E3" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Mark S1 E4 watched: Downstream" })).toBeVisible();

  await open(page, "/library?kind=movie");
  await page.getByRole("button", { name: "Actions for Harbour Lights" }).click();
  await page.getByRole("menuitem", { name: "Mark as Completed" }).click();
  await expect(page.getByText("Marked as Completed")).toBeVisible();
  const harbour = cards(page).filter({ hasText: "Harbour Lights" });
  await expect(harbour).toContainText("Completed");

  await page.getByRole("button", { name: "Actions for Ninety Winters" }).click();
  await page.getByRole("menuitem", { name: "Edit tags" }).click();
  const dialog = page.getByRole("dialog", { name: "Tags for Ninety Winters" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("New tag").fill("rainy");
  await dialog.getByRole("button", { name: "Add tag" }).click();
  await expect(dialog.getByRole("button", { name: "Remove tag rainy" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(cards(page).filter({ hasText: "Ninety Winters" })).toContainText("rainy");
});

test("a background refresh keeps an open quick menu", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `keep${Date.now()}`, "Keep Tester");
  await seed(page);
  const lib = await (await page.request.get("/api/v1/library?limit=500")).json();
  const id = (title: string) => lib.items.find((i: { item: { title: string } }) => i.item.title === title).item.id;
  // a change elsewhere reaches the page as a server event and refetches the rows under the
  // open menu: first another row changes (grid), then the menu's own row (list)
  const changes = [
    { view: "grid", change: () => page.request.put(`/api/v1/library/${id("Harbour Lights")}/status`, { data: { status: "completed" } }) },
    { view: "list", change: () => page.request.post(`/api/v1/library/${id("Ninety Winters")}/tags`, { data: { tag: "rainy" } }) },
  ];
  for (const { view, change } of changes) {
    await open(page, `/library?kind=movie&view=${view}`);
    await page.getByRole("button", { name: "Actions for Ninety Winters" }).click();
    const menu = page.getByRole("menu", { name: "Actions for Ninety Winters" });
    await expect(menu).toBeVisible();
    const refetched = page.waitForResponse((res) => res.url().includes("/_fn/library_query"));
    expect((await change()).ok()).toBeTruthy();
    await refetched;
    await expect(page.locator("#lib-results")).not.toHaveAttribute("aria-busy", "true");
    if (view === "grid") await expect(cards(page).filter({ hasText: "Harbour Lights" })).toContainText("Completed");
    else await expect(page.getByRole("row").filter({ hasText: "Ninety Winters" })).toContainText("rainy");
    await expect(menu).toBeVisible();
    await menu.getByRole("menuitem", { name: "Edit tags" }).click({ timeout: 5_000 });
    const dialog = page.getByRole("dialog", { name: "Tags for Ninety Winters" });
    await expect(dialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  }
});

// WP4-3: the bulk bar of the list view (design.md 3.5, 6.2) and removing from the library
const rowSelect = (page: Page, title: string) => page.getByRole("checkbox", { name: `Select ${title}` });
const bar = (page: Page) => page.getByRole("toolbar", { name: "Bulk actions" });
// one toast of the toaster (several can be up at once; each has its own Undo)
const toast = (page: Page, text: string) => page.getByRole("status").locator(":scope > div").filter({ hasText: text });

test("bulk: select three, add a tag and undo; remove with a confirm and undo", async ({ page }) => {
  test.setTimeout(150_000);
  await signUp(page, `bulk${Date.now()}`, "Bulk Tester");
  await seed(page);
  await open(page, "/library?view=list&sort=title");
  await expect(page.getByRole("table")).toBeVisible();
  for (const title of ["Harbour Lights", "Ninety Winters", "The Glass Ferry"]) await rowSelect(page, title).check();
  await expect(bar(page)).toContainText("3 selected");

  await bar(page).getByRole("button", { name: "Add tag" }).click();
  const dialog = page.getByRole("dialog", { name: "Add a tag to the selected entries" });
  await dialog.getByLabel("New tag").fill("weekend");
  await dialog.getByRole("button", { name: "Add tag" }).click();
  const tagged = toast(page, "Tagged 3 entries with weekend");
  await expect(tagged).toBeVisible();
  const rows = page.getByRole("row");
  for (const title of ["Harbour Lights", "Ninety Winters", "The Glass Ferry"]) {
    await expect(rows.filter({ hasText: title })).toContainText("weekend");
  }
  await expect(rows.filter({ hasText: "Downstream" })).not.toContainText("weekend");
  // the selection stays through the refresh after the change
  await expect(bar(page)).toContainText("3 selected");
  await tagged.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Undone")).toBeVisible();
  await expect(rows.filter({ hasText: "weekend" })).toHaveCount(0);

  // remove two with a confirm naming the count; Cancel keeps them
  await rowSelect(page, "The Glass Ferry").uncheck();
  await expect(bar(page)).toContainText("2 selected");
  await bar(page).getByRole("button", { name: "Remove" }).click();
  const confirm = page.getByRole("dialog", { name: "Remove 2 entries from your library?" });
  await expect(confirm).toBeVisible();
  await confirm.getByRole("button", { name: "Cancel" }).click();
  await expect(rows.filter({ hasText: "Harbour Lights" })).toHaveCount(1);
  await bar(page).getByRole("button", { name: "Remove" }).click();
  await page.getByRole("dialog", { name: "Remove 2 entries from your library?" }).getByRole("button", { name: "Remove" }).click();
  const removed = toast(page, "Removed 2 entries from your library");
  await expect(removed).toBeVisible();
  await expect(rows.filter({ hasText: "Harbour Lights" })).toHaveCount(0);
  await expect(rows.filter({ hasText: "Ninety Winters" })).toHaveCount(0);
  await expect(bar(page)).toHaveCount(0);
  await removed.getByRole("button", { name: "Undo" }).click();
  await expect(rows.filter({ hasText: "Harbour Lights" })).toHaveCount(1);
  await expect(rows.filter({ hasText: "Ninety Winters" })).toHaveCount(1);
  // Harbour Lights got its tag back
  await expect(rows.filter({ hasText: "Harbour Lights" })).toContainText("cosy");

  // "Select all on this page", Set status, and a filter change clears the selection
  await page.getByRole("checkbox", { name: "Select all on this page" }).check();
  await expect(bar(page)).toContainText("5 selected");
  await bar(page).getByRole("button", { name: "Set status" }).click();
  await page.getByRole("menuitem", { name: "Mark as Paused" }).click();
  await expect(toast(page, "5 entries marked as Paused")).toBeVisible();
  await page.getByRole("radio", { name: "Watch" }).click();
  await expect(page).toHaveURL(/group=watch/);
  await expect(bar(page)).toHaveCount(0);
});

test("bulk: the selection survives a background refresh; quick menu add to list and remove", async ({ page }) => {
  test.setTimeout(150_000);
  await signUp(page, `bulkkeep${Date.now()}`, "Bulk Keep");
  await seed(page);
  const lib = await (await page.request.get("/api/v1/library?limit=500")).json();
  const id = (title: string) => lib.items.find((i: { item: { title: string } }) => i.item.title === title).item.id;
  await open(page, "/library?view=list");
  await rowSelect(page, "Harbour Lights").check();
  await rowSelect(page, "Downstream").check();
  await expect(bar(page)).toContainText("2 selected");
  // a change elsewhere refetches the rows (server event); the ticks and the bar stay
  const refetched = page.waitForResponse((res) => res.url().includes("/_fn/library_query"));
  expect((await page.request.post(`/api/v1/library/${id("Downstream")}/tags`, { data: { tag: "rainy" } })).ok()).toBeTruthy();
  await refetched;
  await expect(page.getByRole("row").filter({ hasText: "Downstream" })).toContainText("rainy");
  await expect(rowSelect(page, "Harbour Lights")).toBeChecked();
  await expect(rowSelect(page, "Downstream")).toBeChecked();
  await expect(bar(page)).toContainText("2 selected");

  // the bar in both themes: axe, and no horizontal scroll at this width (360 px in its own test)
  for (const theme of ["light", "dark"] as const) {
    await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
    await open(page, "/library?view=list");
    await page.getByRole("checkbox", { name: "Select all on this page" }).check();
    await expect(bar(page)).toBeVisible();
    await expectNoA11yViolations(page, `bulk bar ${theme}`);
  }
  await expectReflow(page, "bulk bar");

  // quick menu: add to a list, then remove with Undo
  const made = await page.request.post("/api/v1/lists", { data: { name: "Weekend picks" } });
  expect(made.ok()).toBeTruthy();
  await open(page, "/library?kind=movie");
  await page.getByRole("button", { name: "Actions for Ninety Winters" }).click();
  await page.getByRole("menuitem", { name: "Add to list..." }).click();
  const pick = page.getByRole("dialog", { name: "Add to which list?" });
  await pick.getByRole("button", { name: "Weekend picks" }).click();
  await expect(toast(page, "Added one entry to Weekend picks")).toBeVisible();
  const lists = await (await page.request.get("/api/v1/lists")).json();
  expect(lists.find((l: { name: string }) => l.name === "Weekend picks").item_ids).toContain(id("Ninety Winters"));

  await page.getByRole("button", { name: "Actions for Ninety Winters" }).click();
  await page.getByRole("menuitem", { name: "Remove from library" }).click();
  const gone = toast(page, "Removed Ninety Winters from your library");
  await expect(gone).toBeVisible();
  await expect(cards(page).filter({ hasText: "Ninety Winters" })).toHaveCount(0);
  await gone.getByRole("button", { name: "Undo" }).click();
  await expect(cards(page).filter({ hasText: "Ninety Winters" })).toHaveCount(1);

  // the item page overflow removes too; its history stays
  await open(page, `/items/${id("Ninety Winters")}`);
  await page.getByRole("button", { name: "More actions for Ninety Winters" }).click();
  await page.getByRole("menuitem", { name: "Remove from library" }).click();
  await expect(toast(page, "Removed Ninety Winters from your library")).toBeVisible();
  await expect(page.getByRole("button", { name: "Want to watch" })).toBeVisible();
  const history = await (await page.request.get(`/api/v1/items/${id("Ninety Winters")}/progress`)).json();
  expect(history.length).toBeGreaterThan(0);
});

for (const size of [
  { name: "1280", width: 1280, height: 800 },
  { name: "360", width: 360, height: 640 },
]) {
  test.describe(`axe at ${size.name} px`, () => {
    test.use({ viewport: { width: size.width, height: size.height } });

    test("the library has no violations in light and dark", async ({ page }) => {
      test.setTimeout(180_000);
      await signUp(page, `axe${size.name}${Date.now()}`, "Axe Library");
      await open(page, "/library");
      await expectNoA11yViolations(page, `${size.name} empty`);
      await seed(page);
      for (const theme of ["light", "dark"] as const) {
        await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
        await open(page, "/library?group=watch");
        await expect(cards(page)).toHaveCount(3);
        await page.getByRole("button", { name: /^Filters/ }).click();
        await expect(page.locator("#lib-filters")).toBeVisible();
        await expectNoA11yViolations(page, `${size.name} ${theme} grid`);
        await page.getByRole("button", { name: "Actions for Downstream" }).click();
        await expect(page.getByRole("menu")).toBeVisible();
        await expectNoA11yViolations(page, `${size.name} ${theme} quick menu`);
        await page.keyboard.press("Escape");
        await page.getByRole("button", { name: /^Filters/ }).click();
        await open(page, "/library?group=watch&view=list");
        await expect(page.getByRole("table")).toBeVisible();
        await expectNoA11yViolations(page, `${size.name} ${theme} list`);
        await open(page, "/library?kind=show&status=completed");
        await expect(page.getByRole("heading", { name: "Nothing matches these filters" })).toBeVisible();
        await expectNoA11yViolations(page, `${size.name} ${theme} no match`);
      }
    });
  });
}

test.describe("the bulk bar at 360 px", () => {
  test.use({ viewport: { width: 360, height: 640 } });

  test("a remove counts as logged; axe and reflow in both themes; the focused row and the toasts stay clear of it", async ({ page }) => {
    // UX4-7, UX4-15, UX4-22
    test.setTimeout(180_000);
    const beacons: { events: Record<string, string>[] }[] = [];
    await page.route("**/api/v1/usage/funnel", async (route) => {
      beacons.push(route.request().postDataJSON());
      await route.fulfill({ status: 204 });
    });
    await signUp(page, `bulk360${Date.now()}`, "Bulk Narrow");
    await seed(page);
    await open(page, "/library?view=list&sort=title");

    // one remove: the bulk flow ends as logged, never as abandoned
    await rowSelect(page, "Meeple Harbour").check();
    await bar(page).getByRole("button", { name: "Remove" }).click();
    await page.getByRole("dialog", { name: /^Remove/ }).getByRole("button", { name: "Remove" }).click();
    await expect(toast(page, "Removed one entry from your library")).toBeVisible();
    await expect(bar(page)).toHaveCount(0);
    const bulk = () => beacons.flatMap((b) => b.events).filter((e) => e.flow === "bulk");
    await expect.poll(() => bulk().length, { timeout: 20_000 }).toBeGreaterThan(0);
    expect(bulk()).toEqual([expect.objectContaining({ flow: "bulk", outcome: "logged", taps: expect.any(String) })]);

    for (const theme of ["light", "dark"] as const) {
      await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
      await open(page, "/library?view=list&sort=title");
      await page.getByRole("checkbox", { name: "Select all on this page" }).check();
      await expect(bar(page)).toBeVisible();
      await expectNoA11yViolations(page, `bulk bar 360 ${theme}`);
      await expectReflow(page, `bulk bar 360 ${theme}`);
    }

    // walking the page with Tab, the focused control is never under the sticky bar
    await page.getByRole("checkbox", { name: "Select all on this page" }).focus();
    for (let i = 0; i < 16; i++) {
      await page.keyboard.press("Tab");
      const clear = await page.evaluate(() => {
        const f = document.activeElement as HTMLElement | null;
        const b = document.querySelector("[data-sticky-bar]");
        if (!f || !b || b.contains(f) || f === document.body) return "";
        const fr = f.getBoundingClientRect();
        const br = b.getBoundingClientRect();
        const covered = fr.bottom > br.top + 1 && fr.top < br.bottom - 1;
        return covered ? `${f.outerHTML.slice(0, 80)} at ${fr.top}-${fr.bottom}, bar ${br.top}-${br.bottom}` : "";
      });
      expect(clear, `Tab ${i}`).toBe("");
    }

    // a toast floats above the bar, not on it
    await bar(page).getByRole("button", { name: "Add tag" }).click();
    const dialog = page.getByRole("dialog", { name: "Add a tag to the selected entries" });
    await dialog.getByLabel("New tag").fill("narrow");
    await dialog.getByRole("button", { name: "Add tag" }).click();
    const tagged = toast(page, "Tagged 4 entries with narrow");
    await expect(tagged).toBeVisible();
    const t = await tagged.boundingBox();
    const b = await bar(page).boundingBox();
    expect(t && b && t.y + t.height <= b.y + 1, `toast ${JSON.stringify(t)} bar ${JSON.stringify(b)}`).toBeTruthy();
  });
});

/** The status tabs as the server writes them (`StatusTabs` in `crates/web/src/library.rs`). */
const STATUS_TABS = /<div class="-mx-4 mb-3 overflow-x-auto px-4"><div role="tablist" aria-label="Status"[\s\S]*?<\/div><!><\/div>/;

// Race package F2 (race diagnosis 2.3): the server once sent the Library without its status
// tabs while the browser rendered them, hydration panicked, the page was marked hydrated all
// the same and stayed dead below the kind chips. These tests give the browser that answer.

/** Answer the Library document as the failed run did: a placeholder where the status tabs are. */
async function withoutStatusTabs(page: Page, times?: number) {
  const seen = { swapped: 0 };
  await page.route(
    (url) => url.pathname === "/library",
    async (route) => {
      const res = await route.fetch();
      const html = await res.text();
      const cut = html.replace(STATUS_TABS, "<!>");
      expect(cut, "the status tabs were found in the server's page").not.toBe(html);
      seen.swapped += 1;
      await route.fulfill({ response: res, body: cut });
    },
    times ? { times } : undefined,
  );
  return seen;
}

/** Panics only: a release build prints no detailed hydration message (RACE-12). */
const panicsIn = (failures: string[]) => failures.filter((f) => /panicked at/.test(f)).length;

test("a page whose hydration panics reloads once and works; the guard reports the panic", async ({ page, appFailures }) => {
  test.setTimeout(120_000);
  await signUp(page, `hyd${Date.now()}`, "Hydration Tester");
  await seed(page);
  const seen = await withoutStatusTabs(page, 1);
  // the first page's code waits until a person has typed into the server's page
  let release = () => {};
  const typed = new Promise<void>((r) => (release = r));
  let wasm = 0;
  await page.route(
    (url) => url.pathname.endsWith(".wasm"),
    async (route) => {
      if (wasm++ === 0) await typed;
      await route.continue();
    },
  );
  await page.goto("/library?view=list&sort=title", { waitUntil: "domcontentloaded" });
  await page.locator("#lib-q").fill("Harbour");
  release();
  await hydrated(page);
  // the reloaded page is the server's own answer: alive from top to bottom
  await expect(page.getByRole("checkbox", { name: "Select all on this page" })).toBeEnabled();
  await expect(page.getByRole("tablist", { name: "Status" })).toBeVisible();
  expect(seen.swapped).toBe(1);
  await expect(page.locator("body[data-hydration-failed]")).toHaveCount(0);
  // the text typed before the reload is still there, and the page works with it (RACE-13)
  await expect(page.locator("#lib-q")).toHaveValue("Harbour");
  await expect(page).toHaveURL(/q=Harbour/);
  // the second reading of the page has a reason (RACE-13)
  await expect(page.getByRole("status").filter({ hasText: "did not start properly and was loaded again" })).toHaveCount(1);
  // a later failure reloads again: the note of the reload is gone
  expect(await page.evaluate(() => sessionStorage.getItem("app-hydration-retry"))).toBeNull();
  // and the guard of every test saw it (it fails the test unless the test expects it)
  expect(panicsIn(appFailures)).toBe(1);
  appFailures.length = 0;
});

test("a page whose hydration panics again after the reload is not marked hydrated and says so", async ({ page, appFailures }) => {
  test.setTimeout(120_000);
  await signUp(page, `hyd2${Date.now()}`, "Hydration Twice");
  await seed(page);
  const seen = await withoutStatusTabs(page);
  const loads: string[] = [];
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) loads.push(frame.url());
  });
  await page.goto("/library?view=list&sort=title");
  // one reload, then the page says what it is instead of pretending to work. The notice is
  // shown only by the branch that does not reload (RACE-4): once it is there, no further
  // load can follow, so no fixed wait is needed to rule out a loop (RACE-9)
  const notice = page.getByRole("alert").filter({ hasText: "This page did not start properly" });
  await expect(notice).toBeVisible({ timeout: 30_000 });
  await page.waitForLoadState("load");
  expect(panicsIn(appFailures)).toBe(2);
  expect(seen.swapped, "no reload loop").toBe(2);
  expect(loads, "no reload loop").toHaveLength(2);
  expect(await page.evaluate(() => sessionStorage.getItem("app-hydration-retry")), "the tab is marked as reloaded").toBe("1");
  await expect(page.locator("body[data-hydration-failed]")).toBeAttached();
  await expect(page.locator("body[data-hydrated]")).toHaveCount(0);
  // the notice offers the reload by hand; it loads the page again (the server is still wrong)
  await notice.getByRole("link", { name: "Reload" }).click();
  await expect.poll(() => loads.length, { timeout: 30_000 }).toBe(3);
  await expect(notice).toBeVisible({ timeout: 30_000 });
  expect(panicsIn(appFailures)).toBe(3);
  appFailures.length = 0;
});

test("the event that announces a change from a card is answered at the end of the quiet time, not at once", async ({ page }) => {
  // race package F5 (diagnosis 3.1 point 3): the card refetched the rows, and the event of the
  // same change refetched them a second time at once
  test.setTimeout(120_000);
  await signUp(page, `pace${Date.now()}`, "Pace Tester");
  await seed(page);
  await gateEntityEvents(page);
  await open(page, "/library?kind=movie");
  const starts = requestStarts(page, "/_fn/library_query");
  await holdEvents(page, true);
  await page.getByRole("button", { name: "Actions for Harbour Lights" }).click();
  await page.getByRole("menuitem", { name: "Mark as Completed" }).click();
  await expect(cards(page).filter({ hasText: "Harbour Lights" })).toContainText("Completed");
  await expect.poll(() => starts.length, { timeout: 20_000 }).toBe(1);
  await expect.poll(() => heldEvents(page), { timeout: 20_000 }).toBeGreaterThan(0);
  await holdEvents(page, false);
  await expect.poll(() => starts.length, { timeout: 20_000 }).toBe(2);
  expect(starts[1] - starts[0]).toBeGreaterThanOrEqual(1_900);
});

test("a refetch keeps the kind chips, the status tabs and the focus on them until the answer is there", async ({ page }) => {
  // race package F6 (diagnosis 4): every refetch of the counts swapped the chips and the tabs
  // for their skeletons until the answer came
  test.setTimeout(120_000);
  await signUp(page, `hold${Date.now()}`, "Hold Tester");
  await seed(page);
  const lib = await (await page.request.get("/api/v1/library?limit=500")).json();
  const id = lib.items.find((i: { item: { title: string } }) => i.item.title === "Harbour Lights").item.id;
  await open(page, "/library");
  let release = () => {};
  const held = new Promise<void>((r) => (release = r));
  await page.route("**/_fn/library_counts*", async (route) => {
    await held;
    await route.continue();
  });
  await page.getByRole("tablist", { name: "Status" }).evaluate((el) => ((el as HTMLElement).dataset.same = "1"));
  await page.getByRole("list", { name: "Kinds" }).evaluate((el) => ((el as HTMLElement).dataset.same = "1"));
  const planned = page.getByRole("tab", { name: /^Planned/ });
  await planned.focus();
  // a change from elsewhere starts the refetch; its counts are held
  const asked = page.waitForRequest((r) => r.url().includes("/_fn/library_counts"));
  expect((await page.request.put(`/api/v1/library/${id}/status`, { data: { status: "completed" } })).ok()).toBeTruthy();
  await asked;
  await twoFrames(page);
  await expect(page.locator('[role="tablist"][data-same="1"]')).toBeVisible();
  await expect(page.locator('ul[data-same="1"]')).toBeVisible();
  await expect(planned).toBeFocused();
  await expect(page.getByText("5 items", { exact: true })).toBeVisible();
  const answered = page.waitForResponse((r) => r.url().includes("/_fn/library_counts"));
  release();
  await answered;
  await page.unroute("**/_fn/library_counts*");
  await expect(page.getByRole("tab", { name: /^Completed 2/ })).toBeVisible();
});
