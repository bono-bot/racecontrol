import { NextResponse } from 'next/server';

/**
 * Deep semantic health check for the web dashboard.
 * Validates that upstream APIs return meaningful data, not just HTTP 200.
 *
 * Checks:
 * 1. Fleet health returns pod data
 * 2. Metrics API is responsive
 * 3. Config API returns valid JSON
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
      'fleet_health',
      `${API_BASE}/api/v1/fleet/health`,
      (data) => {
        const pods = Array.isArray(data) ? data : (data as { pods?: unknown[] }).pods;
        const count = Array.isArray(pods) ? pods.length : 0;
        return count > 0
          ? { passed: true, detail: `${count} pods in fleet` }
          : { passed: false, detail: 'fleet health returned no pods' };
      },
    ),
    checkWithTimeout(
      'metrics_api',
      `${API_BASE}/api/v1/health`,
      (data) => {
        const status = (data as { status?: string }).status;
        return status === 'ok'
          ? { passed: true, detail: 'server health ok' }
          : { passed: false, detail: `server status: ${status || 'unknown'}` };
      },
    ),
    checkWithTimeout(
      'config_api',
      `${API_BASE}/api/v1/config`,
      (data) => {
        const hasKeys = data && typeof data === 'object' && Object.keys(data).length > 0;
        return hasKeys
          ? { passed: true, detail: 'config loaded' }
          : { passed: false, detail: 'config returned empty or invalid' };
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
      service: 'web-dashboard',
      timestamp: new Date().toISOString(),
    },
    { status: allPassed ? 200 : 503 },
  );
}
