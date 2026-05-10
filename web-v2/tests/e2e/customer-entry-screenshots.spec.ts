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
    await page.goto(`${BASE}/v2/`, { waitUntil: "networkidle" });

    // Sanity: heading rendered.
    await expect(
      page.getByRole("heading", { name: /Real cars/ })
    ).toBeVisible();

    // Sanity: book CTA present and points to PWA.
    const bookCta = page.locator("a", { hasText: "Book your first sim time" }).first();
    await expect(bookCta).toBeVisible();
    await expect(bookCta).toHaveAttribute(
      "href",
      "https://app.racingpoint.cloud/"
    );

    // Full-page screenshot for visual evidence.
    await page.screenshot({
      path: `tests/screenshots/customer-entry-${vp.name}.png`,
      fullPage: true,
    });
  });
}
