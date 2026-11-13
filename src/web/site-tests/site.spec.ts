import { mkdirSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// WCAG 2.2 AA (2.0 and 2.1 A and AA are part of it)
const WCAG = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];
const PATHS = [
  "/",
  "/features/",
  "/self-hosting/",
  "/hub/",
  "/changelog/",
  "/security/",
  "/attributions/",
  "/imprint/",
  "/privacy/",
  "/404.html",
];
const LOCALES = ["", "/de"];
const THEMES = ["light", "dark"] as const;
const shots = process.env.SITE_SHOTS;

async function open(page: Page, path: string, theme: "light" | "dark") {
  // the theme follows the system setting until the visitor chooses
  await page.emulateMedia({ colorScheme: theme });
  await page.goto(path);
  await expect(page.locator("html")).toHaveClass(theme === "dark" ? /dark/ : /^(?!.*dark)/);
  await page.evaluate(() => document.fonts.ready);
}

/** Scroll every visible picture into view once, so the lazy ones are loaded and decoded. */
async function loadPictures(page: Page) {
  const pictures = page.locator("img:visible");
  for (let i = 0; i < (await pictures.count()); i++) {
    const img = pictures.nth(i);
    await img.scrollIntoViewIfNeeded();
    await expect.poll(() => img.evaluate((el: HTMLImageElement) => el.complete && el.naturalWidth)).toBeGreaterThan(0);
  }
  await page.evaluate(() => window.scrollTo(0, 0));
}

/**
 * axe cannot compute contrast over a gradient or behind a blurred bar and reports such text as
 * "incomplete", not as a violation. Those places are known and were computed by hand for the
 * default brand (docs/reviews/2026-11-13-site-review.md): the page heads on the gradient
 * (`data-band`) and the translucent top bar. Text that axe cannot judge anywhere else is new
 * and fails the test.
 */
async function unjudgedContrast(page: Page, incomplete: { id: string; nodes: { target: unknown[] }[] }[]) {
  const targets = incomplete
    .filter((r) => r.id === "color-contrast")
    .flatMap((r) => r.nodes.map((n) => String(n.target[0])));
  return page.evaluate(
    (selectors) => selectors.filter((sel) => !document.querySelector(sel)?.closest("[data-band], header")),
    targets,
  );
}

function name(path: string) {
  return path === "/" ? "home" : path.replace(/^\/|\/$|\.html$/g, "").replace(/\//g, "-");
}

for (const locale of LOCALES) {
  for (const path of PATHS) {
    const url = `${locale}${path}`;
    for (const theme of THEMES) {
      test(`${url} ${theme}: no axe findings`, async ({ page }) => {
        const failures: string[] = [];
        page.on("console", (m) => m.type() === "error" && failures.push(m.text()));
        page.on("pageerror", (e) => failures.push(String(e)));
        const foreign: string[] = [];
        page.on("request", (r) => {
          if (!r.url().startsWith("http://127.0.0.1:")) foreign.push(r.url());
        });
        await open(page, url, theme);
        for (const width of [1280, 390]) {
          await page.setViewportSize({ width, height: 900 });
          await loadPictures(page);
          const results = await new AxeBuilder({ page }).withTags(WCAG).analyze();
          expect(
            results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(" | ")}`),
            `${url} ${theme} ${width}`,
          ).toEqual([]);
          expect(
            await unjudgedContrast(page, results.incomplete),
            `${url} ${theme} ${width}: contrast axe could not judge, outside the known places`,
          ).toEqual([]);
          if (shots) {
            mkdirSync(shots, { recursive: true });
            const lang = locale ? "de" : "en";
            await page.screenshot({
              path: `${shots}/${lang}-${name(path)}-${width}-${theme}.png`,
              fullPage: true,
            });
          }
        }
        // the 404 page is requested by its file name; every other failure is a defect
        expect(failures, "console errors").toEqual([]);
        expect(foreign, "requests to other origins").toEqual([]);
      });
    }

    test(`${url}: fits from 320 to 2560 px`, async ({ page }) => {
      for (const width of [320, 390, 768, 1280, 1920, 2560]) {
        await page.setViewportSize({ width, height: 800 });
        await page.goto(url);
        const overflow = await page.evaluate(
          () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
        );
        expect(overflow, `horizontal overflow at ${width}px`).toBeLessThanOrEqual(0);
        await expect(page.locator("h1")).toHaveCount(1);
        await expect(page.locator("h1")).toBeVisible();
      }
    });
  }
}

test("the theme button switches and remembers the theme", async ({ page }) => {
  await open(page, "/", "light");
  const button = page.getByRole("button", { name: "Dark theme" });
  await expect(button).toHaveAttribute("aria-pressed", "false");
  await button.click();
  await expect(page.locator("html")).toHaveClass(/dark/);
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await page.goto("/features/");
  await expect(page.locator("html")).toHaveClass(/dark/);
  await page.getByRole("button", { name: "Dark theme" }).click();
  await expect(page.locator("html")).not.toHaveClass(/dark/);
});

test("the menu opens on a phone and shows the navigation", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "Main", exact: true })).toBeHidden();
  const features = page.getByRole("navigation", { name: "Main, menu" }).getByRole("link", { name: "Features" });
  await expect(features).toBeHidden();
  await page.locator("summary").click();
  await expect(features).toBeVisible();
  const box = await features.boundingBox();
  expect(box!.height).toBeGreaterThanOrEqual(44);
  await features.click();
  await expect(page).toHaveURL(/\/features\/$/);
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
});

test("the keyboard reaches the skip link first and focus is visible", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Tab");
  const skip = page.getByRole("link", { name: "Skip to content" });
  await expect(skip).toBeFocused();
  await expect(skip).toBeInViewport();
  // a ring is drawn around it (the ring utility is a box shadow)
  expect(await skip.evaluate((el) => getComputedStyle(el).boxShadow)).not.toBe("none");
  // and around the next stop, the home link of the top bar
  await page.keyboard.press("Tab");
  const home = page.locator("header a").first();
  await expect(home).toBeFocused();
  expect(await home.evaluate((el) => getComputedStyle(el).boxShadow)).not.toBe("none");
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Enter");
  await expect(page.locator("#main")).toBeFocused();
});

test("the language link leads to the same page in German and back", async ({ page }) => {
  await page.goto("/features/");
  await page.getByRole("link", { name: "Deutsch" }).click();
  await expect(page).toHaveURL(/\/de\/features\/$/);
  await expect(page.locator("html")).toHaveAttribute("lang", "de");
  await page.getByRole("link", { name: "English" }).click();
  await expect(page).toHaveURL(/\/features\/$/);
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
});

test("an unknown address answers 404 with the not-found page", async ({ page }) => {
  const response = await page.goto("/no-such-page/");
  expect(response!.status()).toBe(404);
  await expect(page.getByRole("heading", { level: 1, name: "Page not found" })).toBeVisible();
});

test("security.txt and the licence texts are published", async ({ request }) => {
  const security = await request.get("/.well-known/security.txt");
  expect(security.status()).toBe(200);
  // a build that is published has a contact (SITE_STRICT=1); a working build may still say
  // that there is none
  const text = await security.text();
  if (process.env.SITE_STRICT) {
    expect(text).toMatch(/^Contact: (mailto:|https:\/\/)\S+\nExpires: \d{4}-\d\d-\d\dT/);
  } else {
    expect(text).toMatch(/^(# PLACEHOLDER|Contact: )/);
  }
  for (const path of ["/licenses/lucide.txt", "/brand/fonts/OFL.txt", "/sitemap.xml", "/robots.txt"]) {
    expect((await request.get(path)).status(), path).toBe(200);
  }
});

for (const theme of THEMES) {
  test(`the captures of the application are shown in the ${theme} theme`, async ({ page }) => {
    for (const locale of LOCALES) {
      const requested: string[] = [];
      page.on("request", (r) => r.url().includes("/assets/shots/") && requested.push(new URL(r.url()).pathname));
      await open(page, `${locale}/`, theme);
      const pictures = page.locator("img:visible");
      // the hero, the wide picture and four phones; the other theme's six stay hidden
      await expect(pictures).toHaveCount(6);
      await expect(page.locator("img")).toHaveCount(12);
      await loadPictures(page);
      const lang = locale ? "de" : "en";
      for (let i = 0; i < 6; i++) {
        const img = pictures.nth(i);
        await expect(img).toHaveAttribute("src", new RegExp(`^/assets/shots/[a-z-]+-${lang}-${theme}\\.png$`));
        expect(((await img.getAttribute("alt")) ?? "").length).toBeGreaterThan(60);
        // the size attributes are those of the file: no layout shift, no distortion
        const size = await img.evaluate((el: HTMLImageElement) => [el.naturalWidth, el.naturalHeight, el.width, el.height]);
        expect(size[0]).toBe(Number(await img.getAttribute("width")));
        expect(size[1]).toBe(Number(await img.getAttribute("height")));
        expect(Math.abs(size[2] / size[3] - size[0] / size[1])).toBeLessThan(0.02);
      }
      // the hidden pictures below the first screen are not fetched
      const other = theme === "light" ? "dark" : "light";
      expect(requested.filter((r) => r.includes(`-${other}.png`) && !r.includes("home-top")), "lazy").toEqual([]);
      page.removeAllListeners("request");
    }
  });
}
