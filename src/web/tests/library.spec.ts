import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations, hydrated, signUp } from "./helpers";

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
  await expect
    .poll(async () => (await (await page.request.get(`/api/v1/imports/${id}`)).json()).state, { timeout: 30_000 })
    .toBe("done");
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
  const step = page.getByRole("button", { name: "Watched S1 E2: Downstream" });
  await step.click();
  const toast = page.getByRole("status").filter({ hasText: "Logged: Watched S1 E2" });
  await expect(toast).toBeVisible();
  await expect(page.getByRole("button", { name: "Watched S1 E3: Downstream" })).toBeVisible();
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Undone")).toBeVisible();
  await expect(page.getByRole("button", { name: "Watched S1 E2: Downstream" })).toBeVisible();
  // after each refetch the new card's button works: two more clicks log E2, then E3 (UX1-2)
  await page.getByRole("button", { name: "Watched S1 E2: Downstream" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Watched S1 E2" })).toBeVisible();
  await page.getByRole("button", { name: "Watched S1 E3: Downstream" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Watched S1 E3" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Watched S1 E4: Downstream" })).toBeVisible();

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
