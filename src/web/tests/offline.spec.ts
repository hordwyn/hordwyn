import { test, expect, type Page } from "@playwright/test";
import { changeElsewhere, changeMine, countEntityEvents, entityEvents, entityIds, expectNoA11yViolations, expectReflow, hydrated, importDone, signIn, signUp } from "./helpers";

// WP4-5 offline queue (DEC-237, design.md 5.4, 2.4, 6.15): offline, the next step is queued
// with "Saved offline, will sync" and Undo; the banner counts the waiting changes; back online
// (or on the next page load) the queue goes out once; Undo while offline leaves nothing; a
// refused command is dropped with a toast naming it; sign-out sends the queue first or asks
// (UX4-6); the banner passes axe and reflows at 360 px (UX4-20); offline, a background refresh
// never replaces Home with an error.

const ROWS = [
  {
    source_id: "1",
    kind: "show",
    title: "Downstream",
    year: 2020,
    status: "in_progress",
    history: [{ date: "2026-02-03", progress: { type: "episode", season: 1, episode: 1 } }],
  },
];

async function seed(page: Page): Promise<string> {
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...ROWS.map((r) => JSON.stringify(r))].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  expect(started.status()).toBe(202);
  const { id } = await started.json();
  await importDone(page, id);
  const lib = await (await page.request.get("/api/v1/library?limit=5")).json();
  return lib.items[0].item.id;
}

/** The episodes logged for `item`, as `S1E2` strings. */
async function episodes(page: Page, item: string): Promise<string[]> {
  const events = await (await page.request.get(`/api/v1/items/${item}/progress`)).json();
  return events
    .filter((e: { progress: { type: string } }) => e.progress.type === "episode")
    .map((e: { progress: { season: number; episode: number } }) => `S${e.progress.season}E${e.progress.episode}`)
    .sort();
}

async function home(page: Page) {
  await page.goto("/");
  await hydrated(page);
  return page.getByTestId("continue-card").filter({ hasText: "Downstream" });
}

test("offline the next step is queued, counted and sent once when back online", async ({ page, context }) => {
  test.setTimeout(120_000);
  await signUp(page, `offline${Date.now()}`);
  const item = await seed(page);
  const card = await home(page);

  await context.setOffline(true);
  await expect(page.getByText("You are offline.")).toBeVisible();
  await card.getByRole("button", { name: "Watched S1 E2: Downstream" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Saved offline, will sync" })).toBeVisible();
  await expect(page.getByText("1 change waiting")).toBeVisible();

  await context.setOffline(false);
  await expect(page.getByText("1 change waiting")).toHaveCount(0);
  await expect.poll(() => episodes(page, item)).toEqual(["S1E1", "S1E2"]);
  // a reload flushes an empty queue: still logged once
  await home(page);
  await expect.poll(() => episodes(page, item)).toEqual(["S1E1", "S1E2"]);
});

test("a change queued offline is sent on the next page load", async ({ page, context }) => {
  test.setTimeout(120_000);
  await signUp(page, `offload${Date.now()}`);
  const item = await seed(page);
  const card = await home(page);
  await context.setOffline(true);
  await card.getByRole("button", { name: "Watched S1 E2: Downstream" }).click();
  await expect(page.getByText("1 change waiting")).toBeVisible();
  await page.close();

  await context.setOffline(false);
  const again = await context.newPage();
  await again.goto("/");
  await hydrated(again);
  await expect.poll(() => episodes(again, item)).toEqual(["S1E1", "S1E2"]);
  await expect(again.getByText("change waiting")).toHaveCount(0);
});

test("Undo while offline takes the change out of the queue", async ({ page, context }) => {
  test.setTimeout(120_000);
  await signUp(page, `offundo${Date.now()}`);
  const item = await seed(page);
  const card = await home(page);

  await context.setOffline(true);
  await card.getByRole("button", { name: "Watched S1 E2: Downstream" }).click();
  const toast = page.getByRole("status").filter({ hasText: "Saved offline, will sync" });
  await expect(toast).toBeVisible();
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.getByText("change waiting")).toHaveCount(0);

  await context.setOffline(false);
  await home(page);
  await expect.poll(() => episodes(page, item)).toEqual(["S1E1"]);
});

/** Keys this browser keeps for `user` (queue and recent items). */
async function storedFor(page: Page, user: string): Promise<string[]> {
  return page.evaluate((u) => Object.keys(localStorage).filter((k) => k.endsWith(`-${u}`)), user);
}

async function signOut(page: Page) {
  await page.getByRole("button", { name: /^Account menu for/ }).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();
}

/** Queue the next step on Home while the sync push cannot get through. */
async function queueOne(page: Page, context: import("@playwright/test").BrowserContext) {
  await page.route("**/api/v1/sync/commands", (r) => r.abort());
  const card = await home(page);
  await context.setOffline(true);
  await card.getByRole("button", { name: "Watched S1 E2: Downstream" }).click();
  await expect(page.getByText("1 change waiting")).toBeVisible();
  await context.setOffline(false);
  await expect(page.getByText("You are offline.")).toHaveCount(0);
  await expect(page.getByText("1 change waiting")).toBeVisible();
}

test("sign-out sends waiting changes first; when it cannot, it says how many and the person may stay", async ({ page, context }) => {
  test.setTimeout(150_000);
  const user = `offout${Date.now()}`;
  await signUp(page, user);
  const item = await seed(page);
  await page.goto(`/items/${item}`);
  await hydrated(page);
  await queueOne(page, context);
  expect((await storedFor(page, user)).sort()).toEqual([`app-offline-${user}`, `app-recent-${user}`]);

  // the push fails: nothing is lost without asking
  await signOut(page);
  const ask = page.getByRole("dialog", { name: "One change is not synced yet" });
  await expect(ask).toBeVisible();
  await expect(ask).toContainText("Signing out now discards them.");
  await expectNoA11yViolations(page, "sign-out question");
  await ask.getByRole("button", { name: "Stay signed in" }).click();
  await expect(ask).toHaveCount(0);
  await expect(page.getByText("1 change waiting")).toBeVisible();

  // the server answers again: sign-out sends the change, then signs out and forgets
  await page.unroute("**/api/v1/sync/commands");
  await signOut(page);
  await expect(page).toHaveURL(/\/signed-out$/);
  expect(await storedFor(page, user)).toEqual([]);
  await signIn(page, user);
  await expect.poll(() => episodes(page, item)).toEqual(["S1E1", "S1E2"]);
});

test("sign-out can discard the waiting changes, and leaves nothing of them in the browser", async ({ page, context }) => {
  test.setTimeout(150_000);
  const user = `offdrop${Date.now()}`;
  await signUp(page, user);
  const item = await seed(page);
  await queueOne(page, context);
  await signOut(page);
  const ask = page.getByRole("dialog", { name: "One change is not synced yet" });
  await ask.getByRole("button", { name: "Sign out and discard" }).click();
  await expect(page).toHaveURL(/\/signed-out$/);
  expect(await storedFor(page, user)).toEqual([]);
  await signIn(page, user);
  await expect.poll(() => episodes(page, item)).toEqual(["S1E1"]);
});

test("a change the server refuses is dropped with a toast that names it", async ({ page, context }) => {
  test.setTimeout(120_000);
  await signUp(page, `offref${Date.now()}`);
  await seed(page);
  const card = await home(page);
  await context.setOffline(true);
  await card.getByRole("button", { name: "Watched S1 E2: Downstream" }).click();
  await expect(page.getByText("1 change waiting")).toBeVisible();
  await page.route("**/api/v1/sync/commands", async (route) => {
    const body = route.request().postDataJSON() as { commands: { op_id: string }[] };
    const results = body.commands.map((c) => ({
      op_id: c.op_id,
      ok: false,
      error: { code: "not_found", message: "gone", retry_after_secs: null, field: null },
    }));
    await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(results) });
  });
  await context.setOffline(false);
  const dropped = page.getByRole("status").locator("> div").filter({ hasText: "Not saved: Season 1, episode 2" });
  await expect(dropped).toBeVisible();
  await expect(dropped).toContainText("Downstream");
  await expect(page.getByText("change waiting")).toHaveCount(0);
});

/** Two frames: long enough for the effects of an event or an answer to have run. */
const frames = (page: Page) => page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));

test("offline, an event leaves Home as it is: the next step still works; others' imports send none", async ({ page, baseURL }) => {
  // The event stream can outlive the network (a half-open connection; Playwright's offline
  // mode often keeps it): every event refetched Home, the refetch failed and Home turned into
  // "Home could not be loaded" under the pointer. The browser's own `offline` event stands in
  // for the network here, so the stream delivers for sure. The events were other people's
  // imports (sent to everyone); they no longer reach this page (DEC-490), so the user's own
  // change stands in for them.
  test.setTimeout(240_000);
  await countEntityEvents(page);
  await signUp(page, `offevt${Date.now()}`);
  const item = await seed(page);
  const card = await home(page);
  const failedHome = page.getByRole("alert").filter({ hasText: "Home could not be loaded" });
  const homeCalls: string[] = [];
  page.on("request", (r) => {
    if (r.url().includes("/_fn/home")) homeCalls.push(r.url());
  });

  // offline: the refresh an event asks for waits
  await page.evaluate(() => window.dispatchEvent(new Event("offline")));
  await expect(page.getByText("You are offline.")).toBeVisible();
  homeCalls.length = 0;
  const seen = await entityEvents(page);
  // another account's import sends this page nothing (it went to everyone, DEC-490); the
  // user's own change does
  const elsewhere = await changeElsewhere(baseURL!);
  await changeMine(page);
  await expect.poll(() => entityEvents(page), { timeout: 60_000 }).toBeGreaterThan(seen);
  await frames(page);
  expect(await entityIds(page)).not.toContain(elsewhere);
  expect(homeCalls).toEqual([]);
  await expect(failedHome).toHaveCount(0);
  // back online it runs once
  const caughtUp = page.waitForResponse((r) => r.url().includes("/_fn/home"));
  await page.evaluate(() => window.dispatchEvent(new Event("online")));
  await caughtUp;

  // a refetch that does not reach the server keeps what Home shows
  await page.route("**/_fn/home*", (r) => r.abort("internetdisconnected"));
  const lost = page.waitForEvent("requestfailed", { predicate: (r) => r.url().includes("/_fn/home"), timeout: 120_000 });
  await changeMine(page);
  await lost;
  await frames(page);
  await expect(failedHome).toHaveCount(0);
  await page.unroute("**/_fn/home*");
  await card.getByRole("button", { name: "Watched S1 E2: Downstream" }).click();
  await expect(page.getByText("Logged: Watched S1 E2").first()).toBeVisible();
  await expect.poll(() => episodes(page, item)).toEqual(["S1E1", "S1E2"]);
});

test.describe("the offline banner at 360 px", () => {
  test.use({ viewport: { width: 360, height: 640 } });

  test("passes axe in both themes and reflows", async ({ page, context }) => {
    // a reload in each theme, then offline and the next step (the approach of the first
    // version of this test, which met the failed refetch above)
    test.setTimeout(150_000);
    await signUp(page, `offaxe${Date.now()}`);
    await seed(page);
    for (const theme of ["light", "dark"] as const) {
      await page.evaluate((t) => localStorage.setItem("app-theme", t), theme);
      const card = await home(page);
      await context.setOffline(true);
      await expect(page.getByText("You are offline.")).toBeVisible();
      await card.getByRole("button", { name: /^Watched S1 E\d+: Downstream$/ }).click();
      await expect(page.getByText("1 change waiting")).toBeVisible();
      await expectNoA11yViolations(page, `offline banner ${theme}`);
      await expectReflow(page, `offline banner ${theme}`);
      await context.setOffline(false);
      await expect(page.getByText("change waiting")).toHaveCount(0);
    }
  });
});
