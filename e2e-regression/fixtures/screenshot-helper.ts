// ═══════════════════════════════════════════════════════════════
// Screenshot Helper — descriptive naming for all test screenshots
// ═══════════════════════════════════════════════════════════════

import { Page } from '@playwright/test';
import * as path from 'path';

const SCREENSHOT_DIR = path.resolve(__dirname, '..', 'test-results', 'screenshots');

// Take a screenshot with a descriptive name
export async function screenshot(
  page: Page,
  name: string,
  options?: { fullPage?: boolean },
): Promise<string> {
  const filePath = path.join(SCREENSHOT_DIR, `${name}.png`);
  await page.screenshot({
    path: filePath,
    fullPage: options?.fullPage ?? false,
  });
  return filePath;
}

// Generate matrix test screenshot name
export function matrixScreenshotName(
  testId: string,
  step: string,
  game: string,
  tier: string,
  payment: string,
  endType: string,
): string {
  return `${testId}-${step}-${game}-${tier}-${payment}-${endType}`;
}

// Take a screenshot of every visible element's state (for button audit)
export async function screenshotWithOverlay(
  page: Page,
  name: string,
  highlightSelector?: string,
): Promise<string> {
  if (highlightSelector) {
    // Add a red border around the target element
    await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      if (el) {
        (el as HTMLElement).style.outline = '3px solid red';
        (el as HTMLElement).style.outlineOffset = '2px';
      }
    }, highlightSelector);
  }

  const filePath = await screenshot(page, name);

  if (highlightSelector) {
    // Remove the highlight
    await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      if (el) {
        (el as HTMLElement).style.outline = '';
        (el as HTMLElement).style.outlineOffset = '';
      }
    }, highlightSelector);
  }

  return filePath;
}
