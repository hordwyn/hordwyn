import { test, expect, type Page } from "./fixtures";
import { addPerson, ADMIN, expectNoA11yViolations, expectReflow, hydrated, importDone, requestLink, signIn, signUp } from "./helpers";

// Settings > Notifications (design.md 6.8, 5.8; WP3-5): the inbox grouped by day with "Mark
// all read"; a signed webhook added with "Add and send a test" (the result on its card); the
// rules matrix with an instance default, a switch and the defaults offer for a new channel;
// the delivery log with its error to unfold; axe and reflow at 360 px on every tab.

const PAGE = "/settings/notifications";

async function tab(page: Page, name: string | RegExp) {
  await page.getByRole("tab", { name }).click();
  await expect(page.getByRole("tab", { name })).toHaveAttribute("aria-selected", "true");
}

async function addChannel(page: Page, kind: string, name: string, fields: Record<string, string>) {
  await page.getByRole("radio", { name: kind }).click();
  await page.getByLabel("Name", { exact: true }).fill(name);
  for (const [label, value] of Object.entries(fields)) {
    await page.getByLabel(label, { exact: true }).fill(value);
  }
  await page.getByRole("button", { name: "Add and send a test" }).click();
}

function card(page: Page, name: string) {
  return page.getByRole("list", { name: "Channels" }).getByRole("listitem").filter({ hasText: name });
}

function cell(page: Page, event: string, channel: string) {
  return page.getByRole("switch", { name: `${event} to ${channel}` });
}

test("channels, rules with the defaults offer, the log and the inbox", async ({ page, browser }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  // an instance default the admin set (Admin > Notification defaults shows them read-only)
  const admin = await browser.newContext();
  const adminPage = await admin.newPage();
  await signIn(adminPage, ADMIN);
  const made = await adminPage.request.put("/api/v1/notifications/subscriptions", {
    data: { event: "boardgame.loan_due", instance_default: true, targets: { type: "all" } },
  });
  expect(made.ok(), `default: ${made.status()}`).toBeTruthy();
  const defaultId = (await made.json()).id as string;
  const other = await browser.newContext();
  try {
    const user = `npia${stamp}`;
    await signUp(page, user, "Pia");
    await page.goto(PAGE);
    await hydrated(page);
    // nothing unread: the page opens on the channels
    await expect(page.getByRole("tab", { name: "Channels" })).toHaveAttribute("aria-selected", "true");

    // the constraint is checked when leaving the field
    await page.getByRole("radio", { name: "Webhook (signed)" }).click();
    await page.getByLabel("URL", { exact: true }).fill("hooks.example.org");
    await page.getByLabel("Name", { exact: true }).click();
    await expect(page.getByText("Enter an address that starts with https:// or http://.")).toBeVisible();

    // a loopback target is refused by the server's guard: the test fails, on the card
    await addChannel(page, "Webhook (signed)", "Automation", { URL: "http://127.0.0.1:9/in" });
    const automation = card(page, "Automation");
    await expect(automation.getByRole("alert")).toContainText("The test was not delivered");
    await expect(automation.getByText("Signing secret")).toBeVisible();
    await expect(automation.getByText("Never used")).toBeVisible();

    await tab(page, "Rules");
    const loanRow = page.getByRole("row").filter({ has: page.getByRole("rowheader", { name: /Lent game due back/ }) });
    await expect(loanRow).toContainText("Instance default");
    await expect(cell(page, "Lent game due back", "Automation")).toHaveAttribute("aria-checked", "true");
    await cell(page, "Link request", "Automation").click();
    await expect(cell(page, "Link request", "Automation")).toHaveAttribute("aria-checked", "true");
    await cell(page, "Lent game due back", "Automation").click();
    await expect(cell(page, "Lent game due back", "Automation")).toHaveAttribute("aria-checked", "false");
    await expect(page.getByRole("row").filter({ has: page.getByRole("rowheader", { name: /Lent game due back/ }) })).not.toContainText(
      "Instance default",
    );

    // a second channel: its first switch offers the instance's suggestion
    await tab(page, "Channels");
    await addChannel(page, "ntfy", "Phone", { "Server URL": "http://127.0.0.1:9", Topic: "my-topic" });
    await expect(card(page, "Phone").getByRole("alert")).toContainText("The test was not delivered");
    await tab(page, "Rules");
    await cell(page, "Link request", "Phone").click();
    const offer = page.getByRole("status").filter({ hasText: "Phone is new" });
    await expect(offer).toContainText("Lent game due back");
    await offer.getByRole("button", { name: "Use the suggestions" }).click();
    await expect(cell(page, "Link request", "Phone")).toHaveAttribute("aria-checked", "true");
    await expect(cell(page, "Lent game due back", "Phone")).toHaveAttribute("aria-checked", "true");
    await expect(cell(page, "Link request", "Automation")).toHaveAttribute("aria-checked", "true");
    await expect(cell(page, "Lent game due back", "Automation")).toHaveAttribute("aria-checked", "false");

    // someone asks Pia to confirm a person: inbox row, and deliveries that fail
    const max = await other.newPage();
    await signUp(max, `nmax${stamp}`, "Max");
    await max.goto("/people");
    await hydrated(max);
    await addPerson(max, "P.");
    await requestLink(max, "P.", user);

    // the worker tries the delivery; the log shows it failed, with the error to unfold
    await expect(async () => {
      await page.goto(PAGE);
      await hydrated(page);
      await tab(page, "Log");
      await expect(page.getByTestId("nf-log").getByText("Failed").first()).toBeVisible({ timeout: 2_000 });
    }).toPass({ timeout: 30_000 });
    // the log follows the second delivery when that fails next (sec0): the row is named by
    // its delivery, not by its place among the failed ones
    const delivery = await page.getByTestId("nf-log").getByRole("row").filter({ hasText: "Failed" }).first().getAttribute("data-delivery");
    expect(delivery).toBeTruthy();
    const failed = page.getByTestId("nf-log").locator(`[data-delivery="${delivery}"]`);
    await expect(failed).toContainText("Failed");
    await expect(failed).toContainText("Link request");
    await failed.getByText("Show the error").click();
    await expect(failed.locator("details p")).toBeVisible();
    await tab(page, "Channels");
    await expect(card(page, "Automation").getByText(/^Last delivery failed/)).toBeVisible();

    // the inbox, grouped by day; Mark all read
    await tab(page, /^Inbox/);
    await expect(page.getByRole("heading", { name: "Today", level: 2 })).toBeVisible();
    await expect(page.getByRole("listitem").filter({ hasText: 'Max says you are "P."' })).toBeVisible();
    await page.getByRole("button", { name: "Mark all read" }).click();
    await expect(page.getByRole("button", { name: /^Mark as read/ })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Mark all read" })).toHaveCount(0);

    await page.setViewportSize({ width: 360, height: 740 });
    for (const name of [/^Inbox/, "Channels", "Rules", "Log"]) {
      await tab(page, name);
      await expectReflow(page, `360 ${name}`);
      await expectNoA11yViolations(page, `360 ${name}`);
    }
  } finally {
    await adminPage.request.delete(`/api/v1/notifications/subscriptions/${defaultId}`);
    await admin.close();
    await other.close();
  }
});

test("a channel is edited in place keeping its secret; a rule for one item (DEC-344)", async ({ page }) => {
  test.setTimeout(90_000);
  const stamp = Date.now();
  await signUp(page, `nedit${stamp}`, "Eda");
  const title = `Signal Ridge ${stamp}`;
  const file = [JSON.stringify({ format: "library-export", version: 1 }), JSON.stringify({ source_id: "1", kind: "movie", title, year: 2012, status: "planned" })].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  const { id: importId } = await started.json();
  await page.goto(PAGE);
  await hydrated(page);
  // the import started above may have finished already: its "Import finished" is unread
  // then and the page opens on the inbox (sec0: seen once in ten runs)
  await tab(page, "Channels");

  await addChannel(page, "ntfy", "Phone", { "Server URL": "http://127.0.0.1:9", Topic: "first-topic", "Access token (optional)": "tk_secret" });
  const phone = card(page, "Phone");
  await expect(phone.getByRole("alert")).toContainText("The test was not delivered");
  await phone.getByRole("button", { name: "Edit Phone" }).click();
  const name = phone.getByLabel("Name", { exact: true });
  await expect(name).toHaveValue("Phone");
  await expect(phone.getByLabel("Topic", { exact: true })).toHaveValue("first-topic");
  // secrets are write-only: never shown, blank keeps the stored one
  await expect(phone.getByLabel("Access token (optional)", { exact: true })).toHaveValue("");
  await name.fill("Pocket");
  await phone.getByLabel("Topic", { exact: true }).fill("second-topic");
  const put = page.waitForRequest((r) => r.method() === "PUT" && r.url().includes("/api/v1/notifications/channels/"));
  await phone.getByRole("button", { name: "Save" }).click();
  const body = (await put).postDataJSON();
  expect(body.name).toBe("Pocket");
  expect(body.config).toMatchObject({ kind: "ntfy", topic: "second-topic", token: "********" });
  const pocket = card(page, "Pocket");
  await expect(pocket).toContainText("second-topic");
  const channels = await (await page.request.get("/api/v1/notifications/channels")).json();
  const saved = channels.find((c: { name: string }) => c.name === "Pocket");
  expect(saved.config.token, "the stored token is kept (still set, shown redacted)").toBe("********");
  await expectNoA11yViolations(page, "channel edit");

  // a rule for one item, picked with the search palette
  await importDone(page, importId);
  await tab(page, "Rules");
  await page.getByText("Add a rule for one list, item or person").click();
  await page.getByRole("button", { name: "Choose an item..." }).click();
  const palette = page.getByRole("dialog", { name: "Search" });
  await expect(palette.getByTestId("palette-target")).toHaveText("Choosing a result makes it the subject of the rule.");
  await palette.getByRole("combobox").fill(title);
  const option = palette.getByRole("option", { name: new RegExp(`^${title}`) }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(palette).toHaveCount(0);
  await expect(page.locator("#nf-r-for")).toHaveValue(/^item:/);
  await expect(page.locator("#nf-r-for")).toContainText(`Item: ${title}`);
  await page.getByRole("button", { name: "Add rule" }).click();
  await expect(page.getByRole("list").filter({ hasText: "one item" }).first()).toBeVisible();
  const subs = await (await page.request.get("/api/v1/notifications/subscriptions")).json();
  expect(subs.some((s: { filter: { items?: string[] } }) => (s.filter?.items ?? []).length === 1)).toBe(true);
});

test("the actions of a channel card share one size and none is filled", async ({ page }) => {
  // OT-12 card-footer rule on a second card page
  test.setTimeout(60_000);
  await signUp(page, `nfoot${Date.now()}`, "Nia");
  await page.goto(PAGE);
  await hydrated(page);
  await addChannel(page, "Webhook (signed)", "Footer", { URL: "http://127.0.0.1:9/in" });
  const c = card(page, "Footer");
  const actions = [
    c.getByRole("button", { name: "Send test to Footer" }),
    c.getByRole("button", { name: "Edit Footer" }),
    c.getByRole("button", { name: "Delete Footer" }),
  ];
  const looks = [];
  for (const action of actions) {
    await expect(action).toBeVisible();
    looks.push(
      await action.evaluate((el) => {
        const s = getComputedStyle(el);
        return { height: el.getBoundingClientRect().height, font: s.fontSize, weight: s.fontWeight, radius: s.borderRadius, border: s.borderTopWidth };
      }),
    );
  }
  expect(looks[0].height).toBe(32);
  expect(looks[1]).toEqual(looks[0]);
  expect(looks[2]).toEqual(looks[0]);
  await page.setViewportSize({ width: 390, height: 800 });
  for (const action of actions) {
    expect((await action.boundingBox())!.height).toBe(40);
  }
});

// sec0: the open page and a delivery that the worker finishes on its own time. The first
// test above once failed at "Last delivery failed": its page was read after the first of two
// deliveries had failed and before the second one, and nothing told the page about the second.

/** Pia with the webhook "Automation" that gets "Link request"; the page is left on Rules. */
async function ruledChannel(page: Page, user: string) {
  await signUp(page, user, "Pia");
  await page.goto(PAGE);
  await hydrated(page);
  await addChannel(page, "Webhook (signed)", "Automation", { URL: "http://127.0.0.1:9/in" });
  await expect(card(page, "Automation").getByRole("alert")).toContainText("The test was not delivered");
  await tab(page, "Rules");
  await cell(page, "Link request", "Automation").click();
  await expect(cell(page, "Link request", "Automation")).toHaveAttribute("aria-checked", "true");
}

/** Max asks `user` to confirm a person; resolves once the delivery to "Automation" has failed on the server. */
async function failedDelivery(page: Page, max: Page, stamp: number, user: string) {
  await signUp(max, `nmax${stamp}`, "Max");
  await max.goto("/people");
  await hydrated(max);
  await addPerson(max, "P.");
  await requestLink(max, "P.", user);
  await expect
    .poll(
      async () => {
        const channels = await (await page.request.get("/api/v1/notifications/channels")).json();
        return channels.find((c: { name: string }) => c.name === "Automation")?.health?.state;
      },
      { timeout: 30_000 },
    )
    .toBe("failing");
}

test("a delivery that fails while the page is open shows on its channel without a reload", async ({ page, browser }) => {
  test.setTimeout(90_000);
  const stamp = Date.now();
  const user = `nlive${stamp}`;
  const other = await browser.newContext();
  try {
    await ruledChannel(page, user);
    // a fresh tab, so that no answer of the page above is taken for one of this page: when
    // its event stream has opened, the page reads once more (the test below); that read is
    // done before any read is kept back here
    page = await page.context().newPage();
    const readOnOpening = page.waitForResponse((r) => r.url().includes("/_fn/notifications"));
    await page.goto(PAGE);
    await hydrated(page);
    await readOnOpening;
    const automation = card(page, "Automation");
    await expect(automation.getByText("Never used")).toBeVisible();
    await page.getByLabel("Name", { exact: true }).fill("Half typed");
    // the read that the new inbox row asks for is kept back, so it cannot be the one that
    // shows the outcome: only the announcement of the delivery can bring the card up to date
    let release = () => {};
    const kept = new Promise<void>((r) => (release = r));
    let held = 0;
    await page.route("**/_fn/notifications*", async (route) => {
      if (held === 0) {
        held += 1;
        await kept;
      }
      await route.continue();
    });
    await failedDelivery(page, await other.newPage(), stamp, user);
    await expect(automation.getByText(/^Last delivery failed/)).toBeVisible();
    expect(held).toBe(1);
    // the card changed on its own: the page's polite live region says so (SEC0-2)
    await expect(page.getByRole("status").filter({ hasText: "Automation" })).toHaveText("Automation: the last delivery failed");
    // shown in place: the page was not built anew, the form keeps what was typed
    await expect(page.getByLabel("Name", { exact: true })).toHaveValue("Half typed");
    release();
    await expect(page.getByRole("tab", { name: /^Inbox/ })).toContainText("1 unread");
    await expect(automation.getByText(/^Last delivery failed/)).toBeVisible();
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await other.close();
  }
});

test("a delivery that failed before the page's event stream opened shows once it opens", async ({ page, browser }) => {
  test.setTimeout(90_000);
  const stamp = Date.now();
  const user = `ngap${stamp}`;
  const other = await browser.newContext();
  try {
    await ruledChannel(page, user);
    // the page is read, then its event stream is kept from opening until the delivery has
    // failed: the page hears neither of the inbox row nor of the delivery
    let open = () => {};
    const opened = new Promise<void>((r) => (open = r));
    await page.route("**/api/v1/events", async (route) => {
      await opened;
      await route.continue();
    });
    await page.goto(PAGE);
    await hydrated(page);
    const automation = card(page, "Automation");
    await expect(automation.getByText("Never used")).toBeVisible();
    await failedDelivery(page, await other.newPage(), stamp, user);
    await expect(automation.getByText("Never used")).toBeVisible();
    open();
    await expect(automation.getByText(/^Last delivery failed/)).toBeVisible();
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await other.close();
  }
});

// Review of sec0 (SEC0-3): the page is built once and every later read is shown in place. A
// new inbox row or a save on the page once built the whole page anew, which emptied a form
// that someone was filling in and dropped the focus.
test("an open form keeps its text and the focus when an inbox row arrives and after a save on the page", async ({ page, browser }) => {
  test.setTimeout(90_000);
  const stamp = Date.now();
  const user = `nform${stamp}`;
  const other = await browser.newContext();
  try {
    await signUp(page, user, "Pia");
    await page.goto(PAGE);
    await hydrated(page);
    await addChannel(page, "Webhook (signed)", "Automation", { URL: "http://127.0.0.1:9/in" });
    const automation = card(page, "Automation");
    await expect(automation.getByRole("alert")).toContainText("The test was not delivered");

    // an edit form half changed, and a new channel half typed with the focus in its name
    await automation.getByRole("button", { name: "Edit Automation" }).click();
    const edited = automation.getByLabel("Name", { exact: true });
    await edited.fill("Robot");
    const add = page.getByRole("region", { name: "Add a channel" });
    await add.getByRole("radio", { name: "Gotify" }).click();
    const server = add.getByLabel("Server URL", { exact: true });
    await server.fill("https://half.example.org");
    const name = add.getByLabel("Name", { exact: true });
    await name.fill("Half typed");
    await expect(name).toBeFocused();

    // a new inbox row arrives: the tab counts it, the forms and the focus are untouched
    const max = await other.newPage();
    await signUp(max, `nmax${stamp}`, "Max");
    await max.goto("/people");
    await hydrated(max);
    await addPerson(max, "P.");
    await requestLink(max, "P.", user);
    await expect(page.getByRole("tab", { name: /^Inbox/ })).toContainText("1 unread");
    await expect(name).toHaveValue("Half typed");
    await expect(name).toBeFocused();
    await expect(add.getByRole("radio", { name: "Gotify" })).toBeChecked();
    await expect(server).toHaveValue("https://half.example.org");
    await expect(edited).toHaveValue("Robot");

    // a save on the page: the card takes the new name in place, the other form stays
    await automation.getByRole("button", { name: "Save" }).click();
    await expect(card(page, "Robot")).toBeVisible();
    await expect(card(page, "Robot").getByRole("button", { name: "Edit Robot" })).toBeVisible();
    await expect(name).toHaveValue("Half typed");
    await expect(server).toHaveValue("https://half.example.org");
    await expect(add.getByRole("radio", { name: "Gotify" })).toBeChecked();

    // and marking the row read, from the inbox
    await tab(page, /^Inbox/);
    await page.getByRole("button", { name: "Mark all read" }).click();
    await expect(page.getByRole("button", { name: "Mark all read" })).toHaveCount(0);
    await tab(page, "Channels");
    await expect(name).toHaveValue("Half typed");
  } finally {
    await other.close();
  }
});
