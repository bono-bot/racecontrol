// ═══════════════════════════════════════════════════════════════
// Staff PIN Lifecycle — Silent-revert regression test
// Codifies the Vishal 2026-04-09 incident into a permanent E2E test.
//
// Flow: create staff → validate PIN → wait sync → change PIN →
//       validate new PIN → wait full sync cycle → re-validate
//       (catches silent revert) → verify old PIN rejected → cleanup
//
// Runtime: ~120s (2x 35s sync waits + 70s main sync-wait)
// Requires: CLOUD_API_URL, VENUE_API_URL, ADMIN_PIN env vars
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { API_BASE, ADMIN_PIN as DEFAULT_ADMIN_PIN } from '../../fixtures/test-data';
import { getAdminToken } from '../../fixtures/auth';

// ─── Configuration ─────────────────────────────────────────────
// Cloud = authoritative for staff mutations. Venue = syncs from cloud.
const CLOUD_URL = process.env.CLOUD_API_URL || API_BASE;
const VENUE_URL = process.env.VENUE_API_URL || API_BASE;
const SYNC_WAIT_MS = 70_000; // sync_interval_secs(30) * 2 + 10s safety margin
const INITIAL_SYNC_MS = 35_000; // one sync cycle + buffer

const TEST_PIN_INITIAL = '9999';
const TEST_PIN_CHANGED = '8888';
const TEST_NAME = `TEST_PIN_LIFECYCLE_${Date.now()}`;

// ─── Helpers ───────────────────────────────────────────────────
async function apiPost(baseUrl: string, path: string, body: unknown, token?: string): Promise<{ status: number; body: Record<string, unknown> }> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  const resp = await fetch(`${baseUrl}${path}`, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
  const data = await resp.json().catch(() => ({}));
  return { status: resp.status, body: data as Record<string, unknown> };
}

async function apiPut(baseUrl: string, path: string, body: unknown, token: string): Promise<{ status: number; body: Record<string, unknown> }> {
  const resp = await fetch(`${baseUrl}${path}`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
    body: JSON.stringify(body),
  });
  const data = await resp.json().catch(() => ({}));
  return { status: resp.status, body: data as Record<string, unknown> };
}

async function apiDelete(baseUrl: string, path: string, token: string): Promise<{ status: number; body: Record<string, unknown> }> {
  const resp = await fetch(`${baseUrl}${path}`, {
    method: 'DELETE',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
  });
  const data = await resp.json().catch(() => ({}));
  return { status: resp.status, body: data as Record<string, unknown> };
}

// ─── Test Suite ────────────────────────────────────────────────
test.describe('Staff PIN lifecycle (silent-revert regression)', () => {
  let adminToken: string;
  let testStaffId: string;

  // Each test in this describe block is sequential and depends on the previous
  test.describe.configure({ mode: 'serial' });

  test.beforeAll(async () => {
    adminToken = await getAdminToken(CLOUD_URL);
    expect(adminToken).toBeTruthy();
  });

  test('1. Create test staff with PIN 9999 on cloud', async () => {
    const r = await apiPost(CLOUD_URL, '/staff', {
      name: TEST_NAME,
      phone: '',
      pin: TEST_PIN_INITIAL,
    }, adminToken);

    expect(r.status, `create_staff failed: ${JSON.stringify(r.body)}`).toBe(200);
    expect(r.body.id).toBeTruthy();
    expect(String(r.body.id)).toMatch(/^staff_/);
    testStaffId = String(r.body.id);
    console.log(`Created test staff: ${testStaffId} (${TEST_NAME})`);
  });

  test('2. Validate PIN 9999 on cloud', async () => {
    const r = await apiPost(CLOUD_URL, '/staff/validate-pin', { pin: TEST_PIN_INITIAL });
    expect(r.status, `validate-pin on cloud failed: ${JSON.stringify(r.body)}`).toBe(200);
    expect(r.body.staff_id).toBe(testStaffId);
  });

  test('3. Wait for cloud→venue sync, then validate PIN 9999 on venue', async () => {
    test.setTimeout(INITIAL_SYNC_MS + 30_000); // extra buffer for test framework
    await new Promise(r => setTimeout(r, INITIAL_SYNC_MS));

    const r = await apiPost(VENUE_URL, '/staff/validate-pin', { pin: TEST_PIN_INITIAL });
    expect(r.status, `validate-pin on venue after sync failed: ${JSON.stringify(r.body)}`).toBe(200);
    expect(r.body.staff_id).toBe(testStaffId);
  });

  test('4. Change PIN 9999 → 8888 via PUT /staff/{id} on cloud', async () => {
    const r = await apiPut(CLOUD_URL, `/staff/${testStaffId}`, {
      pin: TEST_PIN_CHANGED,
    }, adminToken);

    expect(r.status, `update_staff PIN change failed: ${JSON.stringify(r.body)}`).toBe(200);
    expect(r.body.verified).toBe(true);
    expect(r.body.correlation_id).toBeTruthy();
    console.log(`PIN changed: correlation_id=${r.body.correlation_id}`);
  });

  test('5. Immediately (t+2s) validate PIN 8888 on cloud + venue', async () => {
    await new Promise(r => setTimeout(r, 2000));

    const cloud = await apiPost(CLOUD_URL, '/staff/validate-pin', { pin: TEST_PIN_CHANGED });
    expect(cloud.status, `new PIN rejected on cloud immediately: ${JSON.stringify(cloud.body)}`).toBe(200);

    const venue = await apiPost(VENUE_URL, '/staff/validate-pin', { pin: TEST_PIN_CHANGED });
    expect(venue.status, `new PIN rejected on venue immediately: ${JSON.stringify(venue.body)}`).toBe(200);
  });

  test('6. Wait SYNC_WAIT_MS (70s), re-validate PIN 8888 — catches silent revert', async () => {
    // THIS IS THE KEY ASSERTION — the Vishal 2026-04-09 bug would fail here.
    // Cloud sync reverted the PIN change after one sync cycle.
    test.setTimeout(SYNC_WAIT_MS + 30_000);
    await new Promise(r => setTimeout(r, SYNC_WAIT_MS));

    const cloud = await apiPost(CLOUD_URL, '/staff/validate-pin', { pin: TEST_PIN_CHANGED });
    expect(cloud.status, `PIN 8888 reverted on cloud after sync! Silent-revert bug detected.`).toBe(200);

    const venue = await apiPost(VENUE_URL, '/staff/validate-pin', { pin: TEST_PIN_CHANGED });
    expect(venue.status, `PIN 8888 reverted on venue after sync! Silent-revert bug detected.`).toBe(200);

    console.log('PIN survived sync cycle — silent-revert regression PASSED');
  });

  test('7. Old PIN 9999 no longer works on cloud + venue', async () => {
    const cloud = await apiPost(CLOUD_URL, '/staff/validate-pin', { pin: TEST_PIN_INITIAL });
    expect(cloud.status, `old PIN 9999 still valid on cloud — PIN change did not take effect`).toBe(401);

    const venue = await apiPost(VENUE_URL, '/staff/validate-pin', { pin: TEST_PIN_INITIAL });
    expect(venue.status, `old PIN 9999 still valid on venue — PIN change did not propagate`).toBe(401);
  });

  test.afterAll(async () => {
    if (!testStaffId) return;

    // Soft-delete the test staff
    const del = await apiDelete(CLOUD_URL, `/staff/${testStaffId}`, adminToken);
    console.log(`Cleanup: DELETE /staff/${testStaffId} → ${del.status}`);

    // Verify deletion propagated to venue after sync
    await new Promise(r => setTimeout(r, INITIAL_SYNC_MS));
    const venue = await apiPost(VENUE_URL, '/staff/validate-pin', { pin: TEST_PIN_CHANGED });
    if (venue.status !== 401) {
      console.warn(`WARNING: Deleted staff PIN still valid on venue (status=${venue.status}). Sync may be slow.`);
    }
  });
});
