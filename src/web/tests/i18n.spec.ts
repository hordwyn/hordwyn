import { test, expect, type Page } from "@playwright/test";
import { importDone } from "./helpers";

// Per-user language (owner ruling DEC-171, DEC-201): the account menu switches the language,
// the server renders the page in it and sets `<html lang>`; "Automatic" goes back.

async function hydrated(page: Page) {
  await expect(page.locator("body[data-hydrated]")).toBeAttached({ timeout: 20_000 });
}

test("the account menu switches the language for this account", async ({ page }) => {
  const user = `lena${Date.now()}`;
  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill(user);
  await page.getByLabel("Password").fill("correct horse battery");
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/library$/);
  await hydrated(page);

  await page.getByRole("button", { name: `Account menu for ${user}` }).click();
  const deutsch = page.getByRole("menuitemradio", { name: "Deutsch" });
  await expect(deutsch).toHaveAttribute("aria-checked", "false");
  await expect(deutsch.locator("[lang=de]")).toHaveCount(1);
  await deutsch.click();
  await expect(page.locator("html")).toHaveAttribute("lang", "de");
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: "Bibliothek" })).toBeVisible();

  await page.getByRole("button", { name: `Kontomen\u00fc von ${user}` }).click();
  await page.getByRole("menuitemradio", { name: "Automatisch" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.getByRole("heading", { level: 1, name: "Library" })).toBeVisible();
});

test("the phase 1 pages render in German", async ({ page }) => {
  // UX1-11: the rendered catalogue, not only its parity
  test.setTimeout(120_000);
  const user = `jonas${Date.now()}`;
  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill(user);
  await page.getByLabel("Display name").fill("Jonas");
  await page.getByLabel("Password").fill("correct horse battery");
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/library$/);
  const file = [
    JSON.stringify({ format: "library-export", version: 1 }),
    JSON.stringify({ source_id: "1", kind: "movie", title: "Harbour Lights", year: 2021, status: "planned" }),
  ].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  const { id } = await started.json();
  await importDone(page, id);
  const lib = await (await page.request.get("/api/v1/library")).json();
  const item = lib.items[0].item.id as string;
  await hydrated(page);
  await page.getByRole("button", { name: /^Account menu for/ }).click();
  await page.getByRole("menuitemradio", { name: "Deutsch" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "de");

  for (const [path, h1] of [
    ["/", "Willkommen zur\u00fcck, Jonas"],
    ["/library", "Bibliothek"],
    ["/history", "Verlauf"],
    ["/stats", "Statistik"],
  ]) {
    await page.goto(path);
    await hydrated(page);
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await expect(page.getByRole("heading", { level: 1, name: h1 })).toBeVisible();
  }
  await page.goto(`/items/${item}`);
  await hydrated(page);
  await expect(page.locator("html")).toHaveAttribute("lang", "de");
  await expect(page.getByTestId("action-bar").getByRole("button", { name: /^Gesehen/ })).toBeVisible();
  await expect(page.getByRole("region", { name: "Angaben zum Eintrag" })).toBeVisible();
});
