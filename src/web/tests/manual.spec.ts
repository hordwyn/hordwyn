import { test, expect, type Page } from "./fixtures";
import { expectNoA11yViolations, expectReflow, hydrated, signUp } from "./helpers";

// v1-manual (v1-plan.md 4a): something no source knows is added by hand from the palette,
// tracked like any other item, found again, and its details are corrected on its page. The
// picture address a person types is loaded by the browser only (the server never fetches
// it; apps/server/tests/it/manual.rs), with no referrer.

const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==", "base64");
const PICTURES = "https://img.example/";
const bar = (page: Page) => page.getByTestId("action-bar");

async function searchFor(page: Page, scope: string, text: string) {
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search" });
  await expect(palette).toBeVisible();
  await palette.getByRole("radio", { name: scope }).click();
  await palette.getByRole("combobox").fill(text);
  return palette;
}

test("an item no source knows is added by hand, played, found again and its details corrected", async ({ page }) => {
  test.setTimeout(180_000);
  const stamp = Date.now();
  const title = `Zine of the Marsh ${stamp}`;
  const asked: string[] = [];
  await page.route(
    (url) => url.href.startsWith(PICTURES),
    (route) => {
      asked.push(JSON.stringify(route.request().headers()["referer"] ?? null));
      return route.fulfill({ status: 200, contentType: "image/png", body: PNG });
    },
  );
  await signUp(page, `hand${stamp}`, "Hand Tester");

  // the last row of an answered search offers to add what was typed
  let palette = await searchFor(page, "Play", title);
  const offer = palette.getByRole("option", { name: `Add "${title}" by hand...` });
  await expect(offer).toBeVisible({ timeout: 20_000 });
  await expect(palette.getByRole("option").last()).toHaveAttribute("id", (await offer.getAttribute("id"))!);
  await offer.click();
  const add = page.getByRole("dialog", { name: "Add by hand" });
  await expect(add).toBeVisible();
  await expect(palette).toHaveCount(0);
  await expect(add.getByLabel("Title")).toHaveValue(title);
  await expect(add.getByLabel("Title")).toBeFocused();
  // Play holds two kinds: none is chosen for the person (the kind cannot be changed later);
  // every enabled kind can be
  const kind = add.getByLabel("Kind");
  await expect(kind).toHaveValue("");
  await expect(kind.getByRole("option", { name: "Movie" })).toBeAttached();
  await expect(add.getByText("The picture is loaded from this address by every browser that shows it.")).toBeVisible();
  await expectNoA11yViolations(page, "add by hand");

  // what the form can tell without the server is said at the field
  await kind.selectOption({ label: "Board game" });
  await add.getByLabel("Year").fill("soon");
  await add.getByRole("button", { name: "Add to library" }).click();
  await expect(add.getByRole("alert")).toHaveText("Enter the year as a number, such as 1995, or leave the field empty.");
  await expect(add.getByLabel("Year")).toBeFocused();
  await expect(add.getByLabel("Year")).toHaveAttribute("aria-invalid", "true");
  // and what only the server knows comes back to the field as well
  await add.getByLabel("Year").fill("9999");
  await add.getByRole("button", { name: "Add to library" }).click();
  await expect(add.getByRole("alert")).toHaveText("Enter the year as a number, such as 1995, or leave the field empty.");
  await expect(add).toBeVisible();
  await add.getByLabel("Year").fill("2019");
  await add.getByRole("button", { name: "Add to library" }).click();

  // its page opens: in the library, the title in the cover box, the step of a board game
  await expect(page).toHaveURL(/\/items\/[0-9a-f-]{36}$/);
  await expect(add).toHaveCount(0);
  const itemUrl = page.url();
  const id = itemUrl.split("/").pop()!;
  await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
  await expect(page.getByTestId("ip-kind")).toHaveText(/Board game/);
  await expect(page.getByTestId("ip-status")).toHaveText("Planned");
  await expect(page.getByRole("img", { name: title })).toContainText(title);
  await expect(page.getByText("Fetching details...")).toHaveCount(0);
  const stored = await (await page.request.get(`/api/v1/items/${id}`)).json();
  expect(stored.state).toBe("full");
  expect(stored.external_ids).toEqual([]);
  expect(stored.provenance).toEqual({ cover_url: "user", title: "user", year: "user" });

  // a play with two people
  await bar(page).getByRole("button", { name: "Log a play" }).click();
  const play = page.getByRole("dialog", { name: "Log a play" });
  await expect(play.getByRole("button", { name: "Hand Tester", exact: true })).toHaveAttribute("aria-pressed", "true");
  const people = play.getByRole("combobox", { name: "Add a person" });
  await people.fill("Ben");
  await people.press("Enter");
  await expect(play.getByRole("button", { name: "Ben", exact: true })).toHaveAttribute("aria-pressed", "true");
  await play.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Logged: Play" })).toBeVisible();
  await expect(play).toHaveCount(0);

  // the search knows it at once (the outbox carries it to the index): among the person's own
  await expect
    .poll(async () => (await (await page.request.get(`/api/v1/search?q=${encodeURIComponent(title)}`)).json()).items.length, { timeout: 30_000 })
    .toBeGreaterThanOrEqual(1);
  palette = await searchFor(page, "Play", title);
  await expect(palette.getByRole("option", { name: new RegExp(`^${title} \\(2019\\)`) })).toBeVisible({ timeout: 20_000 });
  await expect(palette.getByRole("listbox")).toContainText("In your library");

  // a second attempt, by keyboard: the row is the last one, the dialog takes the focus,
  // Escape closes it and the focus is back on the page
  const box = palette.getByRole("combobox");
  await expect(palette.getByRole("option", { name: `Add "${title}" by hand...` })).toBeVisible({ timeout: 20_000 });
  await box.press("ArrowUp");
  await expect(palette.getByRole("option").last()).toHaveAttribute("aria-selected", "true");
  await box.press("Enter");
  await expect(add).toBeVisible();
  await expect(add.getByLabel("Title")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(add).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await page.evaluate(() => document.activeElement !== null && document.body.contains(document.activeElement))).toBe(true);

  // again, with the pointer: the item that is there is named before a second one is made,
  // also when Enter comes before the list could show (the server refuses the title once)
  palette = await searchFor(page, "Play", title);
  await palette.getByRole("option", { name: `Add "${title}" by hand...` }).click();
  await add.getByLabel("Kind").selectOption({ label: "Board game" });
  await add.getByLabel("Title").press("Enter");
  await expect(add.getByRole("alert")).toHaveText(
    "An item of this kind with this title is already here, listed below. Use it, or choose Add to library again to add a second one.",
  );
  await expect(add.getByLabel("Title")).toBeFocused();
  await expect(add.getByLabel("Title")).toHaveAttribute("aria-invalid", "true");
  await expect(add).toBeVisible();
  const existing = add.getByTestId("mi-existing");
  await expect(existing.getByRole("heading", { name: "Already here" })).toBeVisible();
  await expect(existing.getByRole("listitem")).toHaveText([new RegExp(`^${title} \\(2019\\)`)]);
  // another kind has none of that title
  await add.getByLabel("Kind").selectOption({ label: "Movie" });
  await expect(existing).toHaveCount(0);
  await add.getByLabel("Kind").selectOption({ label: "Board game" });
  await expectNoA11yViolations(page, "add by hand, already here");
  await existing.getByRole("button", { name: `Use this one: ${title} (2019)` }).click();
  await expect(page).toHaveURL(itemUrl);
  await expect(add).toHaveCount(0);
  const titled = await (await page.request.get(`/api/v1/items/titled?kind=board_game&title=${encodeURIComponent(title)}`)).json();
  expect(titled.map((i: { id: string }) => i.id), "no second item was made").toEqual([id]);

  // "Edit details" on its page: title, year and a picture address
  await hydrated(page);
  const more = bar(page).getByRole("button", { name: `More actions for ${title}` });
  await more.click();
  await page.getByRole("menuitem", { name: "Edit details" }).click();
  const edit = page.getByRole("dialog", { name: "Edit details" });
  await expect(edit).toBeVisible();
  await expect(edit.getByLabel("Title")).toHaveValue(title);
  await expect(edit.getByLabel("Title")).toBeFocused();
  await expect(edit.getByLabel("Year")).toHaveValue("2019");
  await expect(edit.getByLabel("Picture address")).toHaveValue("");
  await expect(edit.getByLabel("Kind")).toHaveCount(0);
  await expect(edit.getByTestId("mi-existing")).toHaveCount(0);
  // an address the server would not take is refused at the field
  await edit.getByLabel("Picture address").fill("http://img.example/plain.png");
  await edit.getByRole("button", { name: "Save" }).click();
  await expect(edit.getByRole("alert")).toHaveText(
    "The picture address must start with https:// and name a host, without a user name and without blanks.",
  );
  await expect(edit.getByLabel("Picture address")).toBeFocused();
  await edit.getByLabel("Picture address").fill("https://me:secret@img.example/box.png");
  await edit.getByRole("button", { name: "Save" }).click();
  await expect(edit.getByRole("alert")).toHaveText(
    "The picture address must start with https:// and name a host, without a user name and without blanks.",
  );
  const picture = `${PICTURES}box-${stamp}.png`;
  await edit.getByLabel("Picture address").fill(picture);
  await edit.getByLabel("Title").fill(`${title}, second printing`);
  await edit.getByRole("button", { name: "Save" }).click();
  await expect(edit).toHaveCount(0);
  await expect(page.getByRole("heading", { level: 1, name: `${title}, second printing` })).toBeVisible();
  const cover = page.locator(`img[src="${picture}"]`).first();
  await expect(cover).toBeVisible();
  await expect(cover).toHaveAttribute("referrerpolicy", "no-referrer");
  await expect.poll(() => cover.evaluate((i: HTMLImageElement) => i.naturalWidth)).toBe(1);

  // the card of the library shows the picture too, from the same address
  await page.goto("/library");
  await hydrated(page);
  const card = page.getByTestId("library-card").filter({ hasText: `${title}, second printing` });
  const onCard = card.locator(`img[src="${picture}"]`);
  await expect(onCard).toBeVisible();
  await expect.poll(() => onCard.evaluate((i: HTMLImageElement) => i.naturalWidth)).toBe(1);
  expect(asked.length).toBeGreaterThan(0);
  expect(new Set(asked), "the picture host learns no address of this instance").toEqual(new Set(["null"]));
  // the server kept the address as the person's and stored no picture of its own
  const after = await (await page.request.get(`/api/v1/items/${id}`)).json();
  expect(after.cover).toMatchObject({ url: picture, variants: [] });
  expect(after.provenance.cover_url).toBe("user");
});

test("the dialog of an item added by hand reflows at 360 px and has no axe violations in the dark theme", async ({ page }) => {
  test.setTimeout(90_000);
  await signUp(page, `hand360${Date.now()}`, "Hand Narrow");
  await page.setViewportSize({ width: 360, height: 740 });
  await page.evaluate(() => localStorage.setItem("app-theme", "dark"));
  await page.goto("/");
  await hydrated(page);
  await expect(page.locator("html")).toHaveClass(/dark/);
  await page.getByRole("button", { name: /^Search or add/ }).first().click();
  const palette = page.getByRole("dialog", { name: "Search" });
  const text = `A very small press game with a long name ${Date.now()}`;
  await palette.getByRole("combobox").fill(text);
  const offer = palette.getByRole("option", { name: `Add "${text}" by hand...` });
  await expect(offer).toBeVisible({ timeout: 30_000 });
  await expectNoA11yViolations(page, "palette with the by-hand row, dark 360");
  await expectReflow(page, "palette with the by-hand row 360");
  await offer.click();
  const add = page.getByRole("dialog", { name: "Add by hand" });
  await expect(add).toBeVisible();
  // every kind in scope: none is chosen for the person, and the form says so when sent
  await expect(add.getByLabel("Kind")).toHaveValue("");
  await add.getByRole("button", { name: "Add to library" }).click();
  await expect(add.getByRole("alert")).toHaveText("Choose a kind.");
  await expect(add.getByLabel("Kind")).toBeFocused();
  await expectNoA11yViolations(page, "add by hand, dark 360");
  await expectReflow(page, "add by hand 360");
  await expect(add.getByRole("button", { name: "Add to library" })).toBeInViewport({ ratio: 1 });
});
