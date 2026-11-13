import { test, expect, type Page } from "./fixtures";

// Review SEC1-2 (DEC-660): the standing proof that the policy guard of tests/fixtures.ts
// still fires. Every other spec is green when the guard sees nothing, also when it sees
// nothing because an upgrade of Playwright or of the browser made it blind (another way to
// the built-in context, another wording of the console message, a binding that is not
// there). So these tests break a policy on purpose, with a document of their own (the
// server is not asked: the route answers):
//
// - "sees": both signs arrive, for the built-in page and for the contexts a spec opens
//   itself; the test then empties the list, which is the one deliberate way past the guard.
// - "fails": nothing is emptied, so the teardown of the guard has to fail the test.
//   `test.fail()` turns that into the expected outcome: when the guard stays quiet the test
//   passes and Playwright reports it as "expected to fail, but passed".

const CANARY = "/policy-guard-canary";

/** Open a document that breaks its own policy: an inline script under `script-src 'none'`. */
async function breakThePolicy(page: Page, baseURL: string | undefined) {
  await page.route(`**${CANARY}`, (route) =>
    route.fulfill({
      status: 200,
      headers: { "content-type": "text/html; charset=utf-8", "content-security-policy": "script-src 'none'" },
      body: '<!doctype html><html lang="en"><title>canary</title><p id="canary">canary</p><script>document.title = "ran";</script></html>',
    }),
  );
  await page.goto(`${baseURL}${CANARY}`);
}

/** Both signs of the guard are there: the event of the document and the browser's console line. */
async function expectBothSigns(page: Page, found: string[]) {
  await expect(page.locator("#canary")).toBeVisible();
  await expect(page, "the inline script did not run").toHaveTitle("canary");
  await expect
    .poll(() => found.filter((v) => v.startsWith("event on ") && v.includes("script-src")).length, { message: `the event: ${found.join(" | ")}` })
    .toBeGreaterThan(0);
  await expect.poll(() => found.filter((v) => v.startsWith("console (")).length, { message: `the console line: ${found.join(" | ")}` }).toBeGreaterThan(0);
}

/** Wait for a sign without failing when none comes: whether the test fails is the guard's business. */
async function aSignOrTenSeconds(page: Page, found: string[]) {
  for (let i = 0; i < 100 && found.length === 0; i += 1) {
    await page.waitForTimeout(100);
  }
}

test("the guard sees a violation on the built-in page", async ({ page, baseURL, policyViolations }) => {
  await breakThePolicy(page, baseURL);
  await expectBothSigns(page, policyViolations);
  policyViolations.length = 0;
});

test("the guard sees a violation in a context the spec opened", async ({ browser, baseURL, policyViolations }) => {
  const context = await browser.newContext();
  try {
    const page = await context.newPage();
    await breakThePolicy(page, baseURL);
    await expectBothSigns(page, policyViolations);
  } finally {
    await context.close();
  }
  policyViolations.length = 0;
});

test("the guard sees a violation on a page the spec opened from the browser", async ({ browser, baseURL, policyViolations }) => {
  const page = await browser.newPage();
  try {
    await breakThePolicy(page, baseURL);
    await expectBothSigns(page, policyViolations);
  } finally {
    await page.close();
  }
  policyViolations.length = 0;
});

test("the guard fails a test whose built-in page broke a policy", async ({ page, baseURL, policyViolations }) => {
  test.fail(true, "the canary: the teardown of the policy guard must fail this test");
  await breakThePolicy(page, baseURL);
  await aSignOrTenSeconds(page, policyViolations);
});

test("the guard fails a test whose own context broke a policy", async ({ browser, baseURL, policyViolations }) => {
  test.fail(true, "the canary: the teardown of the policy guard must fail this test");
  const context = await browser.newContext();
  const page = await context.newPage();
  await breakThePolicy(page, baseURL);
  await aSignOrTenSeconds(page, policyViolations);
  await context.close();
});
