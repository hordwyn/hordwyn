import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations, expectReflow, hydrated, signUp, importDone } from "./helpers";

// Lists (WP3-6, design.md 6.5, 5.10; mockup lists.html): the overview with Mine / Shared with
// me / On this instance and cards; the list page with the editable title and description,
// keyboard "Move up/down" (focus stays on the moved row, the order persists), Remove, "Add"
// through the search palette scoped to the list, "Add all to my library" for a list of
// someone else; axe and reflow at 360 px.

const ROWS = [
  { source_id: "1", kind: "movie", title: "Alpha Night", year: 2001, status: "planned" },
  { source_id: "2", kind: "book", title: "Bravo Pages", year: 2002, status: "planned" },
  { source_id: "3", kind: "board_game", title: "Charlie Tiles", year: 2003, status: "planned" },
];

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

async function titles(page: Page) {
  return page.getByTestId("lst-row").getByRole("link").allInnerTexts();
}

test("keyboard reorder, edit, remove, add through the palette; others add it all", async ({ page, browser }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  await signUp(page, `lown${stamp}`, "Lou");
  const items = await seed(page);
  const made = await page.request.post("/api/v1/lists", { data: { name: "Order test" } });
  expect(made.status()).toBe(201);
  const list = (await made.json()).id as string;
  for (const t of ["Alpha Night", "Bravo Pages", "Charlie Tiles"]) {
    const r = await page.request.post(`/api/v1/lists/${list}/items`, { data: { item_id: items[t] } });
    expect(r.ok()).toBeTruthy();
  }

  await page.goto("/lists");
  await hydrated(page);
  const lcard = page.getByTestId("ls-card").filter({ hasText: "Order test" });
  await expect(lcard).toContainText("3 items - Movie, Book, Board game");
  await expect(lcard).toContainText("Private");
  await expectNoA11yViolations(page, "lists");
  await lcard.getByRole("link", { name: "Order test" }).click();
  await expect(page).toHaveURL(new RegExp(`/lists/${list}$`));
  await hydrated(page);
  expect(await titles(page)).toEqual(["Alpha Night (2001)", "Bravo Pages (2002)", "Charlie Tiles (2003)"]);
  await expect(page.getByRole("button", { name: "Move up: Alpha Night" })).toBeDisabled();

  // keyboard: the moved row keeps the focus, the order is announced and persists
  const up = page.getByRole("button", { name: "Move up: Charlie Tiles" });
  await up.focus();
  await page.keyboard.press("Enter");
  await expect.poll(() => titles(page)).toEqual(["Alpha Night (2001)", "Charlie Tiles (2003)", "Bravo Pages (2002)"]);
  await expect(page.getByTestId("lst-status")).toHaveText("Charlie Tiles is now number 2 of 3.");
  await expect(page.getByRole("button", { name: "Move up: Charlie Tiles" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect.poll(() => titles(page)).toEqual(["Charlie Tiles (2003)", "Alpha Night (2001)", "Bravo Pages (2002)"]);
  // at the top "Move up" is disabled: the focus goes to the same row's "Move down" (UX3-6)
  await expect(page.getByRole("button", { name: "Move down: Charlie Tiles" })).toBeFocused();
  // and to the other end: Alpha Night moves down to the last place, its "Move up" takes the focus
  await page.getByRole("button", { name: "Move down: Alpha Night" }).focus();
  await page.keyboard.press("Enter");
  await expect.poll(() => titles(page)).toEqual(["Charlie Tiles (2003)", "Bravo Pages (2002)", "Alpha Night (2001)"]);
  await expect(page.getByRole("button", { name: "Move up: Alpha Night" })).toBeFocused();
  await page.getByRole("button", { name: "Move up: Alpha Night" }).focus();
  await page.keyboard.press("Enter");
  await expect.poll(() => titles(page)).toEqual(["Charlie Tiles (2003)", "Alpha Night (2001)", "Bravo Pages (2002)"]);
  await page.reload();
  await hydrated(page);
  expect(await titles(page)).toEqual(["Charlie Tiles (2003)", "Alpha Night (2001)", "Bravo Pages (2002)"]);

  // title and description, then remove one
  await page.getByRole("button", { name: "Edit title and description" }).click();
  await page.getByLabel("Title").fill("Ordered picks");
  await page.getByLabel("Description").fill("Best first.");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Ordered picks" })).toBeVisible();
  await expect(page.getByText("Best first.")).toBeVisible();
  await page.getByRole("button", { name: "Remove Bravo Pages from the list" }).click();
  await expect(page.getByText("Removed Bravo Pages.")).toBeVisible();
  await expect.poll(() => titles(page)).toEqual(["Charlie Tiles (2003)", "Alpha Night (2001)"]);

  // "Add" opens the palette scoped to this list
  await page.getByRole("button", { name: "Add to Ordered picks" }).click();
  const palette = page.getByRole("dialog", { name: "Search" });
  await expect(palette.getByTestId("palette-target")).toHaveText('Choosing a result adds it to "Ordered picks".');
  await palette.getByRole("combobox").fill("fight club");
  const option = palette.getByRole("option", { name: /Fight Club \(1999\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page.getByText("Added Fight Club (1999) to Ordered picks.")).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`/lists/${list}$`));
  await expect.poll(() => titles(page)).toEqual(["Charlie Tiles (2003)", "Alpha Night (2001)", "Fight Club (1999)"]);

  await page.setViewportSize({ width: 360, height: 800 });
  await expectReflow(page, "list 360");
  await expectNoA11yViolations(page, "list 360");

  // everyone here: another member finds it under "On this instance" and adds it all
  const r = await page.request.put(`/api/v1/lists/${list}/visibility`, { data: { visibility: "instance" } });
  expect(r.ok()).toBeTruthy();
  const other = await browser.newContext();
  const guest = await other.newPage();
  await signUp(guest, `lgst${stamp}`, "Gus");
  await guest.goto("/lists");
  await hydrated(guest);
  await guest.getByRole("tab", { name: /^On this instance/ }).click();
  const shared = guest.getByRole("tabpanel", { name: /^On this instance/ }).getByTestId("ls-card").filter({ hasText: "Ordered picks" });
  await expect(shared).toContainText("by Lou");
  await expect(shared).toContainText("Everyone here");
  await guest.setViewportSize({ width: 360, height: 800 });
  await expectReflow(guest, "lists 360");
  await expectNoA11yViolations(guest, "lists 360");
  await shared.getByRole("link", { name: "Ordered picks" }).click();
  await hydrated(guest);
  await expect(guest.getByRole("button", { name: "Move up: Alpha Night" })).toHaveCount(0);
  await guest.getByRole("button", { name: "Add all to my library" }).click();
  await expect(guest.getByText("Added 3 to your library; 0 were there already.")).toBeVisible();
  await other.close();
});

test("drag handles reorder with the pointer; the order persists and the keyboard buttons stay", async ({ page }) => {
  // WP4-3 (DEC-353): the handle is aria-hidden, Move up/down remain (WCAG 2.5.7)
  test.setTimeout(120_000);
  await signUp(page, `ldrag${Date.now()}`, "Drag Tester");
  const items = await seed(page);
  const made = await page.request.post("/api/v1/lists", { data: { name: "Drag test" } });
  const list = (await made.json()).id as string;
  for (const t of ["Alpha Night", "Bravo Pages", "Charlie Tiles"]) {
    expect((await page.request.post(`/api/v1/lists/${list}/items`, { data: { item_id: items[t] } })).ok()).toBeTruthy();
  }
  await page.goto(`/lists/${list}`);
  await hydrated(page);
  const handles = page.getByTestId("lst-handle");
  await expect(handles).toHaveCount(3);
  await expect(handles.first()).toHaveAttribute("aria-hidden", "true");
  // Charlie Tiles to the top
  await handles.nth(2).dragTo(page.getByTestId("lst-row").first(), { targetPosition: { x: 10, y: 2 } });
  await expect.poll(() => titles(page)).toEqual(["Charlie Tiles (2003)", "Alpha Night (2001)", "Bravo Pages (2002)"]);
  await expect(page.getByTestId("lst-status")).toHaveText("Charlie Tiles is now number 1 of 3.");
  // Charlie Tiles back down to the end
  await page.getByTestId("lst-handle").first().dragTo(page.getByTestId("lst-row").last(), { targetPosition: { x: 10, y: 40 } });
  await expect.poll(() => titles(page)).toEqual(["Alpha Night (2001)", "Bravo Pages (2002)", "Charlie Tiles (2003)"]);
  await page.reload();
  await hydrated(page);
  expect(await titles(page)).toEqual(["Alpha Night (2001)", "Bravo Pages (2002)", "Charlie Tiles (2003)"]);
  // the keyboard still works
  await page.getByRole("button", { name: "Move up: Bravo Pages" }).click();
  await expect.poll(() => titles(page)).toEqual(["Bravo Pages (2002)", "Alpha Night (2001)", "Charlie Tiles (2003)"]);
  await expectNoA11yViolations(page, "list with handles");
});
