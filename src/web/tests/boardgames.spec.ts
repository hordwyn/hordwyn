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
