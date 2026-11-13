import { test, expect, type Page } from "./fixtures";
import { readFileSync } from "node:fs";
import { expectNoA11yViolations, expectReflow, hydrated, signUp } from "./helpers";

// WP3-7 Settings > Imports and export (design.md 6.11, R-35): upload the legacy fixture,
// read its preview (rows per kind, samples, mapping, what is lost), run it, read the result,
// download both exports; a Trakt file's unmatched row is matched through the search palette
// and imported; axe and reflow at 360 px on the preview.

const repo = `${require("node:path").resolve(__dirname, "../../..")}/`;
const LEGACY = `${repo}src/rust/crates/testkit/fixtures/import/legacy/export.json`;
const TRAKT = `${repo}src/rust/crates/importers/fixtures/trakt/history.json`;

async function open(page: Page) {
  await page.goto("/settings/imports");
  await hydrated(page);
}

/** The newest import's panel (the first on the page). */
const latest = (page: Page) => page.getByTestId("imp-panel").first();

async function upload(page: Page, card: string, file: string) {
  await page.getByTestId(`imp-card-${card}`).locator('input[type="file"]').setInputFiles(file);
  await expect(latest(page)).toHaveAttribute("data-state", "previewed", { timeout: 30_000 });
}

test("the legacy export: preview, run, result and both exports", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `imp${Date.now()}`, "Import Tester");
  await open(page);
  await expect(page.getByRole("heading", { level: 1, name: "Imports and export" })).toBeVisible();
  await expect(page.getByTestId("imp-card-goodreads")).toContainText("Waiting for a sample");
  await expect(page.getByTestId("imp-trakt-credit")).toContainText("uses Trakt but is not endorsed or certified by Trakt");
  // the hidden file input shows its keyboard focus on its label (UX3-7, WCAG 2.4.7)
  const pick = page.getByTestId("imp-card-legacy").locator('input[type="file"]');
  const label = page.getByTestId("imp-card-legacy").locator("label", { hasText: "Choose a file" });
  const ring = () => label.evaluate((el) => getComputedStyle(el).boxShadow);
  expect(await ring()).toBe("none");
  await pick.focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  await expect(pick).toBeFocused();
  await expect.poll(ring).not.toBe("none");

  await upload(page, "legacy", LEGACY);
  const panel = latest(page);
  await expect(panel.getByTestId("imp-summary")).toContainText("21 rows with 82 history entries");
  const kinds = panel.getByTestId("imp-kinds");
  await expect(kinds.getByRole("row", { name: /Movie/ })).toContainText("8");
  await expect(kinds.getByRole("row", { name: /Video game/ })).toContainText("3");
  await expect(panel.getByTestId("imp-samples").getByRole("listitem")).toHaveCount(5);
  await expect(panel.getByTestId("imp-mapping")).toContainText("seen_history");
  await expect(panel.getByTestId("imp-lost")).toContainText("31 views without a date");
  await expect(panel).toContainText("Lists: Fast and Furious, Monitoring, Owned, WatchWithPartner");
  // nothing is in the library yet
  const before = await (await page.request.get("/api/v1/library?limit=5")).json();
  expect(before.total).toBe(0);

  await panel.getByRole("button", { name: "Import 21 rows" }).click();
  await expect(latest(page)).toHaveAttribute("data-state", "done", { timeout: 30_000 });
  await expect(latest(page).getByTestId("imp-result")).toHaveText("Imported 21, skipped 0, without a match 0.");
  await expect(page.getByTestId("imp-earlier")).toContainText("Checked, not imported");
  const after = await (await page.request.get("/api/v1/library?limit=5")).json();
  expect(after.total).toBe(21);

  for (const [id, name, first] of [
    ["imp-export-jsonl", "library-export.jsonl", '{"format":"library-export","version":1}'],
    ["imp-export-csv", "library-export.csv", "source_id,kind,title,year,status,rating,notes,external_ids,finished_dates,lists"],
  ]) {
    const download = page.waitForEvent("download");
    await page.getByTestId(id).click();
    const d = await download;
    expect(d.suggestedFilename()).toBe(name);
    const text = readFileSync(await d.path(), "utf8");
    expect(text.split("\n")[0]).toBe(first);
    expect(text).toContain("Fast and Furious");
  }

  // the History page links here
  await page.goto("/history");
  await hydrated(page);
  await page.getByTestId("hist-export").click();
  await expect(page).toHaveURL(/\/settings\/imports#export$/);
});

test("an unmatched Trakt row is matched through the palette and imported", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `impm${Date.now()}`, "Match Tester");
  await open(page);
  await upload(page, "trakt", TRAKT);
  await expect(latest(page).getByTestId("imp-summary")).toContainText("One row has no id to match");
  await latest(page).getByRole("button", { name: "Import 3 rows" }).click();
  await expect(latest(page)).toHaveAttribute("data-state", "done", { timeout: 30_000 });
  await expect(latest(page).getByTestId("imp-result")).toHaveText("Imported 2, skipped 0, without a match 1.");
  const row = latest(page).getByTestId("imp-unmatched").getByRole("listitem");
  await expect(row).toContainText("A Home Movie");

  // reloading keeps the list
  await open(page);
  await latest(page).getByRole("button", { name: "Find a match for A Home Movie" }).click();
  const palette = page.getByRole("dialog", { name: "Search" });
  await expect(palette.getByTestId("palette-match")).toContainText('"A Home Movie"');
  await palette.getByRole("combobox").fill("fight club");
  const option = palette.getByRole("option", { name: /Zombie Fight Club/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page.getByText("Matched and imported.")).toBeVisible();
  await expect(latest(page).getByTestId("imp-result")).toHaveText("Imported 3, skipped 0, without a match 0.");
  const lib = await (await page.request.get("/api/v1/library?limit=10")).json();
  expect(lib.total).toBe(3);
});

test.describe("at 360 px", () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test("the page and a preview have no violations and reflow", async ({ page }) => {
    test.setTimeout(90_000);
    await signUp(page, `impa${Date.now()}`, "Axe Tester");
    await open(page);
    await expectNoA11yViolations(page, "imports empty");
    await expectReflow(page, "imports empty");
    await upload(page, "legacy", LEGACY);
    await expectNoA11yViolations(page, "imports preview");
    await expectReflow(page, "imports preview");
  });
});
