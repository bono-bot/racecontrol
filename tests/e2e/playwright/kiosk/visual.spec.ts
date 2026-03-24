import { test, expect } from '../fixtures/cleanup';

// ---- Visual regression tests ----
// Captures screenshots and compares against baselines.
// First run generates baselines in __screenshots__/.
// Subsequent runs diff against baselines — fails if >0.5% pixel difference.
//
// Update baselines: npx playwright test --project=kiosk --update-snapshots

const VISUAL_ROUTES = [
  { path: '/', name: 'customer-landing' },
  { path: '/staff', name: 'staff-login' },
  { path: '/fleet', name: 'fleet-health' },
  { path: '/spectator', name: 'spectator-view' },
];

for (const route of VISUAL_ROUTES) {
  test(`visual: ${route.name} matches baseline`, async ({ page }) => {
    await page.goto(route.path, { waitUntil: 'networkidle' });

    // Wait for any loading spinners to resolve
    await page.waitForTimeout(1000);

    // Hide dynamic content that changes between runs (clocks, timers, connection status)
    await page.evaluate(() => {
      // Hide clock displays
      document.querySelectorAll('[data-testid="clock"], [data-testid="ws-status"]').forEach((el) => {
        (el as HTMLElement).style.visibility = 'hidden';
      });
      // Hide any spinner/loading indicators
      document.querySelectorAll('.animate-spin, .animate-pulse').forEach((el) => {
        (el as HTMLElement).style.animationPlayState = 'paused';
      });
    });

    await expect(page).toHaveScreenshot(`${route.name}.png`, {
      maxDiffPixelRatio: 0.005, // 0.5% tolerance for anti-aliasing
      animations: 'disabled',
    });
  });
}
