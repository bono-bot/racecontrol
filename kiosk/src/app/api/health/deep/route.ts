import { NextResponse } from 'next/server';

/**
 * Deep semantic health check for the kiosk app.
 * Validates that upstream APIs return meaningful data, not just HTTP 200.
 *
 * Checks:
 * 1. Pods API returns >0 pods (catches empty DB after server restart)
 * 2. Games catalog is non-empty
 * 3. Billing pricing API is reachable and returns tiers
 */

const API_BASE = process.env.NEXT_PUBLIC_API_URL || 'http://192.168.31.23:8080';
const TIMEOUT_MS = 8000;

interface CheckResult {
  name: string;
  passed: boolean;
  detail: string;
}

async function checkWithTimeout(
  name: string,
  url: string,
  validate: (data: Record<string, unknown>) => { passed: boolean; detail: string },
): Promise<CheckResult> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    const res = await fetch(url, { signal: controller.signal });
    if (!res.ok) {
      return { name, passed: false, detail: `HTTP ${res.status}` };
    }
    const data = await res.json();
    const result = validate(data);
    return { name, ...result };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { name, passed: false, detail: `error: ${msg}` };
  } finally {
    clearTimeout(timeout);
  }
}

export async function GET() {
  const checks = await Promise.all([
    checkWithTimeout(
      'pods_available',
      `${API_BASE}/api/v1/fleet/health`,
      (data) => {
        const pods = Array.isArray(data) ? data : (data as { pods?: unknown[] }).pods;
        const count = Array.isArray(pods) ? pods.length : 0;
        return count > 0
          ? { passed: true, detail: `${count} pods registered` }
          : { passed: false, detail: 'pods table empty — kiosk will show "Waiting for pods"' };
      },
    ),
    checkWithTimeout(
      'games_catalog',
      `${API_BASE}/api/v1/games`,
      (data) => {
        const games = (data as { games?: unknown[] }).games;
        const count = Array.isArray(games) ? games.length : 0;
        return count > 0
          ? { passed: true, detail: `${count} games available` }
          : { passed: false, detail: 'games catalog empty' };
      },
    ),
    checkWithTimeout(
      'billing_pricing',
      `${API_BASE}/api/v1/pricing`,
      (data) => {
        const tiers = (data as { tiers?: unknown[] }).tiers;
        const count = Array.isArray(tiers) ? tiers.length : 0;
        return count > 0
          ? { passed: true, detail: `${count} pricing tiers` }
          : { passed: false, detail: 'no pricing tiers configured' };
      },
    ),
  ]);

  const allPassed = checks.every((c) => c.passed);
  const failedChecks = checks.filter((c) => !c.passed);

  const summary = allPassed
    ? 'all upstream APIs healthy'
    : failedChecks.map((c) => `${c.name}: ${c.detail}`).join('; ');

  return NextResponse.json(
    {
      healthy: allPassed,
      summary,
      checks,
      service: 'kiosk',
      timestamp: new Date().toISOString(),
    },
    { status: allPassed ? 200 : 503 },
  );
}
