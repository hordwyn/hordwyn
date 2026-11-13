import { test, expect, type Page } from "./fixtures";
import { ADMIN, E2E_CONTACT, expectNoA11yViolations, hydrated, signIn, signUp } from "./helpers";

// v1-books (docs/fable/round2/v1-plan.md 3): books come from Open Library (the mock in
// tests/openlibrary-mock.mjs serves the recorded answers). A title search in the Read scope
// finds a book, its page shows the cover, the authors and "of N pages (estimate)"; the
// reader corrects the total, logs a page and finishes; Stats counts every page; the page
// credits Open Library. An ISBN finds the book it belongs to. The operator contact is one
// field on Admin > Data sources.

const bar = (page: Page) => page.getByTestId("action-bar");

async function searchRead(page: Page, text: string) {
  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog", { name: "Search" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("radio", { name: "Read" }).click();
  await dialog.getByRole("combobox").fill(text);
  return dialog;
}

test("a book from Open Library: cover, authors, page total, Finished counts the pages left", async ({ page }) => {
  test.setTimeout(180_000);
  await signUp(page, `book${Date.now()}`, "Book Tester");
  const dialog = await searchRead(page, "dune");
  // the source's answer arrives over SSE after the local one
  const option = dialog.getByRole("option", { name: /Dune \(1965\)/ }).first();
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();

  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: "Dune" })).toBeVisible();
  // the cover is a picture, not the title box (loaded from the source or from storage)
  const cover = page.locator('img[alt="Cover of Dune"]');
  await expect(cover).toBeVisible();
  await expect.poll(() => cover.evaluate((el: HTMLImageElement) => el.naturalWidth), { timeout: 20_000 }).toBeGreaterThan(0);
  // the search row brings the author; the description follows with the refresh of the page
  const about = page.getByRole("tabpanel", { name: "About" });
  await expect(about).toContainText("Frank Herbert");
  await expect(about).toContainText("Synthetic description of the e2e tests.", { timeout: 20_000 });
  const credit = page.getByTestId("attribution");
  await expect(credit).toContainText("Book data and covers from Open Library.");
  await expect(credit.getByRole("link", { name: "https://openlibrary.org/" })).toHaveAttribute("href", "https://openlibrary.org/");

  await page.getByRole("button", { name: "Want to read" }).click();
  const field = bar(page).getByLabel("Current page");
  await expect(field).toBeVisible();
  // the item is shared by everybody on the instance, and so is its page total: an earlier
  // run of this test may have set one (the estimate is checked on a book no test changes,
  // in the ISBN test below)
  await expect(bar(page).getByTestId("pages-of")).toHaveText(/^of \d+ pages( \(estimate\))?$/);
  await expectNoA11yViolations(page, "book page");

  // the reader's own copy has another number of pages
  const pages = 500 + (Date.now() % 90);
  const change = bar(page).getByRole("button", { name: "Change the page total" });
  await expect(change).toHaveText("Change");
  await change.click();
  const total = bar(page).getByLabel("Pages in this book");
  await expect(total).toBeFocused();
  await expect(total).toHaveValue(/^\d+$/);
  await total.fill("lots");
  await total.press("Enter");
  await expect(bar(page).getByText("Enter a number from 1 to 100000.")).toBeVisible();
  await expect(total).toHaveAttribute("aria-invalid", "true");
  await expectNoA11yViolations(page, "page total form");
  await total.fill(String(pages));
  await total.press("Enter");
  await expect(page.getByRole("status").filter({ hasText: `Page total set to ${pages}.` })).toBeVisible();
  await expect(bar(page).getByTestId("pages-of")).toHaveText(`of ${pages} pages`);
  await expect(bar(page).getByLabel("Pages in this book")).toHaveCount(0);
  await expect(bar(page).getByRole("button", { name: "Change the page total" })).toBeFocused();

  await field.fill("100");
  await field.press("Enter");
  await expect(page.getByRole("status").filter({ hasText: "Logged: Page 100" })).toBeVisible();
  await field.fill("700");
  await field.press("Enter");
  await expect(bar(page).getByText(`This has ${pages} pages.`)).toBeVisible();
  // Finished logs the last page, then completes: the pages that were left are counted
  await bar(page).getByRole("button", { name: "Finished" }).click();
  await expect(page.getByRole("status").filter({ hasText: `Logged: Page ${pages}` })).toBeVisible();
  await expect(page.getByRole("status").filter({ hasText: "Marked as Completed" })).toBeVisible();
  await expect(page.getByTestId("item-facts")).toContainText("Completed");

  // the person's total survives a refresh of the item (here: the page is opened again)
  await page.reload();
  await hydrated(page);
  await expect(bar(page).getByTestId("pages-of")).toHaveText(`of ${pages} pages`);

  // every page counts, and the book set to Completed is a finished book (LB-13)
  await page.goto("/stats");
  await hydrated(page);
  await expect(page.getByTestId("banked-tile-books").locator("dd")).toHaveText([`${pages} pages`, "One book finished"]);
  await expect(page.getByTestId("banked-notes")).toContainText("Books finished without a page count add no pages.");
});

test("an ISBN finds the book it belongs to", async ({ page }) => {
  await signUp(page, `isbn${Date.now()}`, "Isbn Tester");
  // the ISBN-13 of one edition, typed with hyphens
  const dialog = await searchRead(page, "978-0-14-032872-1");
  const option = dialog.getByRole("option", { name: /Fantastic Mr Fox \(1970\)/ });
  await expect(option).toBeVisible({ timeout: 20_000 });
  await option.click();
  await expect(page).toHaveURL(/\/items\//);
  await hydrated(page);
  await expect(page.getByRole("heading", { level: 1, name: "Fantastic Mr Fox" })).toBeVisible();
  await expect(page.getByRole("tabpanel", { name: "About" })).toContainText("Roald Dahl");
  // the page total is the median over the editions of the work: said to be an estimate
  // (once the refresh of the page has brought the details and where they come from)
  await page.getByRole("button", { name: "Want to read" }).click();
  await expect(bar(page).getByTestId("pages-of")).toHaveText("of 96 pages (estimate)", { timeout: 20_000 });
  await expect(bar(page).getByRole("button", { name: "Change the page total" })).toHaveText("Change");
});

test("the operator contact is saved on Admin > Data sources and Open Library works without a key", async ({ page }) => {
  await signIn(page, ADMIN);
  await page.goto("/admin/sources");
  await hydrated(page);
  const form = page.getByTestId("operator-contact");
  const field = form.getByLabel("Operator contact");
  await expect(field).toBeVisible();
  await expect(form).toContainText("Sent to Open Library and Wikidata with every request");
  // not an address: refused with what is expected
  await field.fill("me at home");
  await form.getByRole("button", { name: "Save" }).click();
  await expect(form.getByRole("alert")).toContainText("Enter an e-mail address or an address that starts with https://");
  await field.fill("books@example.org");
  await form.getByRole("button", { name: "Save" }).click();
  await expect(form.getByRole("status")).toContainText("Saved.");
  await page.reload();
  await hydrated(page);
  await expect(page.getByTestId("operator-contact").getByLabel("Operator contact")).toHaveValue("books@example.org");

  const row = page.getByTestId("src-row").filter({ hasText: "Open Library" });
  await expect(row).toContainText("Book");
  await expect(row).toContainText("Working");
  await row.getByRole("button", { name: "Details of Open Library" }).click();
  const card = page.locator("tr#adm-src-openlibrary");
  await expect(card).toContainText("This source needs no key.");
  await expect(card.getByTestId("src-note")).toContainText("A high-traffic commercial service must not rely on this source.");
  await expect(card).toContainText("Book data and covers from Open Library.");
  await expectNoA11yViolations(page, "sources with the contact field");

  // the contact of the other specs is put back (Wikidata needs one; removing it is
  // contact.spec.ts, which runs after every other spec)
  await page.getByTestId("operator-contact").getByLabel("Operator contact").fill(E2E_CONTACT);
  await page.getByTestId("operator-contact").getByRole("button", { name: "Save" }).click();
  await expect(page.getByTestId("operator-contact").getByRole("status")).toContainText("Saved.");
});
