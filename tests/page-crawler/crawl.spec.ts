// Page Crawler — Playwright test that visits all frontend pages and saves screenshots.
//
// Usage:
//   npx playwright test --config tests/page-crawler/playwright.config.ts
//   CRAWL_APP=web npx playwright test --config tests/page-crawler/playwright.config.ts
//   CRAWL_PAGE=/billing npx playwright test --config tests/page-crawler/playwright.config.ts
//   CRAWL_APP=kiosk CRAWL_PAGES=/kiosk/staff,/kiosk/control npx playwright test --config tests/page-crawler/playwright.config.ts
//
// Environment variables:
//   CRAWL_APP   — filter by app: 'web', 'admin', 'kiosk', or unset for all
//   CRAWL_PAGE  — filter by single page path, e.g. '/billing'
//   CRAWL_PAGES — comma-separated list of paths, e.g. '/billing,/fleet,/drivers'
//
// Screenshots are saved to: tests/screenshots/{app}/{route}/{timestamp}.png

import { test, expect, BrowserContext } from '@playwright/test';
import { getAllRoutes, AppRoute } from './routes';
import { ensureAuth, ensureGitignore } from './auth-setup';
import * as fs from 'fs';
import * as path from 'path';

const REPO_ROOT = path.resolve(__dirname, '..', '..');
const SCREENSHOTS_DIR = path.join(REPO_ROOT, 'tests', 'screenshots');

// Ensure .gitignore has the auth cache entry on first import
ensureGitignore(REPO_ROOT);

/**
 * Sanitize a route path into a filesystem-safe directory name.
 * e.g. "/billing/history" -> "billing_history"
 */
function sanitizeRoute(routePath: string): string {
  return routePath
    .replace(/^\//, '') // remove leading slash
    .replace(/\//g, '_') // replace remaining slashes
    || 'root'; // handle "/" -> ""
}

/**
 * Generate an ISO-like timestamp safe for Windows filenames.
 * e.g. "2026-04-06T18-30-45"
 */
function fileTimestamp(): string {
  return new Date()
    .toISOString()
    .replace(/:/g, '-')
    .replace(/\.\d+Z$/, '');
}

/**
 * Check if a route matches the CRAWL_PAGE / CRAWL_PAGES filter.
 * Returns true if the route should be crawled.
 */
function matchesPageFilter(route: AppRoute): boolean {
  const singlePage = process.env.CRAWL_PAGE;
  const multiPages = process.env.CRAWL_PAGES;

  if (!singlePage && !multiPages) {
    return true; // no filter = crawl everything
  }

  if (singlePage && route.path === singlePage) {
    return true;
  }

  if (multiPages) {
    const pages = multiPages.split(',').map((p) => p.trim());
    if (pages.includes(route.path)) {
      return true;
    }
  }

  return false;
}

// Get the list of apps to crawl
const appFilter = process.env.CRAWL_APP;
const appEntries = getAllRoutes(appFilter);

// Summary tracking
let totalCrawled = 0;
let totalFailed = 0;

for (const entry of appEntries) {
  const { app, baseUrl, routes } = entry;

  test.describe(`${app} app (${baseUrl})`, () => {
    let storageStatePath: string | undefined;

    test.beforeAll(async () => {
      try {
        storageStatePath = await ensureAuth(baseUrl);
        console.log(`[${app}] Auth storageState cached at: ${storageStatePath}`);
      } catch (err) {
        console.warn(
          `[${app}] Auth failed — unauthenticated routes only: ${err}`,
        );
      }
    });

    for (const route of routes) {
      test(`${route.name} (${route.path})`, async ({ browser }) => {
        // Page filter
        if (!matchesPageFilter(route)) {
          test.skip();
          return;
        }

        // Per-test timeout: 60 seconds for slow pages
        test.setTimeout(60_000);

        // Create context — with or without auth
        let context: BrowserContext;
        if (route.requiresAuth && storageStatePath) {
          context = await browser.newContext({
            storageState: storageStatePath,
          });
        } else {
          context = await browser.newContext();
        }

        const page = await context.newPage();

        try {
          // Navigate
          const url = `${baseUrl}${route.path}`;
          // Use domcontentloaded instead of networkidle — authenticated
          // dashboard pages have persistent WebSocket connections that
          // prevent networkidle from ever firing.
          const response = await page.goto(url, {
            waitUntil: 'domcontentloaded',
            timeout: 30_000,
          });

          // Wait for client-side rendering (React hydration + data fetch)
          await page.waitForTimeout(3_000);

          // Log non-OK responses but continue — the error page is useful evidence
          if (response) {
            const status = response.status();
            if (status >= 400) {
              console.warn(
                `[${app}] ${route.path} returned HTTP ${status}`,
              );
            }
          }

          // Wait for custom selector if specified
          if (route.waitForSelector) {
            try {
              await page.waitForSelector(route.waitForSelector, {
                timeout: 5_000,
              });
            } catch {
              console.warn(
                `[${app}] ${route.path} — waitForSelector "${route.waitForSelector}" timed out`,
              );
            }
          }

          // Build screenshot path: tests/screenshots/{app}/{route}/{timestamp}.png
          const routeDir = path.join(
            SCREENSHOTS_DIR,
            app,
            sanitizeRoute(route.path),
          );
          fs.mkdirSync(routeDir, { recursive: true });

          const timestamp = fileTimestamp();
          const screenshotPath = path.join(routeDir, `${timestamp}.png`);

          // Capture full-page screenshot
          await page.screenshot({ fullPage: true, path: screenshotPath });

          // Verify the screenshot was saved and has content
          const stat = fs.statSync(screenshotPath);
          expect(stat.size).toBeGreaterThan(0);

          totalCrawled++;
          console.log(
            `[${app}] ${route.path} -> ${path.relative(REPO_ROOT, screenshotPath)} (${stat.size} bytes)`,
          );
        } catch (err) {
          totalFailed++;
          console.error(`[${app}] ${route.path} FAILED: ${err}`);
          throw err;
        } finally {
          await context.close();
        }
      });
    }

    test.afterAll(() => {
      console.log(
        `\n[${app}] Summary — crawled: ${totalCrawled}, failed: ${totalFailed}\n`,
      );
    });
  });
}
