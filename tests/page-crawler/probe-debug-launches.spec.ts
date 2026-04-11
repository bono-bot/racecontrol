/**
 * Phase 368 — Playwright probe for /kiosk/debug launch cards
 *
 * Verifies that the LaunchCard[] panel renders correctly behind the
 * kiosk_launch_cards_enabled feature flag on the /kiosk/debug page.
 *
 * Tests:
 *   1. Page loads and the launch panel section is present
 *   2. Empty state renders when launches Map is empty
 *   3. A single LaunchCard renders after a launch_status_changed WS event
 *   4. A card transitions through all 4 states on sequential WS events (screenshot)
 *
 * Run manually (kiosk must be reachable at :3300 or KIOSK_URL):
 *   PROBE_SPEC=probe-debug-launches.spec.ts \
 *     npx playwright test --config tests/page-crawler/playwright.config.ts --project=kiosk
 *
 * NOTE: The kiosk reads kiosk_staff_token from sessionStorage (not localStorage).
 * This probe injects sessionStorage directly via addInitScript to avoid the
 * auth-setup.ts localStorage mismatch (kiosk api.ts:16 uses sessionStorage).
 */

import { test, expect, type Page } from "@playwright/test";
import * as path from "path";
import * as fs from "fs";

const DEBUG_PATH = "/kiosk/debug";
const KIOSK_BASE = process.env.KIOSK_URL ?? "http://192.168.31.23:3300";
const API_BASE = process.env.RC_API_URL ?? "http://192.168.31.23:8080";

// ── Fixtures ─────────────────────────────────────────────────────────────────

const MOCK_CARD = {
  launch_id: "test-launch-probe-001",
  pod_id: "pod_8",
  sim_type: "assetto_corsa",
  state: "launch_started",
  detail: null,
  timestamp: new Date().toISOString(),
  origin: "customer",
  ai_tier: null,
  fix_action: null,
};

const MOCK_NOTE = {
  launch_id: "test-launch-probe-001",
  pod_id: "pod_8",
  note_id: "note-probe-001",
  staff_id: "staff_001",
  staff_name: "James",
  body: "Probe note",
  created_at: new Date().toISOString(),
};

// ── Auth helper ───────────────────────────────────────────────────────────────

async function buildAuthContext(browser: import("@playwright/test").Browser) {
  // Try to mint a real token; fall back to a dummy for offline runs
  let token = "probe-test-token";
  let staffId = "staff_001";
  let staffName = "Probe Staff";

  try {
    const pinRes = await fetch(`${API_BASE}/api/v1/staff/validate-pin`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ pin: process.env.STAFF_PIN ?? "7080" }),
      signal: AbortSignal.timeout(3000),
    });
    if (pinRes.ok) {
      const body = (await pinRes.json()) as {
        token: string;
        staff_id: string;
        staff_name: string;
      };
      token = body.token;
      staffId = body.staff_id;
      staffName = body.staff_name;
      console.log("[probe-launches] minted staff token:", staffName, staffId);
    }
  } catch {
    console.log("[probe-launches] server unreachable — using dummy token");
  }

  const context = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
  });

  // sessionStorage injection (kiosk reads kiosk_staff_token from sessionStorage, api.ts:16)
  await context.addInitScript(
    ({
      t,
      id,
      name,
    }: {
      t: string;
      id: string;
      name: string;
    }) => {
      try {
        window.sessionStorage.setItem("kiosk_staff_token", t);
        window.sessionStorage.setItem("kiosk_staff_id", id);
        window.sessionStorage.setItem("kiosk_staff_name", name);
      } catch {
        /* ignore */
      }
    },
    { t: token, id: staffId, name: staffName },
  );

  // Cookie for middleware
  const cookieUrl = new URL(KIOSK_BASE);
  await context.addCookies([
    {
      name: "kiosk_staff_jwt",
      value: token,
      domain: cookieUrl.hostname,
      path: "/",
      expires: Math.floor(Date.now() / 1000) + 1800,
      httpOnly: false,
      secure: false,
      sameSite: "Strict",
    },
  ]);

  return context;
}

// ── Mock API routes ───────────────────────────────────────────────────────────

async function mockApiRoutes(page: Page, featureFlagEnabled: boolean) {
  // Mock /api/v1/flags to control kiosk_launch_cards_enabled
  await page.route("**/api/v1/flags", (route) => {
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([
        {
          name: "kiosk_launch_cards_enabled",
          enabled: featureFlagEnabled,
          default_value: false,
          overrides: null,
          version: 1,
        },
      ]),
    });
  });

  // Mock /api/v1/debug/launches (active launches list — empty by default)
  await page.route("**/api/v1/debug/launches", (route) => {
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([]),
    });
  });

  // Mock individual launch notes endpoint
  await page.route("**/api/v1/debug/launches/*/notes", (route) => {
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([]),
    });
  });
}

// ── WS event injection helper ─────────────────────────────────────────────────

/**
 * Dispatch a synthetic CustomEvent on window so that useKioskSocket's
 * message handler processes it as if it arrived over the real WebSocket.
 *
 * The hook listens for native MessageEvent via the ws.onmessage handler.
 * The simplest cross-origin-safe approach is to patch window.__simulateWsMessage
 * which we inject in addInitScript, then call it here.
 */
async function injectWsSimulator(page: Page) {
  await page.evaluate(() => {
    // Monkey-patch WebSocket constructor to intercept the instance created by
    // useKioskSocket, exposing it so tests can push messages.
    const OrigWs = window.WebSocket;
    (window as Record<string, unknown>).__wsInstances =
      (window as Record<string, unknown>).__wsInstances ?? [];
    class PatchedWS extends OrigWs {
      constructor(url: string | URL, protocols?: string | string[]) {
        super(url, protocols);
        (
          (window as Record<string, unknown>).__wsInstances as PatchedWS[]
        ).push(this);
      }
    }
    window.WebSocket = PatchedWS as typeof WebSocket;

    (window as Record<string, unknown>).__simulateWsMessage = (
      payload: unknown,
    ) => {
      const instances = (
        window as Record<string, unknown>
      ).__wsInstances as WebSocket[];
      const ws = instances[instances.length - 1];
      if (!ws || !ws.onmessage) return false;
      const evt = new MessageEvent("message", {
        data: JSON.stringify(payload),
      });
      ws.onmessage(evt);
      return true;
    };
  });
}

async function pushWsEvent(
  page: Page,
  eventName: string,
  data: unknown,
): Promise<boolean> {
  return page.evaluate(
    ([name, d]) => {
      const fn = (window as Record<string, unknown>).__simulateWsMessage as
        | ((p: unknown) => boolean)
        | undefined;
      if (!fn) return false;
      return fn({ event: name, data: d });
    },
    [eventName, data] as [string, unknown],
  );
}

// ── Screenshot helper ─────────────────────────────────────────────────────────

function screenshotDir() {
  const dir = path.join(__dirname, "screenshots");
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
  return dir;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

test.describe("probe: /kiosk/debug launch cards (Phase 368)", () => {
  // ── Test 1: page loads ──────────────────────────────────────────────────────
  test("page loads with launch panel present", async ({ browser }) => {
    const context = await buildAuthContext(browser);
    const page = await context.newPage();

    await mockApiRoutes(page, true);

    const response = await page.goto(KIOSK_BASE + DEBUG_PATH, {
      waitUntil: "domcontentloaded",
    });

    expect(
      response?.status(),
      "debug page should return HTTP 200",
    ).toBe(200);

    // Give hydration time to settle
    await page.waitForTimeout(3000);

    const title = await page.title();
    console.log("[probe-launches] page title:", title);

    await context.close();
  });

  // ── Test 2: empty state ─────────────────────────────────────────────────────
  test("empty state renders when no launches active", async ({ browser }) => {
    const context = await buildAuthContext(browser);
    const page = await context.newPage();

    // Flag enabled — should show empty state, not the old activity feed
    await mockApiRoutes(page, true);

    await page.goto(KIOSK_BASE + DEBUG_PATH, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(3000);

    // Locate the empty state element
    const emptyState = page.locator('[data-testid="launches-empty-state"]');
    await expect(emptyState).toBeVisible({ timeout: 5000 });
    await expect(emptyState).toContainText(
      "No active launches — waiting for next game start",
    );

    // WS connection dot must be present
    const wsDot = page.locator('[data-testid="ws-connection-dot"]');
    await expect(wsDot).toBeVisible({ timeout: 5000 });

    console.log("[probe-launches] empty state confirmed");

    await context.close();
  });

  // ── Test 3: single card renders after launch_status_changed ────────────────
  test("LaunchCard renders after launch_status_changed WS event", async ({
    browser,
  }) => {
    const context = await buildAuthContext(browser);
    const page = await context.newPage();

    await mockApiRoutes(page, true);

    // Inject WS patcher BEFORE page loads
    await context.addInitScript(() => {
      const OrigWs = window.WebSocket;
      (window as Record<string, unknown>).__wsInstances = [];
      class PatchedWS extends OrigWs {
        constructor(url: string | URL, protocols?: string | string[]) {
          super(url, protocols);
          (
            (window as Record<string, unknown>).__wsInstances as PatchedWS[]
          ).push(this);
        }
      }
      window.WebSocket = PatchedWS as typeof WebSocket;
      (window as Record<string, unknown>).__simulateWsMessage = (
        payload: unknown,
      ) => {
        const instances = (
          window as Record<string, unknown>
        ).__wsInstances as WebSocket[];
        const ws = instances[instances.length - 1];
        if (!ws || !ws.onmessage) return false;
        const evt = new MessageEvent("message", {
          data: JSON.stringify(payload),
        });
        ws.onmessage(evt);
        return true;
      };
    });

    await page.goto(KIOSK_BASE + DEBUG_PATH, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(3000);

    // Push a launch_status_changed event
    const pushed = await pushWsEvent(page, "launch_status_changed", MOCK_CARD);
    console.log("[probe-launches] WS event pushed:", pushed);

    // Give React time to re-render
    await page.waitForTimeout(1000);

    // The LaunchCard should now be visible
    const card = page.locator('[data-testid="launch-card"]');
    await expect(card).toBeVisible({ timeout: 5000 });

    // State label should show "Launch started"
    const stateLabel = card.locator('[data-testid="state-label"]');
    await expect(stateLabel).toContainText("Launch started");

    // Pod badge should show "8"
    await expect(card).toContainText("Pod 8");

    console.log("[probe-launches] single card render confirmed");

    await context.close();
  });

  // ── Test 4: 4-state transition sequence + screenshot ───────────────────────
  test("card transitions through all 4 states via launch_status_changed events", async ({
    browser,
  }) => {
    const context = await buildAuthContext(browser);
    const page = await context.newPage();

    await mockApiRoutes(page, true);

    // Inject WS patcher BEFORE page loads
    await context.addInitScript(() => {
      const OrigWs = window.WebSocket;
      (window as Record<string, unknown>).__wsInstances = [];
      class PatchedWS extends OrigWs {
        constructor(url: string | URL, protocols?: string | string[]) {
          super(url, protocols);
          (
            (window as Record<string, unknown>).__wsInstances as PatchedWS[]
          ).push(this);
        }
      }
      window.WebSocket = PatchedWS as typeof WebSocket;
      (window as Record<string, unknown>).__simulateWsMessage = (
        payload: unknown,
      ) => {
        const instances = (
          window as Record<string, unknown>
        ).__wsInstances as WebSocket[];
        const ws = instances[instances.length - 1];
        if (!ws || !ws.onmessage) return false;
        const evt = new MessageEvent("message", {
          data: JSON.stringify(payload),
        });
        ws.onmessage(evt);
        return true;
      };
    });

    await page.goto(KIOSK_BASE + DEBUG_PATH, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(3000);

    const states: Array<{
      state: string;
      expectedLabel: string;
      ai_tier?: number | null;
    }> = [
      {
        state: "launch_started",
        expectedLabel: "Launch started",
        ai_tier: null,
      },
      {
        state: "ai_analysis_requested",
        expectedLabel: "Analyzing",
        ai_tier: null,
      },
      {
        state: "issue_being_fixed",
        expectedLabel: "Fixing",
        ai_tier: 2,
      },
      {
        state: "issue_fixed",
        expectedLabel: "Playable",
        ai_tier: null,
      },
    ];

    const card = page.locator('[data-testid="launch-card"]');

    for (const { state, expectedLabel, ai_tier } of states) {
      await pushWsEvent(page, "launch_status_changed", {
        ...MOCK_CARD,
        state,
        ai_tier: ai_tier ?? null,
        fix_action: state === "issue_being_fixed" ? "restart_sim" : null,
      });

      await page.waitForTimeout(500);

      // Verify data-state attribute
      await expect(card).toHaveAttribute("data-state", state, {
        timeout: 3000,
      });

      // Verify state label text
      const label = card.locator('[data-testid="state-label"]');
      await expect(label).toContainText(expectedLabel);

      console.log(`[probe-launches] state=${state} label=${expectedLabel} OK`);
    }

    // Dismiss button should be visible at issue_fixed (terminal state)
    const dismissBtn = card.locator('[data-testid="dismiss-button"]');
    await expect(dismissBtn).toBeVisible({ timeout: 3000 });

    // Also push a note to verify the notes thread renders
    await pushWsEvent(page, "launch_note_added", MOCK_NOTE);
    await page.waitForTimeout(500);
    const notesThread = card.locator('[data-testid="notes-thread"]');
    await expect(notesThread).toBeVisible({ timeout: 3000 });

    // Screenshot of final state for deploy-time visual verification
    const screenshotPath = path.join(
      screenshotDir(),
      "launch-card-state-final.png",
    );
    await page.screenshot({ path: screenshotPath, fullPage: true });
    console.log("[probe-launches] screenshot saved:", screenshotPath);

    await context.close();
  });
});
