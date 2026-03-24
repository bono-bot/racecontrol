import { test, expect } from '@playwright/test';

/**
 * POS-specific flow tests: payment method, discount, refund lifecycle.
 * Tests the complete staff workflow paths.
 */

const API = process.env.API_BASE_URL ?? 'http://192.168.31.23:8080/api/v1';

async function api(path: string, opts?: RequestInit) {
  const res = await fetch(`${API}${path}`, {
    headers: { 'Content-Type': 'application/json', ...opts?.headers },
    ...opts,
  });
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
  const list = await api('/billing/sessions?limit=1&status=completed');
  if (list.json.sessions.length === 0) {
    test.skip();
    return;
  }
  const sid = list.json.sessions[0].id;
  const { status, json } = await api(`/billing/${sid}/refunds`);
  expect(status).toBe(200);
  expect(Array.isArray(json)).toBe(true);

  // If refunds exist, verify fields
  if (json.length > 0) {
    expect(json[0]).toHaveProperty('amount_paise');
    expect(json[0]).toHaveProperty('method');
    expect(json[0]).toHaveProperty('reason');
    expect(json[0]).toHaveProperty('created_at');
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
  expect(Array.isArray(json)).toBe(true);

  // Verify structure if entries exist
  if (json.length > 0) {
    expect(json[0]).toHaveProperty('table_name');
    expect(json[0]).toHaveProperty('action');
    expect(json[0]).toHaveProperty('created_at');
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

// ---- LIVE: Dashboard live endpoint structure ----

test('LIVE: dashboard/live pod entries have correct structure', async () => {
  const { status, json } = await api('/dashboard/live');
  expect(status).toBe(200);

  for (const pod of json.pods) {
    expect(typeof pod.pod_number).toBe('number');
    expect(['active', 'idle']).toContain(pod.status);
    expect(typeof pod.low_time).toBe('boolean');

    if (pod.status === 'active') {
      expect(pod.driver_name).toBeTruthy();
      expect(typeof pod.remaining_seconds).toBe('number');
    }

    if (pod.status === 'idle') {
      expect(pod.driver_name).toBeNull();
    }
  }
});

// ---- ANA: Analytics aggregation ----

test('ANA: tier_breakdown sums match daily_revenue totals', async () => {
  const { status, json } = await api('/dashboard/analytics');
  expect(status).toBe(200);

  const dailyTotal = json.daily_revenue.reduce(
    (sum: number, d: { revenue_paise: number }) => sum + d.revenue_paise,
    0
  );
  const tierTotal = json.tier_breakdown.reduce(
    (sum: number, t: { revenue_paise: number }) => sum + t.revenue_paise,
    0
  );

  // Totals should match (same date range, same data)
  expect(tierTotal).toBe(dailyTotal);
});

test('ANA: heatmap entries have valid day/hour ranges', async () => {
  const { json } = await api('/dashboard/analytics');

  for (const h of json.utilization_heatmap) {
    expect(h.day_of_week).toBeGreaterThanOrEqual(0);
    expect(h.day_of_week).toBeLessThanOrEqual(6);
    expect(h.hour).toBeGreaterThanOrEqual(0);
    expect(h.hour).toBeLessThanOrEqual(23);
    expect(h.pod_count).toBeGreaterThan(0);
  }
});

// ---- PWA-06: PDF receipt generation (unit-style) ----

test('PWA: public session endpoint returns privacy-safe data', async () => {
  const list = await api('/billing/sessions?limit=1');
  if (list.json.sessions.length === 0) {
    test.skip();
    return;
  }
  const sid = list.json.sessions[0].id;
  const res = await fetch(
    `${API.replace('/api/v1', '')}/public/sessions/${sid}`,
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
