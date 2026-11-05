import { defineConfig, devices } from "@playwright/test";

// The site must be built first (`cargo xtask e2e` runs `cargo leptos build`).
const port = Number(process.env.E2E_PORT ?? 3010);

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: process.env.E2E_SERVER_CMD ?? "../rust/target/debug/server",
    cwd: "../rust",
    url: `http://127.0.0.1:${port}/healthz`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    env: {
      LEPTOS_SITE_ROOT: "target/site",
      LEPTOS_OUTPUT_NAME: "app",
      LEPTOS_SITE_ADDR: `127.0.0.1:${port}`,
      LEPTOS_ENV: "DEV",
    },
  },
});
