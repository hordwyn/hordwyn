import { test, expect, type Page } from "./fixtures";
import { addPerson, expectNoA11yViolations, expectReflow, hydrated, signUp, importDone } from "./helpers";

// People (WP3-6, design.md 6.7, 5.11; mockup companions.html): person cards with initials,
// link state and the stats (together, plays, win rate with a tie, last time, most played),
// the duplicate suggestion for names equal after normalising, the person page with the
// history filtered to the person, "delete keeps history", and axe and reflow at 360 px.

// titles no other spec searches for: specs run in parallel on one catalog (sources.spec
// searches "catan" and expects nothing)
const ROWS = [
  { source_id: "1", kind: "board_game", title: "Tidal Range", year: 2017, status: "planned" },
  { source_id: "2", kind: "board_game", title: "Quarry Night", year: 1995, status: "planned" },
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

async function person(page: Page, name: string): Promise<string> {
  const r = await page.request.post("/api/v1/companions", { data: { name } });
  expect(r.status()).toBe(201);
  return (await r.json()).id;
}

async function play(page: Page, item: string, players: { companion_id: string; winner: boolean }[], at: string) {
  const r = await page.request.post(`/api/v1/items/${item}/plays`, { data: { players, started_at: at } });
  expect(r.ok(), `play: ${r.status()}`).toBeTruthy();
}

function card(page: Page, name: string) {
  return page.getByTestId("pp-card").filter({ has: page.getByRole("link", { name, exact: true }) });
}

test("cards and the person page show the stats; axe and reflow at 360 px", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `ppl${Date.now()}`, "Pia");
  const items = await seed(page);
  const me = (await (await page.request.get("/api/v1/auth/me")).json()).self_companion as string;
  const anna = await person(page, "Anna");
  await person(page, "anna.");
  // Anna wins, a tie, Pia wins: 2 of 3 decided plays are Anna's wins (67%)
  await play(page, items["Tidal Range"], [{ companion_id: me, winner: false }, { companion_id: anna, winner: true }], "2026-03-01T20:00:00Z");
  await play(page, items["Tidal Range"], [{ companion_id: me, winner: true }, { companion_id: anna, winner: true }], "2026-03-02T20:00:00Z");
  await play(page, items["Quarry Night"], [{ companion_id: me, winner: true }, { companion_id: anna, winner: false }], "2026-03-03T20:00:00Z");

  await page.goto("/people");
  await hydrated(page);
  const a = card(page, "Anna");
  await expect(a).toContainText("Together3");
  await expect(a).toContainText("Plays3");
  await expect(a).toContainText("Win rate67%");
  await expect(a).toContainText("Last: Quarry Night");
  await expect(a).toContainText("Most played: Tidal Range, 2 plays");
  await expect(a).toContainText("No account");
  await expect(card(page, "anna.")).toContainText("No winner yet");
  await expect(page.getByTestId("pp-suggestion")).toContainText('"Anna" and "anna." look like the same person.');
  await expectNoA11yViolations(page, "people");

  await a.getByRole("link", { name: "Anna", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/people/${anna}$`));
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: "Anna" })).toBeVisible();
  await expect(page.getByText("Win rate67%")).toBeVisible();
  const history = page.getByRole("region", { name: "History with Anna" });
  await expect(history.getByTestId("activity-row")).toHaveCount(3);
  await expect(history.getByRole("link", { name: "All history with Anna" })).toHaveAttribute("href", `/history?person=${anna}`);
  await expect(page.getByRole("region", { name: "Lists shared by Anna" })).toContainText(
    "Lists show here once Anna has an account linked to this entry.",
  );
  await expectNoA11yViolations(page, "person");

  await page.setViewportSize({ width: 360, height: 800 });
  await expectReflow(page, "person 360");
  await expectNoA11yViolations(page, "person 360");
  await page.goto("/people");
  await hydrated(page);
  await expectReflow(page, "people 360");
  await expectNoA11yViolations(page, "people 360");
});

test("add a person with a note, edit, and delete: the history keeps the plays", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `pdel${Date.now()}`, "Pam");
  const items = await seed(page);
  const ben = await person(page, "Ben");
  await play(page, items["Tidal Range"], [{ companion_id: ben, winner: true }], "2026-03-01T20:00:00Z");

  await page.goto("/people");
  await hydrated(page);
  await page.getByRole("button", { name: "Add a person" }).click();
  const sheet = page.getByRole("dialog", { name: "Add a person" });
  await sheet.getByLabel("Name", { exact: true }).fill("Mara");
  await sheet.getByLabel("Note (optional)").fill("Tuesday group");
  await sheet.getByRole("button", { name: "Add person" }).click();
  await expect(page.getByText("Added Mara.")).toBeVisible();
  await expect(card(page, "Mara")).toContainText("No account");
  await addPerson(page, "Zed");

  // rename from the card menu
  await page.getByRole("button", { name: "More actions for Zed" }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  const edit = page.getByRole("dialog", { name: "Edit Zed" });
  await edit.getByLabel("Name", { exact: true }).fill("Zoe");
  await edit.getByRole("button", { name: "Save" }).click();
  await expect(card(page, "Zoe")).toBeVisible();

  // Zoe has no history: deleted outright; Ben has a play: removed, the history keeps it
  await page.getByRole("button", { name: "More actions for Zoe" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await expect(page.getByRole("dialog", { name: "Delete Zoe?" })).toContainText("Zoe is deleted.");
  await page.getByRole("dialog", { name: "Delete Zoe?" }).getByRole("button", { name: "Delete" }).click();
  await expect(card(page, "Zoe")).toHaveCount(0);

  await card(page, "Ben").getByRole("link", { name: "Ben", exact: true }).click();
  await hydrated(page);
  await page.getByRole("button", { name: "More actions for Ben" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  const confirm = page.getByRole("dialog", { name: "Delete Ben?" });
  await expect(confirm).toContainText("Ben disappears from People and the log sheet. Your history keeps the session with them.");
  await confirm.getByRole("button", { name: "Delete" }).click();
  await expect(page).toHaveURL(/\/people$/);
  await hydrated(page);
  await expect(card(page, "Ben")).toHaveCount(0);
  await page.goto("/history");
  await hydrated(page);
  await expect(page.getByTestId("activity-row").filter({ hasText: "Tidal Range" })).toContainText("Ben");
});

test("Log with a person: the palette picks the item, its sheet opens with them (DEC-353)", async ({ page }) => {
  test.setTimeout(90_000);
  const stamp = Date.now();
  await signUp(page, `plw${stamp}`, "Lou");
  const title = `Lantern Cove ${stamp}`;
  const file = [JSON.stringify({ format: "library-export", version: 1 }), JSON.stringify({ source_id: "1", kind: "movie", title, year: 2011, status: "planned" })].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const ada = await person(page, "Ada");

  await page.goto("/people");
  await hydrated(page);
  await card(page, "Ada").getByRole("button", { name: "Log with Ada" }).click();
  const palette = page.getByRole("dialog", { name: "Search" });
  await expect(palette.getByTestId("palette-target")).toHaveText("Choosing a result logs it with Ada.");
  await palette.getByRole("combobox").fill(title);
  const option = palette.getByRole("option", { name: new RegExp(`^${title}`) }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page).toHaveURL(/\/items\/.+\?log=1&with=/);
  const sheet = page.getByRole("dialog", { name: "Watched", exact: true });
  await expect(sheet).toBeVisible();
  await expect(sheet.getByRole("button", { name: "Ada", exact: true })).toHaveAttribute("aria-pressed", "true");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Logged: Watched").first()).toBeVisible();
  await page.goto(`/history?person=${ada}`);
  await hydrated(page);
  await expect(page.getByTestId("activity-row").filter({ hasText: title })).toHaveCount(1);
});

test("game groups: create, log a play with the group, count it, edit and delete (WP4-6)", async ({ page }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  await signUp(page, `pgg${stamp}`, "Gil");
  const title = `Saltmarsh Keep ${stamp}`;
  const file = [
    JSON.stringify({ format: "library-export", version: 1 }),
    JSON.stringify({ source_id: "1", kind: "board_game", title, year: 2019, status: "planned" }),
  ].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  await person(page, "Ada");
  await person(page, "Bo");

  await page.goto("/people");
  await hydrated(page);
  await page.getByRole("tab", { name: "Game groups 0" }).click();
  await expect(page.getByText("No game groups yet")).toBeVisible();
  await page.getByRole("button", { name: "New group" }).click();
  const form = page.getByRole("dialog", { name: "New game group" });
  await form.getByLabel("Name", { exact: true }).fill("Friday table");
  await form.getByRole("button", { name: "Ada", exact: true }).click();
  await form.getByRole("button", { name: "Bo", exact: true }).click();
  await form.getByRole("button", { name: "Save group" }).click();
  await expect(page.getByText("Friday table created.")).toBeVisible();
  const group = page.getByTestId("pp-group-card").filter({ hasText: "Friday table" });
  await expect(group).toContainText("With Ada, Bo");
  await expect(group.getByTestId("pp-group-plays")).toHaveText("0");
  await expect(group).toContainText("Not played as a group yet.");
  await expectNoA11yViolations(page, "game groups");

  // "Log a play with": the palette picks the game, the play sheet opens with the group
  await group.getByRole("button", { name: "Log a play with Friday table" }).click();
  const palette = page.getByRole("dialog", { name: "Search" });
  await expect(palette.getByTestId("palette-target")).toHaveText("Choosing a game logs a play with Friday table.");
  await palette.getByRole("combobox").fill(title);
  const option = palette.getByRole("option", { name: new RegExp(`^${title}`) }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page).toHaveURL(/\/items\/.+\?log=1&group=/);
  const sheet = page.getByRole("dialog", { name: "Log a play" });
  await expect(sheet).toBeVisible();
  await expect(sheet.getByRole("group", { name: "Groups" }).getByRole("button", { name: "Friday table" })).toHaveAttribute("aria-pressed", "true");
  for (const name of ["Gil", "Ada", "Bo"]) {
    await expect(sheet.getByRole("button", { name, exact: true })).toHaveAttribute("aria-pressed", "true");
  }
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();

  await page.goto("/people");
  await hydrated(page);
  await page.getByRole("tab", { name: "Game groups 1" }).click();
  await expect(group.getByTestId("pp-group-plays")).toHaveText("1");
  await expect(group).toContainText(`Last: ${title}, Today`);

  // edit: rename, Bo leaves
  await page.getByRole("button", { name: "Actions for Friday table" }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  const edit = page.getByRole("dialog", { name: "Edit Friday table" });
  await edit.getByLabel("Name", { exact: true }).fill("Game night");
  await edit.getByRole("button", { name: "Bo", exact: true }).click();
  await edit.getByRole("button", { name: "Save group" }).click();
  await expect(page.getByText("Game night saved.")).toBeVisible();
  const renamed = page.getByTestId("pp-group-card").filter({ hasText: "Game night" });
  await expect(renamed).toContainText("With Ada");
  await expect(renamed).not.toContainText("Bo");

  // delete: the play stays in the history
  await page.getByRole("button", { name: "Actions for Game night" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  const confirm = page.getByRole("dialog", { name: "Delete Game night?" });
  await expect(confirm).toContainText("Its play stays in your history.");
  await confirm.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByText("Game night deleted.")).toBeVisible();
  await expect(page.getByTestId("pp-group-card")).toHaveCount(0);
  await page.setViewportSize({ width: 360, height: 800 });
  await expectReflow(page, "groups 360");
  await page.goto("/history");
  await hydrated(page);
  await expect(page.getByTestId("activity-row").filter({ hasText: title })).toHaveCount(1);
});

test("a game group chip fills the players and the play counts for the group (WP4-6)", async ({ page }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  await signUp(page, `grp${stamp}`, "Group Tester");
  // only a game: this test needs no show seasons (and adds no catalog refresh to the lane)
  const title = `Meeple Harbour ${stamp}`;
  const file = [JSON.stringify({ format: "library-export", version: 1 }), JSON.stringify({ source_id: "1", kind: "board_game", title, year: 2017, status: "planned" })].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  const gameId = lib.items.find((i: { item: { title: string } }) => i.item.title === title).item.id as string;
  const person = async (name: string) => {
    const r = await page.request.post("/api/v1/companions", { data: { name } });
    expect(r.status()).toBe(201);
    return (await r.json()).id as string;
  };
  const [ann, bo] = [await person("Ann"), await person("Bo")];
  const created = await page.request.post("/api/v1/game-groups", { data: { name: "Harbour crew", members: [ann, bo] } });
  expect(created.ok(), `group: ${created.status()}`).toBeTruthy();
  const group = await created.json();

  await page.goto(`/items/${gameId}`);
  await hydrated(page);
  await page.getByTestId("action-bar").getByRole("button", { name: "Log a play" }).click();
  const sheet = page.getByRole("dialog", { name: "Log a play" });
  const chips = sheet.getByRole("group", { name: "Groups" });
  const chip = chips.getByRole("button", { name: "Harbour crew", exact: true });
  await expect(chip).toHaveAttribute("aria-pressed", "false");
  await chip.click();
  await expect(chip).toHaveAttribute("aria-pressed", "true");
  for (const name of ["Group Tester", "Ann", "Bo"]) {
    await expect(sheet.getByRole("button", { name, exact: true })).toHaveAttribute("aria-pressed", "true");
  }
  await expectNoA11yViolations(page, "play sheet with groups");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();

  const sessions = await (await page.request.get("/api/v1/sessions")).json();
  expect((sessions[0].participants as { name: string }[]).map((p) => p.name).sort()).toEqual(["Ann", "Bo", "Group Tester"]);
  const groups = await (await page.request.get("/api/v1/game-groups")).json();
  expect(groups.find((g: { id: string }) => g.id === group.id)).toMatchObject({ plays: 1 });
});

test("the actions of a person card share one size", async ({ page }) => {
  // OT-12: "History" was a hand-styled link (36 px, 14 px text) beside two small buttons
  await signUp(page, `psize${Date.now()}`, "Sia");
  await person(page, "Anna");
  await page.goto("/people");
  await hydrated(page);
  const a = card(page, "Anna");
  const actions = [
    a.getByRole("link", { name: "History with Anna" }),
    a.getByRole("button", { name: "Invite Anna" }),
    a.getByRole("button", { name: "Log with Anna" }),
  ];
  const looks = [];
  for (const action of actions) {
    await expect(action).toBeVisible();
    looks.push(
      await action.evaluate((el) => {
        const s = getComputedStyle(el);
        return { height: el.getBoundingClientRect().height, font: s.fontSize, weight: s.fontWeight, radius: s.borderRadius, border: s.borderTopWidth };
      }),
    );
  }
  expect(looks[0]).toEqual(looks[1]);
  expect(looks[2]).toEqual(looks[1]);
  // the link keeps a visible keyboard focus
  await actions[0].focus();
  await expectNoA11yViolations(page, "people card actions");
});

test("card actions grow to 40 px on a phone, all of them", async ({ page }) => {
  // OT-12 card-footer rule: one size per footer, 32 px with a pointer, 40 px below 640 px
  await page.setViewportSize({ width: 390, height: 800 });
  await signUp(page, `pfoot${Date.now()}`, "Fia");
  await person(page, "Anna");
  await page.goto("/people");
  await hydrated(page);
  const a = card(page, "Anna");
  for (const action of [
    a.getByRole("link", { name: "History with Anna" }),
    a.getByRole("button", { name: "Invite Anna" }),
    a.getByRole("button", { name: "Log with Anna" }),
  ]) {
    await expect(action).toBeVisible();
    expect((await action.boundingBox())!.height).toBe(40);
  }
  await expectReflow(page, "people card 390");
});
