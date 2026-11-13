import { defineConfig, devices } from "@playwright/test";

// Browser checks of the project website (src/rust/apps/website). The site must be built first:
// `cargo xtask site` writes it to <repo>/build/public (SITE_DIR points elsewhere). Run with
// `npx playwright test -c playwright.site.config.ts`; SITE_SHOTS=<dir> also writes screenshots.
const port = Number(process.env.SITE_PORT ?? 3500);

export default defineConfig({
  testDir: "./site-tests",
  fullyParallel: true,
  workers: 2,
  retries: 0,
  reporter: "list",
  outputDir: "./test-results/site",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "site", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "node site-tests/serve.mjs",
    url: `http://127.0.0.1:${port}/robots.txt`,
    reuseExistingServer: false,
    env: { SITE_PORT: String(port) },
  },
});
