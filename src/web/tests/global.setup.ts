import { test as setup, expect, request, type Page } from "@playwright/test";
import { ADMIN, PASSWORD, REAUTH_ADMIN, expectNoA11yViolations, expectReflow, hydrated } from "./helpers";

// Runs before every spec (project dependency). On a fresh database it walks the whole setup
// wizard in the browser (design.md 5.1): every page leads to /setup, the first account becomes
// the instance admin, Home is the preselected hosting mode, data sources are skippable, the
// Done step lists the next steps, and /setup is a 404 afterwards. Each step is checked with axe
// in light and dark at 1280 and 360 px, and reloading resumes at the same step. Then a second
// admin for the re-authentication spec.

/** Axe and reflow for the current wizard step in both themes at 1280 and 360 px. */
async function checkStep(page: Page, heading: string) {
  for (const width of [1280, 360]) {
    await page.setViewportSize({ width, height: width === 360 ? 740 : 800 });
    for (const theme of ["light", "dark"] as const) {
      await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
      await page.reload();
      await hydrated(page);
      if (theme === "dark") {
        await expect(page.locator("html")).toHaveClass(/dark/);
      }
      // a reload resumes at the same step (the server keeps the wizard's state)
      await expect(page.getByRole("heading", { level: 2, name: heading })).toBeVisible();
      await expectNoA11yViolations(page, `/setup "${heading}" ${theme} ${width}`);
      await expectReflow(page, `/setup "${heading}" ${width}`);
    }
  }
  await page.evaluate(() => localStorage.setItem("app-theme", "light"));
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.reload();
  await hydrated(page);
}

async function wizard(page: Page) {
  const progress = page.getByRole("list", { name: "Setup progress" });
  await expect(progress).toBeVisible();
  await checkStep(page, "Create the admin account");
  await page.getByLabel("Username").fill(ADMIN);
  await page.getByLabel("Display name").fill("E2E Admin");
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Create admin account" }).click();

  // step 2: two cards, Home preselected (owner ruling)
  await expect(page.getByRole("heading", { level: 2, name: "How do you host?" })).toBeVisible();
  await hydrated(page);
  await expect(page.getByRole("radio", { name: /^Home/ })).toBeChecked();
  await checkStep(page, "How do you host?");
  await page.getByRole("button", { name: "Continue" }).click();

  // step 3: one card per source, everything skippable
  await expect(page.getByRole("heading", { level: 2, name: "Data sources" })).toBeVisible();
  await checkStep(page, "Data sources");
  await page.getByRole("button", { name: "Continue" }).click();

  // step 4: the checklist, then Home
  await expect(page.getByRole("heading", { level: 2, name: "Ready" })).toBeVisible();
  await expect(page.getByRole("link", { name: /Add your first item/ })).toHaveAttribute("href", "/library");
  // ... and opens the search palette once the page runs (design.md 5.1 step 4)
  await hydrated(page);
  await page.getByRole("link", { name: /Add your first item/ }).click();
  await expect(page.getByRole("dialog", { name: "Search" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Search" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/setup$/);
  await checkStep(page, "Ready");
  await page.getByRole("button", { name: "Go to Home" }).click();
  await expect(page).toHaveURL(/\/$/);
}

setup("the setup wizard makes the instance admin", async ({ page, baseURL }) => {
  setup.setTimeout(240_000);
  // before the first account every page leads to the wizard (server-side redirect)
  await page.goto("/library");
  await hydrated(page);
  if (page.url().endsWith("/setup")) {
    await wizard(page);
  } else {
    // a reused server already has its admin: sign in instead
    const r = await page.request.post("/api/v1/auth/sign-in", { data: { username: ADMIN, password: PASSWORD } });
    expect(r.ok(), `admin sign-in ${r.status()}`).toBeTruthy();
  }
  // the wizard is locked now
  const locked = await page.goto("/setup");
  expect(locked?.status()).toBe(404);

  const other = await request.newContext({ baseURL });
  const signUp = await other.post("/api/v1/auth/sign-up", {
    data: { username: REAUTH_ADMIN, display_name: "Rea Admin", password: PASSWORD },
  });
  if (signUp.ok()) {
    const id = (await signUp.json()).user.id;
    const promote = await page.request.put(`/api/v1/admin/users/${id}/role`, { data: { role: "admin" } });
    expect(promote.ok(), `promote ${promote.status()} ${await promote.text()}`).toBeTruthy();
  }
  await other.dispose();
});
