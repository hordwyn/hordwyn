import { test, expect, type Page } from "./fixtures";
import { E2E_CONTACT, expectNoA11yViolations, hydrated, signUp } from "./helpers";

// v1-wikidata (docs/fable/round2/v1-plan.md 4b): board games come from Wikidata (the mock in
// tests/wikidata-mock.mjs serves the recorded answers; the operator contact the source needs
// is set in global.setup.ts). A search in the Play scope finds Catan; its page says players,
// playing time and age in words, shows the year, credits Wikidata (CC0) and links to the item
// on Wikidata and on BoardGameGeek; there is no picture, and nothing is asked of a Wikimedia
// host by the browser. A play with scores is logged and Stats counts it. Without the contact:
// contact.spec.ts (it runs after every other spec).

const MOCK = `http://127.0.0.1:${process.env.WIKIDATA_MOCK_PORT ?? 3016}`;
const bar = (page: Page) => page.getByTestId("action-bar");
type Asked = { path: string; words: string; ids: string[]; userAgent: string };
const asked = async (page: Page) => (await (await page.request.get(`${MOCK}/requests`)).json()) as Asked[];

test("a board game from Wikidata: facts in words, the credit, two links and no picture", async ({ page }) => {
  test.setTimeout(180_000);
  const wikimedia: string[] = [];
  page.on("request", (r) => {
    const host = new URL(r.url()).hostname;
    if (/(^|\.)(wikimedia|wikidata|wikipedia)\.org$/.test(host)) wikimedia.push(r.url());
  });
  await signUp(page, `game${Date.now()}`, "Game Tester");
  // the mock notes every request since it started; this spec reads only its own (WD-8)
  const before = (await asked(page)).length;
  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog", { name: "Search" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("radio", { name: "Play" }).click();
  await dialog.getByRole("combobox").fill("Catan");
  // the source's answer arrives over SSE after the local one
  const option = dialog.getByRole("option", { name: /The Settlers of Catan \(1995\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();

  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: "The Settlers of Catan" })).toBeVisible();
  await expect(page.locator("article header")).toContainText("1995");
  // no picture: the title box (DEC-642)
  await expect(page.locator('img[alt="Cover of The Settlers of Catan"]')).toHaveCount(0);
  // a game without a play opens on About: the facts in words, not four raw numbers
  const about = page.getByRole("tabpanel", { name: "About" });
  await expect(about.getByText("3-4 players", { exact: true })).toBeVisible();
  await expect(about.getByText("75 min", { exact: true })).toBeVisible();
  await expect(about.getByText("from 10 years", { exact: true })).toBeVisible();
  await expect(about.getByText("Minimum players")).toHaveCount(0);
  const credit = page.getByTestId("attribution");
  await expect(credit).toContainText("Data from Wikidata (CC0).");
  await expect(credit.getByRole("link", { name: "https://www.wikidata.org/" })).toHaveAttribute("href", "https://www.wikidata.org/");
  const links = page.getByTestId("item-links");
  await expect(links).toContainText("Elsewhere:");
  await expect(links.getByRole("link", { name: "Wikidata", exact: true })).toHaveAttribute("href", "https://www.wikidata.org/wiki/Q17271");
  await expect(links.getByRole("link", { name: "BoardGameGeek", exact: true })).toHaveAttribute("href", "https://boardgamegeek.com/boardgame/13");
  await expectNoA11yViolations(page, "board game page");

  // the source was asked, for this game, with the operator contact in the User-Agent: the one
  // global.setup.ts sets, or the one books.spec sets for a while beside this spec
  const mine = (await asked(page)).slice(before).filter((r) => r.words === "Catan" || r.ids.includes("Q17271"));
  expect(mine.length).toBeGreaterThan(0);
  for (const r of mine) {
    expect(r.path).toBe("/w/api.php");
    expect([`(${E2E_CONTACT})`, "(books@example.org)"].some((c) => r.userAgent.endsWith(c)), r.userAgent).toBe(true);
  }

  // a play with a person and scores; Stats counts the play and its time
  await page.getByRole("button", { name: "Want to play" }).click();
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  const sheet = page.getByRole("dialog", { name: "Log a play" });
  await expect(sheet).toBeVisible();
  const people = sheet.getByRole("combobox", { name: "Add a person" });
  await people.fill("Ben");
  await people.press("Enter");
  await sheet.getByLabel("Score of Game Tester").fill("10");
  await sheet.getByLabel("Score of Ben").fill("8");
  await sheet.getByLabel("Duration", { exact: true }).fill("2:00");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  await expect(bar(page)).toContainText("One play");

  await page.goto("/stats");
  await hydrated(page);
  await expect(page.getByTestId("banked-tile-games").locator("dd")).toHaveText(["2 hours", "One play"]);
  expect(wikimedia).toEqual([]);
});

// LB-33 (v1-release item 8): a game Wikidata does not have is added by hand from the same
// search, and its page has the form of a game from Wikidata where that makes sense: it opens
// on About (no play yet), its next step is "Log a play", and a play is counted the same way.
// It has no facts in words (none were given), no credit of Wikidata and no links to it.
test("a game Wikidata does not have is added by hand in the same search and gets the form of a game", async ({ page }) => {
  test.setTimeout(180_000);
  const stamp = Date.now();
  const title = `Marsh Pieces ${stamp}`;
  await signUp(page, `handgame${stamp}`, "Hand Gamer");
  const before = (await asked(page)).length;
  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog", { name: "Search" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("radio", { name: "Play" }).click();
  await dialog.getByRole("combobox").fill(title);
  // the source answered (nothing) and the by-hand row is the last one
  const offer = dialog.getByRole("option", { name: `Add "${title}" by hand...` });
  await expect(offer).toBeVisible({ timeout: 20_000 });
  await expect(dialog.getByRole("option").last()).toHaveAttribute("id", (await offer.getAttribute("id"))!);
  await expect.poll(async () => (await asked(page)).slice(before).some((r) => r.words === title), { timeout: 20_000 }).toBe(true);
  await offer.click();
  const add = page.getByRole("dialog", { name: "Add by hand" });
  await expect(add).toBeVisible();
  await add.getByLabel("Kind").selectOption({ label: "Board game" });
  await add.getByLabel("Year").fill("2021");
  await add.getByRole("button", { name: "Add to library" }).click();

  await expect(page).toHaveURL(/\/items\/[0-9a-f-]{36}$/);
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
  await expect(page.getByTestId("ip-kind")).toHaveText(/Board game/);
  await expect(page.locator("article header")).toContainText("2021");
  // like a game from Wikidata without a play: About first, then "Log a play"
  await expect(page.getByRole("tab", { name: "About" })).toHaveAttribute("aria-selected", "true");
  const about = page.getByRole("tabpanel", { name: "About" });
  await expect(about.getByText("No details yet.")).toBeVisible();
  await expect(about.getByText("Minimum players")).toHaveCount(0);
  await expect(bar(page).getByRole("button", { name: "Log a play" })).toBeVisible();
  await expect(page.getByTestId("item-links")).toHaveCount(0);
  await expect(page.getByTestId("attribution").filter({ hasText: "Wikidata" })).toHaveCount(0);
  await expectNoA11yViolations(page, "board game added by hand");

  // a play with a score is counted like any other
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  const sheet = page.getByRole("dialog", { name: "Log a play" });
  await expect(sheet).toBeVisible();
  await sheet.getByLabel("Score of Hand Gamer").fill("12");
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  await expect(bar(page)).toContainText("One play");
});
