/**
 * Playwright Global Setup — Acquires staff JWT ONCE for all test suites.
 *
 * WHY: Each test file was independently calling validate-pin, triggering
 * the server's per-IP rate limiter (Phase 348). With 5+ POS files + kiosk
 * tests + retries, the IP gets locked out mid-run. This global setup runs
 * once before all projects, writes the JWT to a shared file, and every
 * test reads from it.
 *
 * ECOSYSTEM PATTERN: Any new test file that needs auth should call
 * getSharedJwt() from ./shared-auth.ts instead of calling validate-pin.
 */
import fs from 'fs';
import path from 'path';

const API = process.env.RC_API_URL ?? process.env.API_BASE_URL ?? 'http://192.168.31.23:8080';
const STAFF_PIN = process.env.STAFF_PIN ?? '2799'; // TEST_ONLY_E2E
const AUTH_STATE_PATH = path.join(__dirname, '.auth-state.json');

export default async function globalSetup() {
  // Single validate-pin call for the entire test run
  const res = await fetch(`${API}/api/v1/staff/validate-pin`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ pin: STAFF_PIN }),
  });

  let token = '';
  let staffId = '';
  let staffName = '';
  let role = '';

  if (res.ok) {
    const body = await res.json();
    token = body.token ?? '';
    staffId = body.staff_id ?? '';
    staffName = body.staff_name ?? '';
    role = body.role ?? '';
  }

  // Write for all suites to read
  fs.writeFileSync(AUTH_STATE_PATH, JSON.stringify({ token, staffId, staffName, role }));

  if (!token) {
    console.warn('[global-setup] WARNING: Failed to acquire staff JWT — auth-dependent tests will fail');
  } else {
    console.log(`[global-setup] JWT acquired for ${staffName} (${role})`);
  }
}
