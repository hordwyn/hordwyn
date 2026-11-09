import { test, expect, type Page } from "@playwright/test";
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
    const failed = page.getByTestId("nf-log").getByRole("row").filter({ hasText: "Failed" }).first();
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
