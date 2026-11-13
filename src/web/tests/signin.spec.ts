import { test, expect, request, type APIRequestContext, type Browser, type Page } from "./fixtures";
import {
  ADMIN,
  PASSWORD,
  ageSessions,
  expectNoA11yViolations,
  expectReflow,
  hydrated,
  signIn,
  signUp,
  sql,
  totp,
} from "./helpers";

// UX phase 3 (WP3-4), sign-in with identity providers against the mock provider in
// tests/oidc-mock.mjs (addendum-1 4.2-4.5, DEC-326): an admin adds the provider through Admin >
// Sign-in and SSO; provider-first sign-in with the return path; local-first sign-out through
// the provider to /signed-out, which stays; the outage alert; break-glass /sign-in/local;
// "Is this your account?"; Link and Unlink on Settings > Account; "Confirm with <provider>";
// silent sign-in in auto mode (then back to page mode). Axe and reflow at 360 px on each new
// page. This file runs in its own project after every other spec (playwright.config.ts): a
// provider changes the sign-in page and auto mode changes every signed-out visit. Each test
// makes sure the provider exists, so the tests do not depend on their order.

const MOCK = `http://127.0.0.1:${process.env.OIDC_MOCK_PORT ?? 3012}`;
const NAME = "Mock";
const SLUG = "mock";

async function visit(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

/** Axe at 1280 and 360 px and reflow at 360 px for the page as it is. */
async function checkPage(page: Page, where: string) {
  await expectNoA11yViolations(page, `1280 ${where}`);
  await page.setViewportSize({ width: 360, height: 740 });
  await expectReflow(page, `360 ${where}`);
  await expectNoA11yViolations(page, `360 ${where}`);
  await page.setViewportSize({ width: 1280, height: 800 });
}

/** If the re-auth dialog asks, confirm with the password. */
async function confirmIfAsked(page: Page) {
  const dialog = page.getByRole("dialog", { name: "Confirm it is you" });
  if (await dialog.isVisible().catch(() => false)) {
    await dialog.getByLabel("Password").fill(PASSWORD);
    await dialog.getByRole("button", { name: "Confirm" }).click();
  }
}

type Provider = { id: string; slug: string; last_test: { ok: boolean } | null };

async function providers(api: APIRequestContext): Promise<Provider[]> {
  const r = await api.get("/api/v1/admin/oidc");
  expect(r.ok(), `admin oidc ${r.status()}`).toBeTruthy();
  return (await r.json()).providers;
}

/** Add the mock provider through the admin UI (test 1 does exactly this). */
async function addThroughUi(page: Page) {
  await visit(page, "/admin/sso");
  await page.getByRole("button", { name: /^Add a provider/ }).click();
  const add = page.locator("#adm-sso-add");
  await add.getByLabel("Display name").fill(NAME);
  await expect(add.getByLabel("Short name")).toHaveValue(SLUG);
  await add.getByLabel("Issuer URL").fill(MOCK);
  await add.getByLabel("Client ID").fill("shelf-e2e");
  await add.getByLabel("Client secret").fill("e2e-client-secret");
  await add.getByLabel(/^Default provider/).check();
  await add.getByRole("button", { name: "Add the provider" }).click();
  await confirmIfAsked(page);
  await expect(add.getByText("Saved.", { exact: true })).toBeVisible();
}

/** The mock provider exists and its Test passed (as the admin in `page`'s context). */
async function ensureProvider(page: Page): Promise<Provider> {
  await signIn(page, ADMIN);
  let mock = (await providers(page.request)).find((p) => p.slug === SLUG);
  if (!mock) {
    await addThroughUi(page);
    mock = (await providers(page.request)).find((p) => p.slug === SLUG);
  }
  expect(mock, "mock provider").toBeTruthy();
  const t = await page.request.post(`/api/v1/admin/oidc/providers/${mock!.id}/test`, { data: {} });
  expect(t.ok(), `provider test ${t.status()}`).toBeTruthy();
  expect((await t.json()).ok, "discovery against the mock works").toBeTruthy();
  return mock!;
}

/** A browser of its own (no admin cookie), for the person signing in. */
async function person(browser: Browser) {
  const context = await browser.newContext();
  return { context, page: await context.newPage() };
}

/** Sign in at the mock provider's own form as `user`. */
async function atProvider(page: Page, user: string) {
  await expect(page.getByRole("heading", { name: "Mock provider sign-in" })).toBeVisible();
  await page.getByLabel("Mock username").fill(user);
  await page.getByRole("button", { name: "Sign in at the mock provider" }).click();
}

test("an admin adds a provider through Sign-in and SSO and its Test passes", async ({ page }) => {
  await signIn(page, ADMIN);
  if (!(await providers(page.request)).some((p) => p.slug === SLUG)) {
    await addThroughUi(page);
  }
  await visit(page, "/admin/sso");
  const row = page.getByTestId("sso-row").filter({ hasText: NAME });
  await expect(row).toContainText(MOCK);
  const details = row.getByRole("button", { name: `Details of ${NAME}` });
  if ((await details.getAttribute("aria-expanded")) !== "true") {
    await details.click();
  }
  const detail = page.locator(`#adm-sso-${SLUG}`);
  await detail.getByRole("button", { name: "Test", exact: true }).click();
  const result = detail.getByTestId("sso-test-result");
  await expect(result).toContainText("Test passed.");
  await expect(result).toContainText("works");
  // the client secret is write-only: a stored line with Replace, never the value
  await expect(detail.getByText("A client secret is stored.")).toBeVisible();
  await expect(detail.locator("input[type=password]")).toHaveCount(0);
  await checkPage(page, "/admin/sso with a provider");
});

test("provider-first sign-in brings the person back to the page they wanted", async ({ page, browser }) => {
  await ensureProvider(page);
  const { context, page: p } = await person(browser);
  await visit(p, "/sign-in?next=/history");
  const button = p.getByRole("link", { name: `Continue with ${NAME}` });
  await expect(button).toBeVisible();
  // the password form waits behind its disclosure
  await expect(p.getByText("Sign in with a password instead")).toBeVisible();
  await expect(p.getByLabel("Username")).toBeHidden();
  await checkPage(p, "/sign-in with a provider");
  await button.click();
  const user = `pia${Date.now()}`;
  await atProvider(p, user);
  await expect(p).toHaveURL(/\/history$/);
  await hydrated(p);
  await expect(p.getByRole("button", { name: /^Account menu for/ })).toBeVisible();
  // the provider is this account's only way in: Unlink is refused and says why
  await visit(p, "/settings/account");
  await p.getByRole("button", { name: /^Sign-in/ }).click();
  const linked = p.getByTestId("linked-row").filter({ hasText: NAME });
  await expect(linked).toContainText("This is your only way to sign in.");
  await linked.getByRole("button", { name: `Unlink ${NAME}` }).click();
  await expect(p.getByRole("alert").filter({ hasText: "only way left to sign in" })).toBeVisible();
  await context.close();
});

test("sign-out ends the session here, then at the provider, and /signed-out stays", async ({ page, browser }) => {
  await ensureProvider(page);
  const { context, page: p } = await person(browser);
  await visit(p, "/sign-in");
  await p.getByRole("link", { name: `Continue with ${NAME}` }).click();
  await atProvider(p, `sol${Date.now()}`);
  await expect(p).toHaveURL(/127\.0\.0\.1:\d+\/$/);
  await hydrated(p);
  // every request to the provider's authorization endpoint from now on
  const toProvider: string[] = [];
  p.on("request", (r) => {
    if (r.url().startsWith(`${MOCK}/authorize`)) toProvider.push(r.url());
  });
  await p.getByRole("button", { name: /^Account menu for/ }).click();
  await p.getByRole("menuitem", { name: "Sign out" }).click();
  await expect(p).toHaveURL(/\/signed-out(\?|$)/);
  await expect(p.getByRole("heading", { level: 1, name: "You are signed out" })).toBeVisible();
  // no automatic sign-in from here (OD-20): the page hydrates and the network goes quiet
  // without a single request to the provider (UX3-22, no fixed wait)
  await hydrated(p);
  await p.waitForLoadState("networkidle");
  expect(toProvider, "requests to the provider after sign-out").toEqual([]);
  await expect(p).toHaveURL(/\/signed-out(\?|$)/);
  await checkPage(p, "/signed-out");
  // the provider ended its session too: the next sign-in asks there again
  await visit(p, "/sign-in");
  await p.getByRole("link", { name: `Continue with ${NAME}` }).click();
  await expect(p.getByRole("heading", { name: "Mock provider sign-in" })).toBeVisible();
  await context.close();
});

test("a provider outage is explained, with Try again and the local sign-in", async ({ page, browser }) => {
  await ensureProvider(page);
  const control = await request.newContext();
  const { context, page: p } = await person(browser);
  try {
    expect((await control.post(`${MOCK}/control`, { data: { outage: true } })).ok()).toBeTruthy();
    await visit(p, "/sign-in");
    await p.getByRole("link", { name: `Continue with ${NAME}` }).click();
    await expect(p).toHaveURL(/\/sign-in\?error=oidc_unavailable/);
    await hydrated(p);
    const alert = p.getByRole("alert").filter({ hasText: `${NAME} is not answering` });
    await expect(alert).toBeVisible();
    await expect(alert.getByRole("link", { name: "Admins: sign in with a local account" })).toHaveAttribute("href", "/sign-in/local");
    // the password form is open on this page
    await expect(p.getByLabel("Username")).toBeVisible();
    await checkPage(p, "/sign-in outage");
  } finally {
    await control.post(`${MOCK}/control`, { data: { outage: false } });
    await control.dispose();
    await context.close();
  }
});

test("break-glass /sign-in/local signs an admin in with the password and is audited", async ({ page, browser }) => {
  await ensureProvider(page);
  const { context, page: p } = await person(browser);
  await visit(p, "/sign-in/local");
  await expect(p.getByRole("heading", { level: 1, name: "Local sign-in" })).toBeVisible();
  await checkPage(p, "/sign-in/local");
  await p.getByLabel("Username").fill(ADMIN);
  await p.getByLabel("Password").fill(PASSWORD);
  await p.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(p).toHaveURL(/127\.0\.0\.1:\d+\/$/);
  const audit = await page.request.get("/api/v1/admin/audit?action=auth.local_sign_in");
  expect(audit.ok()).toBeTruthy();
  expect((await audit.json()).entries.length).toBeGreaterThan(0);
  await context.close();
});

test("a verified address of an existing account asks 'Is this your account?' once", async ({ page, browser }) => {
  await ensureProvider(page);
  const user = `ana${Date.now()}`;
  const { context, page: p } = await person(browser);
  const made = await p.request.post("/api/v1/auth/sign-up", {
    data: { username: user, display_name: "Ana", password: PASSWORD, email: `${user}@example.test` },
  });
  expect(made.ok()).toBeTruthy();
  await context.clearCookies();
  await visit(p, "/sign-in");
  await p.getByRole("link", { name: `Continue with ${NAME}` }).click();
  await atProvider(p, user);
  await expect(p).toHaveURL(/\/sign-in\/link$/);
  await hydrated(p);
  await expect(p.getByRole("heading", { level: 1, name: "Is this your account?" })).toBeVisible();
  await expect(p.getByText(`the same address as the account ${user} here`)).toBeVisible();
  await checkPage(p, "/sign-in/link");
  await p.getByLabel(`Password for ${user}`).fill(PASSWORD);
  await p.getByRole("button", { name: "Link and sign in" }).click();
  await expect(p).toHaveURL(/127\.0\.0\.1:\d+\/$/);
  // linked now: the provider alone signs in
  await visit(p, "/settings/account");
  await p.getByRole("button", { name: /^Sign-in/ }).click();
  await expect(p.getByTestId("linked-row").filter({ hasText: NAME })).toBeVisible();
  await context.close();
});

test("Settings > Account: Link a provider, then Unlink it", async ({ page, browser }) => {
  await ensureProvider(page);
  const { context, page: p } = await person(browser);
  await signUp(p, `lin${Date.now()}`);
  await visit(p, "/settings/account");
  await p.getByRole("button", { name: /^Sign-in/ }).click();
  await expect(p.getByText("No provider linked.")).toBeVisible();
  await p.getByRole("button", { name: `Link ${NAME}` }).click();
  await atProvider(p, `linm${Date.now()}`);
  await expect(p).toHaveURL(/\/settings\/account$/);
  await hydrated(p);
  const toggle = p.getByRole("button", { name: /^Sign-in/ });
  if ((await toggle.getAttribute("aria-expanded")) !== "true") {
    await toggle.click();
  }
  const row = p.getByTestId("linked-row").filter({ hasText: NAME });
  await expect(row).toBeVisible();
  await checkPage(p, "/settings/account with a linked sign-in");
  await row.getByRole("button", { name: `Unlink ${NAME}` }).click();
  await expect(p.getByText("Unlinked.")).toBeVisible();
  await expect(p.getByTestId("linked-row")).toHaveCount(0);
  await context.close();
});

test("a password-less account confirms a gated change with the provider", async ({ page, browser }) => {
  await ensureProvider(page);
  const user = `rea${Date.now()}`;
  const { context, page: p } = await person(browser);
  await visit(p, "/sign-in");
  await p.getByRole("link", { name: `Continue with ${NAME}` }).click();
  await atProvider(p, user);
  await expect(p).toHaveURL(/127\.0\.0\.1:\d+\/$/);
  ageSessions(user);
  await visit(p, "/settings/account");
  await p.getByRole("button", { name: /^Sign-in/ }).click();
  await p.getByRole("button", { name: "Set up an authenticator" }).click();
  const dialog = p.getByRole("dialog", { name: "Confirm it is you" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("Password")).toHaveCount(0);
  await expectNoA11yViolations(p, "re-auth dialog with a provider");
  await dialog.getByRole("button", { name: `Confirm with ${NAME}` }).click();
  // max_age=0: the provider asks again even though its session is live
  await atProvider(p, user);
  await expect(p).toHaveURL(/\/settings\/account$/);
  await hydrated(p);
  await expect(p.getByText(/^You confirmed it is you/)).toBeVisible();
  const toggle = p.getByRole("button", { name: /^Sign-in/ });
  if ((await toggle.getAttribute("aria-expanded")) !== "true") {
    await toggle.click();
  }
  await p.getByRole("button", { name: "Set up an authenticator" }).click();
  await expect(p.getByLabel("Key", { exact: true })).toHaveValue(/^[A-Z2-7]+=*$/);
  await context.close();
});

test("the reset link page refuses a dead link in words", async ({ browser }) => {
  const { context, page: p } = await person(browser);
  await visit(p, "/reset-password/not-a-real-token");
  await expect(p.getByRole("heading", { level: 1, name: "Choose a new password" })).toBeVisible();
  await checkPage(p, "/reset-password");
  await p.getByLabel("New password").fill(`${PASSWORD} new`);
  await p.getByRole("button", { name: "Set the new password" }).click();
  await expect(p.getByRole("alert")).toHaveText("This link is not valid any more. Ask your admin for a new one.");
  await context.close();
});

/** An admin with a password and an authenticator: auto mode needs one (DEC-321). */
async function adminWithAuthenticator(admin: APIRequestContext) {
  const other = await request.newContext({ baseURL: test.info().project.use.baseURL });
  const user = `ssoadm${Date.now()}`;
  const made = await other.post("/api/v1/auth/sign-up", { data: { username: user, display_name: "SSO Admin", password: PASSWORD } });
  expect(made.ok()).toBeTruthy();
  const begin = await other.post("/api/v1/auth/totp");
  expect(begin.ok(), `totp ${begin.status()}`).toBeTruthy();
  const code = totp((await begin.json()).secret_base32);
  expect((await other.post("/api/v1/auth/totp/confirm", { data: { code } })).ok()).toBeTruthy();
  const id = (await made.json()).user.id;
  const promote = await admin.put(`/api/v1/admin/users/${id}/role`, { data: { role: "admin" } });
  expect(promote.ok(), `promote ${promote.status()}`).toBeTruthy();
  await other.dispose();
}

test("auto mode: a person with a provider session gets in without any form; /signed-out stays", async ({ page, browser }) => {
  test.setTimeout(120_000);
  await ensureProvider(page);
  await adminWithAuthenticator(page.request);
  try {
    // the two probes, as the admin's browser round trips (OD-19, DEC-324)
    await visit(page, "/admin/sso");
    await page.getByRole("button", { name: `Details of ${NAME}` }).click();
    await page.getByRole("link", { name: "Check the confirmation round trip" }).click();
    await atProvider(page, "probe");
    await expect(page).toHaveURL(/\/admin\/sso\?probe=max_age_ok$/);
    await hydrated(page);
    await expect(page.getByRole("status").filter({ hasText: "the provider asked you to sign in again" })).toBeVisible();
    await page.getByRole("button", { name: `Details of ${NAME}` }).click();
    await page.getByRole("link", { name: "Check silent sign-in" }).click();
    await expect(page).toHaveURL(/\/admin\/sso\?probe=silent_ok$/);
    await hydrated(page);
    // now go straight to the provider
    await page.getByRole("radio", { name: `Go straight to ${NAME}` }).check();
    const form = page.locator("form").filter({ has: page.getByRole("radio", { name: `Go straight to ${NAME}` }) });
    await form.getByRole("button", { name: "Save", exact: true }).click();
    await confirmIfAsked(page);
    await expect(form.getByText("Saved.")).toBeVisible();

    // a person: the first visit goes through the provider's form (no session there yet) ...
    const { context, page: p } = await person(browser);
    const user = `aut${Date.now()}`;
    await p.goto("/history");
    await atProvider(p, user);
    await expect(p).toHaveURL(/\/history$/);
    // ... the app session ends, the provider session stays: the next visit is silent
    const op = (await context.cookies()).filter((c) => c.name === "mock_op_session");
    await context.clearCookies();
    await context.addCookies(op);
    const authorize: string[] = [];
    const forms: string[] = [];
    p.on("request", (r) => {
      if (r.url().startsWith(`${MOCK}/authorize`)) authorize.push(r.url());
    });
    p.on("response", (r) => {
      if (r.url().startsWith(`${MOCK}/authorize`) && r.status() === 200) forms.push(r.url());
    });
    await p.goto("/stats");
    await expect(p).toHaveURL(/\/stats$/);
    await hydrated(p);
    await expect(p.getByRole("button", { name: /^Account menu for/ })).toBeVisible();
    expect(authorize.some((u) => new URL(u).searchParams.get("prompt") === "none"), authorize.join(" ")).toBeTruthy();
    expect(forms, "no form at the provider").toEqual([]);
    // /signed-out never sends anyone to the provider (OD-20)
    const out = await browser.newContext();
    const o = await out.newPage();
    const r = await o.goto("/signed-out");
    expect(r?.status()).toBe(200);
    await expect(o).toHaveURL(/\/signed-out$/);
    await out.close();
    await context.close();
  } finally {
    // back to the sign-in page for everything after this test
    const reset = await page.request.put("/api/v1/admin/settings", { data: { values: { "auth.sign_in_mode": "page" } } });
    if (!reset.ok()) {
      sql("UPDATE instance_settings SET sign_in_mode = 'page'");
    }
  }
});

test("an account without a password links the provider through an emailed one-time link", async ({ page, browser }) => {
  const sinkSmtp = Number(process.env.MAIL_SINK_SMTP_PORT ?? 3013);
  const sinkHttp = `http://127.0.0.1:${process.env.MAIL_SINK_HTTP_PORT ?? 3014}`;
  await ensureProvider(page);
  // mail through the sink (tests/mail-sink.mjs); the admin signed in just now, so it is fresh
  const on = await page.request.put("/api/v1/admin/settings", {
    data: {
      values: {
        "mail.host": "127.0.0.1",
        "mail.port": sinkSmtp,
        "mail.security": "none",
        "mail.from": "Shelf <noreply@example.test>",
      },
    },
  });
  expect(on.ok(), `mail on ${on.status()} ${await on.text()}`).toBeTruthy();
  const user = `pia${Date.now()}`;
  const address = `${user}@example.test`;
  const { context, page: p } = await person(browser);
  try {
    // an account that signs in without a password (as one made through a provider would)
    const made = await p.request.post("/api/v1/auth/sign-up", {
      data: { username: user, display_name: "Pia", password: PASSWORD, email: address },
    });
    expect(made.ok()).toBeTruthy();
    sql(`UPDATE credentials SET password_hash = NULL WHERE user_id = (SELECT id FROM users WHERE username = '${user}')`);
    await context.clearCookies();
    await visit(p, "/sign-in");
    await p.getByRole("link", { name: `Continue with ${NAME}` }).click();
    await atProvider(p, user);
    await expect(p).toHaveURL(/\/sign-in\/link$/);
    await hydrated(p);
    await expect(p.getByRole("button", { name: "Email me a link" })).toBeVisible();
    await checkPage(p, "/sign-in/link email offer");
    await p.getByRole("button", { name: "Email me a link" }).click();
    await expect(p.getByTestId("link-emailed")).toContainText(`We sent a link to p***@example.test`);
    // the message at the sink names the link (quoted-printable may fold it)
    let link = "";
    await expect(async () => {
      const list = await (await p.request.get(`${sinkHttp}/messages?to=${address}`)).json();
      expect(list.length).toBeGreaterThan(0);
      const body = String(list[list.length - 1].data).replace(/=\n/g, "").replace(/=3D/g, "=");
      link = /https?:\/\/\S+\/sign-in\/link\?code=[A-Za-z0-9_-]+/.exec(body)?.[0] ?? "";
      expect(link).not.toBe("");
    }).toPass();
    // another browser cannot use it
    const { context: other, page: q } = await person(browser);
    await visit(q, link);
    await expect(q.getByRole("alert")).toContainText("This link request has expired");
    // the single-use code does not stay in the address or the history (UX4-22)
    await expect(q).toHaveURL(/\/sign-in\/link$/);
    await other.close();
    // this browser: linked and signed in
    await p.goto(link);
    await expect(p).toHaveURL(/127\.0\.0\.1:\d+\/$/);
    await visit(p, "/settings/account");
    await p.getByRole("button", { name: /^Sign-in/ }).click();
    await expect(p.getByTestId("linked-row").filter({ hasText: NAME })).toBeVisible();
    // once only
    await visit(p, link);
    await expect(p.getByRole("alert")).toBeVisible();
    await expect(p).toHaveURL(/\/sign-in\/link$/);
  } finally {
    await context.close();
    const off = await page.request.put("/api/v1/admin/settings", {
      data: { values: { "mail.host": null, "mail.port": null, "mail.security": null, "mail.from": null } },
    });
    expect(off.ok(), `mail off ${off.status()}`).toBeTruthy();
  }
});
