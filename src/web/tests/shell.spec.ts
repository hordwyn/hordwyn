import { test, expect, type Page } from "@playwright/test";
import { addPerson, contrast, expectNoA11yViolations, hydrated, importDone, PASSWORD, requestLink, signUp, token } from "./helpers";

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

  test("the current tab has a bar above it, not only another colour and weight", async ({ page }) => {
    // review OT2D-4 (WCAG 1.4.1): accent against grey is about 1.1:1 in luminance, so the
    // current tab also carries a 2 px bar in the accent, which has 3:1 against the tab bar
    await signUp(page, `tabcue${Date.now()}`, "Tab");
    for (const theme of ["light", "dark"] as const) {
      await useTheme(page, theme);
      await visit(page, "/library", theme);
      const tabs = page.getByRole("navigation", { name: "Main navigation" });
      const bar = (name: string) =>
        tabs.getByRole("link", { name }).evaluate((a) => {
          const s = getComputedStyle(a, "::before");
          return { shown: s.content !== "none" && s.display !== "none", colour: s.backgroundColor, height: s.height, width: parseFloat(s.width) };
        });
      const current = await bar("Library");
      await expect(tabs.getByRole("link", { name: "Library" })).toHaveAttribute("aria-current", "page");
      expect(current.shown, `${theme} current tab has a bar`).toBe(true);
      expect(current.height).toBe("2px");
      expect(current.width).toBeGreaterThanOrEqual(24);
      expect(current.colour).toBe(await token(page, "primary"));
      expect(await contrast(page, current.colour, await token(page, "background")), `${theme} bar against the tab bar`).toBeGreaterThanOrEqual(3);
      for (const other of ["Home", "History", "You"]) {
        expect((await bar(other)).shown, `${theme} ${other} has no bar`).toBe(false);
      }
    }
  });

  test("the chord hint takes no room until a chord waits, also over a dialog", async ({ page }) => {
    // review OT2D-13: an empty bordered pill floated above the tab bar on every page
    await signUp(page, `pill${Date.now()}`, "Pil");
    const hint = page.getByTestId("chord-hint");
    const area = () =>
      hint.evaluate((el) => {
        const r = el.getBoundingClientRect();
        const s = getComputedStyle(el);
        return { box: Math.round(r.width * r.height), border: s.borderTopWidth, padding: s.paddingLeft };
      });
    const nothing = { box: 1, border: "0px", padding: "0px" };
    for (const path of ["/", "/library", "/settings/account"]) {
      await page.goto(path);
      await hydrated(page);
      await expect(hint).toHaveText("");
      expect(await area(), `${path} empty hint`).toEqual(nothing);
    }
    // it stays a live region in the page, so the words are announced when they come
    await expect(hint).toHaveAttribute("role", "status");
    await page.keyboard.press("g");
    await expect(hint).toHaveText(/^G pressed/);
    expect((await area()).box).toBeGreaterThan(1000);
    await page.keyboard.press("x");
    await expect(hint).toHaveText("");
    expect(await area()).toEqual(nothing);
    await page.keyboard.press("Control+k");
    await expect(page.getByRole("dialog", { name: "Search" })).toBeVisible();
    expect(await area(), "over a dialog").toEqual(nothing);
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
  // the cursor lands in the search field, so typing goes straight into it
  await expect(palette.getByRole("combobox")).toBeFocused();
  await page.keyboard.type("x");
  await expect(palette.getByRole("combobox")).toHaveValue("x");
  await page.keyboard.press("Backspace");
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
  const added = page.locator("[role=status][aria-live] > div").filter({ hasText: "Added to your library as Planned" });
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

test("wide screens: grid pages and the top bar are 96 rem wide, record pages 80 rem", async ({ page }) => {
  // OT-14 (DEC-541, supersedes DEC-496): 96 rem (1536 px) for grids and dashboards and for
  // the top bar, 80 rem (1280 px) for records, streams, settings and admin
  test.setTimeout(120_000);
  await page.setViewportSize({ width: 2560, height: 1200 });
  await signUp(page, `wide${Date.now()}`, "Wide Tester");
  // something in the library, one list and one person, so every page shows its content
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", {
    data: [JSON.stringify({ format: "library-export", version: 1 }), JSON.stringify({ source_id: "1", kind: "movie", title: "Wide Water", year: 2020, status: "planned" })].join("\n"),
    headers: { "content-type": "application/octet-stream" },
  });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  expect((await page.request.post("/api/v1/lists", { data: { name: "Wide list" } })).ok()).toBeTruthy();
  await page.goto("/people");
  await hydrated(page);
  await addPerson(page, "Wide Friend");
  const width =async (selector: string) => (await page.locator(selector).first().boundingBox())!.width;
  // the offline line sits in the page's column (measured through its box: it is empty online)
  const statusLine = () => page.locator("main#main").evaluate((m) => getComputedStyle(m.previousElementSibling!).maxWidth);
  // the content uses the column, not only `main` (review OT2D-2): the lists, grids, panels
  // and sections of the page reach the right edge of the column less its 1 rem gutter
  const unusedRight = () =>
    page.locator("main#main").evaluate((main) => {
      const edge = main.getBoundingClientRect().right - parseFloat(getComputedStyle(main).paddingRight);
      const blocks = Array.from(main.querySelectorAll('ul, ol, dl, section, figure, table, [role="tabpanel"]'));
      const reach = Math.max(0, ...blocks.map((b) => b.getBoundingClientRect().right));
      // a strip that bleeds into the gutter (-mx-4) reaches further: that is no unused room
      return blocks.length === 0 ? -1 : Math.max(0, Math.round(edge - reach));
    });
  for (const path of ["/", "/library", "/lists", "/people", "/stats"]) {
    await page.goto(path);
    await hydrated(page);
    expect(await width("main#main"), `${path} main`).toBe(1536);
    expect(await width("header > div"), `${path} top bar`).toBe(1536);
    expect(await statusLine(), `${path} status line`).toBe("1536px");
    await expect.poll(unusedRight, { message: `${path}: unused room right of the content` }).toBe(0);
  }
  // Lists and People put more cards in a row on the wide column (DEC-541 "more columns")
  for (const [path, label, columns] of [["/lists", "Lists", 4], ["/people", "People", 3]] as const) {
    await page.goto(path);
    await hydrated(page);
    const cards = page.getByRole("main").getByRole("list", { name: label, exact: true });
    await expect(cards).toBeVisible();
    expect(await cards.evaluate((ul) => getComputedStyle(ul).gridTemplateColumns.split(" ").length), `${path} columns`).toBe(columns);
  }
  for (const path of ["/history", "/settings/account", "/you", "/items/00000000-0000-0000-0000-000000000000"]) {
    await page.goto(path);
    await hydrated(page);
    expect(await width("main#main"), `${path} main`).toBe(1280);
    expect(await width("header > div"), `${path} top bar`).toBe(1536);
    expect(await statusLine(), `${path} status line`).toBe("1280px");
  }
  // the width follows a navigation inside the app, without a reload
  await page.goto("/library");
  await hydrated(page);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "History" }).click();
  await expect(page).toHaveURL(/\/history$/);
  await expect.poll(() => width("main#main")).toBe(1280);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "People" }).click();
  await expect(page).toHaveURL(/\/people$/);
  await expect.poll(() => width("main#main")).toBe(1536);
  // below the cap nothing changes: the page keeps its 1 rem gutter
  await page.setViewportSize({ width: 1000, height: 800 });
  expect(await width("main#main")).toBe(1000);
});

test("palette: a typo still lists what the server found", async ({ page }) => {
  // the palette filtered server hits again by substring, so a typo-tolerant hit never showed
  test.setTimeout(90_000);
  await signUp(page, `typo${Date.now()}`, "Ty");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  await palette.getByRole("combobox").fill("fihgt clbu");
  await expect(palette.getByRole("option", { name: /^Fight Club \(1999\)/ }).first()).toBeVisible({ timeout: 20_000 });
  // the idle list (commands and recent items) is still filtered as typed
  await palette.getByRole("combobox").fill("g");
  await expect(palette.getByRole("option", { name: "Go to Lists" })).toBeVisible();
  await expect(palette.getByRole("option", { name: "New list" })).toHaveCount(0);
});

test("palette: arrow keys move through provider results; the active row is marked, scrolled into view and kept when a late batch arrives; Enter opens it", async ({ page }) => {
  // OT-1. The TMDB mock answers the show search of "fight club <number>" late, so one
  // batch arrives after the keys were pressed (a query of its own: the server caches a
  // source's answer per query). Soft assertions: every cause shows in one run.
  test.setTimeout(90_000);
  await signUp(page, `arrow${Date.now()}`, "Ari");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  const box = palette.getByRole("combobox");
  const options = palette.getByRole("option");
  await expect(options.first()).toBeVisible();
  // the pointer rests where the results will appear: a list that moves under it is no hover
  const list = (await palette.getByRole("listbox").boundingBox())!;
  await page.mouse.move(list.x + list.width / 2, list.y + 60);
  await box.fill(`fight club ${Date.now() % 1_000_000}`);
  await expect(palette.getByRole("option", { name: /^Fight Club \(1999\)/ }).first()).toBeVisible({ timeout: 20_000 });
  await expect.poll(() => options.count()).toBeGreaterThanOrEqual(12);
  await expect.soft(options.nth(0), "the first row is active, not the one under the resting pointer").toHaveAttribute("aria-selected", "true");

  await box.press("ArrowDown");
  await box.press("ArrowDown");
  const third = (await options.nth(2).getAttribute("id"))!;
  await expect.soft(options.nth(2)).toHaveAttribute("aria-selected", "true");
  await expect.soft(options.nth(0)).toHaveAttribute("aria-selected", "false");
  await expect.soft(box).toHaveAttribute("aria-activedescendant", third);
  await expect(box).toBeFocused();
  // the keys were pressed before the late batch: otherwise this test proves nothing about
  // it (OT1A-6; raise LATE_MS in tmdb-mock.mjs if a loaded host gets here too late)
  await expect(palette.getByRole("option", { name: /^Fight Club Nights/ })).toHaveCount(0);

  // the late batch (the show search) must not move the active row
  await expect(palette.getByRole("option", { name: /^Fight Club Nights/ })).toBeVisible({ timeout: 20_000 });
  await expect.soft(box, "a late batch keeps the active row").toHaveAttribute("aria-activedescendant", third);

  // WCAG 1.4.11: the active row is marked by an outline of at least 3:1 against the row and
  // the list, in both themes (the background tint alone is about 1.1:1)
  for (const theme of ["light", "dark"]) {
    await page.evaluate((dark) => document.documentElement.classList.toggle("dark", dark), theme === "dark");
    const look = (n: number) =>
      options.nth(n).evaluate((el) => {
        const s = getComputedStyle(el);
        return { style: s.outlineStyle, width: parseFloat(s.outlineWidth), colour: s.outlineColor, background: s.backgroundColor };
      });
    const active = await look((await options.evaluateAll((all) => all.findIndex((o) => o.getAttribute("aria-selected") === "true"))) || 0);
    const idle = await look(5);
    const popover = await palette.getByRole("listbox").evaluate((el) => getComputedStyle(el.parentElement!).backgroundColor);
    expect.soft(active.style, `${theme}: the active row has an outline`).toBe("solid");
    expect.soft(active.width, `${theme}: outline width`).toBeGreaterThanOrEqual(2);
    expect.soft(idle.style, `${theme}: other rows have none`).toBe("none");
    expect.soft(await contrast(page, active.colour, active.background), `${theme}: outline against the row`).toBeGreaterThanOrEqual(3);
    expect.soft(await contrast(page, active.colour, popover), `${theme}: outline against the list`).toBeGreaterThanOrEqual(3);
  }
  await page.evaluate(() => document.documentElement.classList.remove("dark"));

  // ArrowUp wraps to the last row, far below the fold of the list: it scrolls into view,
  // and the list moving under the resting pointer does not take the active row away
  for (let n = 0; n < 3; n++) {
    await box.press("ArrowUp");
  }
  await expect.soft(options.last()).toHaveAttribute("aria-selected", "true");
  await expect.soft(options.last(), "the active row scrolls into view").toBeInViewport({ ratio: 0.99 });
  await page.waitForTimeout(300);
  await expect.soft(options.last(), "still active after the list scrolled under the pointer").toHaveAttribute("aria-selected", "true");
  await box.press("ArrowDown");
  await expect.soft(options.first()).toHaveAttribute("aria-selected", "true");
  await expect.soft(options.first()).toBeInViewport({ ratio: 0.99 });

  // Enter opens the active row
  const target = (await box.getAttribute("aria-activedescendant"))!.replace(/^search-opt-/, "");
  await box.press("Enter");
  await expect(page).toHaveURL(new RegExp(`/items/${target}$`));
});

test("palette: rows of the previous query are filtered by the new text until its answer is there; Enter opens no stale row", async ({ page }) => {
  // OT1A-1: server results are shown unfiltered (a typo still lists them), but only while
  // they answer the text in the field. Typed on, the old rows stayed up with the first one
  // active, and Enter or Ctrl+Enter took it.
  test.setTimeout(90_000);
  await signUp(page, `stale${Date.now()}`, "Stale Tester");
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  const box = palette.getByRole("combobox");
  const options = palette.getByRole("option");
  await box.fill("fight club");
  await expect(palette.getByRole("option", { name: /^Fight Club \(1999\)/ }).first()).toBeVisible({ timeout: 20_000 });
  const library = async () => (await (await page.request.get("/api/v1/library")).json()).items.length;
  expect(await library()).toBe(0);

  // the next search is held back: its answer is not there when the keys are pressed
  let release: () => void = () => {};
  const held = new Promise<void>((done) => (release = done));
  let asked = 0;
  await page.route("**/_fn/search*", async (route) => {
    asked += 1;
    await held;
    await route.abort();
  });
  await box.fill("fight club zq");
  await expect.poll(() => asked).toBe(1);
  await expect(options, "no row of the old query without the typed text").toHaveCount(0);
  await box.press("Enter");
  await box.press("Control+Enter");
  await page.waitForTimeout(500);
  await expect(page).not.toHaveURL(/\/items\//);
  await expect(palette).toBeVisible();
  expect(await library(), "Ctrl+Enter added nothing").toBe(0);

  // the search fails: the rows of the old query do not come back as if they answered it
  release();
  await page.waitForTimeout(500);
  await expect(options).toHaveCount(0);

  // back to a text the old rows hold: they are filtered as typed, as before
  await box.fill("fight clu");
  await expect.poll(() => asked).toBe(2);
  await expect(palette.getByRole("option", { name: /^Fight Club \(1999\)/ }).first()).toBeVisible();
});
