import { test, expect, type Page } from "@playwright/test";
import { ADMIN, PASSWORD, expectNoA11yViolations, expectReflow, hydrated, signIn, signUp } from "./helpers";

// The phase 2 behaviours DEC-281 left for phase 3 (WP3-4): every settings page reflows and
// passes axe at 360 px, the password change, the privacy switch, the Home nudge's Snooze, the
// palette's group scope reaching the server, and a member's own PIN for TheTVDB through
// Settings > Data sources. Plus the time zone with the browser's zone as a suggestion.

const SETTINGS = [
  "/settings/account",
  "/settings/notifications",
  "/settings/integrations",
  "/settings/sources",
  "/settings/imports",
  "/settings/privacy",
];

async function visit(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

/** Open the Sign-in disclosure of Settings > Account. */
async function openSignIn(page: Page) {
  const toggle = page.getByRole("button", { name: /^Sign-in/ });
  if ((await toggle.getAttribute("aria-expanded")) !== "true") {
    await toggle.click();
  }
}

test("every settings page reflows and passes axe at 360 px", async ({ page }) => {
  await signUp(page, `refl${Date.now()}`);
  await page.setViewportSize({ width: 360, height: 740 });
  for (const path of SETTINGS) {
    await visit(page, path);
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
    await expectReflow(page, `360 ${path}`);
    await expectNoA11yViolations(page, `360 ${path}`);
  }
});

test("the password change signs in with the new password only", async ({ page }) => {
  const user = `pwc${Date.now()}`;
  await signUp(page, user);
  await visit(page, "/settings/account");
  await openSignIn(page);
  await page.getByLabel("Current password").fill(PASSWORD);
  await page.getByLabel("New password").fill(`${PASSWORD} again`);
  await page.getByRole("button", { name: "Change password" }).click();
  await expect(page.getByText("Password changed. Your other devices are signed out.")).toBeVisible();
  const old = await page.request.post("/api/v1/auth/sign-in", { data: { username: user, password: PASSWORD } });
  expect(old.status()).toBe(401);
  const fresh = await page.request.post("/api/v1/auth/sign-in", { data: { username: user, password: `${PASSWORD} again` } });
  expect(fresh.ok()).toBeTruthy();
});

test("the privacy switch is kept", async ({ page }) => {
  await signUp(page, `prv${Date.now()}`);
  await visit(page, "/settings/privacy");
  const share = page.getByLabel("Linked participants see the sessions I log");
  const before = await share.isChecked();
  await share.click();
  await expect(share).toBeChecked({ checked: !before });
  await visit(page, "/settings/privacy");
  await expect(page.getByLabel("Linked participants see the sessions I log")).toBeChecked({ checked: !before });
});

test("Snooze hides the admin nudge on Home for this browser", async ({ page }) => {
  await signIn(page, ADMIN);
  await visit(page, "/");
  await page.evaluate(() => localStorage.removeItem("app-nudge-snooze"));
  await visit(page, "/");
  const snooze = page.getByRole("button", { name: "Snooze for 7 days" });
  await expect(snooze).toBeVisible();
  await snooze.click();
  await expect(snooze).toHaveCount(0);
  await visit(page, "/");
  await expect(page.getByRole("button", { name: "Snooze for 7 days" })).toHaveCount(0);
  await page.evaluate(() => localStorage.removeItem("app-nudge-snooze"));
});

test("the palette sends the chosen group with the search", async ({ page }) => {
  await signUp(page, `scp${Date.now()}`);
  await page.getByRole("button", { name: /^Search or add/ }).first().click();
  const dialog = page.getByRole("dialog", { name: "Search" });
  await dialog.getByRole("radio", { name: "Watch" }).click();
  const sent = page.waitForRequest((r) => r.url().includes("/_fn/search") && (r.postData() ?? "").includes("group=watch"));
  await dialog.getByRole("combobox").fill("arrival");
  await sent;
});

test("a member saves their own TheTVDB PIN under Settings > Data sources", async ({ page }) => {
  await signUp(page, `pin${Date.now()}`);
  await visit(page, "/settings/sources");
  const item = page.getByTestId("member-sources").getByRole("listitem").filter({ hasText: "TheTVDB" });
  await item.getByLabel("Subscriber PIN").fill("4711");
  await item.getByRole("button", { name: "Save" }).click();
  await expect(item.getByText("Saved.")).toBeVisible();
  await visit(page, "/settings/sources");
  const again = page.getByTestId("member-sources").getByRole("listitem").filter({ hasText: "TheTVDB" });
  await expect(again.getByText(/^Your key is set/)).toBeVisible();
});

test("the time zone: the browser's zone is one click away and Home drops the UTC marker", async ({ browser }) => {
  const context = await browser.newContext({ timezoneId: "Europe/Berlin" });
  const page = await context.newPage();
  await signUp(page, `tz${Date.now()}`);
  await visit(page, "/settings/account");
  await expect(page.getByTestId("tz-current")).toHaveText("Times are shown in UTC: no time zone chosen yet.");
  await page.getByRole("button", { name: "Use Europe/Berlin (this browser)" }).click();
  await expect(page.getByTestId("tz-current")).toHaveText("Times are shown in Europe/Berlin.");
  await hydrated(page);
  await expect(page.getByRole("button", { name: /this browser/ })).toHaveCount(0);
  // a name the time zone database does not know is refused in words
  await page.getByLabel("Time zone name").fill("Mars/Olympus");
  await page.getByRole("button", { name: "Save the time zone" }).click();
  await expect(page.getByRole("alert")).toHaveText("This is not a time zone name. Use one like Europe/Berlin.");
  await page.setViewportSize({ width: 360, height: 740 });
  await expectReflow(page, "360 account with time zone");
  await expectNoA11yViolations(page, "360 account with time zone");
  await context.close();
});
