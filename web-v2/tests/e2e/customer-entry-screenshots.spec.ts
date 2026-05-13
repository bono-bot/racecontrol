/**
 * V2 Customer-entry frontpage — visual screenshot capture
 * Authored 2026-05-11 ~04:00 IST as PRE-DEPLOY visual evidence for the
 * page.tsx replacement of Phase 0.1 scaffold. Hits the temp standalone
 * server (port 3502) so production pm2 (port 3500) stays untouched.
 *
 * Output: tests/screenshots/customer-entry-{viewport}.png
 *
 * NOT a regression test — there is no baseline yet. v0.1 anchor capture.
 */
import { test, expect } from "@playwright/test";

const BASE = process.env.SCREENSHOT_BASE || "http://localhost:3502";
const URL_PATH = process.env.SCREENSHOT_PATH || "/v2/";
const FILE_SUFFIX = process.env.SCREENSHOT_SUFFIX || "";

const VIEWPORTS = [
  { name: "mobile", width: 375, height: 812 },
  { name: "tablet", width: 768, height: 1024 },
  { name: "desktop", width: 1440, height: 900 },
];

for (const vp of VIEWPORTS) {
  test(`customer-entry at ${vp.name} (${vp.width}x${vp.height})`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    // Clear cookies so the first visit is a clean state — but note that
    // middleware (`7b0f212f` close on UI-REVIEW BLOCK-1) sets rp_returning on
    // every /v2/ visit, so server-side `cookies()` reads true even on the
    // very first request. Hero CTA defaults to returning-state ("Continue to
    // your dashboard"). Test stays agnostic to the resulting branch — both
    // are valid customer-day surfaces and both link into the PWA app.
    await page.context().clearCookies();
    await page.goto(`${BASE}${URL_PATH}`, { waitUntil: "networkidle" });

    // Capture screenshot FIRST so visual evidence persists even if assertions
    // surface a later regression.
    await page.screenshot({
      path: `tests/screenshots/customer-entry-${vp.name}${FILE_SUFFIX}.png`,
      fullPage: true,
    });

    // Sanity: heading rendered.
    await expect(
      page.getByRole("heading", { name: /Real cars/ })
    ).toBeVisible();

    // Sanity: primary Hero CTA points into the PWA app — either /book
    // (first-visit) or apex (returning visitor). The /v2/ middleware sets
    // rp_returning on first visit so the returning branch is the default
    // observed state in production.
    const ctaPrimary = page
      .locator("a", { hasText: /Book your first sim time|Continue to your dashboard/ })
      .first();
    await expect(ctaPrimary).toBeVisible();
    await expect(ctaPrimary).toHaveAttribute(
      "href",
      /^https:\/\/app\.racingpoint\.cloud(\/book)?$/
    );
  });
}
