import { test, expect, type Page } from "@playwright/test";

// M13: a link request reaches the other person's inbox; channels and subscriptions can be
// set up on the notifications page.

async function hydrated(page: Page) {
  await expect(page.locator("body[data-hydrated]")).toBeAttached({ timeout: 20_000 });
}

async function signUp(page: Page, user: string, display: string) {
  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill(user);
  await page.getByLabel("Display name").fill(display);
  await page.getByLabel("Password").fill("correct horse battery");
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/library$/);
}

test("link requests arrive in the inbox; channels and subscriptions", async ({ page, browser }) => {
  const stamp = Date.now();
  const other = await browser.newContext();
  const pia = await other.newPage();
  await signUp(pia, `pia${stamp}`, "Pia");

  await signUp(page, `max${stamp}`, "Max");
  await page.goto("/people");
  await hydrated(page);
  await page.getByLabel("Name", { exact: true }).fill("P.");
  await page.getByRole("button", { name: "Add companion" }).click();
  await page.getByLabel("Companion", { exact: true }).selectOption({ label: "P." });
  await page.getByLabel("Username").fill(`pia${stamp}`);
  await page.getByRole("button", { name: "Send link request" }).click();
  await expect(page.getByText("Link request sent.")).toBeVisible();

  await pia.goto("/notifications");
  await hydrated(pia);
  const inbox = pia.getByRole("list", { name: "Inbox" });
  await expect(inbox.getByRole("listitem").filter({ hasText: 'Max says you are "P."' })).toBeVisible();
  // (other inbox items may exist: the first account is admin and hears about sign-ups)
  const markRead = pia.getByRole("button", { name: `Mark as read Max says you are "P."` });
  await markRead.click();
  await expect(markRead).toHaveCount(0);

  // a signed webhook channel shows its generated secret once
  await pia.getByLabel("Type", { exact: true }).selectOption({ label: "Webhook (signed)" });
  await pia.getByLabel("Name", { exact: true }).fill("Automation");
  await pia.getByLabel("URL", { exact: true }).fill("https://hooks.example.org/in");
  await pia.getByRole("button", { name: "Add channel" }).click();
  await expect(pia.getByText(/signing secret is shown only once: whsec_/)).toBeVisible();
  await expect(
    pia.getByRole("list", { name: "Channels" }).getByRole("listitem").filter({ hasText: "Automation" }),
  ).toBeVisible();

  await pia.getByLabel("Event", { exact: true }).selectOption({ label: "All social events" });
  await pia.getByLabel("Delivery", { exact: true }).selectOption({ label: "Daily digest" });
  await pia.getByRole("button", { name: "Subscribe", exact: true }).click();
  await expect(pia.getByText("Subscribed.")).toBeVisible();
  await expect(
    pia.getByRole("list", { name: "Subscriptions" }).getByRole("listitem").filter({ hasText: "social.*" }),
  ).toContainText("Daily digest (0 8 * * *)");
  await other.close();
});
