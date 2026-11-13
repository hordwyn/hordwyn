import { test as setup, expect, request, type Page } from "./fixtures";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { ADMIN, E2E_CONTACT, PASSWORD, REAUTH_ADMIN, expectNoA11yViolations, expectReflow, hydrated } from "./helpers";

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

  // board games come from Wikidata, which is asked only with the operator contact
  // (v1-wikidata); the instance keeps one while the specs run
  const contact = await page.request.put("/api/v1/admin/settings", {
    data: { values: { "instance.operator_contact": E2E_CONTACT } },
  });
  expect(contact.ok(), `operator contact ${contact.status()} ${await contact.text()}`).toBeTruthy();

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

// sec1: the policy guard of tests/fixtures.ts sees a test only when the spec takes `test`
// from there
//
// Review SEC1-3: the check must not be passed by accident (single quotes, `require`, a
// dynamic import, a helper module that hands the package's `test` on). So no file under
// tests/ but fixtures.ts may name a Playwright package as a module at all, in any kind of
// quotes; what a spec or a helper needs from it (`expect`, `request`, the types), it takes
// from "./fixtures". (`@axe-core/playwright` is another package and not meant.)
const namesPlaywright = (source: string) => /["'`]@?playwright(-core)?(\/[\w.\/-]*)?["'`]/.test(source);

/** Every script file under `dir`, relative to it. */
function scripts(dir: string, under = ""): string[] {
  return readdirSync(join(dir, under), { withFileTypes: true }).flatMap((entry) => {
    const path = under ? `${under}/${entry.name}` : entry.name;
    if (entry.isDirectory()) return scripts(dir, path);
    return /\.[cm]?[jt]sx?$/.test(entry.name) ? [path] : [];
  });
}

setup("every spec uses the shared fixtures", () => {
  // the package names are put together here, so that this file does not name them itself
  const pw = "play" + "wright";
  const pkg = `@${pw}/test`;
  for (const taken of [
    `import { test } from "${pkg}";`,
    `import { test } from '${pkg}';`,
    `import {\n  test,\n  expect,\n} from\n  '${pkg}';`,
    `export * from '${pkg}';`,
    `const { test } = require("${pkg}");`,
    `const { test } = require('${pkg}');`,
    `const { test } = await import("${pkg}");`,
    "const pw = await import(`" + pkg + "`);",
    `import { test } from "${pw}/test";`,
    `import { chromium } from "${pw}";`,
    `import { chromium } from "${pw}-core";`,
  ]) {
    expect(namesPlaywright(taken), taken).toBe(true);
  }
  for (const fine of [
    'import { test, expect } from "./fixtures";',
    `import AxeBuilder from "@axe-core/${pw}";`,
    `// never from the ${pkg} package`,
  ]) {
    expect(namesPlaywright(fine), fine).toBe(false);
  }

  const files = scripts(__dirname).filter((f) => f !== "fixtures.ts");
  expect(files.filter((f) => f.endsWith(".spec.ts")).length).toBeGreaterThan(20);
  expect(files).toContain("helpers.ts");
  expect(files).toContain("global.setup.ts");
  const direct = files.filter((f) => namesPlaywright(readFileSync(join(__dirname, f), "utf8")));
  expect(direct, 'these name a Playwright package themselves instead of taking what they need from "./fixtures"').toEqual([]);
});
