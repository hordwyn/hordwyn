import { test, expect, type Page } from "@playwright/test";

// M15 acceptance: promote a companion by invite, the invited person signs up and sees the
// shared history (and copies it to their log), then the owner demotes and merges; the
// owner's history stays unchanged throughout.

async function hydrated(page: Page) {
  await expect(page.locator("body[data-hydrated]")).toBeAttached({ timeout: 20_000 });
}

async function signUp(page: Page, path: string, user: string, display: string) {
  await page.goto(path);
  await hydrated(page);
  await page.getByLabel("Username").fill(user);
  await page.getByLabel("Display name").fill(display);
  await page.getByLabel("Password").fill("correct horse battery");
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/library$/);
  await hydrated(page);
}

async function addCompanion(page: Page, name: string) {
  await page.getByLabel("Name", { exact: true }).fill(name);
  await page.getByRole("button", { name: "Add companion" }).click();
  await expect(page.getByRole("listitem").filter({ hasText: name })).toBeVisible();
}

async function historyRow(page: Page) {
  await page.goto("/history");
  await hydrated(page);
  const row = page.getByRole("list", { name: "Your sessions" }).getByRole("listitem").first();
  await expect(row).toContainText("Fight Club");
  return (await row.innerText()).trim();
}

test("promote, accept, see history, demote, merge", async ({ page, browser }) => {
  const stamp = Date.now();
  await signUp(page, "/sign-up", `own${stamp}`, "Olive");

  await page.goto("/people");
  await hydrated(page);
  await addCompanion(page, "Carl");
  await addCompanion(page, "Karl");

  // watch a movie with both
  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("combobox").fill("fight club");
  const option = dialog.getByRole("option", { name: /Fight Club \(1999\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  await page.getByRole("button", { name: "Add to library" }).click();
  await expect(page.getByText("In your library")).toBeVisible();
  await page.getByLabel("Carl", { exact: true }).check();
  await page.getByLabel("Karl", { exact: true }).check();
  await page.getByRole("button", { name: "Mark as done" }).click();
  await expect(page.getByText("Logged.")).toBeVisible();

  const before = await historyRow(page);
  expect(before).toContain("Carl");
  expect(before).toContain("Karl");

  // promote Carl: an invite link
  await page.goto("/people");
  await hydrated(page);
  await page.getByRole("button", { name: "Invite Carl" }).click();
  const link = page.getByTestId("invite-link");
  await expect(link).toContainText("/invite/");
  const inviteUrl = (await link.innerText()).trim();

  // Carl accepts in another browser and sees the session he was part of
  const carlContext = await browser.newContext();
  const carl = await carlContext.newPage();
  await signUp(carl, inviteUrl, `carl${stamp}`, "Carl C.");
  await carl.goto("/history");
  await hydrated(carl);
  const shared = carl.getByRole("list", { name: "Shared with you" }).getByRole("listitem").first();
  await expect(shared).toContainText("Fight Club");
  await expect(shared).toContainText("by Olive");
  await shared.getByRole("button", { name: "Copy to my log" }).click();
  await expect(carl.getByText("Copied to your log.")).toBeVisible();
  await expect(
    carl.getByRole("list", { name: "Your sessions" }).getByRole("listitem").first(),
  ).toContainText("Fight Club");
  await carlContext.close();

  // the owner sees Carl linked, then demotes (unlinks) him
  await page.goto("/people");
  await hydrated(page);
  const carlRow = page.getByRole("listitem").filter({ hasText: /^Carl/ });
  await expect(carlRow.getByText("Linked")).toBeVisible();
  await page.getByRole("button", { name: "Unlink Carl" }).click();
  await expect(page.getByRole("button", { name: "Invite Carl" })).toBeVisible();

  // merge the duplicate Karl into Carl
  await page.getByLabel("Keep").selectOption({ label: "Carl" });
  await page.getByLabel("Duplicate", { exact: true }).selectOption({ label: "Karl" });
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(
    page.getByRole("list", { name: "Companions" }).getByRole("listitem").filter({ hasText: "Karl" }),
  ).toHaveCount(0);

  // history unchanged
  expect(await historyRow(page)).toBe(before);
});

test("a list shared by link is readable without an account", async ({ page, browser }) => {
  await signUp(page, "/sign-up", `lst${Date.now()}`, "Lena");
  await page.goto("/lists");
  await hydrated(page);
  await page.getByLabel("List name").fill("Weekend picks");
  await page.getByRole("button", { name: "Create list" }).click();
  await page.getByRole("button", { name: "Share by link Weekend picks" }).click();
  const link = page.getByTestId("share-link");
  await expect(link).toContainText("/shared/lists/");
  const url = (await link.innerText()).trim();

  const anon = await browser.newContext();
  const visitor = await anon.newPage();
  await visitor.goto(url);
  await expect(visitor.getByRole("heading", { level: 1, name: "Weekend picks" })).toBeVisible();
  await expect(visitor.getByText("by Lena")).toBeVisible();
  await anon.close();
});
