import { expect, request, type APIRequestContext, type Page } from "./fixtures";
import AxeBuilder from "@axe-core/playwright";
import { execFileSync } from "node:child_process";
import { createHmac } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

// Shared by the specs: hydration marker, the WCAG A/AA axe check (design.md 8) and sign-up.

export const WCAG_A_AA = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];
export const PASSWORD = "correct horse battery";

export async function hydrated(page: Page) {
  await expect(page.locator("body[data-hydrated]")).toBeAttached({ timeout: 20_000 });
}

export async function expectNoA11yViolations(page: Page, where = "") {
  const results = await new AxeBuilder({ page }).withTags(WCAG_A_AA).analyze();
  expect(
    results.violations.map(
      (v) => `${where} ${v.id}: ${v.help} -> ${v.nodes.map((n) => `${n.target.join(" ")} ${n.failureSummary ?? ""}`).join("; ")}`,
    ),
  ).toEqual([]);
}

export async function signUp(page: Page, user: string, display = "") {
  await page.goto("/sign-up");
  await hydrated(page);
  await page.getByLabel("Username").fill(user);
  if (display) {
    await page.getByLabel("Display name").fill(display);
  }
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/$/);
  await hydrated(page);
}

// The instance admin made by `global.setup.ts` before any spec runs (the first account is
// the admin, so the setup creates it through the setup wizard's API), and a second admin
// whose sessions the re-authentication spec may age.
export const ADMIN = "e2eadmin";
export const REAUTH_ADMIN = "e2ereauth";
// The operator contact the setup gives the instance (v1-wikidata): Wikidata, the source of board
// games, is asked only with one. Only contact.spec.ts takes it away, after every other spec.
export const E2E_CONTACT = "e2e@example.org";

/** Sign in through the API; the browser context keeps the session cookie. */
export async function signIn(page: Page, user: string) {
  const r = await page.request.post("/api/v1/auth/sign-in", { data: { username: user, password: PASSWORD } });
  expect(r.ok(), `sign-in ${user}: ${r.status()}`).toBeTruthy();
}

/**
 * No horizontal scroll at the current viewport (WCAG 1.4.10 reflow): not of the page, and not
 * inside an open dialog or any other box that scrolls up and down (a `position: fixed` dialog
 * that is wider than the screen does not widen the document, review OT2D-1). A strip that
 * only scrolls sideways (a row of tabs, a shelf of covers) is meant to and is not counted.
 */
export async function expectReflow(page: Page, where = "") {
  const over = await page.evaluate(() => {
    const root = document.documentElement;
    const found: string[] = [];
    if (root.scrollWidth - root.clientWidth > 1) {
      found.push(`document by ${root.scrollWidth - root.clientWidth}px`);
    }
    for (const el of Array.from(document.body.querySelectorAll<HTMLElement>("*"))) {
      if (el.clientWidth === 0 || el.scrollWidth - el.clientWidth <= 1) {
        continue;
      }
      const dialog = el.matches('[role="dialog"], [role="alertdialog"], dialog[open]');
      const y = getComputedStyle(el).overflowY;
      const scrollsDown = (y === "auto" || y === "scroll") && el.scrollHeight - el.clientHeight > 1;
      if (dialog || scrollsDown) {
        const name = el.getAttribute("role") || el.id || el.className;
        found.push(`${el.tagName.toLowerCase()} (${name}) by ${el.scrollWidth - el.clientWidth}px`);
      }
    }
    return found;
  });
  expect(over, `${where} horizontal overflow`).toEqual([]);
}

/**
 * The value of `key` in `[theme]` / `[theme_dark]` of the brand file the server runs with
 * (`E2E_BRAND_DIR`, else the default brand); `undefined` when the brand does not set it. The
 * specs read brand colours from here and never pin them (review OT2D-9).
 */
export function brandValue(section: "theme" | "theme_dark", key: string): string | undefined {
  const dir = process.env.E2E_BRAND_DIR ?? resolve(__dirname, "../../../config/brand/default");
  let inSection = false;
  for (const line of readFileSync(resolve(dir, "brand.toml"), "utf8").split("\n")) {
    const head = line.match(/^\s*\[([^\]]+)\]/);
    if (head) {
      inSection = head[1].trim() === section;
      continue;
    }
    const pair = inSection ? line.match(/^\s*"?(?:--)?([a-z-]+)"?\s*=\s*"([^"]*)"/) : null;
    if (pair && pair[1] === key) {
      return pair[2];
    }
  }
  return undefined;
}

/** The e2e database (`cargo xtask e2e` passes a fresh one). */
export const E2E_DB = process.env.E2E_DATABASE_URL ?? "postgres://app@127.0.0.1:55432/app_e2e";

/** Run one SQL statement against the e2e database (test set-up only). */
export function sql(statement: string) {
  execFileSync("psql", [E2E_DB, "-v", "ON_ERROR_STOP=1", "-c", statement]);
}

/** Age the proof of every session of `user`, as if it signed in two hours ago. */
export function ageSessions(user: string) {
  sql(
    "UPDATE auth_sessions SET reauth_at = now() - interval '2 hours', reauth_slide_at = NULL " +
      `WHERE user_id = (SELECT id FROM users WHERE username = '${user}')`,
  );
}

/** RFC 6238 TOTP (SHA-1, 6 digits, 30 s) of a base32 secret, as authenticator apps compute it. */
export function totp(base32: string, at = Date.now()): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = "";
  for (const c of base32.replace(/=+$/, "").toUpperCase()) {
    bits += alphabet.indexOf(c).toString(2).padStart(5, "0");
  }
  const bytes = Buffer.from((bits.match(/.{8}/g) ?? []).map((b) => parseInt(b, 2)));
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(at / 30_000)));
  const h = createHmac("sha1", bytes).update(counter).digest();
  const o = h[h.length - 1] & 0xf;
  const n = ((h[o] & 0x7f) << 24) | (h[o + 1] << 16) | (h[o + 2] << 8) | h[o + 3];
  return String(n % 1_000_000).padStart(6, "0");
}

/** People page (WP3-6): "Add a person" opens a sheet with the name; the card appears. */
export async function addPerson(page: Page, name: string) {
  await page.getByRole("button", { name: "Add a person" }).click();
  const sheet = page.getByRole("dialog", { name: "Add a person" });
  await sheet.getByLabel("Name", { exact: true }).fill(name);
  await sheet.getByRole("button", { name: "Add person" }).click();
  await expect(sheet).toHaveCount(0);
  await expect(page.getByTestId("pp-card").filter({ has: page.getByRole("link", { name, exact: true }) })).toBeVisible();
}

/** People page: ask `username` to confirm that the person `name` is them (card menu). */
export async function requestLink(page: Page, name: string, username: string) {
  await page.getByRole("button", { name: `More actions for ${name}` }).click();
  await page.getByRole("menuitem", { name: "Link to account" }).click();
  const dialog = page.getByRole("dialog", { name: `Link ${name} to an account` });
  await dialog.getByLabel("Their username").fill(username);
  await dialog.getByRole("button", { name: "Send link request" }).click();
  await expect(page.getByText("Link request sent.")).toBeVisible();
}

/**
 * Wait until import `id` has ended (done, failed or cancelled), then require "done". The import
 * lane is shared by every spec running in parallel and runs strictly in order, so the wait is
 * generous, and a failed import fails at once with its state instead of polling to the end
 * (UX3-22).
 */
export async function importDone(page: { request: APIRequestContext }, id: string) {
  let state = "";
  await expect
    .poll(
      async () => {
        state = (await (await page.request.get(`/api/v1/imports/${id}`)).json()).state;
        return ["done", "failed", "cancelled"].includes(state);
      },
      { timeout: 120_000 },
    )
    .toBe(true);
  expect(state, `import ${id}`).toBe("done");
}

/**
 * Another account imports a new catalog item. Catalog changes reach only the users the item
 * matters to (DEC-490), so an open page of someone else hears nothing of it.
 */
export async function changeElsewhere(baseURL: string) {
  const r = await request.newContext({ baseURL });
  const user = `elsewhere${Date.now()}${Math.floor(Math.random() * 1000)}`;
  const made = await r.post("/api/v1/auth/sign-up", { data: { username: user, password: PASSWORD, display_name: "" } });
  expect(made.ok(), `sign-up ${made.status()}`).toBeTruthy();
  const row = { source_id: user, kind: "movie", title: `Elsewhere ${user}`, year: 2001, status: "planned" };
  const file = [JSON.stringify({ format: "library-export", version: 1 }), JSON.stringify(row)].join("\n");
  const started = await r.post("/api/v1/imports?format=generic-jsonl", {
    data: file,
    headers: { "content-type": "application/octet-stream" },
  });
  expect(started.status()).toBe(202);
  await importDone({ request: r }, (await started.json()).id);
  const lib = await (await r.get("/api/v1/library?limit=5")).json();
  await r.dispose();
  return lib.items[0].item.id as string;
}

/**
 * The signed-in user makes a new list through the API: an `entity.updated` for this user that
 * changes nothing Home or History shows, i.e. a background refresh with the page's data
 * unchanged (what an unchanged refresh of one of the user's items looks like to an open page).
 */
export async function changeMine(page: Page) {
  const name = `Event ${Date.now()}${Math.floor(Math.random() * 1000)}`;
  const r = await page.request.post("/api/v1/lists", { data: { name } });
  expect(r.ok(), `list ${r.status()}`).toBeTruthy();
}

/** Count the `entity.updated` events the page's event stream delivers and keep the ids they name (call before `goto`). */
export async function countEntityEvents(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as { __entityEvents: number; __entityIds: string[] };
    w.__entityEvents = 0;
    w.__entityIds = [];
    const Source = window.EventSource;
    window.EventSource = class extends Source {
      constructor(url: string | URL, init?: EventSourceInit) {
        super(url, init);
        this.addEventListener("entity.updated", (e) => {
          w.__entityEvents += 1;
          w.__entityIds.push(JSON.parse((e as MessageEvent).data)?.entity?.id ?? "");
        });
      }
    } as typeof EventSource;
  });
}

/** `entity.updated` events delivered to the current page so far (see `countEntityEvents`). */
export function entityEvents(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as { __entityEvents?: number }).__entityEvents ?? 0);
}

/** The entity ids named by the `entity.updated` events delivered so far (see `countEntityEvents`). */
export function entityIds(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __entityIds?: string[] }).__entityIds ?? []);
}

/** WCAG contrast ratio of two CSS colours (any syntax the browser paints). */
export async function contrast(page: Page, a: string, b: string): Promise<number> {
  return page.evaluate(
    ([a, b]) => {
      const lum = (colour: string) => {
        const c = document.createElement("canvas");
        c.width = c.height = 1;
        const ctx = c.getContext("2d", { willReadFrequently: true })!;
        ctx.fillStyle = "#fff";
        ctx.fillRect(0, 0, 1, 1);
        ctx.fillStyle = colour;
        ctx.fillRect(0, 0, 1, 1);
        const [r, g, bl] = Array.from(ctx.getImageData(0, 0, 1, 1).data).map((v) => {
          const s = v / 255;
          return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
      };
      const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
      return (x + 0.05) / (y + 0.05);
    },
    [a, b],
  );
}

/** The colour a theme token (`group-watch-soft`) paints on this page, as the browser reports colours. */
export async function token(page: Page, name: string): Promise<string> {
  return page.evaluate((name) => {
    const probe = document.createElement("span");
    probe.style.color = `var(--${name})`;
    document.body.append(probe);
    const colour = getComputedStyle(probe).color;
    probe.remove();
    return colour;
  }, name);
}

/**
 * Let a test hold back the `entity.updated` events of the page's stream (call before `goto`):
 * `holdEvents(page, true)` keeps them, `holdEvents(page, false)` delivers the kept ones in
 * order and lets the next ones through. A test decides with it whether the answer of an own
 * action or the event that announces the same action reaches the page first.
 */
export async function gateEntityEvents(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as { __holdEvents: boolean; __heldEvents: (() => void)[] };
    w.__holdEvents = false;
    w.__heldEvents = [];
    const Source = window.EventSource;
    window.EventSource = class extends Source {
      addEventListener(type: string, listener: EventListenerOrEventListenerObject, options?: boolean | AddEventListenerOptions) {
        if (type !== "entity.updated" || typeof listener !== "function") {
          super.addEventListener(type, listener, options);
          return;
        }
        super.addEventListener(
          type,
          (e: Event) => {
            if (w.__holdEvents) w.__heldEvents.push(() => listener.call(this, e));
            else listener.call(this, e);
          },
          options,
        );
      }
    } as typeof EventSource;
  });
}

/** See `gateEntityEvents`. Answers how many events had been kept. */
export function holdEvents(page: Page, hold: boolean): Promise<number> {
  return page.evaluate((hold) => {
    const w = window as unknown as { __holdEvents: boolean; __heldEvents: (() => void)[] };
    w.__holdEvents = hold;
    const kept = hold ? [] : w.__heldEvents.splice(0);
    kept.forEach((deliver) => deliver());
    return hold ? w.__heldEvents.length : kept.length;
  }, hold);
}

/** Events kept by `holdEvents(page, true)` so far. */
export function heldEvents(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as { __heldEvents?: unknown[] }).__heldEvents?.length ?? 0);
}

/** Two animation frames: an answer is rendered a few microtasks after its response. */
export function twoFrames(page: Page) {
  return page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
}

/**
 * The start times (browser clock, ms) of the page's requests whose URL contains `part`, in
 * the order they were sent; filled as the requests finish (call before the first of them).
 */
export function requestStarts(page: Page, part: string): number[] {
  const starts: number[] = [];
  page.on("requestfinished", (r) => {
    if (r.url().includes(part)) {
      starts.push(r.timing().startTime);
      starts.sort((a, b) => a - b);
    }
  });
  return starts;
}
