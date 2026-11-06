import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

// UX phase 0, form discipline (design.md 5.2): rules before typing, live normalisation,
// availability with suggestions, inline errors plus an error summary that takes focus.

const WCAG_A_AA = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

async function hydrated(page: Page) {
  await expect(page.locator("body[data-hydrated]")).toBeAttached({ timeout: 20_000 });
}

async function expectNoA11yViolations(page: Page) {
  const results = await new AxeBuilder({ page }).withTags(WCAG_A_AA).analyze();
  expect(
    results.violations.map(
      (v) => `${v.id}: ${v.help} -> ${v.nodes.map((n) => `${n.target.join(" ")} ${n.failureSummary ?? ""}`).join("; ")}`,
    ),
  ).toEqual([]);
}

const PASSWORD = "correct horse battery";

test("signing up 'Robin' succeeds as 'robin'", async ({ page }) => {
  const name = `Robin${Date.now()}`;
  const stored = name.toLowerCase();
  await page.goto("/sign-up");
  await hydrated(page);
  await expect(page.getByText("Used in links and for sign-in. 3 to 32 characters.")).toBeVisible();
  await page.getByLabel("Username").fill(name);
  // the normalised value is shown, not applied silently
  const status = page.locator("#username-status");
  await expect(status).toContainText(`Will be saved as ${stored} (capitals become lowercase)`);
  // availability after a pause
  await expect(status).toContainText(`'${stored}' is available.`, { timeout: 5_000 });
  await expect(page.locator("#username-rules li[data-state=fail]")).toHaveCount(0);
  await page.getByLabel("Password").fill(PASSWORD);
  await expect(page.locator("#password-rules")).toContainText("At least 10 characters (21 so far)");
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/library$/);
  await hydrated(page);
  await expect(page.getByRole("button", { name: `Account menu for ${stored}` })).toBeVisible();
});

test("'anna k' shows the space rule inline with a fix", async ({ page }) => {
  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill("anna k");
  await expect(page.locator("#username-rules li", { hasText: "No spaces" })).toHaveAttribute("data-state", "fail");
  await page.getByLabel("Username").blur();
  await expect(page.locator("#username-error")).toHaveText("'anna k' has a space. Try 'anna-k' or 'anna_k'.");
  await expect(page.getByLabel("Username")).toHaveAttribute("aria-invalid", "true");
});

test("a taken name offers free names that fill the field", async ({ page, browser }) => {
  const name = `anna${Date.now()}`;
  const other = await browser.newContext();
  const first = await other.newPage();
  await first.goto("/sign-up");
  await hydrated(first);
  await first.getByLabel("Username").fill(name);
  await first.getByLabel("Password").fill(PASSWORD);
  await first.getByRole("button", { name: "Create account" }).click();
  await expect(first).toHaveURL(/\/library$/);
  await other.close();

  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill(name);
  await page.getByLabel("Username").blur();
  await expect(page.locator("#username-error")).toContainText(`'${name}' is already taken. Free:`, { timeout: 5_000 });
  const suggestions = page.getByRole("group", { name: "Suggestions" }).getByRole("button");
  await expect(suggestions.first()).toBeVisible();
  const pick = (await suggestions.first().innerText()).trim();
  await suggestions.first().click();
  await expect(page.getByLabel("Username")).toHaveValue(pick);
  await expect(page.getByLabel("Username")).toBeFocused();
  await expect(page.locator("#username-status")).toContainText(`'${pick}' is available.`);
  await expect(page.locator("#username-error")).toHaveCount(0);
});

test("a submit with errors moves focus to the error summary", async ({ page }) => {
  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill("anna k");
  await page.getByLabel("Password").fill("abc123");
  await page.getByRole("button", { name: "Create account" }).click();
  const summary = page.getByRole("group", { name: "There are 2 problems" });
  await expect(summary).toBeFocused();
  await expect(summary).toContainText("'anna k' has a space. Try 'anna-k' or 'anna_k'.");
  await expect(summary).toContainText("The password has 6 characters; it needs at least 10.");
  await expect(page.locator("#password-error")).toHaveText("The password has 6 characters; it needs at least 10.");
  await expect(page.getByText("check the fields")).toHaveCount(0);
  await expectNoA11yViolations(page);
  await summary.getByRole("link", { name: /has a space/ }).click();
  await expect(page.getByLabel("Username")).toBeFocused();
  await expect(page).toHaveURL(/\/sign-up$/);
});

test("wrong credentials give one message that does not say which part", async ({ page }) => {
  await page.goto("/sign-in");
  await hydrated(page);
  await page.getByLabel("Username").fill(`nobody${Date.now()}`);
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).last().click();
  await expect(page.getByRole("alert")).toHaveText("Wrong username or password.");
  await expect(page.locator("[aria-invalid=true]")).toHaveCount(0);
});

for (const path of ["/sign-up", "/sign-in"]) {
  test(`${path} has no WCAG A/AA violations`, async ({ page }) => {
    await page.goto(path);
    await hydrated(page);
    await expectNoA11yViolations(page);
  });
}
