/**
 * Shared helpers for visual regression tests and the before/after script.
 *
 * Handles authentication, navigation, dynamic content masking,
 * and animation disabling for consistent screenshot capture.
 */

import { type Page, type Locator } from '@playwright/test';
import { ensureAuth } from '../page-crawler/auth-setup';
import { getMasksForPage } from './mask-config';
import type { AppRoute } from '../page-crawler/routes';

/** CSS injected to disable animations/transitions for deterministic screenshots. */
const DISABLE_ANIMATIONS_CSS = `
*, *::before, *::after {
  animation-duration: 0s !important;
  transition-duration: 0s !important;
  caret-color: transparent !important;
}
`;

/**
 * Navigate to a route, apply masks, and disable animations.
 *
 * Returns an array of Locator objects for elements that match the
 * mask selectors AND actually exist on the page (filtered by count > 0).
 *
 * @param page - Playwright Page instance (already in a context with or without storageState)
 * @param baseUrl - App base URL (e.g. http://192.168.31.23:3200)
 * @param route - Route definition from the page-crawler route manifest
 * @param storageStatePath - Path to auth storageState JSON, or undefined for public routes
 */
export async function navigateAndMask(
  page: Page,
  baseUrl: string,
  route: AppRoute,
  storageStatePath: string | undefined,
): Promise<Locator[]> {
  // Navigate to the route
  await page.goto(`${baseUrl}${route.path}`, { waitUntil: 'networkidle' });

  // Wait for the route-specific selector if defined
  if (route.waitForSelector) {
    await page.waitForSelector(route.waitForSelector, { timeout: 15_000 });
  }

  // Inject animation-disabling CSS for deterministic screenshots
  await page.addStyleTag({ content: DISABLE_ANIMATIONS_CSS });

  // Get mask selectors for this page and filter to only existing elements
  const selectors = getMasksForPage(route.path);
  const masks: Locator[] = [];

  for (const sel of selectors) {
    const locator = page.locator(sel);
    const count = await locator.count();
    if (count > 0) {
      masks.push(locator);
    }
  }

  return masks;
}
