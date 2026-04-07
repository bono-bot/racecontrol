/**
 * Route manifest for all 3 Racing Point frontend apps.
 *
 * Used by the page crawler (crawl.spec.ts) to enumerate every
 * reachable page across web (:3200), admin (:3201), and kiosk (:3300).
 */

export interface AppRoute {
  path: string;           // e.g. "/billing"
  name: string;           // human-readable name e.g. "Billing"
  requiresAuth: boolean;  // true for most staff pages
  waitForSelector?: string; // optional CSS selector to wait for before screenshot
}

// ---------------------------------------------------------------------------
// Web app routes — :3200 (staff dashboard)
// ---------------------------------------------------------------------------

export const WEB_ROUTES: AppRoute[] = [
  { path: '/', name: 'Home', requiresAuth: true },
  { path: '/login', name: 'Login', requiresAuth: false },
  { path: '/fleet', name: 'Fleet', requiresAuth: true },
  { path: '/billing', name: 'Billing', requiresAuth: true },
  { path: '/billing/history', name: 'Billing History', requiresAuth: true },
  { path: '/billing/pricing', name: 'Billing Pricing', requiresAuth: true },
  { path: '/drivers', name: 'Drivers', requiresAuth: true },
  { path: '/customers', name: 'Customers', requiresAuth: true },
  { path: '/games', name: 'Games', requiresAuth: true },
  { path: '/games/reliability', name: 'Game Reliability', requiresAuth: true },
  { path: '/games/timeline', name: 'Game Timeline', requiresAuth: true },
  { path: '/cameras', name: 'Cameras', requiresAuth: true },
  { path: '/cameras/playback', name: 'Camera Playback', requiresAuth: true },
  { path: '/analytics', name: 'Analytics', requiresAuth: true },
  { path: '/analytics/business', name: 'Business Analytics', requiresAuth: true },
  { path: '/analytics/ebitda', name: 'EBITDA Analytics', requiresAuth: true },
  { path: '/pods', name: 'Pods', requiresAuth: true },
  { path: '/sessions', name: 'Sessions', requiresAuth: true },
  { path: '/settings', name: 'Settings', requiresAuth: true },
  { path: '/ota', name: 'OTA', requiresAuth: true },
  { path: '/metrics', name: 'Metrics', requiresAuth: true },
  { path: '/diagnosis', name: 'Diagnosis', requiresAuth: true },
  { path: '/hr', name: 'HR', requiresAuth: true },
  { path: '/hr/employees', name: 'HR Employees', requiresAuth: true },
  { path: '/hr/shifts', name: 'HR Shifts', requiresAuth: true },
  { path: '/maintenance', name: 'Maintenance', requiresAuth: true },
  { path: '/leaderboards', name: 'Leaderboards', requiresAuth: true },
  { path: '/events', name: 'Events', requiresAuth: true },
  { path: '/flags', name: 'Feature Flags', requiresAuth: true },
  { path: '/cafe', name: 'Cafe', requiresAuth: true },
  { path: '/policy', name: 'Policy', requiresAuth: true },
  { path: '/ac-lan', name: 'AC LAN', requiresAuth: true },
  { path: '/ac-sessions', name: 'AC Sessions', requiresAuth: true },
  { path: '/bookings', name: 'Bookings', requiresAuth: true },
  { path: '/telemetry', name: 'Telemetry', requiresAuth: true },
  { path: '/presenter', name: 'Presenter', requiresAuth: true },
  { path: '/leaderboard-display', name: 'Leaderboard Display', requiresAuth: true },
  { path: '/spectator/circuit', name: 'Spectator Circuit', requiresAuth: false },
];

// ---------------------------------------------------------------------------
// Admin app routes — :3201 (same web app, admin port)
// ---------------------------------------------------------------------------

export const ADMIN_ROUTES: AppRoute[] = WEB_ROUTES.map((r) => ({ ...r }));

// ---------------------------------------------------------------------------
// Kiosk app routes — :3300 (customer + staff facing)
// ---------------------------------------------------------------------------

export const KIOSK_ROUTES: AppRoute[] = [
  { path: '/kiosk', name: 'Kiosk Landing', requiresAuth: false },
  { path: '/kiosk/staff', name: 'Kiosk Staff', requiresAuth: true },
  { path: '/kiosk/control', name: 'Kiosk Control', requiresAuth: true },
  { path: '/kiosk/register', name: 'Kiosk Register', requiresAuth: true },
  { path: '/kiosk/fleet', name: 'Kiosk Fleet', requiresAuth: true },
  { path: '/kiosk/spectator', name: 'Kiosk Spectator', requiresAuth: true },
  { path: '/kiosk/debug', name: 'Kiosk Debug', requiresAuth: true },
  { path: '/kiosk/settings', name: 'Kiosk Settings', requiresAuth: true },
  { path: '/kiosk/shutdown', name: 'Kiosk Shutdown', requiresAuth: true },
  { path: '/kiosk/pod/1', name: 'Kiosk Pod 1', requiresAuth: true },
];

// ---------------------------------------------------------------------------
// Helper: get routes filtered by app name
// ---------------------------------------------------------------------------

interface AppRoutesEntry {
  app: string;
  baseUrl: string;
  routes: AppRoute[];
}

const APP_CONFIGS: AppRoutesEntry[] = [
  {
    app: 'web',
    baseUrl: process.env.WEB_URL ?? 'http://192.168.31.23:3200',
    routes: WEB_ROUTES,
  },
  {
    app: 'admin',
    baseUrl: process.env.ADMIN_URL ?? 'http://192.168.31.23:3201',
    routes: ADMIN_ROUTES,
  },
  {
    app: 'kiosk',
    baseUrl: process.env.KIOSK_URL ?? 'http://192.168.31.23:3300',
    routes: KIOSK_ROUTES,
  },
];

/**
 * Returns routes for the specified app(s).
 * @param appFilter - 'web', 'admin', 'kiosk', or undefined for all
 */
export function getAllRoutes(appFilter?: string): AppRoutesEntry[] {
  if (!appFilter) {
    return APP_CONFIGS;
  }
  return APP_CONFIGS.filter((c) => c.app === appFilter.toLowerCase());
}
