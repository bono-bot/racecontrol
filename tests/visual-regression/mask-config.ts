/**
 * Per-page dynamic content mask definitions for visual regression tests.
 *
 * Masks CSS selectors that contain dynamic content (timestamps, counters,
 * live metrics) to prevent false failures in screenshot comparisons.
 */

export interface MaskConfig {
  route: string;
  selectors: string[];
}

/**
 * Global masks applied to ALL pages — common dynamic content patterns.
 */
const GLOBAL_MASKS: string[] = [
  '[data-testid="timestamp"]',
  '.relative-time',
  'time',
  '.text-muted-foreground:has(time)',
];

/**
 * Per-page mask configs for critical pages with dynamic content.
 */
export const MASK_CONFIGS: MaskConfig[] = [
  {
    route: '/',
    selectors: [
      '.ws-status',
      '.pod-status-indicator',
      '.active-sessions-count',
      '.revenue-today',
      '[data-live="true"]',
    ],
  },
  {
    route: '/fleet',
    selectors: [
      '.pod-uptime',
      '.last-seen',
      '.ws-latency',
      '.build-id',
    ],
  },
  {
    route: '/billing',
    selectors: [
      '.session-timer',
      '.active-billing',
      '.wallet-balance',
    ],
  },
  {
    route: '/billing/history',
    selectors: [
      '.transaction-date',
      '.session-date',
    ],
  },
  {
    route: '/sessions',
    selectors: [
      '.session-duration',
      '.session-start',
    ],
  },
  {
    route: '/drivers',
    selectors: [
      '.last-visit',
      '.total-sessions',
    ],
  },
  {
    route: '/games',
    selectors: [
      '.game-count',
      '.reliability-score',
    ],
  },
  {
    route: '/kiosk',
    selectors: [
      '.clock',
      '.greeting-time',
    ],
  },
  {
    route: '/kiosk/staff',
    selectors: [
      '.staff-clock',
      '.active-count',
    ],
  },
  {
    route: '/kiosk/control',
    selectors: [
      '.pod-status',
      '.session-info',
    ],
  },
];

/**
 * Returns the union of global masks + page-specific masks for a given route.
 * If no page-specific config exists, returns only global masks.
 */
export function getMasksForPage(routePath: string): string[] {
  const pageConfig = MASK_CONFIGS.find((c) => c.route === routePath);
  const pageSelectors = pageConfig ? pageConfig.selectors : [];
  return [...GLOBAL_MASKS, ...pageSelectors];
}
