import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations, expectReflow, hydrated, signUp, importDone } from "./helpers";

// WP4-6 (design.md 6.1, 6.6, 9; research R-04): the Stats page compares this year so far with
// the same days last year and lists the people seen most; opt-in goals are set, edited and
// deleted there, show "n of m in <year>" with a bar, never judge the pace and never appear on
// Home; the History side panel carries the comparison too.

const YEAR = new Date().getUTCFullYear();

async function seed(page: Page, stamp: number): Promise<Record<string, string>> {
  const rows = [
    { source_id: "1", kind: "movie", title: `Quiet Pier ${stamp}`, year: 2020, status: "planned" },
    { source_id: "2", kind: "movie", title: `Low Tide ${stamp}`, year: 2021, status: "planned" },
    { source_id: "3", kind: "book", title: `Paper Moors ${stamp}`, year: 2019, status: "planned" },
  ];
  const file = [JSON.stringify({ format: "library-export", version: 1 }), ...rows.map((r) => JSON.stringify(r))].join("\n");
  const started = await page.request.post("/api/v1/imports?format=generic-jsonl", { data: file, headers: { "content-type": "application/octet-stream" } });
  expect(started.status()).toBe(202);
  await importDone(page, (await started.json()).id);
  const lib = await (await page.request.get("/api/v1/library?limit=50")).json();
  return Object.fromEntries(lib.items.map((i: { item: { title: string; id: string } }) => [i.item.title.split(" ").slice(0, 2).join(" "), i.item.id]));
}

async function log(page: Page, item: string, progress: object, at: string) {
  const r = await page.request.post(`/api/v1/items/${item}/progress`, { data: { progress, occurred_at: at, with: [] } });
  expect(r.ok(), `log: ${r.status()}`).toBeTruthy();
}

async function open(page: Page, path: string) {
  await page.goto(path);
  await hydrated(page);
}

test("goals are set, edited and deleted on Stats, never on Home; the year comparison; axe and reflow", async ({ page }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  await signUp(page, `goal${stamp}`, "Gwen");
  const ids = await seed(page, stamp);
  // January 2 at noon UTC is the same local day in every zone
  await log(page, ids["Quiet Pier"], { type: "finished" }, `${YEAR}-01-02T12:00:00Z`);
  await log(page, ids["Low Tide"], { type: "finished" }, `${YEAR - 1}-01-02T12:00:00Z`);
  await log(page, ids["Paper Moors"], { type: "paged", page: 120 }, `${YEAR}-01-02T12:00:00Z`);

  await open(page, "/stats");
  const compare = page.getByTestId("st-compare");
  await expect(compare.getByRole("heading", { name: `${YEAR} so far` })).toBeVisible();
  await expect(compare).toContainText(`Watch: Watched1 in ${YEAR}, 1 in ${YEAR - 1}`);
  await expect(compare).toContainText(`Read: Pages120 in ${YEAR}, 0 in ${YEAR - 1}`);
  await expect(page.getByTestId("st-often")).toContainText("Log something with a person");
  const goals = page.getByTestId("st-goals");
  await expect(goals.getByTestId("st-goal")).toHaveCount(0);

  // set a goal behind the disclosure
  await goals.getByRole("button", { name: /^Set a goal/ }).click();
  await page.getByLabel("Year", { exact: true }).selectOption(String(YEAR));
  await page.getByLabel("Group", { exact: true }).selectOption("read");
  await page.getByLabel("Measure", { exact: true }).selectOption("pages");
  await page.getByLabel("Target", { exact: true }).fill("500");
  await goals.getByRole("button", { name: "Save goal" }).click();
  const card = goals.getByTestId("st-goal");
  await expect(card).toHaveCount(1);
  await expect(card).toContainText("Read: Pages");
  await expect(card).toContainText(`120 of 500 in ${YEAR}`);
  await expect(card.getByRole("progressbar", { name: `Read: Pages in ${YEAR}` })).toHaveAttribute("aria-valuetext", `120 of 500 in ${YEAR}`);
  // one goal per year, group and measure
  await page.getByLabel("Target", { exact: true }).fill("100");
  await goals.getByRole("button", { name: "Save goal" }).click();
  await expect(goals.getByRole("alert")).toContainText("There is a goal for this year, group and measure already.");
  await expect(goals).not.toContainText(/behind|ahead/i);
  await expectNoA11yViolations(page, "stats with a goal");

  // edit
  await card.getByRole("button", { name: "Edit goal Read: Pages" }).click();
  const edit = page.getByRole("dialog", { name: "Edit goal Read: Pages" });
  await edit.getByLabel("Target", { exact: true }).fill("100");
  await edit.getByRole("button", { name: "Save goal" }).click();
  await expect(edit).toHaveCount(0);
  await expect(card).toContainText(`120 of 100 in ${YEAR}`);

  await page.setViewportSize({ width: 360, height: 800 });
  await expectReflow(page, "stats 360");
  await expectNoA11yViolations(page, "stats 360");
  await page.setViewportSize({ width: 1280, height: 800 });

  // never on Home (R-04)
  await open(page, "/");
  await expect(page.getByRole("heading", { name: "All-time stats" })).toBeVisible();
  await expect(page.getByTestId("st-goal")).toHaveCount(0);
  await expect(page.getByRole("main")).not.toContainText(`of 100 in ${YEAR}`);

  // the History side panel compares too
  await open(page, "/history");
  await expect(page.getByTestId("history-compare")).toContainText(`Watch: Watched1 in ${YEAR}, 1 in ${YEAR - 1}`);

  // delete
  await open(page, "/stats");
  await page.getByTestId("st-goal").getByRole("button", { name: "Delete goal Read: Pages" }).click();
  const confirm = page.getByRole("dialog", { name: "Delete the goal Read: Pages?" });
  await expect(confirm).toContainText("Only the goal goes; everything you logged stays.");
  await confirm.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByTestId("st-goal")).toHaveCount(0);
  const goalsLeft = await (await page.request.get("/api/v1/stats/goals")).json();
  expect(goalsLeft).toEqual([]);
});

test("the people seen most often are listed on Stats", async ({ page }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  await signUp(page, `often${stamp}`, "Otto");
  const ids = await seed(page, stamp);
  const mk = async (name: string) => (await (await page.request.post("/api/v1/companions", { data: { name } })).json()).id as string;
  const [ria, sam] = [await mk("Ria"), await mk("Sam")];
  for (const [item, who] of [
    [ids["Quiet Pier"], [ria, sam]],
    [ids["Low Tide"], [ria]],
  ] as [string, string[]][]) {
    const r = await page.request.post(`/api/v1/items/${item}/progress`, { data: { progress: { type: "finished" }, occurred_at: `${YEAR}-01-02T12:00:00Z`, with: who } });
    expect(r.ok()).toBeTruthy();
  }
  await open(page, "/stats");
  const often = page.getByTestId("st-often");
  await expect(often.getByRole("listitem")).toHaveCount(2);
  await expect(often.getByRole("listitem").first()).toContainText("Ria2 times together");
  await expect(often.getByRole("listitem").nth(1)).toContainText("Samonce together");
  await expect(often.getByRole("link", { name: "Ria" })).toHaveAttribute("href", `/people/${ria}`);
});
