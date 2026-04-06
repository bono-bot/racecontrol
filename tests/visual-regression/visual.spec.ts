/**
 * Visual regression tests for critical frontend pages.
 *
 * Uses Playwright's built-in toHaveScreenshot() for baseline comparison.
 * Dynamic content (timestamps, counters, live metrics) is masked via
 * getMasksForPage() to prevent false failures.
 *
 * First run:
 *   npx playwright test --config tests/visual-regression/playwright.config.ts
 *   Creates baseline screenshots in tests/visual-regression/__screenshots__/
 *
 * Subsequent runs:
 *   npx playwright test --config tests/visual-regression/playwright.config.ts
 *   Compares current state against baselines — fails on visual diff.
 *
 * Update baselines after intentional changes:
 *   npx playwright test --config tests/visual-regression/playwright.config.ts --update-snapshots
 */

import { test, expect } from '@playwright/test';
import { WEB_ROUTES, KIOSK_ROUTES, type AppRoute } from '../page-crawler/routes';
import { ensureAuth } from '../page-crawler/auth-setup';
import { getMasksForPage } from './mask-config';

/** CSS injected to disable animations/transitions for deterministic screenshots. */
const DISABLE_ANIMATIONS_CSS =
  '*, *::before, *::after { animation-duration: 0s !important; transition-duration: 0s !important; caret-color: transparent !important; }';

/**
 * Critical pages to run visual regression on — a curated subset of all routes.
 * Not every route needs VR; these are the most user-visible pages.
 */
const CRITICAL_PAGES: Record<string, string[]> = {
  web: ['/', '/fleet', '/billing', '/billing/history', '/sessions', '/drivers', '/games'],
  kiosk: ['/kiosk', '/kiosk/staff', '/kiosk/control'],
};

/**
 * Find the AppRoute definition for a given path from the route manifest.
 */
function findRoute(routes: AppRoute[], path: string): AppRoute {
  const route = routes.find((r) => r.path === path);
  if (!route) {
    throw new Error(`Route not found in manifest: ${path}`);
  }
  return route;
}

// ---------------------------------------------------------------------------
// Web app visual regression tests
// ---------------------------------------------------------------------------

test.describe('Web app visual regression', () => {
  let storageStatePath: string;

  test.beforeAll(async () => {
    const baseUrl = process.env.WEB_URL ?? 'http://192.168.31.23:3200';
    storageStatePath = await ensureAuth(baseUrl);
  });

  for (const pagePath of CRITICAL_PAGES.web) {
    const route = WEB_ROUTES.find((r) => r.path === pagePath);
    // Skip test definition if route not in manifest (shouldn't happen)
    if (!route) continue;

    test(`${route.name} visual baseline (${route.path})`, async ({ browser }) => {
      test.setTimeout(60_000);

      const baseUrl = process.env.WEB_URL ?? 'http://192.168.31.23:3200';

      // Create context with auth if needed
      const context = route.requiresAuth
        ? await browser.newContext({ storageState: storageStatePath })
        : await browser.newContext();

      const page = await context.newPage();

      try {
        // Navigate to the page
        await page.goto(`${baseUrl}${route.path}`, { waitUntil: 'networkidle' });

        // Wait for route-specific selector if defined
        if (route.waitForSelector) {
          await page.waitForSelector(route.waitForSelector, { timeout: 15_000 });
        }

        // Disable animations/transitions for deterministic screenshots
        await page.addStyleTag({ content: DISABLE_ANIMATIONS_CSS });

        // Build mask locators from per-page config
        const maskSelectors = getMasksForPage(route.path);
        const maskLocators = maskSelectors.map((sel) => page.locator(sel));

        // Compare screenshot against baseline
        await expect(page).toHaveScreenshot({
          mask: maskLocators,
          fullPage: true,
        });
      } finally {
        await context.close();
      }
    });
  }
});

// ---------------------------------------------------------------------------
// Kiosk app visual regression tests
// ---------------------------------------------------------------------------

test.describe('Kiosk app visual regression', () => {
  let storageStatePath: string;

  test.beforeAll(async () => {
    const baseUrl = process.env.KIOSK_URL ?? 'http://192.168.31.23:3300';
    storageStatePath = await ensureAuth(baseUrl);
  });

  for (const pagePath of CRITICAL_PAGES.kiosk) {
    const route = KIOSK_ROUTES.find((r) => r.path === pagePath);
    if (!route) continue;

    test(`${route.name} visual baseline (${route.path})`, async ({ browser }) => {
      test.setTimeout(60_000);

      const baseUrl = process.env.KIOSK_URL ?? 'http://192.168.31.23:3300';

      // Create context with auth if needed
      const context = route.requiresAuth
        ? await browser.newContext({ storageState: storageStatePath })
        : await browser.newContext();

      const page = await context.newPage();

      try {
        // Navigate to the page
        await page.goto(`${baseUrl}${route.path}`, { waitUntil: 'networkidle' });

        // Wait for route-specific selector if defined
        if (route.waitForSelector) {
          await page.waitForSelector(route.waitForSelector, { timeout: 15_000 });
        }

        // Disable animations/transitions for deterministic screenshots
        await page.addStyleTag({ content: DISABLE_ANIMATIONS_CSS });

        // Build mask locators from per-page config
        const maskSelectors = getMasksForPage(route.path);
        const maskLocators = maskSelectors.map((sel) => page.locator(sel));

        // Compare screenshot against baseline
        await expect(page).toHaveScreenshot({
          mask: maskLocators,
          fullPage: true,
        });
      } finally {
        await context.close();
      }
    });
  }
});
