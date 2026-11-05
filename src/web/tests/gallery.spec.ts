import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

const WCAG_A_AA = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

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

async function expectNoA11yViolations(page: Page) {
  const results = await new AxeBuilder({ page }).withTags(WCAG_A_AA).analyze();
  expect(
    results.violations.map(
      (v) => `${v.id}: ${v.help} -> ${v.nodes.map((n) => `${n.target.join(" ")} ${n.failureSummary ?? ""}`).join("; ")}`,
    ),
  ).toEqual([]);
}

test("gallery has no WCAG A/AA violations in light and dark", async ({ page }) => {
  await openGallery(page);
  await expectNoA11yViolations(page);
  await page.getByRole("button", { name: "Switch to dark theme" }).click();
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
  await expect(page.getByRole("status")).toContainText("Your play was logged.");
});
