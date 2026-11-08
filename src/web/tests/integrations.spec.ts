import { test, expect, type Page } from "@playwright/test";
import { expectNoA11yViolations, expectReflow, hydrated, signUp } from "./helpers";

// Settings > Integrations (design.md 6.9, 5.9, R-38; WP3-5): "Connect a sender" in three
// steps; the panel listens over SSE and turns green on the first accepted call (made here
// with a plain HTTP request, as a media server would); a test call from the panel records
// nothing; a refused call is explained inline with its field and the redacted body; revoke
// in the card menu with a confirm; axe and reflow at 360 px.

const PAGE = "/settings/integrations";

const PLAY = {
  kind: "movie",
  title: "Heat",
  year: 1995,
  ids: { tmdb: "949" },
  watched_at: "2026-01-01T21:00:00Z",
  source: "e2e",
};

async function connect(page: Page, name: string) {
  await page.getByRole("button", { name: "Connect a sender" }).click();
  await page.getByLabel("Name", { exact: true }).fill(name);
  await page.getByRole("button", { name: "Create token" }).click();
  const token = await page.locator("#ig-token").inputValue();
  const url = await page.locator("#ig-url").inputValue();
  expect(token).toMatch(/^itk_/);
  return { token, url };
}

async function post(page: Page, url: string, token: string, body: unknown) {
  return page.request.post(url, { headers: { Authorization: `Bearer ${token}` }, data: body });
}

test("connect a sender: the panel turns green, refusals are explained, revoke", async ({ page }) => {
  await signUp(page, `int${Date.now()}`);
  await page.goto(PAGE);
  await hydrated(page);
  await expect(page.getByText("No senders yet.")).toBeVisible();
  const { token, url } = await connect(page, "Media server");
  const wait = page.getByTestId("ig-wait");
  await expect(wait).toContainText("Waiting for the first call...");
  await expect(page.getByText("Shown once. If you lose it, create a new sender.")).toBeVisible();

  // a test call from the panel goes through the checks and records nothing
  await page.getByRole("button", { name: "Send a test call from here" }).click();
  await expect(page.getByText("Test call accepted (nothing was recorded)")).toBeVisible();
  await expect(wait).toHaveAttribute("data-state", "listening");

  // a refused call is explained in the panel, with the field
  const typo = await post(page, url, token, { ...PLAY, episode_title: "Pilot", api_token: "keep-this-secret" });
  expect(typo.status()).toBe(422);
  await expect(wait).toHaveAttribute("data-state", "refused");
  await expect(wait).toContainText("because of the field episode_title");

  // the first accepted call turns the panel green
  const ok = await post(page, url, token, PLAY);
  expect(ok.status()).toBe(200);
  await expect(wait).toHaveAttribute("data-state", "accepted");
  await expect(wait).toContainText("First call received");
  await expect(wait).toContainText("Heat");
  await page.getByRole("button", { name: "Done" }).click();
  const senders = page.getByRole("list", { name: "Senders" });
  const sender = senders.getByRole("listitem").filter({ hasText: "Media server" });
  await expect(sender).toContainText("Working");
  await expect(sender).toContainText("recorded Heat");

  // a later refusal stays on the card, with the body it sent (secrets hidden)
  await post(page, url, token, { ...PLAY, season: "two", password: "keep-this-secret" });
  await expect(sender).toContainText("Last call refused");
  await expect(sender).toContainText("because of the field season");
  await expect(sender).toContainText("Last recorded before that: Heat");
  await sender.getByText("Show the last body (secrets hidden)").click();
  const body = sender.locator("pre");
  await expect(body).toContainText("password");
  await expect(body).not.toContainText("keep-this-secret");

  await page.setViewportSize({ width: 360, height: 740 });
  await expectReflow(page, "360 cards");
  await expectNoA11yViolations(page, "360 cards");
  await page.getByRole("button", { name: "Connect a sender" }).click();
  await expectReflow(page, "360 panel");
  await expectNoA11yViolations(page, "360 panel");
  await page.getByRole("button", { name: "Cancel" }).click();

  // revoke from the card menu, with a confirm; the token stops working
  await page.getByRole("button", { name: "Actions for Media server" }).click();
  await page.getByRole("menuitem", { name: "Revoke" }).click();
  const dialog = page.getByRole("dialog", { name: "Revoke Media server?" });
  await dialog.getByRole("button", { name: "Revoke" }).click();
  await expect(page.getByText("No senders yet.")).toBeVisible();
  expect((await post(page, url, token, PLAY)).status()).toBe(401);
});

test("after a minute without a call the panel says to check the token", async ({ page }) => {
  await page.clock.install();
  await signUp(page, `intq${Date.now()}`);
  await page.goto(PAGE);
  await hydrated(page);
  await connect(page, "Quiet sender");
  const wait = page.getByTestId("ig-wait");
  await expect(wait).toHaveAttribute("data-state", "listening");
  await page.clock.fastForward(61_000);
  await expect(wait).toHaveAttribute("data-state", "quiet");
  await expect(wait).toContainText("Check the token and the URL");
});
