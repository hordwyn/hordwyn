import { test, expect, type Page } from "./fixtures";
import {
  ADMIN,
  PASSWORD,
  REAUTH_ADMIN,
  ageSessions,
  expectNoA11yViolations,
  expectReflow,
  hydrated,
  signIn,
  signUp,
  sql,
  totp,
} from "./helpers";

// the SMTP side of tests/mail-sink.mjs
const MAIL_SINK_SMTP = Number(process.env.MAIL_SINK_SMTP_PORT ?? 3013);

// UX phase 2, admin and settings areas (addendum-1 3.3, 3.4; design.md 6.10, 6.12, 6.13):
// the eight admin pages with axe in both themes and reflow at 360 px, re-authentication with
// the retried change, read-only values from the environment, invites, the self guard, Test
// of a data source against the TMDB fake server, the audit row with a URL filter, the
// member's settings with Ask admin, and TOTP enrolment with its recovery codes.

const ADMIN_PAGES: [string, string][] = [
  ["/admin", "Overview"],
  ["/admin/instance", "Instance"],
  ["/admin/sso", "Sign-in and SSO"],
  ["/admin/users", "Users"],
  ["/admin/sources", "Data sources"],
  ["/admin/notifications", "Notification defaults"],
  ["/admin/storage", "Storage and jobs"],
  ["/admin/audit", "Audit log"],
];

async function visit(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

/** Open the "Advanced" disclosure of Admin > Instance (remembered per browser). */
async function openAdvanced(page: Page) {
  const toggle = page.locator("#adm-advanced").getByRole("button", { name: /^Advanced/ });
  if ((await toggle.getAttribute("aria-expanded")) !== "true") {
    await toggle.click();
  }
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
    await expectNoA11yViolations(page, `360 ${path}`);
  }
  // Branding and Security were folded into Overview and Instance: 404, no redirects
  for (const path of ["/admin/brand", "/admin/security"]) {
    const r = await page.goto(path);
    expect(r?.status(), path).toBe(404);
  }
});

test("a gated change asks to confirm it is you and is retried once confirmed", async ({ page }) => {
  await signIn(page, REAUTH_ADMIN);
  ageSessions(REAUTH_ADMIN);
  await visit(page, "/admin/instance");
  // sessions and cookies live in "Advanced" since phase 3 (addendum-1 3.4)
  await openAdvanced(page);
  await expect(page.getByTestId("reauth-window")).toHaveText(/Not confirmed recently/);
  // values from the environment are read-only and name their variable
  await expect(page.getByText("Set by environment variable APP__AUTH__SECURE_COOKIES")).toBeVisible();
  await expect(page.getByRole("switch", { name: "Secure cookies" })).toBeDisabled();
  const days = page.getByLabel("Session length (days)");
  await days.fill("45");
  await page.locator("#adm-advanced").getByRole("button", { name: "Save", exact: true }).click();
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
  await visit(page, "/admin/instance");
  await openAdvanced(page);
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
  // sign-up mode (on Instance now) comes from the environment here: read-only too
  await visit(page, "/admin/instance");
  await expect(page.getByText("Set by environment variable APP__AUTH__SIGNUP")).toBeVisible();
  await expect(page.getByRole("radio", { name: "Anyone" })).toBeDisabled();
});

// Review V1OPS-8 (WCAG 2.4.3): confirming the key backup removes the reminder and the button
// that had the focus; the focus goes to the "Recorded" message, not back to the document.
// The e2e server runs on a temporary key and has no reminder of its own, so the overview
// the browser fetches says one is due until the confirmation went through; the gate, the
// dialog and the confirmation are the server's.
test("Overview: after the key backup is confirmed the focus is on the confirmation", async ({ page }) => {
  await signIn(page, REAUTH_ADMIN);
  await visit(page, "/admin/instance");
  let confirmed = false;
  await page.route("**/_fn/admin_overview*", async (route) => {
    const response = await route.fetch();
    const json = await response.json();
    if (!confirmed) json.key_backup = { version: 1, key_file: "/data/secrets.key" };
    await route.fulfill({ response, json });
  });
  await page.route("**/_fn/confirm_key_backup*", async (route) => {
    const response = await route.fetch();
    if (response.ok()) confirmed = true;
    await route.fulfill({ response });
  });
  const nav = page.getByRole("navigation", { name: "Settings" }).getByRole("list", { name: "Instance (admin)" });
  await nav.getByRole("link", { name: "Overview" }).click();
  const banner = page.getByTestId("key-backup");
  await expect(banner).toContainText("Key file on the server: /data/secrets.key");
  ageSessions(REAUTH_ADMIN);
  await banner.getByRole("button", { name: "I have backed up the key" }).focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Confirm it is you" });
  await expect(dialog.getByLabel("Password")).toBeFocused();
  await dialog.getByLabel("Password").fill(PASSWORD);
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  await expect(banner).toHaveCount(0);
  const outcome = page.locator("#adm-key-backup-outcome");
  await expect(outcome).toHaveText("Recorded. This reminder returns when a new key is added.");
  await expect(outcome).toBeFocused();
  // the next Tab goes on from the message, not from the top of the page
  await page.keyboard.press("Tab");
  const after = await page.evaluate(() => {
    const from = document.getElementById("adm-key-backup-outcome");
    const at = document.activeElement;
    return !!from && !!at && (from.compareDocumentPosition(at) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
  });
  expect(after, "Tab continues after the message").toBe(true);
  await expectNoA11yViolations(page, "overview after the key backup confirmation");
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

// DEC-437 (WP4-8 item C): an admin save through the re-auth gate counts every remembered
// disclosure open at that moment as "open at a save" (addendum-1 D16)
test("an admin save counts the open Invites disclosure as open at a save", async ({ page }) => {
  const events: Record<string, string>[] = [];
  await page.route("**/api/v1/usage/funnel", async (route) => {
    events.push(...(route.request().postDataJSON().events ?? []));
    await route.fulfill({ status: 204 });
  });
  await signIn(page, ADMIN);
  await visit(page, "/admin/users");
  await page.getByRole("button", { name: /^Open invites/ }).click();
  await page.getByLabel("Note (optional)").fill(`funnel ${Date.now()}`);
  await page.getByRole("button", { name: "Create invite" }).click();
  await expect(page.getByLabel("Invite link")).toHaveValue(/\/sign-up\?invite=/);
  const counted = (outcome: string) =>
    events.some((e) => e.flow === "disclosure" && e.subject === "users/invites" && e.outcome === outcome);
  await expect.poll(() => counted("open_at_save"), { timeout: 20_000 }).toBe(true);
  expect(counted("opened")).toBe(true);
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

test("Instance: the hosting mode is switched to Hosted and back", async ({ page }) => {
  await signIn(page, ADMIN);
  await visit(page, "/admin/instance");
  const form = page.locator("form").filter({ has: page.getByRole("radio", { name: /^Hosted/ }) });
  try {
    await page.getByRole("radio", { name: /^Hosted/ }).check();
    await form.getByRole("button", { name: "Save", exact: true }).click();
    await expect(form.getByText("Saved.")).toBeVisible();
    await visit(page, "/admin/instance");
    await expect(page.getByRole("radio", { name: /^Hosted/ })).toBeChecked();
  } finally {
    await visit(page, "/admin/instance");
    await page.getByRole("radio", { name: /^Home/ }).check();
    await form.getByRole("button", { name: "Save", exact: true }).click();
    await expect(form.getByText("Saved.")).toBeVisible();
  }
  await visit(page, "/admin/instance");
  await expect(page.getByRole("radio", { name: /^Home/ })).toBeChecked();
  // the public address is read-only from the configuration
  await expect(page.getByTestId("public-url")).toContainText("APP__SERVER__PUBLIC_URL");
});

test("Instance > Mail: Save and send a test says what failed, and Overview follows", async ({ page }) => {
  await signIn(page, ADMIN);
  await visit(page, "/admin/instance");
  const mail = page.locator("#adm-mail");
  const toggle = mail.getByRole("button", { name: /^Mail/ });
  if ((await toggle.getAttribute("aria-expanded")) !== "true") {
    await toggle.click();
  }
  try {
    // a sender the transport cannot use: saved, with a warning (UX3-18). The mail step is
    // gated (DEC-397); ADMIN has just signed in, so its proof is fresh and no dialog opens
    await mail.getByLabel("Server").fill("127.0.0.1");
    await mail.getByLabel("Port").fill("1");
    await mail.getByLabel("Connection security").selectOption("none");
    await mail.getByLabel("From address").fill("shelf@@example.test");
    await mail.getByLabel("Send the test to").fill("admin@example.test");
    await expectNoA11yViolations(page, "mail open");
    await mail.getByRole("button", { name: "Save and send a test" }).click();
    await expect(mail.getByTestId("mail-warning")).toHaveText(
      "Saved, but mail cannot be sent with these settings. Check the server, port and security.",
    );
    // nothing listens on port 1: the test fails at once, and says so; the warning goes
    // the typed address outlives the refetch after the first test
    await expect(mail.getByLabel("Send the test to")).toHaveValue("admin@example.test");
    await mail.getByLabel("From address").fill("shelf@example.test");
    await mail.getByRole("button", { name: "Save and send a test" }).click();
    await expect(mail.getByTestId("mail-warning")).toHaveCount(0);
    await expect(mail.getByRole("alert")).toContainText("The test failed:");
    await expect(mail.getByRole("alert")).not.toContainText("sender address");
    await visit(page, "/admin");
    const row = page.getByTestId("health-row").filter({ hasText: "Mail" });
    await expect(row).toContainText("The last test failed");

    // a server that takes the message (tests/mail-sink.mjs): the Overview names the time of
    // the test in the admin's own zone, not in UTC (DEC-336, WP4-2)
    sql(`UPDATE users SET time_zone = 'Pacific/Kiritimati' WHERE username = '${ADMIN}'`);
    const put = await page.request.put("/api/v1/admin/settings", {
      data: { values: { "mail.port": MAIL_SINK_SMTP } },
    });
    expect(put.ok(), `mail port ${put.status()} ${await put.text()}`).toBeTruthy();
    const test = await page.request.post("/api/v1/admin/mail/test", { data: { to: "admin@example.test" } });
    expect((await test.json()).ok, "the sink takes the test").toBeTruthy();
    const at = new Date((await (await page.request.get("/api/v1/admin/mail")).json()).last_test_at);
    const p = Object.fromEntries(
      new Intl.DateTimeFormat("en-US", {
        timeZone: "Pacific/Kiritimati",
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
        hour12: true,
      })
        .formatToParts(at)
        .map((x) => [x.type, x.value]),
    );
    await visit(page, "/admin");
    await expect(row).toContainText(
      `Test message sent ${p.month} ${p.day}, ${p.year}, ${p.hour}:${p.minute} ${p.dayPeriod}`,
    );
    await expect(row).not.toContainText("UTC");
  } finally {
    sql(`UPDATE users SET time_zone = NULL WHERE username = '${ADMIN}'`);
    const r = await page.request.put("/api/v1/admin/settings", {
      data: { values: { "mail.host": null, "mail.port": null, "mail.security": null, "mail.from": null } },
    });
    expect(r.ok(), `reset mail ${r.status()} ${await r.text()}`).toBeTruthy();
  }
});

test("Sign-in and SSO: changing the mode asks to confirm it is you, then explains the preconditions", async ({ page }) => {
  await signIn(page, REAUTH_ADMIN);
  ageSessions(REAUTH_ADMIN);
  await visit(page, "/admin/sso");
  // the URIs to register and why front-channel logout is not used
  await expect(page.getByLabel("Redirect URI", { exact: true })).toHaveValue(/\/api\/v1\/auth\/oidc\/callback$/);
  await expect(page.getByLabel("Post-logout redirect URI")).toHaveValue(/\/signed-out$/);
  await expect(page.getByText(/^Front-channel logout is not used/)).toBeVisible();
  await page.getByRole("radio", { name: /^Go straight to/ }).check();
  const form = page.locator("form").filter({ has: page.getByRole("radio", { name: /^Go straight to/ }) });
  await form.getByRole("button", { name: "Save", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Confirm it is you" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Password").fill(PASSWORD);
  await dialog.getByRole("button", { name: "Confirm" }).click();
  await expect(dialog).toHaveCount(0);
  // the change ran again after the proof; the server refuses auto without a tested provider
  // or without an admin with an authenticator, and the page says which (DEC-321)
  try {
    await expect(form.getByRole("alert")).toHaveText(/^(Test the default provider first|First give at least one admin)/);
  } finally {
    // never leave a (reused) instance in auto mode for the other specs
    await page.request.put("/api/v1/admin/settings", { data: { values: { "auth.sign_in_mode": "page" } } });
  }
  await visit(page, "/admin/sso");
  await expect(page.getByRole("radio", { name: "Show the sign-in page" })).toBeChecked();
});

test("Data sources: a source is switched off and on again", async ({ page }) => {
  // Podcast Index: no other spec searches Listen (TMDB is in use by them in parallel)
  await signIn(page, ADMIN);
  await visit(page, "/admin/sources");
  const table = page.getByTestId("src-table");
  await table.getByRole("button", { name: /^(Details of|Set up) Podcast Index$/ }).click();
  const card = page.locator("tr#adm-src-podcastindex");
  const on = card.getByRole("switch", { name: "Use this source" });
  await expect(on).toBeChecked();
  try {
    await on.click();
    await expect(table.getByRole("row").filter({ hasText: "Podcast Index" }).first()).toContainText("Switched off");
    await expectNoA11yViolations(page, "source switched off");
  } finally {
    await visit(page, "/admin/sources");
    await table.getByRole("button", { name: /^(Details of|Set up) Podcast Index$/ }).click();
    const again = page.locator("tr#adm-src-podcastindex").getByRole("switch", { name: "Use this source" });
    if ((await again.getAttribute("aria-checked")) !== "true") {
      await again.click();
    }
    await expect(again).toBeChecked();
  }
});

test("Users: the row menu resets an authenticator after a confirm; stacked rows at 360 px", async ({ page, browser }) => {
  // a member with an authenticator (enrolled directly: the menu only needs one to exist)
  const user = `rm${Date.now()}`;
  const other = await browser.newContext();
  const made = await other.request.post("/api/v1/auth/sign-up", {
    data: { username: user, display_name: "Rhea", password: PASSWORD },
  });
  expect(made.ok(), `sign-up ${made.status()}`).toBeTruthy();
  await other.close();
  sql(`UPDATE credentials SET totp_enabled = true WHERE user_id = (SELECT id FROM users WHERE username = '${user}')`);

  await signIn(page, ADMIN);
  await visit(page, "/admin/users");
  const row = page.getByTestId("user-row").filter({ hasText: user });
  await expect(row).toContainText("Authenticator");
  await row.getByRole("button", { name: "More actions for Rhea" }).click();
  await page.getByRole("menuitem", { name: "Reset authenticator" }).click();
  // the confirm names the consequence; Cancel changes nothing
  const dialog = page.getByRole("dialog", { name: "Reset the authenticator of Rhea?" });
  await expect(dialog).toContainText("Their recovery codes stop working");
  await expectNoA11yViolations(page, "reset authenticator confirm");
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(row).toContainText("Authenticator");
  await row.getByRole("button", { name: "More actions for Rhea" }).click();
  await page.getByRole("menuitem", { name: "Reset authenticator" }).click();
  await dialog.getByRole("button", { name: "Reset authenticator" }).click();
  await expect(row).not.toContainText("Authenticator");
  // nothing left for the menu: no authenticator, no linked provider
  await expect(row.getByRole("button", { name: "More actions for Rhea" })).toHaveCount(0);

  // below 47.5rem each person is a card with labelled fields; no header row, no sideways scroll
  for (const theme of ["light", "dark"] as const) {
    await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
    await page.setViewportSize({ width: 360, height: 740 });
    await visit(page, "/admin/users");
    await expect(page.getByRole("columnheader", { name: "Person" })).toBeHidden();
    const card = page.getByTestId("user-row").filter({ hasText: user });
    await expect(card.getByText("Signs in with", { exact: true })).toBeVisible();
    await expect(card.getByText("Last seen", { exact: true })).toBeVisible();
    await expectReflow(page, `users 360 ${theme}`);
    await expectNoA11yViolations(page, `users 360 ${theme}`);
  }
  await page.setViewportSize({ width: 1280, height: 800 });
  await visit(page, "/admin/users");
  await expect(page.getByRole("columnheader", { name: "Person" })).toBeVisible();
  await expect(page.getByTestId("user-row").filter({ hasText: user }).getByText("Last seen", { exact: true })).toBeHidden();
  await page.evaluate(() => localStorage.removeItem("app-theme"));
});
