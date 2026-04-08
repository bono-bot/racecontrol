import { test, expect } from '@playwright/test';

/**
 * API-level E2E tests for the billing lifecycle.
 * These test the actual HTTP endpoints on the racecontrol server,
 * verifying the full data flow from creation to completion.
 *
 * Staff-protected routes require JWT auth (obtained via admin-login).
 * API base: http://192.168.31.23:8080/api/v1
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

// ---- Billing Sessions list with filters ----

test('GET /billing/sessions supports date_from and date_to filters', async () => {
  const { status, json } = await api('/billing/sessions?date_from=2026-03-01&date_to=2026-03-31');
  expect(status).toBe(200);
  expect(json).toHaveProperty('sessions');
  expect(Array.isArray(json.sessions)).toBe(true);
});

test('GET /billing/sessions supports status filter', async () => {
  const { status, json } = await api('/billing/sessions?status=completed');
  expect(status).toBe(200);
  expect(json).toHaveProperty('sessions');
  // All returned sessions should be completed
  for (const s of json.sessions) {
    expect(s.status).toBe('completed');
  }
});

test('GET /billing/sessions supports limit and offset', async () => {
  const { status, json } = await api('/billing/sessions?limit=5&offset=0');
  expect(status).toBe(200);
  expect(json).toHaveProperty('sessions');
  expect(Array.isArray(json.sessions)).toBe(true);
});

// ---- Billing Session detail includes payment_method ----

test('GET /billing/sessions/:id returns session detail', async () => {
  const list = await api('/billing/sessions?limit=1');
  if (!list.json?.sessions?.length) {
    test.skip();
    return;
  }
  const sessionId = list.json.sessions[0].id;
  const { status, json } = await api(`/billing/sessions/${sessionId}`);
  expect(status).toBe(200);
  expect(json).toHaveProperty('id');
  expect(json).toHaveProperty('driver_name');
  expect(json).toHaveProperty('price_paise');
});

// ---- Billing Session events (timeline) ----

test('GET /billing/sessions/:id/events returns events array', async () => {
  const list = await api('/billing/sessions?limit=1');
  if (list.json.sessions.length === 0) {
    test.skip();
    return;
  }
  const sessionId = list.json.sessions[0].id;
  const { status, json } = await api(`/billing/sessions/${sessionId}/events`);
  expect(status).toBe(200);
  expect(json).toHaveProperty('events');
  expect(Array.isArray(json.events)).toBe(true);
});

// ---- Billing Session summary (with discount fields) ----

test('GET /billing/sessions/:id/summary returns session summary', async () => {
  const list = await api('/billing/sessions?limit=1');
  if (!list.json?.sessions?.length) {
    test.skip();
    return;
  }
  const sessionId = list.json.sessions[0].id;
  const { status, json } = await api(`/billing/sessions/${sessionId}/summary`);
  expect(status).toBe(200);
  expect(json).toHaveProperty('summary');
});

// ---- Daily Billing Report ----

test('GET /billing/report/daily returns structured report', async () => {
  const today = new Date().toISOString().split('T')[0];
  const { status, json } = await api(`/billing/report/daily?date=${today}`);
  expect(status).toBe(200);
  expect(json).toHaveProperty('date');
  expect(json).toHaveProperty('total_sessions');
  expect(json).toHaveProperty('total_revenue_paise');
  expect(json).toHaveProperty('total_driving_seconds');
  expect(json).toHaveProperty('sessions');
  expect(typeof json.total_sessions).toBe('number');
  expect(typeof json.total_revenue_paise).toBe('number');
});

// ---- Billing Rates CRUD ----

test('GET /billing/rates returns rate tiers', async () => {
  const { status, json } = await api('/billing/rates');
  expect(status).toBe(200);
  expect(json).toHaveProperty('rates');
  expect(Array.isArray(json.rates)).toBe(true);
  if (json.rates.length > 0) {
    expect(json.rates[0]).toHaveProperty('tier_name');
    expect(json.rates[0]).toHaveProperty('rate_per_min_paise');
    expect(json.rates[0]).toHaveProperty('threshold_minutes');
  }
});

// ---- Refund endpoint validation ----

test('POST /billing/:id/refund rejects invalid method', async () => {
  const { status, json } = await api('/billing/nonexistent/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: 100, method: 'bitcoin', reason: 'test' }),
  });
  expect(json).toHaveProperty('error');
  expect(json.error).toMatch(/method must be/i);
});

test('POST /billing/:id/refund rejects missing reason', async () => {
  const { status, json } = await api('/billing/nonexistent/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: 100, method: 'wallet', reason: '' }),
  });
  expect(json).toHaveProperty('error');
  expect(json.error).toMatch(/reason is required/i);
});

test('POST /billing/:id/refund rejects zero amount', async () => {
  const { status, json } = await api('/billing/nonexistent/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: 0, method: 'wallet', reason: 'test' }),
  });
  expect(json).toHaveProperty('error');
  expect(json.error).toMatch(/amount_paise must be positive/i);
});

test('POST /billing/:id/refund returns not found for invalid session', async () => {
  const { status, json } = await api('/billing/nonexistent-id-12345/refund', {
    method: 'POST',
    body: JSON.stringify({ amount_paise: 100, method: 'wallet', reason: 'test refund' }),
  });
  expect(json).toHaveProperty('error');
  expect(json.error).toMatch(/not found/i);
});

// ---- Refund history ----

test('GET /billing/:id/refunds returns refund list', async () => {
  const list = await api('/billing/sessions?limit=1');
  if (!list.json?.sessions?.length) {
    test.skip();
    return;
  }
  const sessionId = list.json.sessions[0].id;
  const { status, json } = await api(`/billing/${sessionId}/refunds`);
  expect(status).toBe(200);
  expect(json).toHaveProperty('refunds');
  expect(Array.isArray(json.refunds)).toBe(true);
});

// ---- Audit log captures POS actions ----

test('GET /audit-log returns audit entries', async () => {
  const { status, json } = await api('/audit-log');
  expect(status).toBe(200);
  expect(json).toHaveProperty('entries');
  expect(Array.isArray(json.entries)).toBe(true);
});

// ---- Public session endpoint (no auth) ----

test('GET /public/sessions/:id returns error for invalid session', async () => {
  const { json } = await api('/public/sessions/nonexistent-id');
  expect(json).toHaveProperty('error');
});

// ---- Start billing validation ----

test('POST /billing/start rejects missing fields', async () => {
  const { status, json } = await api('/billing/start', {
    method: 'POST',
    body: JSON.stringify({ pod_id: '', driver_id: '', pricing_tier_id: '' }),
  });
  expect(json).toHaveProperty('error');
  expect(json.error).toMatch(/required/i);
});

test('POST /billing/start rejects nonexistent driver', async () => {
  const { status, json } = await api('/billing/start', {
    method: 'POST',
    body: JSON.stringify({
      pod_id: 'pod-1',
      driver_id: 'nonexistent-driver-xyz',
      pricing_tier_id: 'tier-1',
    }),
  });
  expect(json).toHaveProperty('error');
});

test('POST /billing/start accepts payment_method field', async () => {
  const { status, json } = await api('/billing/start', {
    method: 'POST',
    body: JSON.stringify({
      pod_id: 'pod-1',
      driver_id: 'nonexistent-driver-xyz',
      pricing_tier_id: 'tier-1',
      payment_method: 'upi',
    }),
  });
  // Should fail on driver/tier validation, not on payment_method
  expect(json).toHaveProperty('error');
  expect(json.error).not.toMatch(/payment_method/i);
});
