import { test, expect, type Page } from "@playwright/test";
import { ADMIN, expectNoA11yViolations, expectReflow, hydrated, signIn, signUp } from "./helpers";

// UX phase 2, "why is nothing happening" (design.md 4.4, 6.1, 6.4): the palette's no-results
// card for a kind group without a source with Ask admin (member) or Set up (admin), the
// footer with source status, axe on the open palette in both themes at 1280 and 360 px, and
// the Home nudge for admins only. In the e2e build only TMDB has a key, so Play has no source.

async function openPalette(page: Page) {
  await page.getByRole("button", { name: /^Search or add/ }).first().click();
  const dialog = page.getByRole("dialog", { name: "Search" });
  await expect(dialog).toBeVisible();
  return dialog;
}

async function searchPlay(page: Page) {
  const dialog = await openPalette(page);
  await dialog.getByRole("radio", { name: "Play" }).click();
  await dialog.getByRole("combobox").fill("catan");
  const card = dialog.getByTestId("no-results");
  await expect(card).toBeVisible();
  return { dialog, card };
}

test("a member searching Play learns there is no source and asks the admin", async ({ page }) => {
  await signUp(page, `pal${Date.now()}`);
  const { dialog, card } = await searchPlay(page);
  await expect(card.getByRole("heading", { name: "Search is not set up yet" })).toBeVisible();
  await expect(card).toContainText("Ask your admin");
  await expect(card.getByRole("link", { name: "Set up" })).toHaveCount(0);
  // the footer names the group too, without a second Ask button
  await expect(dialog.getByRole("list", { name: "Data sources" })).toContainText("Play: no source set up");
  await card.getByRole("button", { name: "Ask admin about Play" }).click();
  await expect(card.getByText("Your admin has been asked.")).toBeVisible();
});

test("an admin searching Play gets the Set up link", async ({ page }) => {
  await signIn(page, ADMIN);
  await page.goto("/");
  await hydrated(page);
  const { card } = await searchPlay(page);
  await expect(card.getByRole("link", { name: "Set up" })).toHaveAttribute("href", "/admin/sources");
  await expect(card.getByRole("button", { name: /^Ask admin/ })).toHaveCount(0);
});

test("the open palette with its no-results card has no axe violations in both themes at 1280 and 360 px", async ({ page }) => {
  test.setTimeout(120_000);
  await signUp(page, `pax${Date.now()}`);
  for (const width of [1280, 360]) {
    await page.setViewportSize({ width, height: width === 360 ? 740 : 800 });
    for (const theme of ["light", "dark"] as const) {
      await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
      await page.goto("/");
      await hydrated(page);
      if (theme === "dark") {
        await expect(page.locator("html")).toHaveClass(/dark/);
      }
      await searchPlay(page);
      await expectNoA11yViolations(page, `palette ${theme} ${width}`);
      await expectReflow(page, `palette ${width}`);
      await page.keyboard.press("Escape");
    }
  }
});

test("the Home nudge shows for the admin with one action, never for a member", async ({ page, browser }) => {
  await signIn(page, ADMIN);
  await page.goto("/");
  await hydrated(page);
  const nudge = page.getByTestId("nudge");
  await expect(nudge).toBeVisible();
  await expect(nudge).toContainText("Play");
  await expect(nudge.getByRole("link")).toHaveCount(1);
  await expect(nudge.getByRole("link", { name: "Set up data sources" })).toHaveAttribute("href", "/admin/sources");

  const member = await browser.newPage();
  await signUp(member, `nud${Date.now()}`);
  await member.goto("/");
  await hydrated(member);
  await expect(member.getByRole("heading", { level: 1 })).toBeVisible();
  await expect(member.getByTestId("nudge")).toHaveCount(0);
  await member.close();
});
