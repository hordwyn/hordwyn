import { expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

// Shared by the specs: hydration marker, the WCAG A/AA axe check (design.md 8) and sign-up.

export const WCAG_A_AA = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];
export const PASSWORD = "correct horse battery";

export async function hydrated(page: Page) {
  await expect(page.locator("body[data-hydrated]")).toBeAttached({ timeout: 20_000 });
}

export async function expectNoA11yViolations(page: Page, where = "") {
  const results = await new AxeBuilder({ page }).withTags(WCAG_A_AA).analyze();
  expect(
    results.violations.map(
      (v) => `${where} ${v.id}: ${v.help} -> ${v.nodes.map((n) => `${n.target.join(" ")} ${n.failureSummary ?? ""}`).join("; ")}`,
    ),
  ).toEqual([]);
}

export async function signUp(page: Page, user: string, display = "") {
  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill(user);
  if (display) {
    await page.getByLabel("Display name").fill(display);
  }
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/library$/);
  await hydrated(page);
}

// The instance admin made by `global.setup.ts` before any spec runs (the first account is
// the admin, so the setup creates it through the setup wizard's API), and a second admin
// whose sessions the re-authentication spec may age.
export const ADMIN = "e2eadmin";
export const REAUTH_ADMIN = "e2ereauth";

/** Sign in through the API; the browser context keeps the session cookie. */
export async function signIn(page: Page, user: string) {
  const r = await page.request.post("/api/v1/auth/sign-in", { data: { username: user, password: PASSWORD } });
  expect(r.ok(), `sign-in ${user}: ${r.status()}`).toBeTruthy();
}

/** No horizontal page scroll at the current viewport (WCAG 1.4.10 reflow). */
export async function expectReflow(page: Page, where = "") {
  const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(over, `${where} horizontal overflow`).toBeLessThanOrEqual(1);
}
