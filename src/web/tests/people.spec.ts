import { test, expect, type Page } from "@playwright/test";
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
