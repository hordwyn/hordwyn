import { test, expect, type Page } from "./fixtures";
import { expectNoA11yViolations, expectReflow, hydrated, signUp, importDone } from "./helpers";

// UX phase 1 item page (design.md 6.3, 5.4, 5.5, 3.4; addendum-1 1.3 Item row): the next step
// with Undo, the "With..." sheet with a person and a rating, the play sheet with players and
// the winner, the episode list's bulk marking with the ask-once question, "Mark show" and the
// backfill over a date range, the private note,
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
  { source_id: "5", kind: "video_game", title: "Pixel Harbour", year: 2020, status: "planned" },
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
  await importDone(page, id);
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
      // the catalog refresh waits behind other specs' jobs on the same lane (UX3-22)
      { timeout: 90_000 },
    )
    .toBeGreaterThan(0);
  return ids;
}

async function open(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

const bar = (page: Page) => page.getByTestId("action-bar");

/** A second show (the mock's synthetic "Lantern Coast") with S1 E1 watched; waits for its seasons. */
async function seedOtherShow(page: Page): Promise<string> {
  const row = {
    source_id: "90",
    kind: "show",
    title: "Lantern Coast",
    year: 2024,
    status: "in_progress",
    external_ids: { tmdb: "990001" },
    history: [{ date: "2026-02-04", progress: { type: "episode", season: 1, episode: 1 } }],
  };
  const file = [JSON.stringify({ format: "library-export", version: 1 }), JSON.stringify(row)].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  const id: string = lib.items.find((i: { item: { title: string } }) => i.item.title === "Lantern Coast").item.id;
  await expect
    .poll(async () => ((await (await page.request.get(`/api/v1/items/${id}/page`)).json()).item.attributes.seasons ?? []).length, { timeout: 90_000 })
    .toBeGreaterThan(0);
  return id;
}

test("the next step logs with Undo; the note and the tags are edited in place", async ({ page }) => {
  test.setTimeout(150_000);
  await signUp(page, `item${Date.now()}`, "Item Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["Harbour Lights"]}`);
  await expect(page.getByRole("heading", { level: 1, name: "Harbour Lights" })).toBeVisible();

  await bar(page).getByRole("button", { name: "Mark as watched", exact: true }).click();
  const toast = page.getByRole("status").filter({ hasText: "Logged: Watched" });
  await expect(toast).toBeVisible();
  await expect(bar(page).getByRole("button", { name: "Log a rewatch" })).toBeVisible();
  await expect(page.getByLabel("Status")).toHaveValue("completed");
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Undone")).toBeVisible();
  await expect(bar(page).getByRole("button", { name: "Mark as watched", exact: true })).toBeVisible();
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
  const opener = page.getByRole("button", { name: "Watched with...: Game of Thrones" });
  await opener.click();
  const sheet = page.getByRole("dialog", { name: "Watched S1 E2", exact: true });
  await expect(sheet).toBeVisible();
  await expect(sheet).toBeFocused();
  // OT2A-12: the hint belongs to the group, so it is read with it
  await expect(sheet.getByRole("group", { name: "Watched with" })).toHaveAccessibleDescription("Leave empty if nobody was with you.");
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

  // a changed sheet asks before it closes; Discard closes it and focus returns. The dialog is
  // named after the step it logs (OT-6), which is the next episode now
  await opener.click();
  const next = page.getByRole("dialog", { name: "Watched S1 E3", exact: true });
  await expect(next).toBeVisible();
  await expect(next.getByRole("group", { name: "Watched with" })).toBeVisible();
  await next.getByRole("button", { name: "Anna", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(next.getByText("Discard your changes?")).toBeVisible();
  await expect(next.getByRole("button", { name: "Keep editing" })).toBeFocused();
  await next.getByRole("button", { name: "Discard" }).click();
  await expect(next).toHaveCount(0);
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
  // ux-owner-test 2.3: the scores are one table with a "Won" column; the radio follows the
  // highest score until one is chosen by hand, and the choice can be taken back
  const scores = sheet.getByRole("table", { name: "Scores" });
  await expect(scores.getByRole("columnheader")).toHaveText(["Player", "Score", "Won"]);
  await expect(scores.getByRole("rowheader")).toHaveText(["Play Tester", "Ben"]);
  const benWon = scores.getByRole("radio", { name: "Ben won" });
  const iWon = scores.getByRole("radio", { name: "Play Tester won" });
  await expect(benWon).toBeChecked();
  await expect(iWon).not.toBeChecked();
  await expect(sheet.getByRole("button", { name: "Use the highest score" })).toHaveCount(0);
  await iWon.check();
  await expect(benWon).not.toBeChecked();
  await sheet.getByRole("button", { name: "Use the highest score" }).click();
  await expect(benWon).toBeChecked();
  await expect(sheet.getByRole("button", { name: "Use the highest score" })).toHaveCount(0);
  await expectNoA11yViolations(page, "play dialog with scores");
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

test("the History tab shows its count apart from the label", async ({ page }) => {
  // OT-8: a whitespace-only text is no flex item, so the count sat on the label
  test.setTimeout(150_000);
  await signUp(page, `tabgap${Date.now()}`, "Tab Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["The Glass Ferry"]}`);
  const tab = page.getByRole("tab", { name: /^History/ });
  // the space stays in the name (and in copied text)
  await expect(tab).toHaveAccessibleName("History 1");
  const labelRight = await tab.evaluate((el) => {
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    let node = walker.nextNode();
    while (node && !(node.textContent ?? "").trim()) node = walker.nextNode();
    const range = document.createRange();
    range.selectNode(node!);
    return range.getBoundingClientRect().right;
  });
  const count = await tab.locator("span").boundingBox();
  expect(count!.x - labelRight).toBeGreaterThanOrEqual(3);
});

test("the play sheet shows the duration in its first row, without opening More", async ({ page }) => {
  // OT-16: the field was behind the closed "More"
  test.setTimeout(150_000);
  await signUp(page, `playdur${Date.now()}`, "Duration Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["Meeple Harbour"]}`);
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  const sheet = page.getByRole("dialog", { name: "Log a play" });
  const more = sheet.getByRole("button", { name: /^More/ });
  await expect(more).toHaveAttribute("aria-expanded", "false");
  await expect(more).not.toContainText(/duration/i);
  const duration = sheet.getByLabel("Duration", { exact: true });
  await expect(sheet.getByText("h:mm or minutes")).toBeVisible();
  await expect(duration).toBeVisible();
  // first row: beside the date, above the players
  const box = (await duration.boundingBox())!;
  const when = (await sheet.getByRole("radio", { name: "Today" }).boundingBox())!;
  const players = (await sheet.getByRole("group", { name: "Players" }).boundingBox())!;
  expect(box.y).toBeLessThan(players.y);
  expect(box.x).toBeGreaterThan(when.x);
  // OD-01: a play of one person opens with five inputs: the date, the duration and the score
  // (text fields), the people finder (a combobox) and the winner (the "Won" column of the
  // score table, ux-owner-test 2.3: the player or "No winner")
  await expect(sheet.getByRole("textbox")).toHaveCount(2);
  await expect(sheet.getByRole("combobox")).toHaveCount(1);
  await expect(sheet.getByRole("table", { name: "Scores" }).getByRole("radio")).toHaveCount(2);
  await duration.fill("ninety");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(sheet.getByRole("alert").filter({ hasText: "Enter the duration as hours:minutes (1:30) or as minutes (90)." })).toBeVisible();
  // OT-16: hours:minutes works as well as plain minutes
  await duration.fill("1:30");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  // stored, and shown where the play is read (OT-11)
  const feed = await (await page.request.get("/api/v1/activity")).json();
  expect(feed.rows[0].play.duration_minutes).toBe(90);
  await page.getByRole("tab", { name: /^History/ }).click();
  await expect(page.getByRole("tabpanel", { name: /^History/ }).getByTestId("activity-row").first()).toContainText("90 min");
  // the next play starts with the duration of the last one
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  await expect(sheet.getByLabel("Duration", { exact: true })).toHaveValue("1:30");
  // a prefilled sheet is not a changed one: Escape closes it without the question
  await page.keyboard.press("Escape");
  await expect(sheet).toHaveCount(0);
});

test("a video game session: played with, the scores wait behind More", async ({ page }) => {
  // OT-6, OT-16: three open inputs (when, duration, played with); scores are a board game matter
  test.setTimeout(150_000);
  await signUp(page, `vgplay${Date.now()}`, "Pad Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["Pixel Harbour"]}`);
  await bar(page).getByRole("button", { name: "Log a session" }).click();
  const sheet = page.getByRole("dialog", { name: "Log a session" });
  await expect(sheet.getByRole("group", { name: "Played with" })).toBeVisible();
  await expect(sheet.getByRole("button", { name: "Pad Tester", exact: true })).toHaveAttribute("aria-pressed", "true");
  const more = sheet.getByRole("button", { name: /^More/ });
  // a fresh account has no stored disclosure state: the dialog itself opens with More closed
  await expect(more).toHaveAttribute("aria-expanded", "false");
  await expect(more).toContainText(/scores/i);
  await expect(sheet.getByLabel("Score of Pad Tester")).toBeHidden();
  await expect(sheet.getByRole("textbox")).toHaveCount(1);
  await expect(sheet.getByRole("combobox")).toHaveCount(1);
  await more.click();
  await expect(sheet.getByLabel("Score of Pad Tester")).toBeVisible();
  await sheet.getByLabel("Duration", { exact: true }).fill("6:00");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  const feed = await (await page.request.get("/api/v1/activity")).json();
  expect(feed.rows[0].play.duration_minutes).toBe(360);
  // OT-7: plays are counted in the header too
  await expect(page.getByTestId("item-fact-line")).toHaveText(/^Played once, on /);
});

test("an item outside the library: two actions, the add toast with Undo, the status pill and the count", async ({ page }) => {
  // OT-4, OT-5, OT-6, OT-7
  test.setTimeout(150_000);
  await signUp(page, `twobtn${Date.now()}`, "Two Tester");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  await palette.getByRole("combobox").fill("fight club");
  const option = palette.getByRole("option", { name: /^Fight Club \(1999\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: "Fight Club" })).toBeVisible();

  // not in the library: two verbs, no state
  const mark = bar(page).getByRole("button", { name: "Mark as watched", exact: true });
  const want = bar(page).getByRole("button", { name: "Want to watch", exact: true });
  await expect(mark).toBeVisible();
  await expect(want).toBeVisible();
  await expect(bar(page).getByRole("button", { name: "Add to library" })).toHaveCount(0);
  const pill = page.getByTestId("ip-status");
  const fact = page.getByTestId("item-fact-line");
  await expect(pill).toHaveCount(0);
  await expectNoA11yViolations(page, "item outside the library");

  // "Want to watch" adds it as Planned, says so, and can be taken back
  await want.click();
  const added = page.locator("[role=status][aria-live] > div").filter({ hasText: "Added to your library as Planned" });
  await expect(added).toBeVisible();
  await expect(added).toContainText("Fight Club");
  await expect(pill).toHaveText("Planned");
  await expect(fact).toHaveText(/^Added on /);
  await expect(want).toHaveCount(0);
  await added.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Undone").first()).toBeVisible();
  await expect(pill).toHaveCount(0);
  await expect(want).toBeVisible();

  // "Mark as watched" adds it too, and says so; its Undo takes the watch and the add back
  // (OT2A-3: the item stayed in the library as Planned)
  // review OT2B-4: once the log went through, both buttons wait for the refetched page (a
  // click in between said "Added to your library as Planned" for a Completed entry)
  let show = () => {};
  const pageHeld = new Promise<void>((r) => (show = r));
  await page.route("**/_fn/item_page*", async (route) => {
    await pageHeld;
    await route.continue();
  });
  await mark.click();
  const logged = page.locator("[role=status][aria-live] > div").filter({ hasText: "Logged: Watched" });
  await expect(logged).toContainText("Fight Club, added to your library");
  await expect(mark).not.toHaveAttribute("aria-busy", "true");
  await expect(mark).toBeDisabled();
  await expect(want).toBeDisabled();
  // review SEC1-1: the held route is answered before its handler is removed (removing it
  // first lets Playwright continue the route itself, and the handler's own
  // `route.continue()` then fails with "Route is already handled!")
  const shown = page.waitForResponse((res) => res.url().includes("/_fn/item_page"));
  show();
  await shown;
  await page.unroute("**/_fn/item_page*");
  await expect(pill).toHaveText("Completed");
  await logged.getByRole("button", { name: "Undo" }).click();
  await expect(want).toBeVisible();
  await expect(pill).toHaveCount(0);
  const id = page.url().split("/items/")[1].split(/[?#]/)[0];
  const after = await (await page.request.get(`/api/v1/items/${id}/page`)).json();
  expect(after.entry ?? null).toBeNull();
  expect(after.progress).toHaveLength(0);

  // "Mark as watched" logs it at once: Completed, counted in words
  await mark.click();
  await expect(logged.last()).toBeVisible();
  await expect(pill).toHaveText("Completed");
  await expect(fact).toHaveText(/^Watched once, on /);
  await bar(page).getByRole("button", { name: "Log a rewatch" }).click();
  await expect(fact).toHaveText(/^Watched 2 times, last on /);
  await expect(page.getByTestId("item-facts")).not.toContainText("Rewatches");

  // "Watched with...": the dialog is named after the step and says which watch this will be
  await page.getByRole("button", { name: "Watched with...: Fight Club" }).click();
  const sheet = page.getByRole("dialog", { name: "Watched again" });
  await expect(sheet).toBeVisible();
  await expect(sheet).toContainText("Watched 2 times before. This will be watch number 3.");
  await expect(sheet.getByRole("group", { name: "Watched with" })).toBeVisible();
  await expectNoA11yViolations(page, "watched with sheet");
});

test("a bulk mark on watched episodes asks to log them again; a mixed range says what it skipped", async ({ page }) => {
  // OT-15 (DEC-538): the marks did nothing on watched episodes, then only said so
  test.setTimeout(240_000);
  await signUp(page, `epagain${Date.now()}`, "Again Tester");
  const ids = await seed(page);
  const show = ids["Game of Thrones"];
  // a second show with its first episode watched too, for the end of the test
  const other = await seedOtherShow(page);
  await open(page, `/items/${show}`);
  const tab = page.getByRole("tabpanel", { name: "Episodes" });
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("1/10");
  const ask = tab.getByTestId("episodes-again");
  await expect(ask).toHaveCount(0);
  const events = async () => ((await (await page.request.get(`/api/v1/items/${show}/progress`)).json()) as unknown[]).length;
  const before = await events();
  const upToFirst = async () => {
    await tab.getByRole("button", { name: "More for S1 E1", exact: true }).click();
    await page.getByRole("menuitem", { name: "Mark up to here" }).click();
  };
  await upToFirst();
  await expect(ask).toContainText("This episode is already watched. Log it again as a rewatch?");
  // the question takes the focus, so a keyboard and a screen reader land on it
  await expect(ask.getByRole("button", { name: "Log it again" })).toBeFocused();
  expect(await events()).toBe(before);
  await ask.getByRole("button", { name: "Cancel" }).click();
  await expect(ask).toHaveCount(0);
  // OT2A-4: the focus goes back to what asked, here the row's menu button
  await expect(tab.getByRole("button", { name: "More for S1 E1", exact: true })).toBeFocused();
  expect(await events()).toBe(before);

  // a mixed range logs the open ones and says how many it left alone
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  const mixed = page.locator("[role=status][aria-live] > div").filter({ hasText: "Logged: 9 episodes" });
  await expect(mixed).toContainText("One already watched, not logged again");
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("10/10");
  await expect(bar(page).getByRole("button", { name: "Mark S2 E1 watched" })).toBeVisible();
  await tab.getByRole("button", { name: /^Season 1/ }).click();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toHaveAttribute("aria-pressed", "true");
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  const seasonAgain = "These 10 episodes are already watched. Log them again as a rewatch?";
  await expect(ask).toContainText(seasonAgain);
  const markSeason = tab.getByRole("button", { name: "Mark season watched" });
  await expect(ask.getByRole("button", { name: "Log 10 again" })).toBeFocused();
  await ask.getByRole("button", { name: "Cancel" }).click();
  await expect(markSeason).toBeFocused();
  // OT2A-4: the question takes the focus when it is asked, not when the page refetches under
  // it (a rating does that): Enter would have logged ten rewatches nobody meant
  await markSeason.click();
  await expect(ask.getByRole("button", { name: "Log 10 again" })).toBeFocused();
  await page.locator("#ip-rate").getByRole("radio", { name: "Rate 4 of 5" }).click();
  await expect(page.getByTestId("item-facts")).toContainText("4 of 5");
  await expect(ask).toContainText(seasonAgain);
  await page.waitForTimeout(300);
  await expect(ask.getByRole("button", { name: "Log 10 again" })).not.toBeFocused();

  // OT1A-2: the question does not outlive what it says. An episode of the season is undone
  // on another device (this page does not hear of it); the next step logged here refetches
  // the page, and the season is no longer watched
  const logged = (await (await page.request.get(`/api/v1/items/${show}/progress`)).json()) as { id: string; progress: { season?: number; episode?: number } }[];
  const last = logged.filter((e) => e.progress.season === 1 && e.progress.episode === 10).pop()!;
  expect((await page.request.post(`/api/v1/progress/${last.id}/undo`)).ok()).toBe(true);
  await bar(page).getByRole("button", { name: "Mark S2 E1 watched" }).click();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("9/10");
  await expect(ask).toHaveCount(0);

  // the season complete again, then logged again as a whole: ten more views, nothing else moves
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("10/10");
  const full = await events();
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  await expect(ask).toContainText(seasonAgain);
  const history = page.getByRole("tab", { name: /^History/ });
  const historyBefore = await history.innerText();
  await ask.getByRole("button", { name: "Log 10 again" }).click();
  await expect(page.locator("[role=status][aria-live] > div").filter({ hasText: "Logged: 10 episodes" }).first()).toBeVisible();
  // OT2A-5: a row menu opened right after the toast stays open through the refetch that
  // follows the bulk log (it was closed under the pointer)
  await tab.getByRole("button", { name: "More for S1 E1", exact: true }).click();
  const upTo = page.getByRole("menuitem", { name: "Mark up to here" });
  await expect(upTo).toBeVisible();
  await expect.poll(events).toBe(full + 10);
  await expect(history).not.toHaveText(historyBefore);
  await expect(ask).toHaveCount(0);
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("10/10");
  await expect(page.getByLabel("Status")).toHaveValue("in_progress");
  await expect(upTo).toBeVisible();
  await upTo.click();
  await expect(ask).toContainText("This episode is already watched. Log it again as a rewatch?");
  // the answer logs it; once the list is back the focus is on what asked
  await ask.getByRole("button", { name: "Log it again" }).click();
  await expect(page.locator("[role=status][aria-live] > div").filter({ hasText: "Logged: Watched S1 E1" }).first()).toBeVisible();
  await expect.poll(events).toBe(full + 11);
  await expect(tab.getByRole("button", { name: "More for S1 E1", exact: true })).toBeFocused();

  // and it does not follow to another show: the question is up again, then the palette opens
  // another show whose first episode is watched as well (the page component stays, only its
  // id changes), so only a cleared question is gone there
  await upToFirst();
  await expect(ask).toContainText("This episode is already watched. Log it again as a rewatch?");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  await palette.getByRole("combobox").fill("Lantern Coast");
  await palette.getByRole("option", { name: /^Lantern Coast/ }).first().click();
  await expect(page).toHaveURL(new RegExp(`/items/${other}`));
  await expect(page.getByRole("heading", { level: 1, name: "Lantern Coast" })).toBeVisible();
  await page.getByRole("tab", { name: "Episodes" }).click();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("1/3");
  await expect(tab.getByTestId("episodes-again")).toHaveCount(0);
});

test("a rewatch of a completed show is a run: start it, mark two episodes, stop; a whole one counts", async ({ page }) => {
  // ot2e (OT-9 model A, DEC-537, DEC-587): In progress while it runs, Completed when it is
  // stopped or whole; the header, the next step and the episode list count within the run
  test.setTimeout(240_000);
  await signUp(page, `rerun${Date.now()}`, "Rerun Tester");
  const show = await seedOtherShow(page);
  // the rest of the show (S1 E2, E3, S2 E1, E2), a year ago, then Completed by hand
  for (const [season, episode] of [[1, 2], [1, 3], [2, 1], [2, 2]]) {
    const r = await page.request.post(`/api/v1/items/${show}/progress`, {
      data: { progress: { type: "episode", season, episode }, occurred_at: "2025-03-09T12:00:00Z" },
    });
    expect(r.ok()).toBe(true);
  }
  expect((await page.request.put(`/api/v1/library/${show}/status`, { data: { status: "completed" } })).ok()).toBe(true);
  const events = async () => ((await (await page.request.get(`/api/v1/items/${show}/progress`)).json()) as unknown[]).length;
  expect(await events()).toBe(5);
  await open(page, `/items/${show}`);
  const pill = page.getByTestId("ip-status");
  const fact = page.getByTestId("item-fact-line");
  const tab = page.getByRole("tabpanel", { name: "Episodes" });
  const season1 = tab.getByRole("button", { name: /^Season 1/ });
  const toast = (text: string) => page.locator("[role=status][aria-live] > div").filter({ hasText: text }).first();
  await expect(pill).toHaveText("Completed");
  // OT2A-7: the day of the pass (S1 E1 was the last of it, 4 Feb 2026), not of a later view
  await expect(fact).toHaveText("Watched once, finished on Feb 4, 2026");
  await expect(season1).toContainText("3/3");

  // start: In progress, the run at zero, the next step is the first episode again
  const more = bar(page).getByRole("button", { name: "More actions for Lantern Coast" });
  await more.click();
  await expect(page.getByRole("menuitem", { name: "Stop the rewatch" })).toHaveCount(0);
  await page.getByRole("menuitem", { name: "Start a rewatch" }).click();
  await expect(toast("Rewatch started")).toBeVisible();
  await expect(pill).toHaveText("In progress");
  await expect(fact).toHaveText("Rewatch: 0 of 5 episodes. Watched once before.");
  await expect(season1).toContainText("0/3");
  await expect(page.getByLabel("Status")).toHaveValue("in_progress");
  expect(await events()).toBe(6);

  // review OT2E-1: a view dated before the start would be logged and not counted in the
  // run. The dialog refuses the day and says when the run started; nothing is logged
  await page.getByRole("button", { name: "Watched with...: Lantern Coast" }).click();
  const sheet = page.getByRole("dialog", { name: "Watched S1 E1", exact: true });
  await sheet.getByRole("radio", { name: "Yesterday" }).click();
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(sheet.getByRole("alert").filter({ hasText: "You started again on" })).toContainText(
    "Pick that day or a later one.",
  );
  expect(await events()).toBe(6);
  // the server refuses it too
  const early = await page.request.post(`/api/v1/items/${show}/progress`, {
    data: { progress: { type: "episode", season: 1, episode: 1 }, occurred_at: "2025-03-09T12:00:00Z" },
  });
  expect(early.status()).toBe(422);
  await sheet.getByRole("radio", { name: "Today" }).click();
  await sheet.getByRole("button", { name: "Cancel" }).click();
  await expect(sheet).toHaveCount(0);
  await expect(fact).toHaveText("Rewatch: 0 of 5 episodes. Watched once before.");

  // two episodes: the button, then the row of the list
  await bar(page).getByRole("button", { name: "Mark S1 E1 watched" }).click();
  await expect(fact).toHaveText("Rewatch: 1 of 5 episodes. Watched once before.");
  await expect(bar(page).getByRole("button", { name: "Mark S1 E2 watched" })).toBeVisible();
  await tab.getByRole("button", { name: "Mark S1 E2 watched" }).click();
  await expect(fact).toHaveText("Rewatch: 2 of 5 episodes. Watched once before.");
  await expect(season1).toContainText("2/3");
  await expect(bar(page)).toContainText("2 of 5 episodes");
  // a run is logged now: no backfill dates while it runs
  await expect(tab.getByRole("radio", { name: "Spread over a date range" })).toHaveCount(0);
  for (const theme of ["light", "dark"] as const) {
    await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
    await open(page, `/items/${show}`);
    await expect(fact).toHaveText("Rewatch: 2 of 5 episodes. Watched once before.");
    await expectNoA11yViolations(page, `rewatch running ${theme}`);
  }
  // the History tab has the start as a row
  await page.getByRole("tab", { name: /^History/ }).click();
  await expect(page.getByRole("tabpanel", { name: /^History/ })).toContainText("Started again from the beginning");
  await page.getByRole("tab", { name: "Episodes" }).click();

  // stop: Completed again, the views stay, the count is still once. The stop is a marker
  // in the history (DEC-590), with its own toast and Undo
  await more.click();
  await expect(page.getByRole("menuitem", { name: "Start a rewatch" })).toHaveCount(0);
  await expect(page.getByRole("menuitem", { name: "Mark as Completed" })).toHaveCount(0);
  await page.getByRole("menuitem", { name: "Stop the rewatch" }).click();
  await expect(toast("Rewatch stopped")).toBeVisible();
  await expect(pill).toHaveText("Completed");
  await expect(fact).toHaveText("Watched once; rewatch stopped after 2 of 5 episodes");
  await expect(season1).toContainText("3/3");
  expect(await events()).toBe(9);
  // Undo brings the run back, stopping again ends it
  await toast("Rewatch stopped").getByRole("button", { name: "Undo" }).click();
  await expect(fact).toHaveText("Rewatch: 2 of 5 episodes. Watched once before.");
  await expect(pill).toHaveText("In progress");
  await more.click();
  await page.getByRole("menuitem", { name: "Stop the rewatch" }).click();
  await expect(fact).toHaveText("Watched once; rewatch stopped after 2 of 5 episodes");
  expect(await events()).toBe(11);
  // review OT2E-2: a stopped run does not come back when the status is set to In progress
  // by hand later (a new season), and "Again" on an episode is not the run's
  await page.getByLabel("Status").selectOption("in_progress");
  await expect(pill).toHaveText("In progress");
  await expect(fact).toHaveText("Up to date: 5 of 5 episodes");
  await expect(season1).toContainText("3/3");
  await page.getByLabel("Status").selectOption("completed");
  await expect(pill).toHaveText("Completed");
  await expect(fact).toHaveText("Watched once; rewatch stopped after 2 of 5 episodes");

  // DEC-538: the bulk question on a Completed show starts a run with its answer, dated now
  // (OT2A-10). Review OT2E-11: the date choice has no say there, so it is hidden while the
  // question stands and the question says so; a whole run gives Completed back, counts once
  await tab.getByRole("radio", { name: "Spread over a date range" }).click();
  await tab.locator("#ep-from").fill("2024-01-01");
  await tab.locator("#ep-to").fill("2024-01-31");
  await tab.getByRole("button", { name: "Mark show watched" }).click();
  const ask = tab.getByTestId("episodes-again");
  await expect(ask).toContainText("These 5 episodes are already watched. Log them again as a rewatch?");
  await expect(ask).toContainText("They are logged with today's date.");
  await expect(tab.getByRole("radio", { name: "Spread over a date range" })).toHaveCount(0);
  await expect(tab.locator("#ep-from")).toHaveCount(0);
  await ask.getByRole("button", { name: "Log 5 again" }).click();
  await expect(toast("Logged: 5 episodes")).toContainText("Rewatch finished: Lantern Coast");
  await expect.poll(events).toBe(17);
  await expect(pill).toHaveText("Completed");
  await expect(fact).toContainText("Watched 2 times, last on ");
  await expect(fact).not.toContainText("2024");
  // Undo takes the episodes and the start back: once, Completed, the stopped run again
  await toast("Logged: 5 episodes").getByRole("button", { name: "Undo" }).click();
  // the effective history (the stored one keeps the six events and their undos)
  const effective = async () => ((await (await page.request.get(`/api/v1/items/${show}/page`)).json()).progress as unknown[]).length;
  await expect.poll(effective).toBe(9);
  expect(await events()).toBe(23);
  await expect(pill).toHaveText("Completed");
  await expect(fact).toHaveText("Watched once; rewatch stopped after 2 of 5 episodes");
});

test("offline, a mixed bulk mark still says what it skipped", async ({ page, context }) => {
  // OT2A-11: the queued toast named the title only
  test.setTimeout(150_000);
  await signUp(page, `epoff${Date.now()}`, "Offline Tester");
  const ids = await seed(page);
  const show = ids["Game of Thrones"];
  await open(page, `/items/${show}`);
  const tab = page.getByRole("tabpanel", { name: "Episodes" });
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("1/10");
  await context.setOffline(true);
  await expect(page.getByText("You are offline.")).toBeVisible();
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  const toast = page.locator("[role=status][aria-live] > div").filter({ hasText: "Saved offline, will sync" });
  await expect(toast).toContainText("One already watched, not logged again");
  await context.setOffline(false);
  await expect
    .poll(async () => ((await (await page.request.get(`/api/v1/items/${show}/progress`)).json()) as unknown[]).length)
    .toBe(10);
});

test("a play shows its people, scores, winner and duration on the item History tab and on History", async ({ page }) => {
  // OT-11 read side: the play was stored, and every row said only "Session logged"
  test.setTimeout(150_000);
  await signUp(page, `playrow${Date.now()}`, "Row Tester");
  const ids = await seed(page);
  const game = ids["Meeple Harbour"];
  const me = (await (await page.request.get("/api/v1/auth/me")).json()).self_companion as string;
  const made = await page.request.post("/api/v1/companions", { data: { name: "Ben" } });
  const ben = (await made.json()).id as string;
  const logged = await page.request.post(`/api/v1/items/${game}/plays`, {
    data: {
      players: [
        { companion_id: me, score: 31, winner: false },
        { companion_id: ben, score: 42.5, winner: true },
      ],
      duration_minutes: 45,
    },
  });
  expect(logged.ok(), `play: ${logged.status()}`).toBeTruthy();
  const feed = await (await page.request.get("/api/v1/activity")).json();
  const row = feed.rows.find((r: { item_id: string }) => r.item_id === game);
  expect(row.play).toMatchObject({ duration_minutes: 45, coop: null });
  expect(row.participants.find((p: { name: string }) => p.name === "Ben")).toMatchObject({ score: 42.5, winner: true });
  expect(row.participants.find((p: { is_me: boolean }) => p.is_me)).toMatchObject({ score: 31, winner: false });

  await open(page, `/items/${game}`);
  await page.getByRole("tab", { name: /^History/ }).click();
  const panel = page.getByRole("tabpanel", { name: /^History/ });
  const mine = panel.getByTestId("activity-row").first();
  await expect(mine).toContainText("with Ben");
  await expect(mine).toContainText("Ben won");
  await expect(mine).toContainText("Scores: You 31, Ben 42.5");
  await expect(mine).toContainText("45 min");

  await open(page, "/history");
  const feedRow = page.getByTestId("activity-row").filter({ hasText: "Meeple Harbour" }).first();
  await expect(feedRow).toContainText("Ben won");
  await expect(feedRow).toContainText("Scores: You 31, Ben 42.5");
  await expect(feedRow).toContainText("45 min");

  // a co-op play says how it ended; a play alone says the score
  const coop = await page.request.post(`/api/v1/items/${game}/plays`, {
    data: { players: [{ companion_id: me }, { companion_id: ben }], coop: "won" },
  });
  expect(coop.ok(), `co-op play: ${coop.status()}`).toBeTruthy();
  const solo = await page.request.post(`/api/v1/items/${game}/plays`, {
    data: { players: [{ companion_id: me, score: 12, winner: true }] },
  });
  expect(solo.ok(), `solo play: ${solo.status()}`).toBeTruthy();
  await open(page, `/items/${game}`);
  await page.getByRole("tab", { name: /^History/ }).click();
  await expect(panel.getByTestId("activity-row").filter({ hasText: "Won together" })).toHaveCount(1);
  const alone = panel.getByTestId("activity-row").filter({ hasText: "Score: 12" });
  await expect(alone).toHaveCount(1);
  await expect(alone).toContainText("You won");
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
  await expect(bar(page).getByRole("button", { name: "Mark S1 E9 watched" })).toBeVisible();
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("10/10");
  await expect(bar(page).getByRole("button", { name: "Mark S2 E1 watched" })).toBeVisible();

  // Undo takes the whole bulk mark back
  const toast = page.getByRole("status").locator("> div").filter({ hasText: "Logged: 2 episodes" });
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("8/10");
});

test("episodes: a season spreads over a date range, a bad range says why, Mark show marks the rest", async ({ page }) => {
  test.setTimeout(240_000);
  await signUp(page, `eprange${Date.now()}`, "Range Tester");
  const ids = await seed(page);
  const show = ids["Game of Thrones"];
  await open(page, `/items/${show}`);
  const tab = page.getByRole("tabpanel", { name: "Episodes" });
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("1/10");

  await tab.getByRole("radio", { name: "Spread over a date range" }).click();
  await tab.getByLabel("From").fill("2025-01-01");
  await tab.getByLabel("To").fill("2025-01-31");
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: 9 episodes" })).toBeVisible();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("10/10");
  const events: { occurred_at: string; progress: { type: string } }[] = await (
    await page.request.get(`/api/v1/items/${show}/progress`)
  ).json();
  const spread = events
    .map((e) => e.occurred_at)
    .filter((d) => d.startsWith("2025-01"))
    .sort();
  expect(spread.length).toBe(9);
  expect(spread[0]).toMatch(/^2025-01-01T12:00:00/);
  expect(spread[8]).toMatch(/^2025-01-31T12:00:00/);

  // a range that ends before it starts is refused, inline
  await tab.getByLabel("To").fill("2024-12-01");
  await tab.getByRole("button", { name: "Mark show watched" }).click();
  await expect(tab.getByRole("alert")).toHaveText("The last day is before the first.");

  // "Mark show": every season's open episodes at once (63 = 73 regular episodes - 10)
  await tab.getByRole("radio", { name: "Now" }).click();
  await expect(bar(page).getByRole("button", { name: "Mark S2 E1 watched" })).toBeVisible();
  await tab.getByRole("button", { name: "Mark show watched" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: 63 episodes" })).toBeVisible({ timeout: 120_000 });
  await expect(tab.getByRole("button", { name: /^Season 8/ })).toContainText("6/6");
  await expect(tab.getByRole("button", { name: /^Season 2/ })).toContainText("10/10");
});

test("the log dialogs are centered from 640 px and bottom sheets below; the footer stays in view with ten players", async ({ page }) => {
  // OT-10 (DEC-539): no side sheet, no stretched rows; the form scrolls between title and footer
  test.setTimeout(240_000);
  await page.setViewportSize({ width: 1280, height: 720 });
  await signUp(page, `dlg${Date.now()}`, "Dialog Tester");
  const ids = await seed(page);
  /** OD-01: what the dialog asks as it opens (a radio group is one input, and so is the
   * winner: the "Won" column of the score table; review OT2F-12). */
  const inputs = async (d: ReturnType<Page["getByRole"]>) =>
    (await d.getByRole("radiogroup").count()) +
    (await d.getByRole("textbox").count()) +
    (await d.getByRole("combobox").count()) +
    (await d.getByRole("table").filter({ has: page.getByRole("radio") }).count());

  // the watched dialog: centered, at most 32 rem wide, as tall as its fields
  await open(page, `/items/${ids["Game of Thrones"]}`);
  const opener = page.getByRole("button", { name: "Watched with...: Game of Thrones" });
  await opener.click();
  const watched = page.getByRole("dialog", { name: "Watched S1 E2", exact: true });
  await expect(watched).toBeVisible();
  let box = (await watched.boundingBox())!;
  expect(Math.abs(box.x + box.width / 2 - 640)).toBeLessThanOrEqual(1);
  expect(Math.abs(box.y + box.height / 2 - 360)).toBeLessThanOrEqual(1);
  expect(box.width).toBeLessThanOrEqual(512);
  expect(box.width).toBeGreaterThan(440);
  expect(box.height).toBeLessThan(560);
  // DEC-567: when and the people; the rating and the note wait behind "More"
  expect(await inputs(watched)).toBe(2);
  // the first field follows the title closely (the side sheet spread its rows over the height)
  const title = (await watched.getByRole("heading", { name: "Watched S1 E2" }).boundingBox())!;
  const when = (await watched.getByRole("radio", { name: "Today" }).boundingBox())!;
  expect(when.y - (title.y + title.height)).toBeLessThan(90);
  // the footer: Cancel and Save; Cancel closes an unchanged dialog and focus returns
  const footer = watched.locator("[data-dialog-footer]");
  await expect(footer.getByRole("button", { name: "Save" })).toBeVisible();
  await footer.getByRole("button", { name: "Cancel" }).click();
  await expect(watched).toHaveCount(0);
  await expect(opener).toBeFocused();
  // Cancel on a changed dialog asks first, like Escape
  await opener.click();
  await watched.getByRole("radio", { name: "Yesterday" }).click();
  await footer.getByRole("button", { name: "Cancel" }).click();
  await expect(watched.getByText("Discard your changes?")).toBeVisible();
  await expect(watched.getByRole("button", { name: "Keep editing" })).toBeFocused();
  await watched.getByRole("button", { name: "Discard" }).click();
  await expect(watched).toHaveCount(0);

  // the play dialog: 42 rem; with ten players its body scrolls and the footer stays
  await open(page, `/items/${ids["Meeple Harbour"]}`);
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  const play = page.getByRole("dialog", { name: "Log a play" });
  await expect(play.getByRole("button", { name: "Dialog Tester", exact: true })).toHaveAttribute("aria-pressed", "true");
  box = (await play.boundingBox())!;
  expect(Math.abs(box.x + box.width / 2 - 640)).toBeLessThanOrEqual(1);
  expect(box.width).toBeGreaterThan(512);
  expect(box.width).toBeLessThanOrEqual(672);
  // a board game with one player: when, duration, the people, one score, and the winner (the
  // "Won" column of the score table: the player or "No winner")
  expect(await inputs(play)).toBe(5);
  await expect(play.getByRole("table", { name: "Scores" }).getByRole("radio")).toHaveCount(2);
  const people = play.getByRole("combobox", { name: "Add a person" });
  for (let n = 2; n <= 10; n++) {
    await people.fill(`Player ${n}`);
    await people.press("Enter");
    await expect(play.getByLabel(`Score of Player ${n}`)).toHaveCount(1);
  }
  const save = play.locator("[data-dialog-footer]").getByRole("button", { name: "Save" });
  const inView = async (height: number) => {
    const d = (await play.boundingBox())!;
    const s = (await save.boundingBox())!;
    expect(d.y).toBeGreaterThanOrEqual(0);
    expect(d.y + d.height).toBeLessThanOrEqual(height);
    expect(s.y + s.height).toBeLessThanOrEqual(d.y + d.height);
    const body = play.locator("[data-dialog-body]");
    expect(await body.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
    // a field at the end of the form scrolls into view above the footer (WCAG 2.4.11)
    const last = play.getByLabel("Score of Player 10");
    await last.focus();
    const f = (await last.boundingBox())!;
    const foot = (await play.locator("[data-dialog-footer]").boundingBox())!;
    expect(f.y + f.height).toBeLessThanOrEqual(foot.y);
    await expect(save).toBeInViewport({ ratio: 1 });
    await expect(play.getByRole("heading", { name: "Log a play" })).toBeInViewport({ ratio: 1 });
  };
  await inView(720);
  // Save is outside the form and still its default button: Enter in a field saves
  await play.getByLabel("Duration", { exact: true }).press("Enter");
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  await expect(play).toHaveCount(0);
  await expect(bar(page)).toContainText("One play");

  // 390 px: a bottom sheet over the full width, Save 44 px high and as wide as the sheet
  await page.setViewportSize({ width: 390, height: 844 });
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  // the next play starts with the same ten
  await expect(play.getByLabel("Score of Player 10")).toHaveCount(1);
  box = (await play.boundingBox())!;
  expect(box.x).toBe(0);
  expect(box.width).toBe(390);
  expect(Math.abs(box.y + box.height - 844)).toBeLessThanOrEqual(1);
  expect(box.height).toBeLessThanOrEqual(844 * 0.85 + 1);
  await inView(844);
  const s = (await save.boundingBox())!;
  expect(s.height).toBeGreaterThanOrEqual(44);
  // 24 px of padding and the 1 px border on each side
  expect(s.width).toBeGreaterThanOrEqual(390 - 48 - 2);
  await expect(play.locator("[data-dialog-footer]").getByRole("button", { name: "Cancel" })).toBeHidden();
  await expectNoA11yViolations(page, "play dialog with ten players at 390");
  await page.keyboard.press("Escape");
  await expect(play).toHaveCount(0);
  await open(page, `/items/${ids["Game of Thrones"]}`);
  await opener.click();
  await expect(watched).toBeVisible();
  box = (await watched.boundingBox())!;
  expect(box.x).toBe(0);
  expect(box.width).toBe(390);
  expect(Math.abs(box.y + box.height - 844)).toBeLessThanOrEqual(1);
  expect(box.y).toBeGreaterThan(844 * 0.15 - 1);
});

test("a refetch of the page leaves an open log dialog alone: focus, typed text, scroll position, one intent, focus back to the opener", async ({ page }) => {
  // review OT2B-1: the dialogs were part of the view that every refetch renders afresh; a
  // refetch that arrived under an open dialog moved the focus to the panel, lost the place in
  // the form and what was typed into "Add a person", and left the focus on the body after
  // closing
  test.setTimeout(240_000);
  const beacons: { events: Record<string, string>[] }[] = [];
  await page.route("**/api/v1/usage/funnel", async (route) => {
    beacons.push(route.request().postDataJSON());
    await route.fulfill({ status: 204 });
  });
  await page.setViewportSize({ width: 1280, height: 600 });
  await signUp(page, `refetch${Date.now()}`, "Refetch Tester");
  const ids = await seed(page);
  /**
   * The page fetches itself again (a rating does that) and the answer is held: whatever is
   * opened meanwhile is open when the new render arrives. Returns the release.
   */
  const heldRefetch = async (stars: number) => {
    let release = () => {};
    const held = new Promise<void>((r) => (release = r));
    await page.route("**/_fn/item_page*", async (route) => {
      await held;
      await route.continue();
    });
    const asked = page.waitForRequest((req) => req.url().includes("/_fn/item_page"));
    await page.locator("#ip-rate").getByRole("radio", { name: `Rate ${stars} of 5` }).click();
    await asked;
    return async () => {
      const answered = page.waitForResponse((res) => res.url().includes("/_fn/item_page"));
      release();
      await answered;
      await page.unroute("**/_fn/item_page*");
      // the new render is there: the facts card behind the dialog shows the rating
      await expect(page.getByTestId("item-facts")).toContainText(`${stars} of 5`);
    };
  };

  await open(page, `/items/${ids["Meeple Harbour"]}`);
  let arrive = await heldRefetch(4);
  const opener = bar(page).getByRole("button", { name: "Log a play" });
  await opener.click();
  const play = page.getByRole("dialog", { name: "Log a play" });
  await expect(play.getByRole("button", { name: "Refetch Tester", exact: true })).toHaveAttribute("aria-pressed", "true");
  const people = play.getByRole("combobox", { name: "Add a person" });
  for (let n = 2; n <= 8; n++) {
    await people.fill(`Player ${n}`);
    await people.press("Enter");
    await expect(play.getByLabel(`Score of Player ${n}`)).toHaveCount(1);
  }
  // typed, not entered: this text lives in the field only
  await people.fill("Not entered yet");
  const last = play.getByLabel("Score of Player 8");
  await last.focus();
  await page.keyboard.type("4");
  const body = play.locator("[data-dialog-body]");
  const top = await body.evaluate((el) => el.scrollTop);
  expect(top).toBeGreaterThan(0);
  await arrive();
  await expect(last).toBeFocused();
  // the next key lands where the person was
  await page.keyboard.type("2");
  await expect(last).toHaveValue("42");
  await expect(people).toHaveValue("Not entered yet");
  expect(await body.evaluate((el) => el.scrollTop)).toBe(top);
  await page.keyboard.press("Escape");
  await expect(play.getByRole("button", { name: "Keep editing" })).toBeFocused();
  await play.getByRole("button", { name: "Discard" }).click();
  await expect(play).toHaveCount(0);
  // the button that opened the dialog was rendered anew meanwhile: its successor has the focus
  await expect(opener).toBeFocused();
  // one opening is one intent: a refetch under the open dialog counted a second one (as a
  // tap). The opening and the click on Discard are two (the beacon leaves 5 s later)
  const sheets = () => beacons.flatMap((b) => b.events).filter((e) => e.flow === "sheet");
  await expect.poll(() => sheets().length, { timeout: 30_000 }).toBe(1);
  expect(sheets()[0]).toEqual(expect.objectContaining({ outcome: "abandoned", taps: "2" }));

  // the watched dialog
  await open(page, `/items/${ids["Game of Thrones"]}`);
  arrive = await heldRefetch(3);
  const withPeople = page.getByRole("button", { name: "Watched with...: Game of Thrones" });
  await withPeople.click();
  const watched = page.getByRole("dialog", { name: "Watched S1 E2", exact: true });
  const yesterday = watched.getByRole("radio", { name: "Yesterday" });
  await yesterday.click();
  await expect(yesterday).toBeFocused();
  await arrive();
  await expect(yesterday).toBeFocused();
  await expect(yesterday).toBeChecked();
  await page.keyboard.press("Escape");
  await watched.getByRole("button", { name: "Discard" }).click();
  await expect(watched).toHaveCount(0);
  await expect(withPeople).toBeFocused();

  // the opening, the click on Yesterday, the click on Discard
  await expect.poll(() => sheets().length, { timeout: 30_000 }).toBe(2);
  expect(sheets()[1]).toEqual(expect.objectContaining({ outcome: "abandoned", taps: "3" }));
});

test("another item opened while a log dialog is open closes the dialog; the next one starts fresh", async ({ page }) => {
  // review OT2B-1: the dialogs live in the page, which stays when the route only changes its id
  test.setTimeout(150_000);
  await signUp(page, `dlgnav${Date.now()}`, "Nav Tester");
  const ids = await seed(page);
  // the empty palette offers what was opened last: this account's own items (a search by
  // name may find another account's item of the same name first)
  await open(page, `/items/${ids["Pixel Harbour"]}`);
  await open(page, `/items/${ids["Meeple Harbour"]}`);
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  await palette.getByRole("option", { name: /^Pixel Harbour \(2020\)/ }).first().click();
  await expect(page).toHaveURL(new RegExp(`/items/${ids["Pixel Harbour"]}`));
  await expect(page.getByRole("heading", { level: 1, name: "Pixel Harbour" })).toBeVisible();
  await bar(page).getByRole("button", { name: "Log a session" }).click();
  const session = page.getByRole("dialog", { name: "Log a session" });
  await expect(session).toContainText("Pixel Harbour");
  await expect(session.getByRole("button", { name: "Nav Tester", exact: true })).toHaveAttribute("aria-pressed", "true");
  await session.getByLabel("Duration", { exact: true }).fill("45");
  // back in the history: the same page component, another item
  await page.goBack();
  await expect(page.getByRole("heading", { level: 1, name: "Meeple Harbour" })).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  // the dialog of this item: its name, its title, its fields
  const play = page.getByRole("dialog", { name: "Log a play" });
  await expect(play).toContainText("Meeple Harbour");
  await expect(play).not.toContainText("Pixel Harbour");
  await expect(play.getByRole("button", { name: "Nav Tester", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(play.getByLabel("Duration", { exact: true })).toHaveValue("");
  // a board game: the scores are open (the video game kept them behind "More")
  await expect(play.getByLabel("Score of Nav Tester")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(play).toHaveCount(0);
});

test("in a low window (320 x 256) the fields of a log dialog stay in reach, with the discard question and with errors", async ({ page }) => {
  // review OT2B-2 (WCAG 1.4.10: 1280 x 1024 at 400 %): header and footer kept their height
  // and the body between them shrank to nothing. OT2B-3: one line per kind of error
  test.setTimeout(150_000);
  await signUp(page, `lowwin${Date.now()}`, "Low Tester");
  const ids = await seed(page);
  await open(page, `/items/${ids["Meeple Harbour"]}`);
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  const play = page.getByRole("dialog", { name: "Log a play" });
  await expect(play.getByRole("button", { name: "Low Tester", exact: true })).toHaveAttribute("aria-pressed", "true");
  const people = play.getByRole("combobox", { name: "Add a person" });
  await people.fill("Player 2");
  await people.press("Enter");
  await expect(play.getByLabel("Score of Player 2")).toHaveCount(1);
  await page.setViewportSize({ width: 320, height: 256 });
  const footer = play.locator("[data-dialog-footer]");
  const save = footer.getByRole("button", { name: "Save" });
  const reach = async (where: string) => {
    for (const el of [play.getByRole("radio", { name: "Today" }), play.getByLabel("Score of Player 2"), save]) {
      await el.scrollIntoViewIfNeeded();
      await expect(el, where).toBeInViewport({ ratio: 1 });
    }
  };
  await reach("as it opens");
  await play.getByLabel("Score of Low Tester").fill("42 pts");
  await play.getByLabel("Score of Player 2").fill("17 pts");
  await play.getByLabel("Duration", { exact: true }).fill("soon");
  // the discard question joins the footer
  await page.keyboard.press("Escape");
  await expect(play.getByRole("button", { name: "Keep editing" })).toBeFocused();
  await reach("with the discard question");
  await play.getByRole("button", { name: "Keep editing" }).scrollIntoViewIfNeeded();
  await play.getByRole("button", { name: "Keep editing" }).click();
  // two wrong scores and a wrong duration: one line per kind of error, the fields are marked
  await play.getByLabel("Duration", { exact: true }).press("Enter");
  const errors = footer.getByRole("alert").getByRole("listitem");
  await expect(errors).toHaveText(["Scores are numbers, for example 42 or 12.5.", "Enter the duration as hours:minutes (1:30) or as minutes (90)."]);
  await expect(play.getByLabel("Score of Low Tester")).toHaveAttribute("aria-invalid", "true");
  await expect(play.getByLabel("Score of Player 2")).toHaveAttribute("aria-invalid", "true");
  await expect(play.getByLabel("Duration", { exact: true })).toHaveAttribute("aria-invalid", "true");
  await reach("with the errors");
  // the discard question on top of that
  await page.keyboard.press("Escape");
  const keep = play.getByRole("button", { name: "Keep editing" });
  await expect(keep).toBeFocused();
  await expect(keep).toBeInViewport({ ratio: 1 });
  await reach("with the errors and the discard question");
  await expectNoA11yViolations(page, "play dialog at 320 x 256 with errors and the discard question");
  await keep.scrollIntoViewIfNeeded();
  await keep.click();
  await play.getByLabel("Score of Low Tester").fill("42");
  await play.getByLabel("Score of Player 2").fill("17");
  await play.getByLabel("Duration", { exact: true }).fill("45");
  await play.getByLabel("Duration", { exact: true }).press("Enter");
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  await expect(play).toHaveCount(0);
});

test("one add at a time, and the add toast's Undo keeps an entry that was changed meanwhile", async ({ page }) => {
  // review OT2A-8 (DEC-564): "Want to watch", then "Mark as watched" within the toast's ten
  // seconds, then Undo on the first toast removed the entry with its status and left the watch
  test.setTimeout(150_000);
  await signUp(page, `addundo${Date.now()}`, "Undo Tester");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  await palette.getByRole("combobox").fill("fight club");
  const option = palette.getByRole("option", { name: /^Fight Club \(1999\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  const id = page.url().split("/items/")[1].split(/[?#]/)[0];
  const mark = bar(page).getByRole("button", { name: "Mark as watched", exact: true });
  const want = bar(page).getByRole("button", { name: "Want to watch", exact: true });
  const pill = page.getByTestId("ip-status");

  // while the add is on its way the other button waits too
  let release = () => {};
  const held = new Promise<void>((r) => (release = r));
  await page.route("**/_fn/add_to_library*", async (route) => {
    await held;
    await route.continue();
  });
  await want.click();
  await expect(want).toBeDisabled();
  await expect(mark).toBeDisabled();
  // review OT2B-4: and both keep waiting until the refetched page replaces the bar (a click
  // in between ran the first step of an item "outside the library" once more)
  let show = () => {};
  const pageHeld = new Promise<void>((r) => (show = r));
  await page.route("**/_fn/item_page*", async (route) => {
    await pageHeld;
    await route.continue();
  });
  const refetching = page.waitForRequest((req) => req.url().includes("/_fn/item_page"));
  release();
  const added = page.locator("[role=status][aria-live] > div").filter({ hasText: "Added to your library as Planned" });
  await expect(added).toBeVisible();
  await refetching;
  await expect(want).not.toHaveAttribute("aria-busy", "true");
  await expect(want).toBeDisabled();
  await expect(mark).toBeDisabled();
  // review SEC1-1: answered first, then the handler goes (see the note further up)
  const shown = page.waitForResponse((res) => res.url().includes("/_fn/item_page"));
  show();
  await shown;
  await page.unroute("**/_fn/item_page*");
  await expect(pill).toHaveText("Planned");

  // the entry changes while the toast still offers Undo
  await mark.click();
  await expect(pill).toHaveText("Completed");
  await added.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Kept in your library: it has changed since you added it." })).toBeVisible();
  await expect(pill).toHaveText("Completed");
  const after = await (await page.request.get(`/api/v1/items/${id}/page`)).json();
  expect(after.entry.status).toBe("completed");
  expect(after.progress).toHaveLength(1);
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
        await page.getByRole("button", { name: "Watched with...: Game of Thrones" }).click();
        const sheet = page.getByRole("dialog", { name: "Watched S1 E2", exact: true });
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

/**
 * WP4-7: a show of its own (the mock's synthetic "Lantern Coast", DEC-445: no shared seed),
 * imported and waited for until the background job has fetched its episode dates.
 */
async function seedReleased(page: Page): Promise<string> {
  const row = { source_id: "1", kind: "show", title: "Lantern Coast", year: 2024, status: "in_progress", external_ids: { tmdb: "990001" } };
  const file = [JSON.stringify({ format: "library-export", version: 1 }), JSON.stringify(row)].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  const id: string = lib.items.find((i: { item: { title: string } }) => i.item.title === "Lantern Coast").item.id;
  await expect
    .poll(
      async () => {
        const p = await (await page.request.get(`/api/v1/items/${id}/page`)).json();
        return (p.episodes ?? []).length;
      },
      // detail refresh, then the episode job on the background lane (UX3-22)
      { timeout: 120_000 },
    )
    .toBe(5);
  return id;
}

test("episodes: a season marks on its release dates; the option is hidden while dates are missing", async ({ page }) => {
  test.setTimeout(240_000);
  await signUp(page, `eprelease${Date.now()}`, "Release Tester");
  const show = await seedReleased(page);
  await open(page, `/items/${show}`);
  const tab = page.getByRole("tabpanel", { name: "Episodes" });
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("0/3");

  await tab.getByRole("radio", { name: "On release dates" }).click();
  await expect(tab.getByText("log each episode on the day it was released")).toBeVisible();
  await tab.getByRole("button", { name: "Mark season watched" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: 3 episodes" })).toBeVisible();
  await expect(tab.getByRole("button", { name: /^Season 1/ })).toContainText("3/3");
  const events: { occurred_at: string }[] = await (await page.request.get(`/api/v1/items/${show}/progress`)).json();
  expect(events.map((e) => e.occurred_at.slice(0, 19)).sort()).toEqual([
    "2024-01-05T12:00:00",
    "2024-01-12T12:00:00",
    "2024-01-19T12:00:00",
  ]);
  // History: one entry on each release day
  await page.getByRole("tab", { name: /^History/ }).click();
  const history = page.getByRole("tabpanel", { name: /^History/ });
  for (const day of ["Jan 5, 2024", "Jan 12, 2024", "Jan 19, 2024"]) {
    await expect(history.getByText(day, { exact: false })).toHaveCount(1);
  }

  // season 2 has an episode still to come: the option is not offered, a hint says why
  await page.getByRole("tab", { name: "Episodes" }).click();
  await tab.getByRole("button", { name: /^Season 2/ }).click();
  await expect(tab.getByRole("radio", { name: "On release dates" })).toHaveCount(0);
  await expect(tab.getByRole("radio", { name: "Now" })).toHaveAttribute("aria-checked", "true");
  await expect(tab.getByTestId("ep-release-missing")).toHaveText(
    '"On release dates" is not offered: one open episode of this season has no release date up to today.',
  );

  // from season 1 (all aired) "Mark show" would take the future episode too: refused, nothing logged
  await tab.getByRole("button", { name: /^Season 1/ }).click();
  await tab.getByRole("radio", { name: "On release dates" }).click();
  await tab.getByRole("button", { name: "Mark show watched" }).click();
  await expect(tab.getByRole("alert")).toHaveText("One of these episodes has no release date up to today, so nothing was marked.");
  await expect(tab.getByRole("button", { name: /^Season 2/ })).toContainText("0/2");
});

// ot2f (OT-11 rest, OT-18): a game page has a "Plays" tab: a strip with plays, time played and
// the person's own win rate, and a table of the plays. A co-op play can end without a result
// (the choice starts there) and then reads "Played together", never "Lost together".
test("a game has a Plays tab with time and the win rate; a co-op play can end without a result", async ({ page }) => {
  test.setTimeout(180_000);
  await signUp(page, `plays${Date.now()}`, "Tab Player");
  const ids = await seed(page);
  const game = ids["Pixel Harbour"];
  await open(page, `/items/${game}`);
  // no play yet: the tab is there, the page opens on About
  const tab = page.getByRole("tab", { name: /^Plays/ });
  await expect(tab).toHaveAccessibleName("Plays 0");
  await expect(page.getByRole("tab", { name: "About" })).toHaveAttribute("aria-selected", "true");
  await tab.click();
  const panel = page.getByRole("tabpanel", { name: /^Plays/ });
  await expect(panel).toContainText("No plays logged yet.");

  // log a co-op play through the dialog: "No result" is where the choice starts
  await bar(page).getByRole("button", { name: "Log a session" }).click();
  const sheet = page.getByRole("dialog", { name: "Log a session" });
  await sheet.getByLabel("Duration", { exact: true }).fill("1:30");
  const more = sheet.getByRole("button", { name: /^More/ });
  if ((await more.getAttribute("aria-expanded")) !== "true") {
    await more.click();
  }
  await sheet.getByRole("switch", { name: "Co-op game" }).click();
  const ended = sheet.getByRole("radiogroup", { name: "How it ended" });
  await expect(ended.getByRole("radio")).toHaveText(["No result", "We won", "We lost"]);
  await expect(ended.getByRole("radio", { name: "No result" })).toBeChecked();
  // a co-op play has no "Won" column
  await expect(sheet.getByRole("table", { name: "Scores" }).getByRole("columnheader")).toHaveText(["Player", "Score"]);
  await expectNoA11yViolations(page, "play dialog, co-op");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  const stored = await (await page.request.get(`/api/v1/items/${game}/page`)).json();
  expect(stored.plays[0].play).toMatchObject({ duration_minutes: 90, coop: "none" });
  expect(stored.play_totals).toMatchObject({ plays: 1, minutes: 90, decided: 0, wins: 0 });

  // the tab counts it and shows it
  await expect(tab).toHaveAccessibleName("Plays 1");
  await tab.click();
  const strip = page.getByTestId("ip-plays-strip");
  await expect(strip.locator("dt")).toHaveText(["Plays", "Time played", "Win rate"]);
  await expect(strip.locator("div > dd:nth-of-type(1)")).toHaveText(["1", "1.5 hours", "no results yet"]);
  const table = panel.getByRole("table", { name: /^Plays of Pixel Harbour/ });
  await expect(table.getByRole("columnheader")).toHaveText(["Date", "Players", "Scores", "Result", "Duration"]);
  const first = table.getByRole("row").nth(1).getByRole("cell");
  // (each cell carries its column name for the cards of a phone, hidden here)
  await expect(first.nth(1)).toHaveText("PlayersYou");
  await expect(first.nth(3)).toHaveText("ResultPlayed together");
  await expect(first.nth(4)).toHaveText("Duration1 h 30 min");
  await expect(page.getByRole("main")).not.toContainText("Lost together");

  // two competitive plays: one win and one loss of mine, and one I only logged for others
  const me = (await (await page.request.get("/api/v1/auth/me")).json()).self_companion as string;
  const ben = (await (await page.request.post("/api/v1/companions", { data: { name: "Ben" } })).json()).id as string;
  const cleo = (await (await page.request.post("/api/v1/companions", { data: { name: "Cleo" } })).json()).id as string;
  for (const players of [
    [{ companion_id: me, score: 50, winner: true }, { companion_id: ben, score: 20, winner: false }],
    [{ companion_id: me, score: 10, winner: false }, { companion_id: ben, score: 20, winner: true }],
    [{ companion_id: cleo, winner: true }, { companion_id: ben, winner: false }],
  ]) {
    const r = await page.request.post(`/api/v1/items/${game}/plays`, { data: { players, duration_minutes: 30 } });
    expect(r.ok(), `play: ${r.status()}`).toBeTruthy();
  }
  await open(page, `/items/${game}`);
  // with plays the page opens on the tab
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(tab).toHaveAccessibleName("Plays 4");
  await expect(strip.locator("div > dd:nth-of-type(1)")).toHaveText(["4", "3 hours", "50%"]);
  await expect(strip).toContainText("1 of 2 plays with a result in your own log");
  await expect(table.getByRole("row")).toHaveCount(5);
  await expect(table.getByRole("row").filter({ hasText: "You 50, Ben 20" }).getByRole("cell").nth(3)).toHaveText("ResultYou won");
  await expect(table.getByRole("row").filter({ hasText: "You 10, Ben 20" }).getByRole("cell").nth(3)).toHaveText("ResultBen won");
  await expect(table.getByRole("row").filter({ hasText: "Cleo, Ben" }).getByRole("cell").nth(3)).toHaveText("ResultCleo won");
  // the line under the title ends with the time played
  await expect(page.getByTestId("item-fact-line")).toContainText(/^Played 4 times, last on .+, 3 hours in total$/);
  // the History tab still has every row, with the co-op one in the same words
  await page.getByRole("tab", { name: /^History/ }).click();
  await expect(page.getByRole("tabpanel", { name: /^History/ }).getByTestId("activity-row").filter({ hasText: "Played together" })).toHaveCount(1);

  // keyboard: the tabs are one tab stop, the arrows move and select
  await page.getByRole("tab", { name: /^History/ }).focus();
  await page.keyboard.press("ArrowLeft");
  await expect(page.getByRole("tab", { name: "About" })).toBeFocused();
  await page.keyboard.press("ArrowLeft");
  await expect(tab).toBeFocused();
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(table).toBeVisible();
  await expectNoA11yViolations(page, "plays tab, light");
  await expectReflow(page, "plays tab 1280");

  // dark, and a phone: each play is a card that names its values
  await page.evaluate(() => localStorage.setItem("app-theme", "dark"));
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, `/items/${game}`);
  await expect(page.locator("html")).toHaveClass(/dark/);
  await expect(strip).toBeVisible();
  const card = table.getByRole("row").nth(1);
  await expect(card).toContainText("Result");
  await expect(card).toContainText("Duration");
  // review OT2F-10: the last card keeps its border (a table row loses the rule under it)
  for (const row of [card, table.getByRole("row").last()]) {
    expect(await row.evaluate((el) => getComputedStyle(el).borderBottomWidth), "card border").toBe("1px");
  }
  await expectNoA11yViolations(page, "plays tab, dark, 390 px");
  await expectReflow(page, "plays tab 390");
});

// Review of ot2f (OT2F-1, 3, 4, 5, 6, 11): the score table names each row's fields after its
// player, one score alone is no result, "No winner" can be chosen, a choice by hand follows
// the table, and the keyboard focus stays in the table when the choice is taken back.
test("the score table: one id per player, the winner by score or by hand, no winner, 320 px", async ({ page }) => {
  test.setTimeout(180_000);
  await signUp(page, `score${Date.now()}`, "Score Tester");
  const ids = await seed(page);
  const game = ids["Meeple Harbour"];
  await open(page, `/items/${game}`);
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  const sheet = page.getByRole("dialog", { name: "Log a play" });
  const scores = sheet.getByRole("table", { name: "Scores" });
  const won = (name: string) => scores.getByRole("radio", { name: `${name} won` });
  const nobody = scores.getByRole("radio", { name: "No winner" });
  const reset = sheet.getByRole("button", { name: "Use the highest score" });
  const live = sheet.getByTestId("play-winner-live");
  const chip = (name: string) => sheet.getByRole("button", { name, exact: true });

  // OT2F-4: a play of one person has no result by its score alone
  await sheet.getByLabel("Score of Score Tester").fill("120");
  await expect(nobody).toBeChecked();
  await expect(won("Score Tester")).not.toBeChecked();
  // OT2F-6: the table says how the mark is set
  await expect(scores).toHaveAccessibleDescription("The highest score is marked as the winner. Choose another if needed.");
  await expect(sheet.getByText("The highest score is marked as the winner. Choose another if needed.")).toBeVisible();

  const people = sheet.getByRole("combobox", { name: "Add a person" });
  for (const name of ["Ben", "Cleo"]) {
    await people.fill(name);
    await people.press("Enter");
    await expect(sheet.getByLabel(`Score of ${name}`)).toHaveCount(1);
  }
  // one score of three is still no result; a second score is, and it is announced
  await expect(nobody).toBeChecked();
  await sheet.getByLabel("Score of Cleo").fill("20");
  await expect(won("Score Tester")).toBeChecked();
  await expect(live).toHaveText("Score Tester is marked as the winner (highest score).");

  // OT2F-1: a player before the last one leaves and another joins: every id is there once and
  // the whole 36 px target of the new row marks the new player
  await chip("Ben").click();
  await expect(sheet.getByLabel("Score of Ben")).toHaveCount(0);
  await people.fill("Dana");
  await people.press("Enter");
  await expect(scores.getByRole("rowheader")).toHaveText(["Score Tester", "Cleo", "Dana"]);
  const found = await scores.locator("[id]").evaluateAll((els) => els.map((e) => e.id));
  expect(found.length).toBeGreaterThanOrEqual(6);
  expect(new Set(found).size, `ids in the score table: ${found.join(" ")}`).toBe(found.length);
  await won("Dana").locator("xpath=..").click({ position: { x: 3, y: 3 } });
  await expect(won("Dana")).toBeChecked();
  await expect(won("Cleo")).not.toBeChecked();
  await expect(sheet.getByLabel("Score of Dana")).toHaveCount(1);
  await sheet.getByLabel("Score of Dana").fill("10");

  // OT2F-3: taking the choice back by keyboard leaves the focus on the mark it gave
  await reset.focus();
  await page.keyboard.press("Enter");
  await expect(won("Score Tester")).toBeChecked();
  await expect(won("Score Tester")).toBeFocused();
  await expect(reset).toHaveCount(0);

  // OT2F-5: a winner chosen by hand who leaves the table is no choice any more
  await won("Dana").check();
  await expect(reset).toBeVisible();
  await chip("Dana").click();
  await expect(sheet.getByLabel("Score of Dana")).toHaveCount(0);
  await expect(won("Score Tester")).toBeChecked();
  await expect(reset).toHaveCount(0);

  // OT2F-4: a scored play can be saved without a winner
  await nobody.check();
  await expect(won("Score Tester")).not.toBeChecked();
  await expect(reset).toBeVisible();
  await expectNoA11yViolations(page, "play dialog, no winner");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  const stored = await (await page.request.get(`/api/v1/items/${game}/page`)).json();
  const players = stored.plays[0].participants as { name: string; score: number | null; winner: boolean }[];
  expect(players.map((p) => p.winner)).toEqual([false, false]);
  expect(players.map((p) => p.score).sort()).toEqual([120, 20]);
  expect(stored.play_totals).toMatchObject({ plays: 1, decided: 0, wins: 0 });

  // OT2F-11: at 320 px a long name is cut, the table stays inside the sheet
  await page.setViewportSize({ width: 320, height: 700 });
  await open(page, `/items/${game}`);
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  await expect(sheet.getByLabel("Score of Cleo")).toHaveCount(1);
  await people.fill("Maximiliane Hohenzollern");
  await people.press("Enter");
  const long = won("Maximiliane Hohenzollern");
  await expect(long).toHaveCount(1);
  await long.scrollIntoViewIfNeeded();
  const target = (await long.locator("xpath=..").boundingBox())!;
  expect(target.x + target.width, "the Won column at 320 px").toBeLessThanOrEqual(320);
  const table = (await scores.boundingBox())!;
  const body = (await sheet.boundingBox())!;
  expect(table.x + table.width).toBeLessThanOrEqual(body.x + body.width);
  await expectReflow(page, "play dialog 320");
});
