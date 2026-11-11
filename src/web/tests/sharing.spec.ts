import { test, expect, type Browser, type Page } from "@playwright/test";
import { addPerson, hydrated, PASSWORD } from "./helpers";

// M15 acceptance, on the People page of UX phase 3 (WP3-6, design.md 5.10, 5.11, 6.7):
// promote a person by invite (the dialog states the consequence, the single-use link has
// Copy), the invited person signs up and sees the shared history (and copies it to their
// log), then the owner unlinks and merges (and undoes the merge from its record); the
// owner's history stays unchanged throughout. A link request is answered from the request
// card; the share dialog's three choices make, replace and revoke the public link.

async function signUp(page: Page, path: string, user: string, display: string) {
  await page.goto(path);
  await hydrated(page);
  await page.getByLabel("Username").fill(user);
  await page.getByLabel("Display name").fill(display);
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/$/);
  await hydrated(page);
}

async function people(page: Page) {
  await page.goto("/people");
  await hydrated(page);
}

function card(page: Page, name: string) {
  return page.getByTestId("pp-card").filter({ has: page.getByRole("link", { name, exact: true }) });
}

async function historyRow(page: Page) {
  await page.goto("/history");
  await hydrated(page);
  const row = page.getByTestId("activity-row").filter({ hasText: "Fight Club" }).first();
  await expect(row).toContainText("Fight Club");
  return (await row.innerText()).trim();
}

async function menu(page: Page, name: string, item: string) {
  await page.getByRole("button", { name: `More actions for ${name}` }).click();
  await page.getByRole("menuitem", { name: item }).click();
}

async function merge(page: Page, keep: string, fold: string) {
  await menu(page, fold, "Merge with...");
  const dialog = page.getByRole("dialog", { name: "Merge duplicates" });
  await dialog.getByLabel("Keep").selectOption({ label: keep });
  await expect(dialog.getByTestId("pp-merge-fold")).toHaveText(fold);
  await expect(dialog.getByRole("status")).toContainText(`${fold}'s sessions will show under ${keep}`);
  await dialog.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByText(`Merged ${fold} into ${keep}.`)).toBeVisible();
}

test("promote, accept, see history, unlink, merge and undo the merge", async ({ page, browser }) => {
  const stamp = Date.now();
  await signUp(page, "/sign-up", `own${stamp}`, "Olive");

  await people(page);
  await addPerson(page, "Carl");
  await addPerson(page, "Karl");

  // watch a movie with both
  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("combobox").fill("fight club");
  const option = dialog.getByRole("option", { name: /Fight Club \(1999\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  await page.getByRole("button", { name: "Want to watch" }).click();
  // the item page's "With..." sheet (design.md 5.4): people as toggle chips
  await page.getByRole("button", { name: "Watched with...: Fight Club" }).click();
  const sheet = page.getByRole("dialog", { name: "Watched", exact: true });
  await sheet.getByRole("button", { name: "Carl", exact: true }).click();
  await sheet.getByRole("button", { name: "Karl", exact: true }).click();
  await sheet.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Logged: Watched")).toBeVisible();

  const before = await historyRow(page);
  expect(before).toContain("Carl");
  expect(before).toContain("Karl");

  // promote Carl: the invite dialog says what happens and has the single-use link
  await people(page);
  await expect(card(page, "Carl")).toContainText("No account");
  await page.getByRole("button", { name: "Invite Carl" }).click();
  const invite = page.getByRole("dialog", { name: "Invite Carl" });
  await expect(invite).toContainText("Carl gets an account on this instance, linked to this entry.");
  await expect(invite).toContainText("Nothing in your log changes.");
  const link = invite.getByLabel("Single-use link, valid 14 days");
  await expect(link).toHaveValue(/\/invite\//);
  await expect(invite.getByRole("button", { name: "Copy" })).toBeVisible();
  const inviteUrl = await link.inputValue();
  await invite.getByRole("button", { name: "Close" }).last().click();

  // Carl accepts in another browser and sees the session he was part of
  const carlContext = await browser.newContext();
  const carl = await carlContext.newPage();
  await signUp(carl, inviteUrl, `carl${stamp}`, "Carl C.");
  await carl.goto("/history");
  await hydrated(carl);
  const shared = carl.getByTestId("activity-row").filter({ hasText: "by Olive" }).first();
  await expect(shared).toContainText("Fight Club");
  await shared.getByRole("button", { name: "Copy to my log" }).click();
  await expect(carl.getByText("Copied to your log.")).toBeVisible();
  // the copy is a row of Carl's own, next to the shared one
  await expect(carl.getByTestId("activity-row").filter({ hasText: "Fight Club" })).toHaveCount(2);
  await expect(carl.getByTestId("activity-row").filter({ hasText: "Fight Club" }).filter({ hasNotText: "by Olive" })).toHaveCount(1);
  await carlContext.close();

  // the owner sees Carl linked, then unlinks him
  await people(page);
  await expect(card(page, "Carl")).toContainText(`Linked to @carl${stamp}`);
  await page.getByRole("tab", { name: /^Linked accounts/ }).click();
  await expect(page.getByRole("tabpanel", { name: /^Linked accounts/ }).getByTestId("pp-card")).toHaveCount(1);
  await page.getByRole("tab", { name: /^All/ }).click();
  await menu(page, "Carl", "Unlink");
  const confirm = page.getByRole("dialog", { name: "Unlink Carl?" });
  await expect(confirm).toContainText("Your history stays as it is.");
  await confirm.getByRole("button", { name: "Unlink" }).click();
  await expect(page.getByText("Carl is no longer linked.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Invite Carl" })).toBeVisible();

  // merge the duplicate Karl into Carl, undo it from the merge record, merge again
  await merge(page, "Carl", "Karl");
  await expect(card(page, "Karl")).toHaveCount(0);
  await page.getByRole("button", { name: "Undo merge" }).click();
  await expect(page.getByText("Karl is a separate entry again.")).toBeVisible();
  await expect(card(page, "Karl")).toHaveCount(1);
  await merge(page, "Carl", "Karl");
  await expect(card(page, "Karl")).toHaveCount(0);
  // Carl's page lists the merged entry with its own Undo
  await card(page, "Carl").getByRole("link", { name: "Carl", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Carl" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Undo merge: Karl" })).toBeVisible();

  // history unchanged
  expect(await historyRow(page)).toBe(before);
});

async function member(browser: Browser, user: string, display: string) {
  const context = await browser.newContext();
  const page = await context.newPage();
  await signUp(page, "/sign-up", user, display);
  return { context, page };
}

test("a link request is accepted from the request card", async ({ page, browser }) => {
  const stamp = Date.now();
  await signUp(page, "/sign-up", `req${stamp}`, "Rita");
  const bo = await member(browser, `bo${stamp}`, "Bo");
  await people(page);
  await addPerson(page, "Bo");
  await page.getByRole("button", { name: "More actions for Bo" }).click();
  await page.getByRole("menuitem", { name: "Link to account" }).click();
  const dialog = page.getByRole("dialog", { name: "Link Bo to an account" });
  await dialog.getByLabel("Their username").fill(`nobody${stamp}`);
  await dialog.getByRole("button", { name: "Send link request" }).click();
  await expect(dialog.getByText("Not found. Check the username.")).toBeVisible();
  await dialog.getByLabel("Their username").fill(`bo${stamp}`);
  await dialog.getByRole("button", { name: "Send link request" }).click();
  await expect(page.getByText("Link request sent.")).toBeVisible();
  await expect(card(page, "Bo")).toContainText("Link request sent");

  await people(bo.page);
  const request = bo.page.getByTestId("pp-request");
  await expect(request).toContainText('Rita says their entry "Bo" is you.');
  await expect(request).toContainText("If you accept, Rita sees the plays and watches you share");
  await request.getByRole("button", { name: "Accept the request from Rita" }).click();
  await expect(bo.page.getByText("You are now linked with Rita.")).toBeVisible();
  await expect(bo.page.getByTestId("pp-request")).toHaveCount(0);
  await bo.context.close();

  await people(page);
  await expect(card(page, "Bo")).toContainText(`Linked to @bo${stamp}`);
});

test("the share dialog makes, replaces and revokes the public link", async ({ page, browser }) => {
  await signUp(page, "/sign-up", `lst${Date.now()}`, "Lena");
  await page.goto("/lists");
  await hydrated(page);
  await page.getByRole("button", { name: "New list" }).click();
  await page.getByRole("dialog", { name: "New list" }).getByLabel("List name").fill("Weekend picks");
  await page.getByRole("button", { name: "Create list" }).click();
  await expect(page).toHaveURL(/\/lists\/[0-9a-f-]+$/);
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: "Weekend picks" })).toBeVisible();

  const open = async () => {
    await page.getByRole("button", { name: "Share Weekend picks" }).click();
    return page.getByRole("dialog", { name: 'Share "Weekend picks"' });
  };
  let dialog = await open();
  await expect(dialog.getByRole("radio", { name: "Private" })).toBeChecked();
  await expect(dialog.getByRole("radio", { name: "Anyone with the link" })).toHaveAccessibleDescription(
    "No account needed. Your name and the items are visible; your ratings and notes are not.",
  );
  await dialog.getByRole("radio", { name: "Anyone with the link" }).check();
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog.getByRole("status")).toHaveText("Saved: anyone with the link can see this list. Copy the link below.");
  const field = dialog.getByLabel("Public link");
  await expect(field).toHaveValue(/\/shared\/lists\//);
  const first = await field.inputValue();

  const anon = await browser.newContext();
  const visitor = await anon.newPage();
  expect((await visitor.goto(first))?.status()).toBe(200);
  await expect(visitor.getByRole("heading", { level: 1, name: "Weekend picks" })).toBeVisible();
  await expect(visitor.getByText("by Lena")).toBeVisible();
  // the return path is one encoded query value (UX3-19)
  const token = new URL(first).pathname.split("/").pop();
  await expect(visitor.getByRole("link", { name: "Sign in to add these to your library" })).toHaveAttribute(
    "href",
    `/sign-in?next=%2Fshared%2Flists%2F${token}`,
  );

  // a new link replaces the old one, and says so
  await dialog.getByRole("button", { name: "Create a new link" }).click();
  await expect(dialog.getByRole("status")).toHaveText("A new link replaces the old one; the old link no longer works.");
  await expect(field).not.toHaveValue(first);
  const second = await field.inputValue();
  // a replaced link answers 404, so link checkers drop it (UX3-16)
  expect((await visitor.goto(first))?.status()).toBe(404);
  await expect(visitor.getByRole("heading", { level: 1, name: "This list is not shared any more" })).toBeVisible();

  // private again revokes it
  await dialog.getByRole("radio", { name: "Private" }).check();
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog.getByRole("status")).toHaveText("Saved: the link no longer works.");
  await expect(dialog.getByLabel("Public link")).toHaveCount(0);
  await dialog.getByRole("button", { name: "Close" }).last().click();
  await expect(page.getByText("Private", { exact: true }).first()).toBeVisible();
  dialog = await open();
  await expect(dialog.getByRole("radio", { name: "Private" })).toBeChecked();
  expect((await visitor.goto(second))?.status()).toBe(404);
  await expect(visitor.getByRole("heading", { level: 1, name: "This list is not shared any more" })).toBeVisible();
  await anon.close();
});
