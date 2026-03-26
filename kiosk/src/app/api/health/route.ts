import { NextResponse } from 'next/server';
import fs from 'fs';
import path from 'path';

/** Every page the kiosk app serves — the deploy contract. */
const EXPECTED_PAGES = [
  '/', '/book', '/control', '/debug', '/fleet',
  '/settings', '/spectator', '/staff',
];

const EXPECTED_APIS = ['/api/health'];

/**
 * Check which built pages actually exist in the .next/server/app directory.
 * Works with both Webpack (.html pre-rendered files) and Turbopack (page.js
 * inside subdirectories) output formats, as well as standalone builds.
 */
function getAvailablePages(): string[] {
  const serverAppDir = path.join(process.cwd(), '.next', 'server', 'app');
  if (!fs.existsSync(serverAppDir)) return [];

  const pages: string[] = [];

  function scan(dir: string, prefix: string) {
    const entries = fs.readdirSync(dir, { withFileTypes: true });

    // Turbopack: a directory containing page.js is a page at this route
    const hasPageJs = entries.some(e => e.isFile() && e.name === 'page.js');
    if (hasPageJs && prefix !== '') {
      pages.push(prefix);
    }

    for (const entry of entries) {
      // Skip route groups like (auth), (dashboard) — they don't affect the URL
      if (entry.isDirectory() && entry.name.startsWith('(')) {
        scan(path.join(dir, entry.name), prefix);
        continue;
      }
      // Webpack: a .html file at the root level = a pre-rendered page
      if (entry.isFile() && entry.name.endsWith('.html')) {
        const routeName = entry.name.replace('.html', '');
        const route = routeName === 'index' ? prefix || '/' : `${prefix}/${routeName}`;
        pages.push(route);
      }
      // Dynamic segments like [id], [number] are valid pages
      if (entry.isDirectory() && entry.name.startsWith('[') && entry.name.endsWith(']')) {
        const dynDir = path.join(dir, entry.name);
        const dynEntries = fs.readdirSync(dynDir);
        if (dynEntries.includes('page.js') || dynEntries.some(f => f.endsWith('.html'))) {
          pages.push(`${prefix}/${entry.name}`);
        }
        continue;
      }
      // Skip internal dirs, api dir, and the 'page' chunk dir (Turbopack artifact)
      if (entry.isDirectory() && !entry.name.startsWith('_') && entry.name !== 'api' && entry.name !== 'page') {
        scan(path.join(dir, entry.name), `${prefix}/${entry.name}`);
      }
    }
  }

  scan(serverAppDir, '');

  // Turbopack: root page.js means '/' exists, also check for index.html
  const rootEntries = fs.readdirSync(serverAppDir);
  const hasRootPage = rootEntries.includes('page.js') || rootEntries.includes('index.html');
  if (hasRootPage && !pages.includes('/')) {
    pages.push('/');
  }

  return [...new Set(pages)].sort();
}

export async function GET() {
  const available = getAvailablePages();
  const missing = EXPECTED_PAGES.filter(p => !available.includes(p));
  const extra = available.filter(p => !EXPECTED_PAGES.includes(p) && !p.startsWith('/_'));

  const hasStatic = fs.existsSync(path.join(process.cwd(), '.next', 'static'));

  const healthy = missing.length === 0 && hasStatic;

  return NextResponse.json({
    status: healthy ? 'ok' : 'degraded',
    service: 'kiosk',
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
