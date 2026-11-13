// The `test` of every spec (sec1): a test fails when a page of it broke the content security
// policy of the server, whatever the test itself was about. Two signs are collected in every
// browser context of the test (the built-in one and those a spec opens with
// `browser.newContext()` or `browser.newPage()`):
//
// - a `securitypolicyviolation` event on a document (enforced or report-only), and
// - a console message of the browser that names the policy (what it prints when it
//   refuses a script, a style, a picture, a connection, a form target or a frame).
//
// The same for a page of the app that broke while it started (race package, F2): a hydration
// error or a panic of the page's code on the browser console fails the test (`appFailures`).
// The app reloads once after a hydration panic, so without this guard a test would pass over a
// page the server rendered wrong.
//
// Specs import `test` and `expect` from this file, never from "@playwright/test"
// (tests/global.setup.ts checks that).
import { test as base, expect, type BrowserContext } from "@playwright/test";

export { expect, request } from "@playwright/test";
export type { APIRequestContext, Browser, BrowserContext, Locator, Page } from "@playwright/test";

type Violation = {
  page: string;
  directive: string;
  blocked: string;
  disposition: string;
  sample: string;
  source: string;
};

const BINDING = "__cspViolation";
const watched = new WeakSet<BrowserContext>();

/// A hydration error (the server's HTML and the browser's render differ) or a panic of the
/// page's code, as the browser console prints them.
export const APP_FAILURE = /hydration error occurred|Unrecoverable hydration error|panicked at/;
/// The renderer updated a view whose nodes were out of the document. Not fatal; noted with
/// the page so that a run names the pages (race diagnosis 6.2), never a failure.
const DETACHED_INSERT = /Non-fatal error at .*renderer\/dom\.rs/;

/// Collect what the pages of `context` report about the policy into `found`, and their
/// hydration errors and panics into `failures`.
export async function watchPolicy(context: BrowserContext, found: string[], failures: string[]) {
  if (watched.has(context)) return;
  watched.add(context);
  context.on("console", (msg) => {
    const text = msg.text();
    const where = msg.page()?.url() ?? "no page";
    if (APP_FAILURE.test(text)) {
      failures.push(`console (${where}): ${text.split("\n")[0]}`);
    }
    if (DETACHED_INSERT.test(text)) {
      try {
        base.info().annotations.push({ type: "detached insert", description: where });
      } catch {
        // no test is running (a page that outlived its test)
      }
    }
    if (/content[ -]security[ -]policy/i.test(text)) {
      found.push(`console (${msg.page()?.url() ?? "no page"}): ${text}`);
    }
  });
  await context.exposeBinding(BINDING, (_source, v: Violation) => {
    found.push(
      `event on ${v.page}: ${v.directive} blocked ${v.blocked || "(inline)"} [${v.disposition}]` +
        (v.source ? ` at ${v.source}` : "") +
        (v.sample ? ` sample: ${v.sample}` : ""),
    );
  });
  await context.addInitScript((binding) => {
    document.addEventListener("securitypolicyviolation", (e) => {
      const report = (window as unknown as Record<string, (v: unknown) => void>)[binding];
      report?.({
        page: location.href,
        directive: e.effectiveDirective,
        blocked: e.blockedURI,
        disposition: e.disposition,
        sample: e.sample,
        source: e.sourceFile ? `${e.sourceFile}:${e.lineNumber}` : "",
      });
    });
  }, BINDING);
}

export const test = base.extend<{ policyViolations: string[]; appFailures: string[]; policyGuard: void }>({
  policyViolations: async ({}, use) => {
    await use([]);
  },
  // a test that provokes a failure on purpose reads it here and empties the list
  appFailures: async ({}, use) => {
    await use([]);
  },
  // contexts a spec opens itself
  policyGuard: [
    async ({ browser, policyViolations, appFailures }, use) => {
      const newContext = browser.newContext;
      browser.newContext = async function (...args: Parameters<typeof newContext>) {
        const context = await newContext.apply(browser, args);
        await watchPolicy(context, policyViolations, appFailures);
        return context;
      };
      await use();
      browser.newContext = newContext;
      expect(policyViolations, "the pages of this test broke the content security policy").toEqual([]);
      expect(appFailures, "a page of this test had a hydration error or its code panicked").toEqual([]);
    },
    { auto: true },
  ],
  // the built-in context, before its first page exists
  context: async ({ context, policyViolations, appFailures }, use) => {
    await watchPolicy(context, policyViolations, appFailures);
    await use(context);
  },
});
