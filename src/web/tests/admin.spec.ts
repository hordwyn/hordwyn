import { test, expect, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { createHmac } from "node:crypto";
import {
  ADMIN,
  PASSWORD,
  REAUTH_ADMIN,
  expectNoA11yViolations,
  expectReflow,
  hydrated,
  signIn,
  signUp,
} from "./helpers";

// UX phase 2, admin and settings areas (addendum-1 3.3, 3.4; design.md 6.10, 6.12, 6.13):
// the eight admin pages with axe in both themes and reflow at 360 px, re-authentication with
// the retried change, read-only values from the environment, invites, the self guard, Test
// of a data source against the TMDB fake server, the audit row with a URL filter, the
// member's settings with Ask admin, and TOTP enrolment with its recovery codes.

const ADMIN_PAGES: [string, string][] = [
  ["/admin", "Overview"],
  ["/admin/users", "Users and sign-up"],
  ["/admin/sources", "Data sources"],
  ["/admin/storage", "Storage and jobs"],
  ["/admin/notifications", "Notification defaults"],
  ["/admin/brand", "Branding"],
  ["/admin/security", "Security and sessions"],
  ["/admin/audit", "Audit log"],
];

async function visit(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

test("an admin reaches the eight admin pages from the account menu; axe and reflow", async ({ page }) => {
  test.setTimeout(240_000);
  await signIn(page, ADMIN);
  await visit(page, "/");
  await page.getByRole("button", { name: /^Account menu for/ }).click();
  await page.getByRole("menuitem", { name: "Admin" }).click();
  await expect(page).toHaveURL(/\/admin$/);
  await expect(page.getByRole("heading", { level: 1, name: "Overview" })).toBeVisible();
  await expect(page.getByTestId("health-row")).toHaveCount(8);
  for (const theme of ["light", "dark"] as const) {
    await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
    for (const [path, title] of ADMIN_PAGES) {
      await visit(page, path);
      await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
      const group = page.getByRole("navigation", { name: "Settings" }).getByRole("list", { name: "Instance (admin)" });
      await expect(group.getByRole("link", { name: title, exact: true })).toHaveAttribute("aria-current", "page");
      await expectNoA11yViolations(page, `${theme} ${path}`);
    }
  }
  // desktop-first, but every page reflows at 360 px (WCAG 1.4.10)
  await page.setViewportSize({ width: 360, height: 640 });
  for (const [path, title] of ADMIN_PAGES) {
    await visit(page, path);
    await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
    await expectReflow(page, `360 ${path}`);
  }
});

/** Age the proof of every session of `user`, as if it signed in two hours ago. */
function ageSessions(user: string) {
  const db = process.env.E2E_DATABASE_URL ?? "postgres://app@127.0.0.1:55432/app_e2e";
  const sql =
    "UPDATE auth_sessions SET reauth_at = now() - interval '2 hours', reauth_slide_at = NULL " +
    `WHERE user_id = (SELECT id FROM users WHERE username = '${user}')`;
  execFileSync("psql", [db, "-v", "ON_ERROR_STOP=1", "-c", sql]);
}

test("a gated change asks to confirm it is you and is retried once confirmed", async ({ page }) => {
  await signIn(page, REAUTH_ADMIN);
  ageSessions(REAUTH_ADMIN);
  await visit(page, "/admin/security");
  await expect(page.getByTestId("reauth-window")).toHaveText(/Not confirmed recently/);
  // values from the environment are read-only and name their variable
  await expect(page.getByText("Set by environment variable APP__AUTH__SECURE_COOKIES")).toBeVisible();
  await expect(page.getByRole("switch", { name: "Secure cookies" })).toBeDisabled();
  const days = page.getByLabel("Session length (days)");
  await days.fill("45");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Confirm it is you" });
  await expect(dialog).toBeVisible();
  // the password field has focus at once (UX2-W18)
  await expect(dialog.getByLabel("Password")).toBeFocused();
  await expectNoA11yViolations(page, "re-auth dialog");
  await dialog.getByLabel("Password").fill(`not ${PASSWORD}`);
  await dialog.getByRole("button", { name: "Confirm" }).click();
  await expect(dialog.getByRole("alert")).toHaveText("Wrong username or password.");
  await dialog.getByLabel("Password").fill(PASSWORD);
  await dialog.getByRole("button", { name: "Confirm" }).click();
  await expect(dialog).toHaveCount(0);
  // the change ran again after the proof and the window shows its state
  await expect(page.getByText("Saved.")).toBeVisible();
  await expect(page.getByTestId("reauth-window")).toHaveText(/^Confirmed for 1[45] more minutes\.$/);
  await visit(page, "/admin/security");
  await expect(page.getByLabel("Session length (days)")).toHaveValue("45");

  // the audit row, found with a filter that lives in the URL
  await visit(page, "/admin/audit");
  await page.getByLabel("What").selectOption({ label: "Setting changed" });
  await page.getByRole("button", { name: "Apply" }).click();
  await expect(page).toHaveURL(/action=setting\.changed/);
  await hydrated(page);
  const row = page.getByTestId("audit-row").filter({ hasText: "auth.session_days: 30 -> 45" });
  await expect(row.first()).toBeVisible();
  await expect(row.first()).toContainText(REAUTH_ADMIN);
  await expect(page.getByRole("link", { name: "Export CSV" })).toHaveAttribute(
    "href",
    "/api/v1/admin/audit.csv?action=setting.changed",
  );
  // sign-up mode comes from the environment here: read-only too
  await visit(page, "/admin/users");
  await expect(page.getByText("Set by environment variable APP__AUTH__SIGNUP")).toBeVisible();
  await expect(page.getByRole("radio", { name: "Anyone" })).toBeDisabled();
});

test("invites: the link is shown once, then the invite is revoked; the self guard", async ({ page }) => {
  await signIn(page, ADMIN);
  await visit(page, "/admin/users");
  await page.getByRole("button", { name: /^Open invites/ }).click();
  const note = `for Pia ${Date.now()}`;
  await page.getByLabel("Note (optional)").fill(note);
  await page.getByRole("button", { name: "Create invite" }).click();
  await expect(page.getByLabel("Invite link")).toHaveValue(/\/sign-up\?invite=/);
  await expect(page.getByText("Copy it now: the link is shown only once.")).toBeVisible();
  const row = page.getByTestId("invite-row").filter({ hasText: note });
  await expect(row).toContainText("Open");
  await row.getByRole("button", { name: `Revoke the invite for ${note}` }).click();
  await expect(page.getByTestId("invite-row").filter({ hasText: note })).toContainText("Revoked");
  // the own row offers no role or account change: the lock-out guard in words
  const own = page.getByTestId("user-row").filter({ hasText: "(you)" });
  await expect(own.getByTestId("self-guard")).toHaveText(/You cannot change your own role or account here/);
  await expect(own.getByRole("button")).toHaveCount(0);
  await expectNoA11yViolations(page, "users with invites open");
});

test("a data source is tested against the fake TMDB server", async ({ page }) => {
  await signIn(page, ADMIN);
  await visit(page, "/admin/sources");
  const table = page.getByTestId("src-table");
  await table.getByRole("button", { name: /^(Details of|Set up) TMDB$/ }).click();
  const card = page.locator("tr#adm-src-tmdb");
  await expect(card).toBeVisible();
  // the key comes from the environment: read-only with its variable, never shown
  await expect(card.getByTestId("setting-origin")).toHaveText("Set by environment variable APP__CONNECTORS__TMDB__API_KEY");
  await expect(card.locator("input[type=password]")).toHaveCount(0);
  await card.getByRole("button", { name: "Test" }).click();
  await expect(card.getByTestId("test-result")).toHaveText(/^Works \(\d+ ms\)\./);
  await expectNoA11yViolations(page, "source card open");
});

test("a member's settings: the side nav, Ask admin, privacy, and no admin pages", async ({ page }) => {
  await signUp(page, `mset${Date.now()}`, "Mo");
  await visit(page, "/settings/sources");
  const nav = page.getByRole("navigation", { name: "Settings" });
  await expect(nav.getByRole("link", { name: "Data sources" })).toHaveAttribute("aria-current", "page");
  await expect(nav.getByText("Instance (admin)")).toHaveCount(0);
  await expect(page.getByTestId("member-sources")).toBeVisible();
  const ask = page.getByRole("button", { name: /^Ask admin about / }).first();
  await ask.click();
  await expect(page.getByText("Your admin has been asked.").first()).toBeVisible();
  await expect(page.locator("#attribution")).toContainText("TMDB");
  await nav.getByRole("link", { name: "Privacy" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Privacy" })).toBeVisible();
  await expect(page.getByLabel("Linked participants see the sessions I log")).toBeVisible();
  await visit(page, "/admin/users");
  await expect(page.getByTestId("admin-only")).toBeVisible();
  await expect(page.getByRole("heading", { level: 1, name: "Admins only" })).toBeVisible();
  await expectNoA11yViolations(page, "member on an admin page");
});

/** RFC 6238 TOTP (SHA-1, 6 digits, 30 s) of a base32 secret, as authenticator apps compute it. */
function totp(base32: string, at = Date.now()): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = "";
  for (const c of base32.replace(/=+$/, "").toUpperCase()) {
    bits += alphabet.indexOf(c).toString(2).padStart(5, "0");
  }
  const bytes = Buffer.from((bits.match(/.{8}/g) ?? []).map((b) => parseInt(b, 2)));
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(at / 30_000)));
  const h = createHmac("sha1", bytes).update(counter).digest();
  const o = h[h.length - 1] & 0xf;
  const n = ((h[o] & 0x7f) << 24) | (h[o + 1] << 16) | (h[o + 2] << 8) | h[o + 3];
  return String(n % 1_000_000).padStart(6, "0");
}

test("TOTP enrolment shows the eight recovery codes once", async ({ page }) => {
  await signUp(page, `totp${Date.now()}`, "Tia");
  await visit(page, "/settings/account");
  await expect(page.getByRole("radiogroup", { name: "Language" })).toBeVisible();
  await page.getByRole("button", { name: /^Sign-in/ }).click();
  await expect(page.getByText("No authenticator yet.")).toBeVisible();
  await page.getByRole("button", { name: "Set up an authenticator" }).click();
  const key = page.getByLabel("Key", { exact: true });
  await expect(key).toHaveValue(/^[A-Z2-7]+=*$/);
  await page.getByLabel("Code from the app").fill(totp(await key.inputValue()));
  await page.getByRole("button", { name: "Turn on" }).click();
  const codes = page.getByTestId("recovery-codes").getByRole("listitem");
  await expect(codes).toHaveCount(8);
  await expect(page.getByText("Shown only now.")).toBeVisible();
  await expect(page.getByText("An authenticator app is set up.")).toBeVisible();
  await expectNoA11yViolations(page, "recovery codes");
  // shown once: a new visit has no codes
  await visit(page, "/settings/account");
  await expect(page.getByTestId("recovery-codes")).toHaveCount(0);
});
