import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations, hydrated, signUp, importDone } from "./helpers";

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
  // (text fields), the people finder and the winner (comboboxes)
  await expect(sheet.getByRole("textbox")).toHaveCount(2);
  await expect(sheet.getByRole("combobox")).toHaveCount(2);
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
  await mark.click();
  const logged = page.locator("[role=status][aria-live] > div").filter({ hasText: "Logged: Watched" });
  await expect(logged).toContainText("Fight Club, added to your library");
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
