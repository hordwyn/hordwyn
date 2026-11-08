import { defineConfig, devices } from "@playwright/test";

// The site must be built first (`cargo xtask e2e` runs `cargo leptos build`) and Postgres must
// be reachable at E2E_DATABASE_URL (`cargo xtask e2e` creates a fresh database).
const port = Number(process.env.E2E_PORT ?? 3010);
const tmdbPort = Number(process.env.TMDB_MOCK_PORT ?? 3011);
const oidcPort = Number(process.env.OIDC_MOCK_PORT ?? 3012);
const repo = `${require("node:path").resolve(__dirname, "../..")}/`;

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
  },
  projects: [
    // the instance admins exist before any spec runs (tests/global.setup.ts)
    { name: "setup", testMatch: /global\.setup\.ts/ },
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
      dependencies: ["setup"],
      testIgnore: /signin\.spec\.ts/,
    },
    // the sign-in specs add an identity provider and switch the instance to `auto` mode for a
    // while, which changes every signed-out page: they run alone, after the other specs
    {
      name: "sso",
      use: { ...devices["Desktop Chrome"] },
      dependencies: ["chromium"],
      testMatch: /signin\.spec\.ts/,
      fullyParallel: false,
    },
  ],
  webServer: [
    {
      command: "node tests/tmdb-mock.mjs",
      url: `http://127.0.0.1:${tmdbPort}/health`,
      reuseExistingServer: !process.env.CI,
      env: { TMDB_MOCK_PORT: String(tmdbPort) },
    },
    {
      command: "node tests/oidc-mock.mjs",
      url: `http://127.0.0.1:${oidcPort}/health`,
      reuseExistingServer: !process.env.CI,
      env: { OIDC_MOCK_PORT: String(oidcPort) },
    },
    {
      command: process.env.E2E_SERVER_CMD ?? "../rust/target/debug/server",
      cwd: "../rust",
      url: `http://127.0.0.1:${port}/healthz`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
      env: {
        LEPTOS_SITE_ROOT: "target/site",
        LEPTOS_OUTPUT_NAME: "app",
        LEPTOS_SITE_ADDR: `127.0.0.1:${port}`,
        LEPTOS_ENV: "DEV",
        APP__SERVER__BIND: `127.0.0.1:${port}`,
        APP__SERVER__PUBLIC_URL: `http://127.0.0.1:${port}`,
        APP__DATABASE__URL: process.env.E2E_DATABASE_URL ?? "postgres://app@127.0.0.1:55432/app_e2e",
        APP__AUTH__SIGNUP: "open",
        APP__AUTH__SECURE_COOKIES: "false",
        APP__SEARCH__BACKEND: "postgres",
        APP__STORAGE__BACKEND: "memory",
        // a temporary secrets key: every run starts on a fresh database (UX2-1)
        APP__SECRETS__EPHEMERAL: "true",
        APP__BRAND__DIR: process.env.E2E_BRAND_DIR ?? `${repo}config/brand/default`,
        APP__CONNECTORS__TMDB__BASE_URL: `http://127.0.0.1:${tmdbPort}`,
        APP__CONNECTORS__TMDB__API_KEY: "e2e-not-a-real-key",
      },
    },
  ],
});
