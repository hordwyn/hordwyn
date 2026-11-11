import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations } from "./helpers";

async function openGallery(page: Page) {
  await page.goto("/dev/gallery");
  await expect(page.getByRole("heading", { level: 1, name: "Component gallery" })).toBeVisible();
  // hydration finished once the WASM handlers react
  await expect(async () => {
    await page.getByRole("button", { name: "Open dialog" }).click();
    await expect(page.getByRole("dialog")).toBeVisible({ timeout: 500 });
  }).toPass({ timeout: 15_000 });
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
}

test("gallery has no WCAG A/AA violations in light and dark", async ({ page }) => {
  await openGallery(page);
  await expectNoA11yViolations(page);
  await page.getByRole("main").getByRole("button", { name: "Switch to dark theme" }).click();
  await expect(page.locator("html")).toHaveClass(/dark/);
  await expectNoA11yViolations(page);
});

test("open dialog and menu have no violations", async ({ page }) => {
  await openGallery(page);
  await page.getByRole("button", { name: "Open dialog" }).click();
  await expectNoA11yViolations(page);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Actions" }).click();
  await expectNoA11yViolations(page);
});

test("dialog: focus moves in, Escape and close button close it", async ({ page }) => {
  await openGallery(page);
  await page.getByRole("button", { name: "Open dialog" }).click();
  const dialog = page.getByRole("dialog", { name: "Log a play" });
  await expect(dialog).toBeFocused();
  await dialog.getByRole("button", { name: "Close" }).click();
  await expect(dialog).toBeHidden();
  // APG: focus returns to the opener
  const opener = page.getByRole("button", { name: "Open dialog" });
  await expect(opener).toBeFocused();
  await opener.press("Enter");
  await expect(dialog).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(opener).toBeFocused();
});

test("form dialog: centered, the rows scroll between the title and the footer", async ({ page }) => {
  // OT-10: the presentation of the log dialogs
  await page.setViewportSize({ width: 1280, height: 600 });
  await openGallery(page);
  const opener = page.getByRole("button", { name: "Open form dialog" });
  await opener.click();
  const dialog = page.getByRole("dialog", { name: "Score sheet" });
  await expect(dialog).toBeFocused();
  const box = (await dialog.boundingBox())!;
  expect(Math.abs(box.x + box.width / 2 - 640)).toBeLessThanOrEqual(1);
  expect(Math.abs(box.y + box.height / 2 - 300)).toBeLessThanOrEqual(1);
  expect(box.width).toBe(672);
  expect(box.height).toBeLessThanOrEqual(600 - 32);
  expect(await dialog.locator("[data-dialog-body]").evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
  await dialog.getByLabel("Score of player 12").focus();
  await expect(dialog.getByLabel("Score of player 12")).toBeInViewport({ ratio: 1 });
  await expect(dialog.getByRole("button", { name: "Save" })).toBeInViewport({ ratio: 1 });
  await expect(dialog.getByRole("heading", { name: "Score sheet" })).toBeInViewport({ ratio: 1 });
  await expectNoA11yViolations(page);
  // below 640 px it is a bottom sheet
  await page.setViewportSize({ width: 390, height: 700 });
  const sheet = (await dialog.boundingBox())!;
  expect(sheet.x).toBe(0);
  expect(sheet.width).toBe(390);
  expect(Math.abs(sheet.y + sheet.height - 700)).toBeLessThanOrEqual(1);
  await expectNoA11yViolations(page);
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(opener).toBeFocused();
});

test("tabs: arrow keys move selection and focus", async ({ page }) => {
  await openGallery(page);
  const overview = page.getByRole("tab", { name: "Overview" });
  await overview.focus();
  await page.keyboard.press("ArrowRight");
  const plays = page.getByRole("tab", { name: "Plays" });
  await expect(plays).toBeFocused();
  await expect(plays).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tabpanel", { name: "Plays" })).toBeVisible();
  await page.keyboard.press("End");
  await expect(page.getByRole("tab", { name: "Stats" })).toHaveAttribute("aria-selected", "true");
});

test("dropdown menu: keyboard selection", async ({ page }) => {
  await openGallery(page);
  const trigger = page.getByRole("button", { name: "Actions" });
  await trigger.click();
  await expect(trigger).toHaveAttribute("aria-expanded", "true");
  await expect(page.getByRole("menuitem", { name: "Edit" })).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("menuitem", { name: "Share" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("menu-choice")).toHaveText("Share");
  await expect(trigger).toBeFocused();
});

test("command palette filters and selects with the keyboard", async ({ page }) => {
  await openGallery(page);
  const input = page.getByRole("combobox", { name: "Search everything" });
  await input.fill("dune");
  await expect(page.locator("#g-cmd-list").getByRole("option")).toHaveCount(1);
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("command-choice")).toHaveText("Dune");
});

test("combobox picks an option", async ({ page }) => {
  await openGallery(page);
  const input = page.getByRole("combobox", { name: "Companion" });
  await input.fill("car");
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("combobox-value")).toHaveText("carla");
});

test("toast is announced in a live region", async ({ page }) => {
  await openGallery(page);
  await page.getByRole("button", { name: "Show toast" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Your play was logged." })).toBeVisible();
});

test("error summary takes focus, links move to the field, no violations", async ({ page }) => {
  await openGallery(page);
  await page.getByRole("button", { name: "Check form" }).click();
  const summary = page.locator("#g-errors");
  await expect(summary).toBeFocused();
  await expect(summary.getByRole("heading", { name: "There is a problem" })).toBeVisible();
  await expectNoA11yViolations(page);
  await summary.getByRole("link", { name: /username/ }).click();
  await expect(page.locator("#g-username")).toBeFocused();
});

test("form field shows live rules, ok line and a reactive Show button", async ({ page }) => {
  await openGallery(page);
  await page.locator("#g-username").fill("anna");
  await expect(page.locator("#g-username-status")).toContainText("Available");
  const pw = page.locator("#g-password");
  await pw.fill("123");
  await expect(page.locator("#g-password-rules li[data-state=fail]")).toHaveCount(2);
  await pw.fill("correct horse battery");
  await expect(page.locator("#g-password-rules li[data-state=pass]")).toHaveCount(2);
  await expect(pw).toHaveAttribute("type", "password");
  await page.getByRole("button", { name: "Show", exact: true }).click();
  await expect(pw).toHaveAttribute("type", "text");
});

test("disclosures open, expand all opens every section, no violations", async ({ page }) => {
  await openGallery(page);
  const advanced = page.locator("#g-dis-advanced");
  await expect(advanced).not.toHaveAttribute("open", /.*/);
  await advanced.locator("summary").click();
  await expect(advanced).toHaveAttribute("open", /.*/);
  const secrets = page.getByRole("button", { name: /Secrets at rest/ });
  await expect(secrets).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator("#g-dis-secrets-content")).toBeHidden();
  await secrets.click();
  await expect(secrets).toHaveAttribute("aria-expanded", "true");
  await expect(page.getByLabel("Key label")).toBeVisible();
  await expect(page.getByRole("button", { name: /Sign-in/ })).toHaveAttribute("aria-expanded", "true");
  await page.getByRole("button", { name: "Expand all" }).click();
  await expect(page.locator("#g-dis-mail")).toHaveAttribute("open", /.*/);
  await expect(page.getByRole("button", { name: "Collapse all" })).toBeVisible();
  await expectNoA11yViolations(page);
});

test("rating: arrow keys move in half steps", async ({ page }) => {
  await openGallery(page);
  const group = page.getByRole("radiogroup", { name: "Your rating" });
  const checked = group.getByRole("radio", { name: "Rate 3.5 of 5" });
  await expect(checked).toHaveAttribute("aria-checked", "true");
  await checked.focus();
  await page.keyboard.press("ArrowRight");
  await expect(group.getByRole("radio", { name: "Rate 4 of 5" })).toBeFocused();
  await expect(page.getByTestId("rating-value")).toHaveText("80");
  await page.keyboard.press("Home");
  await expect(page.getByTestId("rating-value")).toHaveText("10");
});

test("chips, segmented control, tab bar action and copy field", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openGallery(page);
  const books = page.getByRole("button", { name: /Books/ });
  await expect(books).toHaveAttribute("aria-pressed", "false");
  await books.click();
  await expect(books).toHaveAttribute("aria-pressed", "true");
  const grid = page.getByRole("radiogroup", { name: "View" }).getByRole("radio", { name: "Grid" });
  await grid.focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("radio", { name: "List" })).toHaveAttribute("aria-checked", "true");
  await page.getByRole("tab", { name: "Board games" }).click();
  await expect(page.getByRole("tabpanel", { name: "Board games" })).toBeVisible();
  await page.getByRole("navigation", { name: "Main" }).getByRole("button", { name: "Add" }).click();
  await expect(page.getByTestId("tabbar-added")).toHaveText("1");
  await page.getByRole("button", { name: "Copy" }).click();
  await expect(page.locator("#g-token").locator("..").locator("..").getByRole("status")).toHaveText(
    /Copied|Press Ctrl\+C to copy/,
  );
});
