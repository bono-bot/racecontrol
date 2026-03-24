import { NextResponse } from 'next/server';
import fs from 'fs';
import path from 'path';

/** Every page the web dashboard serves — the deploy contract. */
const EXPECTED_PAGES = [
  '/', '/ac-lan', '/ac-sessions', '/ac-sessions/[id]',
  '/ai', '/billing', '/billing/history', '/billing/pricing',
  '/bookings', '/cafe', '/cameras', '/cameras/playback',
  '/drivers', '/events', '/games', '/kiosk',
  '/leaderboards', '/login', '/pods', '/presenter',
  '/results/[id]', '/sessions', '/settings', '/telemetry',
];

const EXPECTED_APIS = ['/api/health'];

/**
 * Check which built pages actually exist in the .next/server/app directory.
 * Works in both dev (cwd/.next) and standalone (.next relative to server.js).
 */
function getAvailablePages(): string[] {
  const serverAppDir = path.join(process.cwd(), '.next', 'server', 'app');
  if (!fs.existsSync(serverAppDir)) return [];

  const pages: string[] = [];

  function scan(dir: string, prefix: string) {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      // Skip route groups like (auth), (dashboard) — they don't affect the URL
      if (entry.isDirectory() && entry.name.startsWith('(')) {
        scan(path.join(dir, entry.name), prefix);
        continue;
      }
      // A .html file = a page exists at that route
      if (entry.isFile() && entry.name.endsWith('.html')) {
        const routeName = entry.name.replace('.html', '');
        const route = routeName === 'index' ? prefix || '/' : `${prefix}/${routeName}`;
        pages.push(route);
      }
      // Recurse into subdirectories (billing/history, cameras/playback, etc.)
      if (entry.isDirectory() && !entry.name.startsWith('_') && entry.name !== 'api') {
        scan(path.join(dir, entry.name), `${prefix}/${entry.name}`);
      }
    }
  }

  scan(serverAppDir, '');
  return pages.sort();
}

export async function GET() {
  const available = getAvailablePages();
  const missing = EXPECTED_PAGES.filter(p => !available.includes(p));
  const extra = available.filter(p => !EXPECTED_PAGES.includes(p));

  const hasStatic = fs.existsSync(path.join(process.cwd(), '.next', 'static'));

  const healthy = missing.length === 0 && hasStatic;

  return NextResponse.json({
    status: healthy ? 'ok' : 'degraded',
    service: 'web-dashboard',
    version: '0.1.0',
    deploy: {
      pages_expected: EXPECTED_PAGES.length,
      pages_available: available.length,
      pages_missing: missing,
      pages_extra: extra,
      static_assets: hasStatic,
      healthy,
    },
  }, { status: healthy ? 200 : 503 });
}
