import { test, expect, type Page } from "@playwright/test";
import { addPerson, expectNoA11yViolations, hydrated, PASSWORD, requestLink, signUp } from "./helpers";

// UX phase 0 review (docs/reviews/2026-11-06-ux0-review.md): the real shell signed in, in
// light and dark, on desktop and at 360 px with the tab bar (UX0-3); the APG menu button and
// dialog focus rules (UX0-1, UX0-2); shortcuts, aria-current, offline, unread, the Show
// toggle and the lock-out text (UX0-12, UX0-4, UX0-5, UX0-13).

const PAGES = [
  "/",
  "/library",
  "/history",
  "/people",
  "/lists",
  "/you",
  "/settings/account",
  "/settings/notifications",
  "/settings/integrations",
  "/settings/sources",
  "/settings/imports",
  "/settings/privacy",
  // a member on an admin page: the "admins only" page
  "/admin",
  "/no-such-page",
];

async function useTheme(page: Page, theme: "light" | "dark") {
  await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
}

async function visit(page: Page, path: string, theme: "light" | "dark") {
  await page.goto(path);
  await hydrated(page);
  if (theme === "dark") {
    await expect(page.locator("html")).toHaveClass(/dark/);
  } else {
    await expect(page.locator("html")).not.toHaveClass(/dark/);
  }
}

test("signed-in pages, the account menu and the shortcut help have no violations on desktop", async ({ page }) => {
  test.setTimeout(240_000);
  await signUp(page, `axe${Date.now()}`, "Axe Tester");
  for (const theme of ["light", "dark"] as const) {
    await useTheme(page, theme);
    for (const path of PAGES) {
      await visit(page, path, theme);
      await expectNoA11yViolations(page, `${theme} ${path}`);
    }
    await visit(page, "/", theme);
    // no tab bar on desktop
    await expect(page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "You" })).toHaveCount(0);
    await page.getByRole("button", { name: "Account menu for Axe Tester" }).click();
    await expect(page.getByRole("menu")).toBeVisible();
    await expectNoA11yViolations(page, `${theme} account menu`);
    await page.keyboard.press("Escape");
    await page.keyboard.press("?");
    await expect(page.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeVisible();
    await expectNoA11yViolations(page, `${theme} shortcut help`);
    await page.keyboard.press("Escape");
  }
});

test.describe("at 360 px", () => {
  test.use({ viewport: { width: 360, height: 640 } });

  test("the tab bar is shown with 48 px targets and no page has violations", async ({ page }) => {
    test.setTimeout(240_000);
    await signUp(page, `narrow${Date.now()}`, "Nora");
    for (const theme of ["light", "dark"] as const) {
      await useTheme(page, theme);
      for (const path of PAGES) {
        await visit(page, path, theme);
        const tabs = page.getByRole("navigation", { name: "Main navigation" });
        await expect(tabs.getByRole("link", { name: "You" })).toBeVisible();
        const targets = tabs.locator("a, button");
        await expect(targets).toHaveCount(5);
        for (const el of await targets.all()) {
          const box = await el.boundingBox();
          expect(box?.height ?? 0, `${path} tab height`).toBeGreaterThanOrEqual(48);
        }
        await expectNoA11yViolations(page, `${theme} 360 ${path}`);
      }
    }
  });

  test("signed-out pages reserve no room for a tab bar", async ({ page }) => {
    for (const path of ["/sign-in", "/sign-up", "/invite/not-a-token"]) {
      await page.goto(path);
      await hydrated(page);
      await expect(page.getByRole("link", { name: "You" })).toHaveCount(0);
      const pad = await page.locator("main").evaluate((m) => getComputedStyle(m).paddingBottom);
      expect(pad, path).toBe("24px");
      await expectNoA11yViolations(page, `360 ${path}`);
    }
  });
});

test("the account menu follows the APG menu button pattern and dialogs return focus", async ({ page }) => {
  const user = `kbd${Date.now()}`;
  await signUp(page, user, "Kim");
  const account = page.getByRole("button", { name: "Account menu for Kim" });
  const menu = page.getByRole("menu");
  const items = menu.locator("[role^=menuitem]");

  // the head names the person and the username
  await account.click();
  await expect(page.getByTestId("account-head")).toContainText("Kim");
  await expect(page.getByTestId("account-head")).toContainText(user);
  await page.keyboard.press("Escape");
  await expect(account).toBeFocused();

  // ArrowDown opens on the first item, ArrowUp on the last; Home, End and wrapping
  await page.keyboard.press("ArrowDown");
  await expect(items.first()).toBeFocused();
  await page.keyboard.press("End");
  await expect(items.last()).toBeFocused();
  await page.keyboard.press("Home");
  await expect(items.first()).toBeFocused();
  await page.keyboard.press("ArrowUp");
  await expect(items.last()).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(account).toBeFocused();
  await page.keyboard.press("ArrowUp");
  await expect(items.last()).toBeFocused();
  await page.keyboard.press("Escape");

  // a theme chosen with Enter closes the menu and focus goes back to the button
  await page.keyboard.press("Enter");
  await expect(items.first()).toBeFocused();
  await page.getByRole("menuitemradio", { name: "Dark" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("html")).toHaveClass(/dark/);
  await expect(menu).toHaveCount(0);
  await expect(account).toBeFocused();

  // Tab closes the menu and moves on as Tab from the button would (APG)
  await page.goto("/people");
  await hydrated(page);
  const focused = () => page.evaluate(() => document.activeElement?.outerHTML ?? "");
  await account.focus();
  await page.keyboard.press("Tab");
  const afterButton = await focused();
  expect(afterButton).not.toContain("<body");
  await account.focus();
  await page.keyboard.press("ArrowDown");
  await expect(items.first()).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(menu).toHaveCount(0);
  expect(await focused()).toBe(afterButton);

  // the shortcut help from the menu: focus inside, Tab stays inside, Escape returns
  await account.click();
  await page.getByRole("menuitem", { name: /^Keyboard shortcuts/ }).click();
  const dialog = page.getByRole("dialog", { name: "Keyboard shortcuts" });
  await expect(dialog).toBeFocused();
  const close = dialog.getByRole("button", { name: "Close" });
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(account).toBeFocused();

  // the search palette opened from the search pill returns focus to it
  const pill = page.getByRole("button", { name: /^Search or add/ });
  await pill.click();
  await expect(page.getByRole("dialog", { name: "Search" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(pill).toBeFocused();
});

test("single-key shortcuts, but not while typing; aria-current marks the section", async ({ page }) => {
  await signUp(page, `keys${Date.now()}`, "Kai");
  await page.goto("/people");
  await hydrated(page);
  const nav = page.getByRole("navigation", { name: "Main navigation" });
  await expect(nav.getByRole("link", { name: "People" })).toHaveAttribute("aria-current", "page");
  await expect(nav.locator("[aria-current=page]")).toHaveCount(1);

  await page.keyboard.press("?");
  await expect(page.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeVisible();
  await page.keyboard.press("Escape");
  await page.keyboard.press("n");
  await expect(page.getByRole("dialog", { name: "Search" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);

  // typing in a field (the "Add a person" sheet, WP3-6) is not a shortcut
  await page.getByRole("button", { name: "Add a person" }).click();
  const name = page.getByRole("dialog", { name: "Add a person" }).getByLabel("Name", { exact: true });
  await name.click();
  await page.keyboard.type("n?");
  await expect(name).toHaveValue("n?");
  await expect(page.getByRole("dialog")).toHaveCount(1);
  await expect(page.getByRole("dialog", { name: "Keyboard shortcuts" })).toHaveCount(0);
  await expect(page.getByRole("dialog", { name: "Search" })).toHaveCount(0);

  await page.goto("/history");
  await hydrated(page);
  await expect(nav.getByRole("link", { name: "History" })).toHaveAttribute("aria-current", "page");
  await page.goto("/settings/notifications");
  await hydrated(page);
  await expect(nav.locator("[aria-current=page]")).toHaveCount(0);
});

test("offline: the banner appears and the sign-in button is disabled", async ({ page, context }) => {
  await page.goto("/sign-in");
  await hydrated(page);
  await expect(page.getByTestId("brand-mark")).toBeVisible();
  const submit = page.getByRole("button", { name: "Sign in", exact: true }).last();
  await expect(submit).toBeEnabled();
  await context.setOffline(true);
  await expect(page.getByRole("status").filter({ hasText: "You are offline. Changes are kept" })).toBeVisible();
  await expect(submit).toBeDisabled();
  await expect(page.locator("form").getByText("You are offline", { exact: true })).toBeVisible();
  await context.setOffline(false);
  await expect(submit).toBeEnabled();
  await expect(page.getByText("You are offline. Changes are kept")).toHaveCount(0);
});

test("an unread notification shows on the bell", async ({ page, browser }) => {
  const stamp = Date.now();
  const other = await browser.newContext();
  const pia = await other.newPage();
  await signUp(pia, `upia${stamp}`, "Pia");
  await signUp(page, `umax${stamp}`, "Max");
  await page.goto("/people");
  await hydrated(page);
  await addPerson(page, "P.");
  await requestLink(page, "P.", `upia${stamp}`);
  await pia.goto("/library");
  await hydrated(pia);
  await expect(pia.getByRole("link", { name: /^Notifications, \d+ unread$/ })).toBeVisible();
  await other.close();
});

test("sign-up: the Show toggle reveals the password and says so", async ({ page }) => {
  await page.goto("/sign-up");
  await hydrated(page);
  const show = page.getByRole("button", { name: "Show" });
  const password = page.locator("#password");
  await expect(show).toHaveAttribute("aria-controls", "password");
  await expect(show).toHaveAttribute("aria-pressed", "false");
  await expect(password).toHaveAttribute("type", "password");
  await show.click();
  await expect(show).toHaveAttribute("aria-pressed", "true");
  await expect(password).toHaveAttribute("type", "text");
  await show.click();
  await expect(password).toHaveAttribute("type", "password");
});

test("the lock-out names the remaining minutes", async ({ page }) => {
  await page.goto("/sign-in");
  await hydrated(page);
  await page.getByLabel("Username").fill(`lock${Date.now()}`);
  await page.getByLabel("Password").fill(`not ${PASSWORD}`);
  const submit = page.getByRole("button", { name: "Sign in", exact: true }).last();
  const alert = page.getByRole("alert");
  for (let i = 0; i < 10; i++) {
    const answer = page.waitForResponse((r) => r.url().includes("/_fn/sign_in"));
    await submit.click();
    await answer;
    await expect(alert).toHaveText("Wrong username or password.");
  }
  await submit.click();
  await expect(alert).toHaveText(/^Too many failed sign-ins\. Try again in 1[45] minutes\.$/);
});

test("palette: Ctrl+Enter adds a provider hit as Planned and stays open, Undo removes it; then the next step; recent items and suggestions", async ({ page }) => {
  test.setTimeout(90_000);
  await signUp(page, `quick${Date.now()}`, "Quinn");
  const library = async () => (await (await page.request.get("/api/v1/library?limit=50")).json()).items as { entry: { status: string }; item: { title: string } }[];

  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  // fewer than two characters: suggested commands (no recent items yet)
  await expect(palette.getByRole("option", { name: "Go to Lists" })).toBeVisible();
  await expect(palette.getByRole("option", { name: "Import a file" })).toBeVisible();
  await palette.getByRole("combobox").fill("fight club");
  const option = palette.getByRole("option", { name: /^Fight Club \(1999\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.hover();
  // the active row shows what Enter and Ctrl+Enter do
  await expect(option).toContainText("Add as Planned or next step");
  await page.keyboard.press("Control+Enter");
  const added = page.locator("[role=status][aria-live] > div").filter({ hasText: "Added to Planned" });
  await expect(added).toBeVisible();
  await expect(added).toContainText("Fight Club (1999)");
  await expect(palette).toBeVisible();
  await expect.poll(async () => (await library()).map((r) => `${r.item.title} ${r.entry.status}`)).toEqual(["Fight Club planned"]);
  await added.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Undone").first()).toBeVisible();
  await expect.poll(async () => (await library()).length).toBe(0);

  // twice: added, then its next step (Watched) is logged; the palette stays open
  await palette.getByRole("combobox").focus();
  await option.hover();
  await page.keyboard.press("Control+Enter");
  await expect.poll(async () => (await library()).length).toBe(1);
  await page.keyboard.press("Control+Enter");
  await expect(page.getByText("Logged: Watched").first()).toBeVisible();
  await expect(palette).toBeVisible();
  await expect.poll(async () => (await library())[0]?.entry.status).toBe("completed");

  // Enter opens the item; the empty palette then lists it as a recent item
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  await page.keyboard.press("Control+k");
  const recent = palette.getByRole("option", { name: /^Fight Club \(1999\)/ });
  await expect(recent).toBeVisible();
  await expect(palette.getByText("Recent", { exact: true })).toBeVisible();
  await palette.getByRole("option", { name: "Go to Lists" }).click();
  await expect(page).toHaveURL(/\/lists$/);
  // recent items stay in this browser
  expect(await page.evaluate(() => Object.keys(localStorage).some((k) => k.startsWith("app-recent-")))).toBe(true);
});
