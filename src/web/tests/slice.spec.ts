import { test, expect, type Page } from "@playwright/test";

// M07 acceptance: sign up -> search a movie (TMDB fixtures via the mock) -> add -> mark watched
// with a companion -> appears in the library.

async function hydrated(page: Page) {
  await expect(page.locator("body[data-hydrated]")).toBeAttached({ timeout: 20_000 });
}

test("sign up, find a movie, watch it with a companion", async ({ page }) => {
  const user = `e2e${Date.now()}`;
  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill(user);
  await page.getByLabel("Display name").fill("E2E User");
  await page.getByLabel("Password").fill("correct horse battery");
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/library$/);
  await hydrated(page);
  await expect(page.getByText("Your library is empty")).toBeVisible();
  // the shell knows the session (owner bug: "Create account" after sign-in)
  await expect(page.getByRole("link", { name: "Create account" })).toHaveCount(0);
  const account = page.getByRole("button", { name: "Account menu for E2E User" });
  await account.click();
  await expect(page.getByRole("menuitemradio", { name: "System" })).toHaveAttribute("aria-checked", "true");
  await page.keyboard.press("Escape");
  await expect(account).toBeFocused();

  await page.goto("/people");
  await hydrated(page);
  await page.getByLabel("Name", { exact: true }).fill("Ben");
  await page.getByRole("button", { name: "Add person" }).click();
  await expect(page.getByRole("listitem").filter({ hasText: "Ben" })).toBeVisible();

  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("combobox").fill("fight club");
  // results of the TMDB connector arrive over SSE after the local answer
  const option = dialog.getByRole("option", { name: /Fight Club \(1999\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();

  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: "Fight Club" })).toBeVisible();
  await expect(page.getByTestId("attribution")).toContainText("uses TMDB and the TMDB APIs");
  await page.getByRole("button", { name: "Add to library" }).click();
  await expect(page.getByText("In your library")).toBeVisible();
  await page.getByLabel("Ben").check();
  await page.getByRole("button", { name: "Mark as completed" }).click();
  await expect(page.getByText("Logged.")).toBeVisible();

  await page.goto("/library");
  await hydrated(page);
  const card = page.getByRole("listitem").filter({ hasText: "Fight Club" });
  await expect(card).toBeVisible();
  await expect(card).toContainText("Completed");
});
