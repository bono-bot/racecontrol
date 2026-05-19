/**
 * ARCH-FOLLOWUP-1 Phase B-5 visual evidence — Captain console screenshots.
 *
 * Authored 2026-05-19 ~22:08 IST as B-5 partial deploy proof. Hits pm2
 * racingpoint-web-v2 on port 3500 (production build serving B-1 routes +
 * mock data per dev-stub controlled bypass).
 *
 * Output: tests/screenshots/admin-{home,incidents}-{viewport}.png
 *
 * NOT a regression test — there is no baseline yet. B-5 anchor capture.
 */
import { test } from "@playwright/test";

const BASE = "http://localhost:3500";

const VIEWPORTS = [
  { name: "desktop", width: 1440, height: 900 },
];

for (const vp of VIEWPORTS) {
  test(`admin home at ${vp.name}`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    await page.goto(`${BASE}/v2/admin`, { waitUntil: "networkidle" });
    await page.screenshot({
      path: `tests/screenshots/admin-home-${vp.name}.png`,
      fullPage: true,
    });
  });

  test(`admin incidents at ${vp.name}`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    await page.goto(`${BASE}/v2/admin/incidents`, { waitUntil: "networkidle" });
    await page.screenshot({
      path: `tests/screenshots/admin-incidents-${vp.name}.png`,
      fullPage: true,
    });
  });
}
