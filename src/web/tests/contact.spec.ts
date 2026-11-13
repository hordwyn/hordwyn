import { test, expect, type Page } from "./fixtures";
import { ADMIN, E2E_CONTACT, expectNoA11yViolations, hydrated, signIn } from "./helpers";

// v1-wikidata (docs/fable/round2/v1-plan.md 4b point 8, 3.1.1): without the operator contact
// Wikidata is not asked at all; the sources page, the search footer and the admin's Home say
// why. The contact is an instance setting that the other specs need (board games), so this
// spec runs in its own project after all of them (playwright.config.ts) and puts it back.

const MOCK = `http://127.0.0.1:${process.env.WIKIDATA_MOCK_PORT ?? 3016}`;
const asked = async (page: Page) =>
  ((await (await page.request.get(`${MOCK}/requests`)).json()) as unknown[]).length;

test("without the operator contact Wikidata is not asked, and the sources page, the search and Home say why", async ({ page }) => {
  test.setTimeout(120_000);
  await signIn(page, ADMIN);
  await page.goto("/admin/sources");
  await hydrated(page);
  const form = page.getByTestId("operator-contact");
  await expect(form).toContainText("Wikidata is not asked without it");
  await form.getByLabel("Operator contact").fill("");
  await form.getByRole("button", { name: "Save" }).click();
  await expect(form.getByRole("status")).toContainText("Removed. Requests carry no contact now.");
  try {
    await page.reload();
    await hydrated(page);
    const row = page.getByTestId("src-row").filter({ hasText: "Wikidata" });
    await expect(row).toContainText("Board game");
    await expect(row).toContainText("Operator contact missing");

    // Home: the nudge names the source and leads to the contact field
    await page.goto("/");
    await hydrated(page);
    const nudge = page.getByTestId("nudge");
    await expect(nudge).toContainText("Wikidata is not asked until you set the operator contact");
    await expect(nudge.getByRole("link", { name: "Set the operator contact" })).toHaveAttribute("href", "/admin/sources");

    // the search in the Play scope does not ask the source, and its footer says why
    const before = await asked(page);
    await page.keyboard.press("Control+k");
    const dialog = page.getByRole("dialog", { name: "Search" });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("radio", { name: "Play" }).click();
    await dialog.getByRole("combobox").fill("Wingspan");
    await expect(dialog.getByTestId("no-results")).toBeVisible({ timeout: 20_000 });
    await expect(dialog.getByRole("list", { name: "Data sources" })).toContainText("Wikidata (Board game): Operator contact missing");
    // what the source cannot find can still be added by hand (LB-33, v1-release item 8)
    await expect(dialog.getByRole("option", { name: 'Add "Wingspan" by hand...' })).toBeVisible();
    await expectNoA11yViolations(page, "palette without the operator contact");
    expect(await asked(page)).toBe(before);
  } finally {
    // the contact goes back for any spec run after this one
    const r = await page.request.put("/api/v1/admin/settings", {
      data: { values: { "instance.operator_contact": E2E_CONTACT } },
    });
    expect(r.ok(), `operator contact ${r.status()}`).toBeTruthy();
  }
});
