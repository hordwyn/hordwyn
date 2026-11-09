import { test, expect, type Page } from "@playwright/test";
import { changeMine, expectNoA11yViolations, hydrated, importDone, signUp } from "./helpers";

// Keyboard map (WP4-4; design.md 3.2, 6.1-6.3, 8): the G chords with their hint and focus on
// the page heading, never while typing or in a dialog; `?` lists every shortcut; the Library
// grid's roving tabindex with arrows, Home/End, Enter, W, T and L; Home J/K and Enter; the
// item page's W, R, T and L; the "Keyboard shortcuts" switch in Settings > Account
// (WCAG 2.1.4).

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

async function open(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

/** Nothing focused (keys go to the page). */
const blur = (page: Page) => page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());

const h1 = (page: Page, name: string) => page.getByRole("heading", { level: 1, name, exact: true });

test("G chords go to a section with focus on its heading, never while typing or in a dialog", async ({ page }) => {
  test.setTimeout(90_000);
  await signUp(page, `chord${Date.now()}`, "Cho");
  await open(page, "/settings/account");

  const hint = page.getByTestId("chord-hint");
  await page.keyboard.press("g");
  await expect(hint).toHaveText("G pressed: then H, L, Y, T or P to go to a section.");
  await page.keyboard.press("l");
  await expect(page).toHaveURL(/\/library$/);
  await expect(h1(page, "Library")).toBeFocused();
  await expect(hint).toHaveText("");

  for (const [key, path, name] of [
    ["y", /\/history$/, "History"],
    ["t", /\/lists$/, "Lists"],
    ["p", /\/people$/, "People"],
  ] as const) {
    await page.keyboard.press("g");
    await page.keyboard.press(key);
    await expect(page).toHaveURL(path);
    await expect(h1(page, name)).toBeFocused();
  }
  await page.keyboard.press("g");
  await page.keyboard.press("h");
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("heading", { level: 1 })).toBeFocused();

  // another key ends the chord; the window runs out after 1.5 s
  await page.keyboard.press("g");
  await page.keyboard.press("x");
  await expect(hint).toHaveText("");
  await page.keyboard.press("l");
  await expect(page).toHaveURL(/\/$/);
  await page.keyboard.press("g");
  await expect(hint).not.toHaveText("");
  await expect(hint).toHaveText("", { timeout: 3_000 });
  await page.keyboard.press("l");
  await expect(page).toHaveURL(/\/$/);

  // typing in a field is typing
  await open(page, "/library");
  await page.locator("#lib-q").click();
  await page.keyboard.type("gl");
  await expect(page.locator("#lib-q")).toHaveValue("gl");
  await expect(page).toHaveURL(/\/library(\?q=gl)?$/);
  await expect(hint).toHaveText("");

  // a modifier is not a chord
  await blur(page);
  await page.keyboard.press("Alt+g");
  await expect(hint).toHaveText("");

  // `?` lists every shortcut, global and per page; inside the dialog G does nothing
  await page.keyboard.press("?");
  const help = page.getByRole("dialog", { name: "Keyboard shortcuts" });
  await expect(help).toBeVisible();
  for (const text of ["Go to Library", "Go to People", "Next step of the focused card", "Move between Continue cards", "Focus your rating", "Add as Planned, or run the next step"]) {
    await expect(help).toContainText(text);
  }
  for (const section of ["Everywhere", "Home", "Library", "Item page", "Search"]) {
    await expect(help.getByRole("heading", { name: section, exact: true })).toBeVisible();
  }
  await expectNoA11yViolations(page, "shortcut help");
  await page.keyboard.press("g");
  await page.keyboard.press("p");
  await expect(help).toBeVisible();
  await expect(page).toHaveURL(/\/library/);
  await page.keyboard.press("Escape");
});

test("the library grid has one tab stop: arrows, Home/End, W, T, L and Enter", async ({ page }) => {
  test.setTimeout(120_000);
  // 400 px: two columns (cards are at least 8.5 rem wide)
  await page.setViewportSize({ width: 400, height: 900 });
  await signUp(page, `grid${Date.now()}`, "Gri");
  await importRows(page, [
    { source_id: "1", kind: "movie", title: "Alpha Night", year: 2001, status: "planned" },
    { source_id: "2", kind: "movie", title: "Bravo Pages", year: 2002, status: "planned" },
    { source_id: "3", kind: "movie", title: "Charlie Tiles", year: 2003, status: "planned" },
  ]);
  const made = await page.request.post("/api/v1/lists", { data: { name: "Keyboard picks" } });
  expect(made.ok()).toBeTruthy();
  await open(page, "/library?sort=title");
  const links = page.getByTestId("library-card").locator("a[id^=card-]");
  await expect(links).toHaveCount(3);
  await expect(page.locator("a[id^=card-][tabindex='0']")).toHaveCount(1);
  await expect(links.nth(0)).toHaveAttribute("tabindex", "0");
  await expect(links.nth(1)).toHaveAttribute("tabindex", "-1");

  await links.nth(0).focus();
  await page.keyboard.press("ArrowRight");
  await expect(links.nth(1)).toBeFocused();
  await expect(links.nth(1)).toHaveAttribute("tabindex", "0");
  await expect(links.nth(0)).toHaveAttribute("tabindex", "-1");
  await page.keyboard.press("ArrowLeft");
  await expect(links.nth(0)).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(links.nth(2)).toBeFocused();
  await page.keyboard.press("ArrowUp");
  await expect(links.nth(0)).toBeFocused();
  await page.keyboard.press("Control+End");
  await expect(links.nth(2)).toBeFocused();
  await page.keyboard.press("Control+Home");
  await expect(links.nth(0)).toBeFocused();
  await page.keyboard.press("End");
  await expect(links.nth(1)).toBeFocused();
  await page.keyboard.press("Home");
  await expect(links.nth(0)).toBeFocused();
  await expect(page.locator("a[id^=card-][tabindex='0']")).toHaveCount(1);

  // T: the tag editor of the focused card
  await page.keyboard.press("t");
  const tags = page.getByRole("dialog", { name: "Tags for Alpha Night" });
  await expect(tags).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(tags).toHaveCount(0);

  // L: the list picker
  await links.nth(0).focus();
  await page.keyboard.press("l");
  const picker = page.getByRole("dialog", { name: "Add to which list?" });
  await expect(picker).toBeVisible();
  await picker.getByRole("button", { name: "Keyboard picks" }).click();
  await expect(page.getByText(/to Keyboard picks/).first()).toBeVisible();

  // W: the next step of the focused card, with Undo
  await links.nth(1).focus();
  await page.keyboard.press("w");
  const logged = page.getByRole("status").locator("> div").filter({ hasText: "Logged: Watched" });
  await expect(logged).toBeVisible();
  await logged.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByRole("status").locator("> div").filter({ hasText: /^Undone/ })).toBeVisible();

  // Enter opens the item
  await links.nth(2).focus();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/items\//);
  await expect(page.getByRole("heading", { level: 1, name: "Charlie Tiles" })).toBeVisible();
});

test("Home: J and K move between Continue cards, Enter runs the next step", async ({ page }) => {
  test.setTimeout(150_000);
  await signUp(page, `home${Date.now()}`, "Hom");
  const ids = await importRows(page, [
    { source_id: "1", kind: "movie", title: "Delta Rows", year: 2004, status: "in_progress", history: [{ date: "2026-01-02", progress: { type: "position", seconds: 600 } }] },
    { source_id: "2", kind: "movie", title: "Echo Fields", year: 2005, status: "in_progress", history: [{ date: "2026-01-03", progress: { type: "position", seconds: 900 } }] },
  ]);
  await open(page, "/");
  const continueCards = page.getByTestId("continue-card");
  await expect(continueCards).toHaveCount(2);
  const step = (i: number) => continueCards.nth(i).locator("[data-next-step]");
  await page.keyboard.press("j");
  await expect(step(0)).toBeFocused();
  // a change of the user's own data that Home does not show refetches Home with the same
  // answer: the focus stays where J put it (a refetch rendered Home afresh and dropped it; seen
  // in a full run, then from other people's imports, which no longer reach this page, DEC-490)
  const refetched = page.waitForResponse((r) => r.url().includes("/_fn/home"), { timeout: 120_000 });
  await changeMine(page);
  await refetched;
  // the answer is rendered a few microtasks after the response: two frames later it is in
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  await expect(step(0)).toBeFocused();
  await page.keyboard.press("j");
  await expect(step(1)).toBeFocused();
  await page.keyboard.press("j");
  await expect(step(1)).toBeFocused();
  await page.keyboard.press("k");
  await expect(step(0)).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByText("Logged: Watched").first()).toBeVisible();
});

test("the item page: W logs the next step, R focuses the rating, T tags, L lists", async ({ page }) => {
  test.setTimeout(90_000);
  await signUp(page, `itemk${Date.now()}`, "Ite");
  const ids = await importRows(page, [{ source_id: "1", kind: "movie", title: "Foxtrot Lane", year: 2006, status: "planned" }]);
  expect((await page.request.post("/api/v1/lists", { data: { name: "Item keys" } })).ok()).toBeTruthy();
  await open(page, `/items/${ids["Foxtrot Lane"]}`);
  await expect(page.getByRole("heading", { level: 1, name: "Foxtrot Lane" })).toBeVisible();

  await page.keyboard.press("r");
  await expect(page.getByRole("radiogroup", { name: "Your rating" }).locator("[tabindex='0']")).toBeFocused();
  await blur(page);
  await page.keyboard.press("t");
  const tags = page.getByRole("dialog", { name: "Tags for Foxtrot Lane" });
  await expect(tags).toBeVisible();
  await page.keyboard.press("Escape");
  await page.keyboard.press("l");
  const picker = page.getByRole("dialog", { name: "Add to which list?" });
  await expect(picker).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(picker).toHaveCount(0);
  await page.keyboard.press("w");
  await expect(page.getByText("Logged: Watched").first()).toBeVisible();
});

test("Settings > Account turns the single-key shortcuts off and on again", async ({ page }) => {
  await signUp(page, `off${Date.now()}`, "Off");
  await open(page, "/settings/account");
  const sw = page.getByRole("switch", { name: "Keyboard shortcuts" });
  await expect(sw).toHaveAttribute("aria-checked", "true");
  await sw.click();
  await expect(sw).toHaveAttribute("aria-checked", "false");

  // off: no `?`, no G chord, no N (Ctrl+K stays)
  await open(page, "/library");
  await blur(page);
  await page.keyboard.press("?");
  await page.keyboard.press("g");
  await page.keyboard.press("p");
  await page.keyboard.press("n");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page).toHaveURL(/\/library$/);
  await page.keyboard.press("Control+k");
  await expect(page.getByRole("dialog", { name: "Search" })).toBeVisible();
  await page.keyboard.press("Escape");

  await open(page, "/settings/account");
  await sw.click();
  await expect(sw).toHaveAttribute("aria-checked", "true");
  await blur(page);
  await page.keyboard.press("?");
  await expect(page.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeVisible();
});

test("the shortcut help at 360 px in both themes; signed out it lists only what works signed out", async ({ page }) => {
  // UX4-15, UX4-22: design.md 8 extends axe and reflow to every page
  test.setTimeout(90_000);
  await page.setViewportSize({ width: 360, height: 640 });
  await signUp(page, `kb360${Date.now()}`, "Kb");
  for (const theme of ["light", "dark"] as const) {
    await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
    await open(page, "/library");
    await blur(page);
    await page.keyboard.press("?");
    const help = page.getByRole("dialog", { name: "Keyboard shortcuts" });
    await expect(help).toBeVisible();
    await expect(help).toContainText("Go to Library");
    await expectNoA11yViolations(page, `shortcut help 360 ${theme}`);
    const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(over, "horizontal overflow").toBeLessThanOrEqual(1);
    await page.keyboard.press("Escape");
  }

  // signed out: search, help and close only; no N, no chords, no page keys
  await page.context().clearCookies();
  await open(page, "/sign-in");
  await blur(page);
  await page.keyboard.press("?");
  const help = page.getByRole("dialog", { name: "Keyboard shortcuts" });
  await expect(help).toBeVisible();
  await expect(help).toContainText("Search or add");
  for (const gone of ["Add something", "Go to Library", "Next step of the focused card", "Focus your rating", "Add as Planned, or run the next step"]) {
    await expect(help).not.toContainText(gone);
  }
  await expect(help.getByRole("heading", { name: "Library", exact: true })).toHaveCount(0);
  await page.keyboard.press("Escape");
  // and the empty palette offers no signed-in commands
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  await expect(palette).toBeVisible();
  for (const gone of ["Go to Lists", "New list", "Import a file"]) {
    await expect(palette.getByText(gone, { exact: true })).toHaveCount(0);
  }
});
