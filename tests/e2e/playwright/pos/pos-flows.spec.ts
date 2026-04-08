import { test, expect } from '@playwright/test';

/**
 * POS-specific flow tests: payment method, discount, refund lifecycle.
 * Tests the complete staff workflow paths.
 *
 * Staff-protected routes require JWT auth (obtained via admin-login).
 */

const API = process.env.API_BASE_URL ?? 'http://192.168.31.23:8080/api/v1';
const ADMIN_PIN = process.env.ADMIN_PIN ?? '261121';

let authToken = '';

test.beforeAll(async () => {
  const res = await fetch(`${API}/auth/admin-login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ pin: ADMIN_PIN }),
  });
  if (res.ok) {
    const body = await res.json();
    authToken = body.token ?? '';
  }
});

async function api(path: string, opts?: RequestInit) {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(authToken ? { Authorization: `Bearer ${authToken}` } : {}),
    ...(opts?.headers as Record<string, string> ?? {}),
  };
  const res = await fetch(`${API}${path}`, { ...opts, headers });
  return { status: res.status, json: await res.json().catch(() => null) };
}

// ---- POS-01: Payment Method on session start ----

test('POS-01: start_billing accepts all 4 payment methods', async () => {
  const methods = ['wallet', 'cash', 'upi', 'card'];

  for (const method of methods) {
    const { json } = await api('/billing/start', {
      method: 'POST',
      body: JSON.stringify({
        pod_id: 'pod-test',
        driver_id: 'driver-test',
        pricing_tier_id: 'tier-test',
        payment_method: method,
      }),
    });
    // Should fail on driver/tier validation, NOT on payment_method
    expect(json.error).toBeDefined();
    expect(json.error).not.toMatch(/payment_method/i);
  }
});

// ---- POS-02: Refund validation rules ----

test('POS-02: refund rejects all invalid inputs correctly', async () => {
  // Bad method
  const r1 = await api('/billing/test-session/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: 100, method: 'crypto', reason: 'test' }),
  });
  expect(r1.json.error).toMatch(/method must be/i);

  // Empty reason
  const r2 = await api('/billing/test-session/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: 100, method: 'wallet', reason: '   ' }),
  });
  expect(r2.json.error).toMatch(/reason/i);

  // Zero amount
  const r3 = await api('/billing/test-session/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: 0, method: 'cash', reason: 'test' }),
  });
  expect(r3.json.error).toMatch(/amount/i);

  // Negative amount
  const r4 = await api('/billing/test-session/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: -50, method: 'upi', reason: 'test' }),
  });
  expect(r4.json.error).toMatch(/amount/i);

  // Missing session
  const r5 = await api('/billing/nonexistent-uuid/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: 100, method: 'wallet', reason: 'genuine test' }),
  });
  expect(r5.json.error).toMatch(/not found/i);
});

// ---- POS-03: Refund list endpoint ----

test('POS-03: refund list returns array for any session', async () => {
  const list = await api('/billing/sessions?limit=1');
  if (!list.json?.sessions?.length) {
    test.skip();
    return;
  }
  const sid = list.json.sessions[0].id;
  const { status, json } = await api(`/billing/${sid}/refunds`);
  expect(status).toBe(200);
  expect(json).toHaveProperty('refunds');
  expect(Array.isArray(json.refunds)).toBe(true);

  // If refunds exist, verify fields
  if (json.refunds.length > 0) {
    expect(json.refunds[0]).toHaveProperty('amount_paise');
    expect(json.refunds[0]).toHaveProperty('method');
    expect(json.refunds[0]).toHaveProperty('reason');
    expect(json.refunds[0]).toHaveProperty('created_at');
  }
});

// ---- POS-04: Discount on start_billing ----

test('POS-04: start_billing accepts discount parameters', async () => {
  const { json } = await api('/billing/start', {
    method: 'POST',
    body: JSON.stringify({
      pod_id: 'pod-test',
      driver_id: 'driver-test',
      pricing_tier_id: 'tier-test',
      staff_discount_paise: 5000,
      discount_reason: 'VIP customer',
    }),
  });
  // Should fail on driver/tier validation, NOT on discount params
  expect(json.error).toBeDefined();
  expect(json.error).not.toMatch(/discount/i);
});

// ---- POS-05: Audit log captures actions ----

test('POS-05: audit log endpoint returns entries', async () => {
  const { status, json } = await api('/audit-log');
  expect(status).toBe(200);
  expect(json).toHaveProperty('entries');
  expect(Array.isArray(json.entries)).toBe(true);

  // Verify structure if entries exist
  if (json.entries.length > 0) {
    expect(json.entries[0]).toHaveProperty('table_name');
    expect(json.entries[0]).toHaveProperty('action');
    expect(json.entries[0]).toHaveProperty('created_at');
  }
});

// ---- Cloud Sync: billing data flows to cloud ----

test('SYNC: billing sessions list endpoint works on cloud-sync schema', async () => {
  const { status, json } = await api('/billing/sessions?limit=10');
  expect(status).toBe(200);
  expect(json).toHaveProperty('sessions');

  if (json.sessions.length > 0) {
    const s = json.sessions[0];
    // Verify all synced fields are present
    expect(s).toHaveProperty('id');
    expect(s).toHaveProperty('driver_name');
    expect(s).toHaveProperty('pod_id');
    expect(s).toHaveProperty('pricing_tier_name');
    expect(s).toHaveProperty('allocated_seconds');
    expect(s).toHaveProperty('driving_seconds');
    expect(s).toHaveProperty('status');
    expect(s).toHaveProperty('price_paise');
    expect(s).toHaveProperty('started_at');
  }
});

// ---- LIVE: Fleet health pod entries have correct structure ----

test('LIVE: fleet/health pod entries have correct structure', async () => {
  const { status, json } = await api('/fleet/health');
  expect(status).toBe(200);
  expect(json).toHaveProperty('pods');
  expect(Array.isArray(json.pods)).toBe(true);

  for (const pod of json.pods) {
    expect(typeof pod.pod_number).toBe('number');
    expect(pod).toHaveProperty('ws_connected');
    expect(pod).toHaveProperty('http_reachable');
    expect(pod).toHaveProperty('experience_score');
  }
});

// ---- ANA: Daily billing report ----

test('ANA: daily billing report returns structured data', async () => {
  const today = new Date().toISOString().split('T')[0];
  const { status, json } = await api(`/billing/report/daily?date=${today}`);
  expect(status).toBe(200);
  expect(json).toHaveProperty('date');
  expect(json).toHaveProperty('total_sessions');
  expect(json).toHaveProperty('total_revenue_paise');
  expect(typeof json.total_sessions).toBe('number');
  expect(typeof json.total_revenue_paise).toBe('number');
});

// ---- PWA-06: PDF receipt generation (unit-style) ----

test('PWA: public session endpoint returns privacy-safe data', async () => {
  const list = await api('/billing/sessions?limit=1');
  if (!list.json?.sessions?.length) {
    test.skip();
    return;
  }
  const sid = list.json.sessions[0].id;
  const res = await fetch(
    `${API}/public/sessions/${sid}`,
    { headers: { 'Content-Type': 'application/json' } }
  );
  const json = await res.json();

  if (!json.error) {
    // Should have public fields only
    expect(json).toHaveProperty('driver_first_name');
    expect(json).toHaveProperty('duration_seconds');
    expect(json).toHaveProperty('total_laps');

    // Should NOT have private fields
    expect(json).not.toHaveProperty('wallet_balance');
    expect(json).not.toHaveProperty('phone');
    expect(json).not.toHaveProperty('email');
    expect(json).not.toHaveProperty('cost_paise');
    expect(json).not.toHaveProperty('wallet_debit_paise');
  }
});
