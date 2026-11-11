import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations, expectReflow, hydrated, signUp, importDone } from "./helpers";

// WP4-6 (design.md 6.1, 6.6, 9; research R-04): the Stats page compares this year so far with
// the same days last year and lists the people seen most; opt-in goals are set, edited and
// deleted there, show "n of m in <year>" with a bar, never judge the pace and never appear on
// Home; the History side panel carries the comparison too.

const YEAR = new Date().getUTCFullYear();

async function seed(page: Page, stamp: number): Promise<Record<string, string>> {
  const rows = [
    { source_id: "1", kind: "movie", title: `Quiet Pier ${stamp}`, year: 2020, status: "planned" },
    { source_id: "2", kind: "movie", title: `Low Tide ${stamp}`, year: 2021, status: "planned" },
    { source_id: "3", kind: "book", title: `Paper Moors ${stamp}`, year: 2019, status: "planned" },
  ];
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...rows.map((r) => JSON.stringify(r))].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  return Object.fromEntries(lib.items.map((i: { item: { title: string; id: string } }) => [i.item.title.split(" ").slice(0, 2).join(" "), i.item.id]));
}

async function log(page: Page, item: string, progress: object, at: string) {
  const r = await page.request.post(`/api/v1/items/${item}/progress`, { data: { progress, occurred_at: at, with: [] } });
  expect(r.ok(), `log: ${r.status()}`).toBeTruthy();
}

async function open(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

test("goals are set, edited and deleted on Stats, never on Home; the year comparison; axe and reflow", async ({ page }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  await signUp(page, `goal${stamp}`, "Gwen");
  const ids = await seed(page, stamp);
  // January 2 at noon UTC is the same local day in every zone
  await log(page, ids["Quiet Pier"], { type: "finished" }, `${YEAR}-01-02T12:00:00Z`);
  await log(page, ids["Low Tide"], { type: "finished" }, `${YEAR - 1}-01-02T12:00:00Z`);
  await log(page, ids["Paper Moors"], { type: "paged", page: 120 }, `${YEAR}-01-02T12:00:00Z`);

  await open(page, "/stats");
  const compare = page.getByTestId("st-compare");
  await expect(compare.getByRole("heading", { name: `${YEAR} so far` })).toBeVisible();
  await expect(compare).toContainText(`Watch: Watched1 in ${YEAR}, 1 in ${YEAR - 1}`);
  await expect(compare).toContainText(`Read: Pages120 in ${YEAR}, 0 in ${YEAR - 1}`);
  // no title here has a known length: the counts lead and "Hours" comes last, in the totals
  // row, the charts and the comparison of a group's own tab (review OT2C-5)
  const row = page.getByTestId("st-totals");
  await expect(row.locator("dt")).toHaveText(["Activity", "Completed entries", "Hours"]);
  await expect(row.locator("dd").last()).toHaveText("0");
  await open(page, "/stats?group=watch");
  await expect(row.locator("dt")).toHaveText(["Watched", "Episodes", "Hours"]);
  await expect(page.getByRole("figure")).toHaveCount(3);
  await expect(page.getByRole("figure").first()).toHaveAccessibleName(/^Watched/);
  await expect(page.getByRole("figure").last()).toHaveAccessibleName(/^Hours/);
  await expect(compare.locator("dt")).toHaveText(["Watched", "Episodes", "Hours"]);
  await open(page, "/stats");
  await expect(page.getByTestId("st-often")).toContainText("Log something with a person");
  const goals = page.getByTestId("st-goals");
  await expect(goals.getByTestId("st-goal")).toHaveCount(0);

  // set a goal behind the disclosure
  await goals.getByRole("button", { name: /^Set a goal/ }).click();
  await page.getByLabel("Year", { exact: true }).selectOption(String(YEAR));
  await page.getByLabel("Group", { exact: true }).selectOption("read");
  await page.getByLabel("Measure", { exact: true }).selectOption("pages");
  await page.getByLabel("Target", { exact: true }).fill("500");
  await goals.getByRole("button", { name: "Save goal" }).click();
  const card = goals.getByTestId("st-goal");
  await expect(card).toHaveCount(1);
  await expect(card).toContainText("Read: Pages");
  await expect(card).toContainText(`120 of 500 in ${YEAR}`);
  await expect(card.getByRole("progressbar", { name: `Read: Pages in ${YEAR}` })).toHaveAttribute("aria-valuetext", `120 of 500 in ${YEAR}`);
  // one goal per year, group and measure
  await page.getByLabel("Target", { exact: true }).fill("100");
  await goals.getByRole("button", { name: "Save goal" }).click();
  await expect(goals.getByRole("alert")).toContainText("There is a goal for this year, group and measure already.");
  await expect(goals).not.toContainText(/behind|ahead/i);
  await expectNoA11yViolations(page, "stats with a goal");

  // edit
  await card.getByRole("button", { name: "Edit goal Read: Pages" }).click();
  const edit = page.getByRole("dialog", { name: "Edit goal Read: Pages" });
  await edit.getByLabel("Target", { exact: true }).fill("100");
  await edit.getByRole("button", { name: "Save goal" }).click();
  await expect(edit).toHaveCount(0);
  await expect(card).toContainText(`120 of 100 in ${YEAR}`);

  await page.setViewportSize({ width: 360, height: 800 });
  await expectReflow(page, "stats 360");
  await expectNoA11yViolations(page, "stats 360");
  await page.setViewportSize({ width: 1280, height: 800 });

  // never on Home (R-04)
  await open(page, "/");
  await expect(page.getByRole("heading", { name: "All-time stats" })).toBeVisible();
  await expect(page.getByTestId("st-goal")).toHaveCount(0);
  await expect(page.getByRole("main")).not.toContainText(`of 100 in ${YEAR}`);

  // the History side panel compares too
  await open(page, "/history");
  await expect(page.getByTestId("history-compare")).toContainText(`Watch: Watched1 in ${YEAR}, 1 in ${YEAR - 1}`);

  // delete
  await open(page, "/stats");
  await page.getByTestId("st-goal").getByRole("button", { name: "Delete goal Read: Pages" }).click();
  const confirm = page.getByRole("dialog", { name: "Delete the goal Read: Pages?" });
  await expect(confirm).toContainText("Only the goal goes; everything you logged stays.");
  await confirm.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByTestId("st-goal")).toHaveCount(0);
  const goalsLeft = await (await page.request.get("/api/v1/stats/goals")).json();
  expect(goalsLeft).toEqual([]);
});

test("the people seen most often are listed on Stats", async ({ page }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  await signUp(page, `often${stamp}`, "Otto");
  const ids = await seed(page, stamp);
  const mk = async (name: string) => (await (await page.request.post("/api/v1/companions", { data: { name } })).json()).id as string;
  const [ria, sam] = [await mk("Ria"), await mk("Sam")];
  for (const [item, who] of [
    [ids["Quiet Pier"], [ria, sam]],
    [ids["Low Tide"], [ria]],
  ] as [string, string[]][]) {
    const r = await page.request.post(`/api/v1/items/${item}/progress`, { data: { progress: { type: "finished" }, occurred_at: `${YEAR}-01-02T12:00:00Z`, with: who } });
    expect(r.ok()).toBeTruthy();
  }
  await open(page, "/stats");
  const often = page.getByTestId("st-often");
  await expect(often.getByRole("listitem")).toHaveCount(2);
  await expect(often.getByRole("listitem").first()).toContainText("Ria2 times together");
  await expect(often.getByRole("listitem").nth(1)).toContainText("Samonce together");
  await expect(often.getByRole("link", { name: "Ria" })).toHaveAttribute("href", `/people/${ria}`);
});

// OT-17 (DEC-542): the Stats page is led by time. The hero tile and the four tiles sit above
// the tabs; in a tab the hours come first (totals row, chart, table, year comparison), in
// hours, never minutes; the entries by status moved behind a closed disclosure below the charts.
test("stats lead with time: hero and tiles on top, hours in the tab, statuses behind a disclosure", async ({ page }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  await signUp(page, `hrs${stamp}`, "Hedda");
  const rows = [
    { source_id: "1", kind: "board_game", title: `Salt Road ${stamp}`, year: 2016, status: "in_progress" },
    { source_id: "2", kind: "movie", title: `Quiet Pier ${stamp}`, year: 2020, status: "planned" },
  ];
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...rows.map((r) => JSON.stringify(r))].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  const game = lib.items.find((i: { item: { kind: string } }) => i.item.kind === "board_game").item.id as string;
  // two plays in January, 90 and 30 minutes: 2 hours
  for (const duration_minutes of [90, 30]) {
    const r = await page.request.post(`/api/v1/items/${game}/plays`, { data: { duration_minutes, players: [], started_at: `${YEAR}-01-02T12:00:00Z` } });
    expect(r.ok(), `play: ${r.status()}`).toBeTruthy();
  }

  await open(page, "/stats");
  const hero = page.getByRole("region", { name: "Time banked" });
  await expect(hero.getByRole("heading", { level: 2, name: "Time banked" })).toBeVisible();
  await expect(hero.getByTestId("banked-total")).toHaveText("2 hours");
  await expect(hero.locator("svg[data-icon='piggy-bank']")).toHaveAttribute("aria-hidden", "true");
  const tiles = page.getByTestId("banked").locator("dl > div");
  await expect(tiles.locator("dt")).toHaveText(["Movies", "Shows", "Books", "Games"]);
  await expect(page.getByTestId("banked-tile-games").locator("dd")).toHaveText(["2 hours", "2 plays"]);
  await expect(page.getByTestId("banked-tile-books").locator("dd")).toHaveText(["0 pages", "0 books finished"]);
  // no pages read: only the note about titles without a length
  await expect(page.getByTestId("banked-notes")).toHaveText("Titles without a known length count as 0.");
  const top = async (l: ReturnType<Page["locator"]>) => (await l.boundingBox())!.y;
  expect(await top(hero)).toBeLessThan(await top(page.getByRole("tablist")));

  // All: hours lead the totals row
  const totals = page.getByTestId("st-totals");
  await expect(totals.locator("dt").first()).toHaveText("Hours");
  await expect(totals.locator("dd").first()).toHaveText("2");

  // Play: hours, then plays; the chart, its sentence and its table are in hours
  await page.getByRole("tab", { name: "Play" }).click();
  await expect(page).toHaveURL(/\/stats\?group=play$/);
  await expect(totals.locator("dt")).toHaveText(["Hours", "Plays"]);
  await expect(totals.locator("dd")).toHaveText(["2", "2"]);
  const figures = page.getByRole("figure");
  await expect(figures).toHaveCount(2);
  const hours = figures.first();
  await expect(hours).toHaveAccessibleName(/^Hours/);
  await expect(hours.getByRole("img", { name: `Hours in Play, Jan ${YEAR} to Jan ${YEAR}: 2 in total, most in Jan ${YEAR}.` })).toBeVisible();
  await hours.getByText("Show as a table").click();
  const table = hours.getByRole("table");
  await expect(table.getByRole("columnheader")).toHaveText(["Month", "Hours"]);
  await expect(table.getByRole("row").filter({ hasText: `Jan ${YEAR}` }).getByRole("cell").last()).toHaveText("2");
  // (the goal form keeps its "Minutes" measure: goals are not part of OT-17 step 1)
  await expect(page.getByRole("figure").filter({ hasText: "Minutes" })).toHaveCount(0);
  await expect(totals).not.toContainText("Minutes");
  await expect(page.getByTestId("st-compare")).toContainText(`Hours2 in ${YEAR}, 0 in ${YEAR - 1}`);

  // the entries by status: below the charts, closed, bars with a sentence and the numbers as text
  const line = page.getByTestId("st-library-line");
  await expect(line).toBeHidden();
  expect(await top(page.getByText("Library by status"))).toBeGreaterThan(await top(figures.last()));
  await page.getByText("Library by status").click();
  await expect(line).toHaveText("Play: 1 entry, 1 in progress");
  await expect(page.getByRole("img", { name: "Play: 0 planned, 1 in progress, 0 completed, 0 paused, 0 dropped" })).toBeVisible();
  await expectNoA11yViolations(page, "stats led by time");

  // on a phone, loaded there (at the top of the page; scrolled, the fixed bottom bar covers
  // whatever is under it)
  await page.setViewportSize({ width: 360, height: 800 });
  await open(page, "/stats?group=play");
  await expect(page.getByRole("tab", { name: "Play" })).toHaveAttribute("aria-selected", "true");
  await expect(hero.getByTestId("banked-total")).toHaveText("2 hours");
  await expectReflow(page, "stats led by time 360");
  await expectNoA11yViolations(page, "stats led by time 360");
});

// ot2f (OT-11 rest): the Play tab says how the person's plays ended: the win rate over the
// plays with a result of theirs, and how many plays it leaves out (a co-op play without a
// result, OT-18; a play they only logged). The other tabs do not show it.
test("the Play tab shows the win rate after hours and plays", async ({ page }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  await signUp(page, `rate${stamp}`, "Rosa");
  const rows = [
    { source_id: "1", kind: "board_game", title: `Salt Road ${stamp}`, year: 2016, status: "in_progress" },
    { source_id: "2", kind: "video_game", title: `Brick Wizards ${stamp}`, year: 2010, status: "in_progress" },
  ];
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...rows.map((r) => JSON.stringify(r))].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  const id = (kind: string) => lib.items.find((i: { item: { kind: string } }) => i.item.kind === kind).item.id as string;
  const me = (await (await page.request.get("/api/v1/auth/me")).json()).self_companion as string;
  const ben = (await (await page.request.post("/api/v1/companions", { data: { name: "Ben" } })).json()).id as string;
  const play = async (item: string, data: object) => {
    const r = await page.request.post(`/api/v1/items/${item}/plays`, { data: { duration_minutes: 60, started_at: `${YEAR}-01-02T12:00:00Z`, ...data } });
    expect(r.ok(), `play: ${r.status()}`).toBeTruthy();
  };
  // nothing decided yet: one co-op play without a result
  await play(id("video_game"), { players: [{ companion_id: me }, { companion_id: ben }], coop: "none" });
  await open(page, "/stats?group=play");
  const results = page.getByTestId("st-results");
  await expect(results.getByRole("heading", { level: 2, name: "Your results" })).toBeVisible();
  await expect(page.getByTestId("st-win-rate")).toHaveText("no results yet");
  await expect(results).toContainText("One play is not counted (no result, or without you).");

  // two wins (one of them co-op) and a loss
  await play(id("board_game"), { players: [{ companion_id: me, score: 40, winner: true }, { companion_id: ben, score: 30 }] });
  await play(id("board_game"), { players: [{ companion_id: me, score: 10 }, { companion_id: ben, score: 30, winner: true }] });
  await play(id("video_game"), { players: [{ companion_id: me }, { companion_id: ben }], coop: "won" });
  await open(page, "/stats?group=play");
  // hours and plays first (the totals row), the win rate second
  const totals = page.getByTestId("st-totals");
  await expect(totals.locator("dt")).toHaveText(["Hours", "Plays"]);
  await expect(totals.locator("dd")).toHaveText(["4", "4"]);
  await expect(page.getByTestId("st-win-rate")).toHaveText("67%");
  await expect(results).toContainText("2 of 3 plays with a result in your own log");
  await expect(results).toContainText("One play is not counted (no result, or without you).");
  const top = async (l: ReturnType<Page["locator"]>) => (await l.boundingBox())!.y;
  expect(await top(totals)).toBeLessThan(await top(results));
  await expectNoA11yViolations(page, "stats play tab");
  await expectReflow(page, "stats play tab");
  // not a number of the other tabs
  await page.getByRole("tab", { name: "Watch" }).click();
  await expect(results).toHaveCount(0);
  await page.getByRole("tab", { name: "All" }).click();
  await expect(results).toHaveCount(0);
  // the games tile on top counts the same four hours
  await expect(page.getByTestId("banked-tile-games").locator("dd")).toHaveText(["4 hours", "4 plays"]);
});

// Review of ot2f (OT2F-2): a play is counted from its history event everywhere (the header of
// the game, the History tab, the totals row, the tiles). A history that came without the
// plays' details (an export file carries the events only) is still counted on the Plays tab
// and under "Your results", and both say what is missing.
test("plays without details are counted on the Plays tab and in the results, and named", async ({ page }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  await signUp(page, `bare${stamp}`, "Imke");
  const history = [3, 4, 5].map((day) => ({
    date: `${YEAR}-01-0${day}`,
    progress: { type: "session", session_id: crypto.randomUUID() },
  }));
  const row = { source_id: "1", kind: "board_game", title: `Echo Harbour ${stamp}`, year: 2015, status: "in_progress", history };
  const file = [JSON.stringify({ format: "library-export", version: 1 }), JSON.stringify(row)].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  const game = lib.items[0].item.id as string;
  const stored = await (await page.request.get(`/api/v1/items/${game}/page`)).json();
  expect(stored.times, "three plays in the history").toBe(3);
  expect(stored.play_totals).toMatchObject({ plays: 0 });
  const bare = "3 plays have no details (players, result, duration). They count as plays, not in the time or the win rate.";

  // the game page: header, Plays tab and History agree
  await open(page, `/items/${game}`);
  await expect(page.getByTestId("item-fact-line")).toContainText(/^Played 3 times, last on /);
  const tab = page.getByRole("tab", { name: /^Plays/ });
  await expect(tab).toHaveAccessibleName("Plays 3");
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tab", { name: /^History/ })).toHaveAccessibleName("History 3");
  const panel = page.getByRole("tabpanel", { name: /^Plays/ });
  const strip = page.getByTestId("ip-plays-strip");
  await expect(strip.locator("div > dd:nth-of-type(1)")).toHaveText(["3", "not logged", "no results yet"]);
  await expect(panel.getByTestId("ip-plays-bare")).toHaveText(bare);
  await expect(panel).not.toContainText("No plays logged yet.");
  await expect(panel.getByRole("table")).toHaveCount(0);
  await expectNoA11yViolations(page, "plays tab without details");

  // the Stats Play tab: the totals row and the results count the same plays
  await open(page, "/stats?group=play");
  const totals = page.getByTestId("st-totals");
  // (no time logged: the counts lead, review OT2C-5)
  await expect(totals.locator("dt")).toHaveText(["Plays", "Hours"]);
  await expect(totals.locator("dd")).toHaveText(["3", "0"]);
  const results = page.getByTestId("st-results");
  await expect(page.getByTestId("st-win-rate")).toHaveText("no results yet");
  await expect(results.getByTestId("st-plays-bare")).toHaveText(bare);
  await expect(results).not.toContainText("not counted");

  // one play with its details on top
  const me = (await (await page.request.get("/api/v1/auth/me")).json()).self_companion as string;
  const ben = (await (await page.request.post("/api/v1/companions", { data: { name: "Ben" } })).json()).id as string;
  const players = [{ companion_id: me, score: 40, winner: true }, { companion_id: ben, score: 30 }];
  const r = await page.request.post(`/api/v1/items/${game}/plays`, { data: { players, duration_minutes: 120, started_at: `${YEAR}-01-06T12:00:00Z` } });
  expect(r.ok(), `play: ${r.status()}`).toBeTruthy();
  await open(page, "/stats?group=play");
  await expect(totals.locator("dt")).toHaveText(["Hours", "Plays"]);
  await expect(totals.locator("dd")).toHaveText(["2", "4"]);
  await expect(page.getByTestId("st-win-rate")).toHaveText("100%");
  await expect(results).toContainText("1 of 1 play with a result in your own log");
  await expect(results.getByTestId("st-plays-bare")).toHaveText(bare);
  await expect(results).not.toContainText("not counted");
  await expect(page.getByTestId("banked-tile-games").locator("dd")).toHaveText(["2 hours", "4 plays"]);
  await expectNoA11yViolations(page, "stats play tab with plays without details");
  await open(page, `/items/${game}`);
  await expect(tab).toHaveAccessibleName("Plays 4");
  await expect(strip.locator("div > dd:nth-of-type(1)")).toHaveText(["4", "2 hours", "100%"]);
  await expect(panel.getByTestId("ip-plays-bare")).toHaveText(bare);
  await expect(panel.getByRole("table").getByRole("row")).toHaveCount(2);
});
