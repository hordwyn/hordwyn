import { test, expect, type Page } from "@playwright/test";

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
