import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations, hydrated, signUp } from "./helpers";

// UX phase 1 item page (design.md 6.3, 5.4, 5.5, 3.4; addendum-1 1.3 Item row): the next step
// with Undo, the "With..." sheet with a person and a rating, the play sheet with players and
// the winner, the episode list's bulk marking with the ask-once question, the private note,
// the tag editor, keyboard focus in a sheet, and axe in both themes at 1280 and 360 px with a
// sheet open.

const ROWS = [
  { source_id: "1", kind: "movie", title: "Harbour Lights", year: 2021, status: "planned" },
  {
    source_id: "2",
    kind: "show",
    title: "Game of Thrones",
    year: 2011,
    status: "in_progress",
    // the TMDB mock knows this show: the catalog refresh brings its seasons
    external_ids: { tmdb: "1399" },
    history: [{ date: "2026-02-03", progress: { type: "episode", season: 1, episode: 1 } }],
  },
  { source_id: "3", kind: "board_game", title: "Meeple Harbour", year: 2017, status: "planned" },
  {
    source_id: "4",
    kind: "book",
    title: "The Glass Ferry",
    year: 2018,
    status: "in_progress",
    history: [{ date: "2026-03-04", progress: { type: "paged", page: 40 } }],
  },
];

/** Import the rows through the API and wait until the show has its seasons. */
async function seed(page: Page): Promise<Record<string, string>> {
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...ROWS.map((r) => JSON.stringify(r))].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  expect(started.status()).toBe(202);
  const { id } = await started.json();
  await expect
    .poll(async () => (await (await page.request.get(`/api/v1/imports/${id}`)).json()).state, { timeout: 30_000 })
    .toBe("done");
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  const ids: Record<string, string> = Object.fromEntries(
    lib.items.map((i: { item: { title: string; id: string } }) => [i.item.title, i.item.id]),
  );
  await expect
    .poll(
      async () => {
        const p = await (await page.request.get(`/api/v1/items/${ids["Game of Thrones"]}/page`)).json();
        return (p.item.attributes.seasons ?? []).length;
      },
      { timeout: 30_000 },
    )
    .toBeGreaterThan(0);
  return ids;
}

async function open(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

const bar = (page: Page) => page.getByTestId("action-bar");

test("the next step logs with Undo; the note and the tags are edited in place", async ({ page }) => {
  test.setTimeout(150_000);
  await signUp(page, `item${Date.now()}`, "Item Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["Harbour Lights"]}`);
  await expect(page.getByRole("heading", { level: 1, name: "Harbour Lights" })).toBeVisible();

  await bar(page).getByRole("button", { name: "Watched", exact: true }).click();
  const toast = page.getByRole("status").filter({ hasText: "Logged: Watched" });
  await expect(toast).toBeVisible();
  await expect(bar(page).getByRole("button", { name: "Watched again" })).toBeVisible();
  await expect(page.getByLabel("Status")).toHaveValue("completed");
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Undone")).toBeVisible();
  await expect(bar(page).getByRole("button", { name: "Watched", exact: true })).toBeVisible();
  await expect(page.getByLabel("Status")).toHaveValue("planned");

  // the private note (Notes tab)
  await page.getByRole("tab", { name: "Notes" }).click();
  await page.getByLabel("Private note").fill("Watch on a rainy evening");
  await page.getByRole("button", { name: "Save note" }).click();
  await expect(page.getByText("Note saved.")).toBeVisible();
  await open(page, `/items/${ids["Harbour Lights"]}`);
  await page.getByRole("tab", { name: "Notes" }).click();
  await expect(page.getByLabel("Private note")).toHaveValue("Watch on a rainy evening");

  // tags
  await page.getByRole("button", { name: "Edit tags" }).click();
  const dialog = page.getByRole("dialog", { name: "Tags for Harbour Lights" });
  await dialog.getByLabel("New tag").fill("rainy");
  await dialog.getByRole("button", { name: "Add tag" }).click();
  await expect(dialog.getByRole("button", { name: "Remove tag rainy" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole("list", { name: "Tags" })).toContainText("rainy");
});

test("the With... sheet logs with a new person and a rating; focus stays in it; a changed sheet asks before closing", async ({ page }) => {
  test.setTimeout(150_000);
  await signUp(page, `sheet${Date.now()}`, "Sheet Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["Game of Thrones"]}`);
  const opener = page.getByRole("button", { name: "Log with details: Game of Thrones" });
  await opener.click();
  const sheet = page.getByRole("dialog", { name: "Log with details" });
  await expect(sheet).toBeVisible();
  await expect(sheet).toBeFocused();
  await expect(sheet.getByTestId("sheet-step")).toHaveText("Watched S1 E2");
  // Tab and Shift+Tab stay inside the sheet
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press("Tab");
    expect(await sheet.evaluate((el) => el.contains(document.activeElement))).toBe(true);
  }
  await page.keyboard.press("Shift+Tab");
  expect(await sheet.evaluate((el) => el.contains(document.activeElement))).toBe(true);

  await sheet.getByRole("radio", { name: "Yesterday" }).click();
  const people = sheet.getByRole("combobox", { name: "Add a person" });
  await people.fill("Anna");
  await expect(sheet.getByRole("option", { name: 'Add "Anna"' })).toBeVisible();
  await people.press("Enter");
  await expect(sheet.getByRole("button", { name: "Anna", exact: true })).toHaveAttribute("aria-pressed", "true");
  await sheet.getByRole("button", { name: /^More/ }).click();
  await sheet.getByRole("radio", { name: "Rate 4 of 5" }).click();
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Watched S1 E2" })).toBeVisible();
  await expect(sheet).toHaveCount(0);
  await expect(page.getByTestId("item-facts")).toContainText("4 of 5");
  await page.getByRole("tab", { name: /^History/ }).click();
  await expect(page.getByRole("tabpanel", { name: /^History/ })).toContainText("Season 1, episode 2");

  // a changed sheet asks before it closes; Discard closes it and focus returns
  await opener.click();
  await expect(sheet).toBeVisible();
  await sheet.getByRole("button", { name: "Anna", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(sheet.getByText("Discard your changes?")).toBeVisible();
  await expect(sheet.getByRole("button", { name: "Keep editing" })).toBeFocused();
  await sheet.getByRole("button", { name: "Discard" }).click();
  await expect(sheet).toHaveCount(0);
  await expect(opener).toBeFocused();
});

test("the play sheet logs players, scores and the winner", async ({ page }) => {
  test.setTimeout(150_000);
  await signUp(page, `play${Date.now()}`, "Play Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["Meeple Harbour"]}`);
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  const sheet = page.getByRole("dialog", { name: "Log a play" });
  await expect(sheet).toBeVisible();
  // you play by default (no earlier play to copy the group from)
  await expect(sheet.getByRole("button", { name: "Play Tester", exact: true })).toHaveAttribute("aria-pressed", "true");
  const people = sheet.getByRole("combobox", { name: "Add a person" });
  await people.fill("Ben");
  await people.press("Enter");
  await sheet.getByLabel("Score of Play Tester").fill("31");
  await sheet.getByLabel("Score of Ben").fill("42,5");
  await expect(sheet.getByLabel("Winner")).toHaveValue("");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  const sessions = await (await page.request.get("/api/v1/sessions")).json();
  const players = sessions[0].participants as { name: string; score: number; winner: boolean }[];
  expect(players.find((p) => p.name === "Ben")).toMatchObject({ score: 42.5, winner: true });
  expect(players.find((p) => p.name === "Play Tester")).toMatchObject({ score: 31, winner: false });
  await expect(bar(page)).toContainText("One play");

  // the next play starts with the same group (design.md 5.4)
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  await expect(sheet.getByRole("button", { name: "Ben", exact: true })).toHaveAttribute("aria-pressed", "true");

  // "More" is remembered, and a changed play sheet asks before it closes (UX1-11)
  const more = sheet.getByRole("button", { name: /^More/ });
  await more.click();
  await expect(more).toHaveAttribute("aria-expanded", "true");
  await sheet.getByRole("button", { name: "Ben", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(sheet.getByText("Discard your changes?")).toBeVisible();
  await sheet.getByRole("button", { name: "Discard" }).click();
  await expect(sheet).toHaveCount(0);
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  await expect(sheet.getByRole("button", { name: /^More/ })).toHaveAttribute("aria-expanded", "true");
});

test("pages: +10 and a typed page log, a bad page says why, Finished completes", async ({ page }) => {
  // UX1-11
  test.setTimeout(150_000);
  await signUp(page, `pages${Date.now()}`, "Page Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["The Glass Ferry"]}`);
  const field = bar(page).getByLabel("Current page");
  await expect(field).toHaveValue("40");
  await bar(page).getByRole("button", { name: "+10" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Page 50" })).toBeVisible();
  await expect(field).toHaveValue("50");
  await field.fill("120");
  await field.press("Enter");
  await expect(page.getByRole("status").filter({ hasText: "Logged: Page 120" })).toBeVisible();
  await field.fill("soon");
  await field.press("Enter");
  await expect(bar(page).getByText("Enter a page number from 1 on.")).toBeVisible();
  await expect(field).toHaveAttribute("aria-invalid", "true");
  await bar(page).getByRole("button", { name: "Finished" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Marked as Completed" })).toBeVisible();
  await expect(page.getByTestId("item-facts")).toContainText("Completed");
});

test("episodes: one mark asks once about the earlier ones, remembered; a season marks at once", async ({ page }) => {
  test.setTimeout(150_000);
  await signUp(page, `eps${Date.now()}`, "Episode Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["Game of Thrones"]}`);
  const tab = page.getByRole("tabpanel", { name: "Episodes" });
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toHaveAttribute("aria-pressed", "true");
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("1/10");

  await tab.getByRole("button", { name: "Mark S1 E5 watched" }).click();
  const ask = tab.getByTestId("episodes-ask");
  await expect(ask).toContainText("Also mark the 3 earlier episodes as watched?");
  await expect(ask.getByRole("button", { name: "Mark all 4" })).toBeFocused();
  await ask.getByLabel("Remember for this show").check();
  await ask.getByRole("button", { name: "Mark all 4" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: 4 episodes" })).toBeVisible();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("5/10");

  // remembered: no question the second time
  await tab.getByRole("button", { name: "Mark S1 E8 watched" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: 3 episodes" })).toBeVisible();
  await expect(tab.getByTestId("episodes-ask")).toHaveCount(0);
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("8/10");

  // each logged episode refetches the page (entity.updated): wait until it settled
  await expect(bar(page).getByRole("button", { name: "Watched S1 E9" })).toBeVisible();
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("10/10");
  await expect(bar(page).getByRole("button", { name: "Watched S2 E1" })).toBeVisible();

  // Undo takes the whole bulk mark back
  const toast = page.getByRole("status").locator("> div").filter({ hasText: "Logged: 2 episodes" });
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("8/10");
});

for (const size of [
  { name: "1280", width: 1280, height: 800 },
  { name: "360", width: 360, height: 640 },
]) {
  test.describe(`axe at ${size.name} px`, () => {
    test.use({ viewport: { width: size.width, height: size.height } });

    test("the item page and its sheets have no violations in light and dark", async ({ page }) => {
      test.setTimeout(180_000);
      await signUp(page, `itemaxe${size.name}${Date.now()}`, "Axe Item");
      const ids = await seed(page);
      for (const theme of ["light", "dark"] as const) {
        await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
        await open(page, `/items/${ids["Game of Thrones"]}`);
        await expect(page.getByRole("tabpanel", { name: "Episodes" })).toBeVisible();
        await expectNoA11yViolations(page, `${size.name} ${theme} show`);
        await page.getByRole("button", { name: "Log with details: Game of Thrones" }).click();
        const sheet = page.getByRole("dialog", { name: "Log with details" });
        await expect(sheet).toBeVisible();
        const more = sheet.getByRole("button", { name: /^More/ });
        if ((await more.getAttribute("aria-expanded")) !== "true") await more.click();
        await expect(more).toHaveAttribute("aria-expanded", "true");
        await expectNoA11yViolations(page, `${size.name} ${theme} watched sheet`);
        await page.keyboard.press("Escape");
        await expect(sheet).toHaveCount(0);
        await open(page, `/items/${ids["Meeple Harbour"]}`);
        await bar(page).getByRole("button", { name: "Log a play" }).click();
        await expect(page.getByRole("dialog", { name: "Log a play" })).toBeVisible();
        await expectNoA11yViolations(page, `${size.name} ${theme} play sheet`);
        await page.keyboard.press("Escape");
      }
    });
  });
}
