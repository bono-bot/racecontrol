/**
 * Layer 7.7 acceptance test: POST /api/v1/cirs/lookup CSRF + SameSite=Lax
 *
 * V-LBAC §S-219 iter4 bono-leg · sub-class FALSE-SUCCESS-REPORT recovery +
 * untracked-file live-sync clobber recovery.
 *
 * V2-PROGRESS-MAP row 7.7 (auth-debt): IN-FLIGHT Post-V2.0-Multi-Origin · customer-impact=YES
 * Composes-with james §S-219 row 7.6 staff-session-cookie (commit b08b0073).
 *
 * DISCOVERY beyond scaffolding:
 *   COOKIE AUTH NOT SHIPPED AT THE HANDLER TODAY. auth/middleware.rs:66-73
 *   extract_staff_claims reads ONLY Authorization: Bearer — NO cookie parsing.
 *   auth_staff.rs staff_validate_pin emits JWT in response BODY, NOT as Set-Cookie.
 *   web-v2 sends credentials:'include' but server discards cookie store.
 *
 *   This means CSRF attack vector for /api/v1/cirs/lookup is STRUCTURALLY ABSENT today:
 *   attacker cannot forge cross-origin POST server will authenticate, because server only
 *   authenticates Bearer tokens (subject to CORS preflight, not CSRF).
 *
 *   Sec-debt-ledger 2026-05-12T05:37:35Z: row CLOSED-VIA-STRUCTURAL-PROTECTION
 *   (Bearer-only auth = no cookie surface = no CSRF vector).
 *
 *   WATCH TRIGGER: when cookie auth lands (row 7.6 closure HttpOnly JWT issuance),
 *   this row REOPENS with SameSite=Strict scope as smallest-reversible closure.
 *
 * ANTI-PATTERN GUARD — DO NOT REMOVE THE ENV GATES.
 *   The 5 env gates are correctness gates, not speedups. Removing them would force
 *   the suite to ASSERT against a surface that hasn't shipped, producing red-fail
 *   noise. CSRF_ENFORCEMENT_ACTIVE / SAMESITE_LAX_ACTIVE are explicit
 *   "closure-phase target has landed" signals.
 *
 * Citations:
 *   - cirs_lookup.rs:130 cirs_lookup_handler (Bearer-only via Extension<StaffClaims>)
 *   - routes.rs:840-844 /cirs/lookup sub-router with auth_rate_limit_layer (sec-debt 7.9
 *     closure PR #68 squash-merge commit 61999f58)
 *   - auth/middleware.rs:66-73 extract_staff_claims Bearer-only no cookie parsing
 *   - james §S-219 row 7.6 staff-session-cookie commit b08b0073 (5 GAPs documented)
 *
 * 5 tests:
 *   T1 cookie-auth baseline: cookie + canonical phone payload returns 200
 *      (closure-phase shipped path; SKIP today)
 *   T2 CSRF rejection (absent token): cookie + cross-origin + NO X-CSRF-Token → 403
 *   T3 CSRF positive (valid token): cookie + valid X-CSRF-Token → 200
 *   T4 SameSite=Lax attribute on Set-Cookie at staff login
 *   T5 rate-limit (7.9 CLOSED) composes-with CSRF (7.7) — 21 rapid hits 429,
 *      post-decay no-CSRF still 403 (no stacking conflict)
 *
 * G4 NOT TESTED (deferred):
 *   - CSRF token rotation semantics
 *   - Origin/Referer header validation as additional layer
 *   - Cookie Path/Domain (Domain covered by row 7.6 GAP-4)
 *   - Logout/revocation (row 7.6 T4)
 *   - Multi-tab session cookie collision
 *   - Token-bucket replenishment edge cases (PR #68 unit tests cover)
 *   - SameSite=Strict promotion (T4 floor is Lax)
 *   - CORS preflight headers
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const TEST_CUSTOMER_COOKIE = process.env.TEST_CUSTOMER_COOKIE;
const TEST_COOKIE_NAME = process.env.TEST_COOKIE_NAME || 'rp_staff_session';
const TEST_CSRF_TOKEN = process.env.TEST_CSRF_TOKEN;
const CSRF_ENFORCEMENT_ACTIVE = process.env.CSRF_ENFORCEMENT_ACTIVE === 'true';
const SAMESITE_LAX_ACTIVE = process.env.SAMESITE_LAX_ACTIVE === 'true';
const TEST_CUSTOMER_PHONE = process.env.TEST_CUSTOMER_PHONE || '+919999900001';
const TEST_STAFF_LOGIN_PATH = process.env.TEST_STAFF_LOGIN_PATH || '/api/v1/staff/validate-pin';
const TEST_STAFF_PIN = process.env.TEST_STAFF_PIN;
const TEST_STAFF_ID = process.env.TEST_STAFF_ID;

const CIRS_LOOKUP_PATH = '/api/v1/cirs/lookup';

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not 'true' - Layer 7.7 contract probes ${CANONICAL_URL}${CIRS_LOOKUP_PATH}.`,
  TEST_CUSTOMER_COOKIE: `TEST_CUSTOMER_COOKIE not set - V2 staff session cookie issued at row 7.6 closure-phase.`,
  TEST_CSRF_TOKEN: `TEST_CSRF_TOKEN not set - valid X-CSRF-Token paired with session cookie.`,
  CSRF_ENFORCEMENT_ACTIVE: `CSRF_ENFORCEMENT_ACTIVE not 'true' - CSRF middleware not deployed today (structural protection via Bearer-only auth).`,
  SAMESITE_LAX_ACTIVE: `SAMESITE_LAX_ACTIVE not 'true' - no Set-Cookie issued at staff login today (row 7.6 GAP-1).`,
  STAFF_LOGIN_INPUTS: `TEST_STAFF_PIN or TEST_STAFF_ID not set.`,
};

interface ApiRead {
  url: string;
  status: number;
  body: any;
  headers: Record<string, string>;
  setCookies: string[];
  read_started_at: number;
  read_finished_at: number;
}

function buildCookieHeader(rawValue: string, name: string): string {
  const firstSegment = rawValue.split(';')[0].trim();
  if (firstSegment.includes('=')) return firstSegment;
  return `${name}=${firstSegment}`;
}

async function apiPost(
  path: string,
  init: {
    cookie?: string;
    bearer?: string;
    csrf?: string;
    body?: unknown;
    origin?: string;
  } = {},
): Promise<ApiRead> {
  const url = `${CANONICAL_URL}${path}`;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (init.cookie) headers['Cookie'] = init.cookie;
  if (init.bearer) headers['Authorization'] = `Bearer ${init.bearer}`;
  if (init.csrf) headers['X-CSRF-Token'] = init.csrf;
  if (init.origin) headers['Origin'] = init.origin;
  const started = Date.now();
  const resp = await fetch(url, {
    method: 'POST',
    headers,
    body: init.body !== undefined ? JSON.stringify(init.body) : undefined,
  });
  const finished = Date.now();
  let body: any = null;
  try {
    body = await resp.json();
  } catch {
    body = null;
  }
  const headerMap: Record<string, string> = {};
  const setCookies: string[] = [];
  resp.headers.forEach((value, key) => {
    headerMap[key.toLowerCase()] = value;
    if (key.toLowerCase() === 'set-cookie') setCookies.push(value);
  });
  const anyHeaders = resp.headers as any;
  if (typeof anyHeaders.getSetCookie === 'function') {
    const arr: string[] = anyHeaders.getSetCookie();
    if (Array.isArray(arr) && arr.length > 0) {
      setCookies.length = 0;
      setCookies.push(...arr);
    }
  }
  return {
    url,
    status: resp.status,
    body,
    headers: headerMap,
    setCookies,
    read_started_at: started,
    read_finished_at: finished,
  };
}

function looksLikeProfilePreview(body: any): boolean {
  if (!body || typeof body !== 'object') return false;
  return typeof body.customer_id === 'string' && Array.isArray(body.profiles) && 'discount_ineligible' in body;
}

function parseCookieAttributes(setCookieValue: string): Record<string, string | true> {
  const parts = setCookieValue.split(';').map((p) => p.trim()).filter(Boolean);
  const out: Record<string, string | true> = {};
  parts.slice(1).forEach((part) => {
    const eq = part.indexOf('=');
    if (eq === -1) {
      out[part.toLowerCase()] = true;
    } else {
      const k = part.slice(0, eq).trim().toLowerCase();
      const v = part.slice(eq + 1).trim();
      out[k] = v;
    }
  });
  const first = parts[0] ?? '';
  const eq = first.indexOf('=');
  if (eq !== -1) {
    out['__name'] = first.slice(0, eq).trim();
    out['__value'] = first.slice(eq + 1).trim();
  }
  return out;
}

test.describe('Layer 7.7 - POST /api/v1/cirs/lookup CSRF + SameSite=Lax', () => {
  test('T1 cookie-auth baseline: cookie + canonical phone payload returns 200', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!CSRF_ENFORCEMENT_ACTIVE, SKIP_REASONS.CSRF_ENFORCEMENT_ACTIVE);
    test.skip(!TEST_CUSTOMER_COOKIE, SKIP_REASONS.TEST_CUSTOMER_COOKIE);
    test.skip(!TEST_CSRF_TOKEN, SKIP_REASONS.TEST_CSRF_TOKEN);

    const cookieHeader = buildCookieHeader(TEST_CUSTOMER_COOKIE!, TEST_COOKIE_NAME);
    const r = await apiPost(CIRS_LOOKUP_PATH, {
      cookie: cookieHeader,
      csrf: TEST_CSRF_TOKEN,
      body: { method: 'phone', phone: TEST_CUSTOMER_PHONE },
    });
    console.log(`T1: status=${r.status} preview=${looksLikeProfilePreview(r.body)}`);
    expect(r.status).toBe(200);
    const isPreview = looksLikeProfilePreview(r.body);
    const isNotFound = r.body && typeof r.body === 'object' && (r.body.result === 'not_found' || 'error' in r.body);
    expect(isPreview || isNotFound).toBe(true);
  });

  test('T2 CSRF rejection: cookie + MISSING CSRF token returns 403', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!CSRF_ENFORCEMENT_ACTIVE, SKIP_REASONS.CSRF_ENFORCEMENT_ACTIVE);
    test.skip(!TEST_CUSTOMER_COOKIE, SKIP_REASONS.TEST_CUSTOMER_COOKIE);

    const cookieHeader = buildCookieHeader(TEST_CUSTOMER_COOKIE!, TEST_COOKIE_NAME);
    const r = await apiPost(CIRS_LOOKUP_PATH, {
      cookie: cookieHeader,
      body: { method: 'phone', phone: TEST_CUSTOMER_PHONE },
      origin: 'https://evil.example',
    });
    console.log(`T2: status=${r.status}`);
    expect(r.status).toBe(403);
    if (r.body && typeof r.body === 'object') {
      expect('customer_id' in r.body).toBe(false);
    }
  });

  test('T3 CSRF positive: cookie + valid CSRF token returns 200', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!CSRF_ENFORCEMENT_ACTIVE, SKIP_REASONS.CSRF_ENFORCEMENT_ACTIVE);
    test.skip(!TEST_CUSTOMER_COOKIE, SKIP_REASONS.TEST_CUSTOMER_COOKIE);
    test.skip(!TEST_CSRF_TOKEN, SKIP_REASONS.TEST_CSRF_TOKEN);

    const cookieHeader = buildCookieHeader(TEST_CUSTOMER_COOKIE!, TEST_COOKIE_NAME);
    const r = await apiPost(CIRS_LOOKUP_PATH, {
      cookie: cookieHeader,
      csrf: TEST_CSRF_TOKEN,
      body: { method: 'phone', phone: TEST_CUSTOMER_PHONE },
    });
    console.log(`T3: status=${r.status} preview=${looksLikeProfilePreview(r.body)}`);
    expect(r.status).toBe(200);
    const isPreview = looksLikeProfilePreview(r.body);
    const isNotFound = r.body && typeof r.body === 'object' && (r.body.result === 'not_found' || 'error' in r.body);
    expect(isPreview || isNotFound).toBe(true);
  });

  test('T4 SameSite=Lax attribute on Set-Cookie at staff login', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!SAMESITE_LAX_ACTIVE, SKIP_REASONS.SAMESITE_LAX_ACTIVE);
    test.skip(!TEST_STAFF_PIN || !TEST_STAFF_ID, SKIP_REASONS.STAFF_LOGIN_INPUTS);

    const r = await apiPost(TEST_STAFF_LOGIN_PATH, {
      body: { staff_id: TEST_STAFF_ID, pin: TEST_STAFF_PIN },
    });
    console.log(`T4: status=${r.status} cookies=${r.setCookies.length}`);
    expect([200, 204]).toContain(r.status);
    expect(r.setCookies.length).toBeGreaterThanOrEqual(1);
    const candidate = r.setCookies.find((c) => {
      const attrs = parseCookieAttributes(c);
      const name = String(attrs['__name'] ?? '');
      return name === 'rp_staff_session' || name === 'staff_session' || name === TEST_COOKIE_NAME;
    });
    expect(candidate).toBeDefined();
    const attrs = parseCookieAttributes(candidate!);
    const sameSite = String(attrs['samesite'] ?? '').toLowerCase();
    console.log(`T4 samesite=${sameSite}`);
    expect(sameSite === 'lax' || sameSite === 'strict').toBe(true);
  });

  test('T5 rate-limit (7.9 CLOSED) composes-with CSRF (7.7) - no stacking conflict', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!CSRF_ENFORCEMENT_ACTIVE, SKIP_REASONS.CSRF_ENFORCEMENT_ACTIVE);
    test.skip(!SAMESITE_LAX_ACTIVE, SKIP_REASONS.SAMESITE_LAX_ACTIVE);
    test.skip(!TEST_CUSTOMER_COOKIE, SKIP_REASONS.TEST_CUSTOMER_COOKIE);
    test.skip(!TEST_CSRF_TOKEN, SKIP_REASONS.TEST_CSRF_TOKEN);

    const cookieHeader = buildCookieHeader(TEST_CUSTOMER_COOKIE!, TEST_COOKIE_NAME);
    const requestBody = { method: 'phone', phone: TEST_CUSTOMER_PHONE };
    const phaseAStatuses: number[] = [];
    for (let i = 0; i < 21; i++) {
      const r = await apiPost(CIRS_LOOKUP_PATH, {
        cookie: cookieHeader,
        csrf: TEST_CSRF_TOKEN,
        body: requestBody,
      });
      phaseAStatuses.push(r.status);
    }
    console.log(`T5(a): statuses=${phaseAStatuses.join(',')}`);
    expect(phaseAStatuses.includes(429)).toBe(true);
    await new Promise((resolve) => setTimeout(resolve, 4000));
    const probe = await apiPost(CIRS_LOOKUP_PATH, {
      cookie: cookieHeader,
      body: requestBody,
      origin: 'https://evil.example',
    });
    console.log(`T5(b): status=${probe.status}`);
    expect(probe.status).toBe(403);
  });
});
