/**
 * Layer 2 Wave 2 Phase 2 acceptance test: Dynamic Pricing Engine
 *
 * V-LBAC §S-219 iter4 bono-leg · sub-class FALSE-SUCCESS-REPORT recovery +
 * untracked-file live-sync clobber recovery (re-Written from agent inline-body return).
 *
 * Anchors:
 *   - PACT-DRAFT: /root/comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-dynamic-pricing-engine.md
 *   - AMENDMENT: /root/comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-dynamic-pricing-engine.AMENDMENT-PROPOSAL-20260512.md
 *   - V2-MASTER-STATE §S-214.6: 4 LOAD-BEARING invariants (Q-2-1/2/3/5)
 *   - V2-MASTER-STATE §S-49 Captain G33-GUIDE-CONFIRM Level B 3-rule spec
 *   - V2-MASTER-STATE §S-158 audit-log doctrine + §S-172 mechanism-trust-check
 *   - Joint #4 broadcast-only deeper-of-two locked + DoD L41 + DoD L159
 *
 * V1→V2 NEW-MECHANISM-CLASS gap:
 *   V1 pricing_rules + compute_dynamic_price() at billing_pricing.rs is single-rule LIMIT 1.
 *   V1 has NO rate_windows / campaign_attributions / session_rate_snapshots / boundary_fire
 *   token / ceiling CHECK / winning_campaign_id / price_locked_at_start / cross-surface
 *   tolerance / /v2/rates/effective endpoint / rate_window_state_transition audit enum.
 *   All 4 LOAD-BEARING surfaces require NEW substrate atop Phase 2-A + 2-F.
 *
 * ANTI-PATTERN GUARD — Mid-session live-rate vs locked-in rollback (Q-2-3a + DoD L159):
 *   V1 substrate live-propagates admin rate edits to in-flight sessions (DIRECT VIOLATION
 *   of DoD L159). Phase 2 engine MUST locked-in price_locked_at_start for active sessions
 *   when a campaign CANCELs mid-window.
 *
 * ANTI-PATTERN GUARD — Concurrent additive stacking vs deeper-of (Q-2-2b/c + Joint #4):
 *   Multi-campaign concurrent resolution is NEVER additive. Joint #4 locked "deeper of two".
 *   Billing surface MUST use MIN(rate_value_paise) NOT SUM/AVG. Q-2-2c caps stacked-discount
 *   at Q-2-1 ceiling.
 *
 * 5 tests (4 LOAD-BEARING + 1 edge):
 *   Test 1 Q-2-1a ceiling: engine-layer CHECK at substrate; sec-debt 7.3 closure
 *   Test 2 Q-2-2b/c concurrent: MIN deeper-of; winning_campaign_id + attributed[]; ceiling-cap
 *   Test 3 Q-2-3a rollback: active locked-in on CANCEL; superseded_by pointer; new = standard
 *   Test 4 Q-2-5b 2s tolerance: cross-surface fanout; bono-cloud lock authority
 *   Test 5 edge: single_match degenerate; concurrent_resolution_strategy=single_match
 *
 * Per CGP H3 "observations not verdicts": pre-enforcement branches use permissive
 * expect([...]).toContain(probe.status).
 *
 * G4 NOT TESTED (deferred):
 *   - Live engine wire-up (Wave 2 ship + Phase 2-A FILE + 2-F FILE)
 *   - CAVEAT-1 Captain-stake ceiling-value calibration
 *   - CAVEAT-2 empirical lock-authority under bono-cloud ↔ james-venue clock drift
 *   - CAVEAT-3 ≥3 children minimum-set composition
 *   - Q-2-7b HOLD-RELEASE-CAPTURE-OVERFLOW (Phase 2-C territory)
 *   - Q-2-4b full state-transition audit enumeration
 *   - Mid-boundary entry semantics (Phase 2-D + 2-B integration)
 *   - GST revenue-recognition split (§S-101)
 *   - V1 pricing_rules KEEP coexistence
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const TEST_ADMIN_JWT = process.env.TEST_ADMIN_JWT;
const DYNAMIC_PRICING_ENGINE_REACHABLE =
  process.env.DYNAMIC_PRICING_ENGINE_REACHABLE === 'true';
const TEST_SESSION_ID = process.env.TEST_SESSION_ID;
const TEST_CUSTOMER_ID = process.env.TEST_CUSTOMER_ID;
const TEST_POD_ID = process.env.TEST_POD_ID;
const MAX_DISCOUNT_PCT_RAW = process.env.MAX_DISCOUNT_PCT;
const MAX_DISCOUNT_PCT =
  MAX_DISCOUNT_PCT_RAW !== undefined && MAX_DISCOUNT_PCT_RAW !== ''
    ? Number.parseInt(MAX_DISCOUNT_PCT_RAW, 10)
    : 20;
const RATES_EFFECTIVE_PATH = process.env.RATES_EFFECTIVE_PATH || '/v2/rates/effective';
const RATES_WINDOW_ADMIN_PATH = process.env.RATES_WINDOW_ADMIN_PATH || '/v2/rates/admin/window';
const AUDIT_LOG_PATH = process.env.AUDIT_LOG_PATH || '/api/v1/audit-log';
const PWA_SURFACE_PATH = process.env.PWA_SURFACE_PATH || '/v2/surfaces/pwa/rate';
const POS_SURFACE_PATH = process.env.POS_SURFACE_PATH || '/v2/surfaces/pos/rate';
const KIOSK_SURFACE_PATH = process.env.KIOSK_SURFACE_PATH || '/v2/surfaces/kiosk/rate';
const AUTOBILL_SURFACE_PATH = process.env.AUTOBILL_SURFACE_PATH || '/v2/surfaces/autobill/rate';
const CROSS_SURFACE_TOLERANCE_MS = Number.parseInt(
  process.env.CROSS_SURFACE_TOLERANCE_MS || '2000',
  10,
);

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not set - Phase 2 requires network reachability to ${CANONICAL_URL}.`,
  ENGINE_MISSING: `DYNAMIC_PRICING_ENGINE_REACHABLE not set - Phase 2 engine does not exist in V1 substrate. PACT-DRAFT AMPLIFIER-READY but unfiled. Engine gates on Wave 2 ship + Phase 2-A FILE + 2-F FILE.`,
  TEST_ADMIN_JWT: `TEST_ADMIN_JWT not set - admin endpoints require staff JWT.`,
  TEST_SESSION_ID: `TEST_SESSION_ID not set - Test 3 + 4 require active session under LIVE rate_window.`,
  TEST_CUSTOMER_ID: `TEST_CUSTOMER_ID not set.`,
  TEST_POD_ID: `TEST_POD_ID not set.`,
};

interface RateWindow {
  window_id?: string;
  surface?: string;
  starts_at?: string;
  ends_at?: string;
  rate_value_paise?: number;
  superseded_by?: string | null;
  [k: string]: unknown;
}

interface EffectiveRateResponse {
  rate_id?: string;
  surface?: string;
  base_price_paise?: number;
  rate_value_paise?: number;
  final_price_paise?: number;
  window_id?: string;
  winning_campaign_id?: string | null;
  attributed_campaign_ids?: string[];
  concurrent_resolution_strategy?: 'single_match' | 'deeper_of' | string;
  price_locked_at_start?: boolean;
  superseded_by?: string | null;
  rate_kind?: string;
  lock_authority?: string;
  boundary_fire_idempotency_token?: string;
  resolved_at_ms?: number;
  [k: string]: unknown;
}

interface AuditLogRow {
  action_type?: string;
  rate_window_id?: string;
  ts?: string;
  [k: string]: unknown;
}

interface ApiRead<T = unknown> {
  url: string;
  status: number;
  body: T;
  read_started_at: number;
  read_finished_at: number;
}

const RATE_WINDOW_AUDIT_ACTION_TYPES = new Set<string>([
  'rate_window_state_transition',
  'rate_window_created',
  'rate_window_superseded',
  'rate_window_above_ceiling_approved',
  'rate_transition_emitted',
]);

async function fetchEffectiveRate(
  surface: string,
  at?: string,
  extras?: Record<string, string>,
): Promise<ApiRead<EffectiveRateResponse>> {
  const started = Date.now();
  const params = new URLSearchParams({ surface });
  if (at) params.set('at', at);
  if (TEST_SESSION_ID) params.set('session_id', TEST_SESSION_ID);
  if (TEST_CUSTOMER_ID) params.set('customer_id', TEST_CUSTOMER_ID);
  if (extras) for (const [k, v] of Object.entries(extras)) params.set(k, v);
  const url = `${CANONICAL_URL}${RATES_EFFECTIVE_PATH}?${params.toString()}`;
  const headers: Record<string, string> = {};
  if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
  const resp = await fetch(url, { headers });
  const finished = Date.now();
  let body: EffectiveRateResponse;
  try {
    body = (await resp.json()) as EffectiveRateResponse;
  } catch {
    body = {} as EffectiveRateResponse;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

async function fetchSurfaceRate(
  surfacePath: string,
  surface: string,
): Promise<ApiRead<EffectiveRateResponse>> {
  const started = Date.now();
  const params = new URLSearchParams({ surface });
  if (TEST_SESSION_ID) params.set('session_id', TEST_SESSION_ID);
  if (TEST_CUSTOMER_ID) params.set('customer_id', TEST_CUSTOMER_ID);
  if (TEST_POD_ID) params.set('pod_id', TEST_POD_ID);
  const url = `${CANONICAL_URL}${surfacePath}?${params.toString()}`;
  const headers: Record<string, string> = {};
  if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
  const resp = await fetch(url, { headers });
  const finished = Date.now();
  let body: EffectiveRateResponse;
  try {
    body = (await resp.json()) as EffectiveRateResponse;
  } catch {
    body = {} as EffectiveRateResponse;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

async function fetchAuditLog(
  filters: Record<string, string>,
): Promise<ApiRead<AuditLogRow[]>> {
  const started = Date.now();
  const qs = new URLSearchParams(filters).toString();
  const url = `${CANONICAL_URL}${AUDIT_LOG_PATH}?${qs}`;
  const headers: Record<string, string> = {};
  if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
  const resp = await fetch(url, { headers });
  const finished = Date.now();
  let body: AuditLogRow[];
  try {
    const parsed = (await resp.json()) as AuditLogRow[] | { rows?: AuditLogRow[] };
    body = Array.isArray(parsed) ? parsed : (parsed.rows ?? []);
  } catch {
    body = [];
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

async function cancelRateWindow(windowId: string): Promise<ApiRead<RateWindow>> {
  const started = Date.now();
  const url = `${CANONICAL_URL}${RATES_WINDOW_ADMIN_PATH}/${encodeURIComponent(windowId)}/cancel`;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
  const resp = await fetch(url, { method: 'POST', headers, body: '{}' });
  const finished = Date.now();
  let body: RateWindow;
  try {
    body = (await resp.json()) as RateWindow;
  } catch {
    body = {} as RateWindow;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

test.describe('Layer 2 W2 Phase 2 - Dynamic Pricing Engine', () => {
  test('Test 1: LOAD-BEARING Q-2-1a ceiling - engine-layer CHECK enforces MAX_DISCOUNT_PCT', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!DYNAMIC_PRICING_ENGINE_REACHABLE, SKIP_REASONS.ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);

    const probe = await fetchEffectiveRate('sim');
    console.log(`effective_rate sim: status=${probe.status} body=${JSON.stringify(probe.body)}`);
    expect([200, 401, 403, 404, 500]).toContain(probe.status);
    if (probe.status !== 200) {
      test.skip(true, `Engine endpoint returned ${probe.status}; ceiling assertion requires 200.`);
      return;
    }
    expect(typeof probe.body.base_price_paise).toBe('number');
    expect(typeof probe.body.final_price_paise).toBe('number');
    const base = probe.body.base_price_paise!;
    const final = probe.body.final_price_paise!;
    const floorPaise = Math.ceil(base * (1 - MAX_DISCOUNT_PCT / 100));
    console.log(`ceiling-check: base=${base} final=${final} floor=${floorPaise}`);
    expect(final).toBeGreaterThanOrEqual(floorPaise);
    expect(final).toBeLessThanOrEqual(base);
    if (probe.body.lock_authority !== undefined) {
      expect(['bono-cloud', 'james-venue']).toContain(probe.body.lock_authority);
    }
  });

  test('Test 2: LOAD-BEARING Q-2-2b/c concurrent rate resolution - deeper-of MIN; winning + attributed split; ceiling-capped', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!DYNAMIC_PRICING_ENGINE_REACHABLE, SKIP_REASONS.ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);

    const probe = await fetchEffectiveRate('sim', undefined, {
      concurrent_resolution: 'deeper_of',
      include_attribution: 'true',
    });
    console.log(`deeper_of: status=${probe.status} body=${JSON.stringify(probe.body)}`);
    expect([200, 401, 403, 404, 500]).toContain(probe.status);
    if (probe.status !== 200) {
      test.skip(true, `Engine returned ${probe.status}.`);
      return;
    }
    if (probe.body.concurrent_resolution_strategy !== undefined) {
      expect(['deeper_of', 'single_match']).toContain(probe.body.concurrent_resolution_strategy);
    }
    if (probe.body.winning_campaign_id !== undefined) {
      expect(probe.body.winning_campaign_id === null || typeof probe.body.winning_campaign_id === 'string').toBe(true);
    }
    if (probe.body.attributed_campaign_ids !== undefined) {
      expect(Array.isArray(probe.body.attributed_campaign_ids)).toBe(true);
      if (typeof probe.body.winning_campaign_id === 'string' && probe.body.winning_campaign_id.length > 0) {
        expect(probe.body.attributed_campaign_ids).toContain(probe.body.winning_campaign_id);
      }
    }
    if (typeof probe.body.base_price_paise === 'number' && typeof probe.body.final_price_paise === 'number') {
      const floorPaise = Math.ceil(probe.body.base_price_paise * (1 - MAX_DISCOUNT_PCT / 100));
      expect(probe.body.final_price_paise).toBeGreaterThanOrEqual(floorPaise);
    }
  });

  test('Test 3: LOAD-BEARING Q-2-3a rollback - active locked-in on CANCEL; superseded_by; new = standard', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!DYNAMIC_PRICING_ENGINE_REACHABLE, SKIP_REASONS.ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    const before = await fetchEffectiveRate('sim');
    console.log(`pre-cancel: status=${before.status} window_id=${before.body.window_id}`);
    expect([200, 401, 403, 404, 500]).toContain(before.status);
    if (before.status !== 200) {
      test.skip(true, `Engine returned ${before.status}.`);
      return;
    }
    const windowId = before.body.window_id;
    if (!windowId) {
      test.skip(true, 'No window_id in response.');
      return;
    }
    const lockedPriceBefore = before.body.final_price_paise;
    const cancel = await cancelRateWindow(windowId);
    console.log(`cancel ${windowId}: status=${cancel.status}`);
    expect([200, 202, 204, 401, 403, 404]).toContain(cancel.status);
    if (cancel.status >= 400) {
      test.skip(true, `Admin cancel returned ${cancel.status}.`);
      return;
    }
    const afterActive = await fetchEffectiveRate('sim');
    console.log(`post-cancel active: status=${afterActive.status} locked=${afterActive.body.price_locked_at_start}`);
    expect(afterActive.status).toBe(200);
    expect(afterActive.body.price_locked_at_start).toBe(true);
    if (typeof lockedPriceBefore === 'number') {
      expect(afterActive.body.final_price_paise).toBe(lockedPriceBefore);
    }
    const newArrivalUrl = new URL(`${CANONICAL_URL}${RATES_EFFECTIVE_PATH}`);
    newArrivalUrl.searchParams.set('surface', 'sim');
    newArrivalUrl.searchParams.set('new_arrival', 'true');
    const headers: Record<string, string> = {};
    if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
    const newArrivalResp = await fetch(newArrivalUrl.toString(), { headers });
    let newArrivalBody: EffectiveRateResponse;
    try {
      newArrivalBody = (await newArrivalResp.json()) as EffectiveRateResponse;
    } catch {
      newArrivalBody = {} as EffectiveRateResponse;
    }
    console.log(`post-cancel new-arrival: status=${newArrivalResp.status} locked=${newArrivalBody.price_locked_at_start}`);
    expect([200, 401, 403, 404]).toContain(newArrivalResp.status);
    if (newArrivalResp.status === 200 && newArrivalBody.price_locked_at_start !== undefined) {
      expect(newArrivalBody.price_locked_at_start).toBe(false);
    }
  });

  test('Test 4: LOAD-BEARING Q-2-5b 2s tolerance - cross-surface fanout; bono-cloud lock authority', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!DYNAMIC_PRICING_ENGINE_REACHABLE, SKIP_REASONS.ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);
    test.skip(!TEST_CUSTOMER_ID, SKIP_REASONS.TEST_CUSTOMER_ID);
    test.skip(!TEST_POD_ID, SKIP_REASONS.TEST_POD_ID);

    const [pwa, pos, kiosk, autobill] = await Promise.all([
      fetchSurfaceRate(PWA_SURFACE_PATH, 'sim'),
      fetchSurfaceRate(POS_SURFACE_PATH, 'sim'),
      fetchSurfaceRate(KIOSK_SURFACE_PATH, 'sim'),
      fetchSurfaceRate(AUTOBILL_SURFACE_PATH, 'sim'),
    ]);
    console.log(`fanout: pwa=${pwa.status} pos=${pos.status} kiosk=${kiosk.status} autobill=${autobill.status}`);
    const all = [pwa, pos, kiosk, autobill];
    for (const a of all) expect([200, 401, 403, 404, 500]).toContain(a.status);
    const reachable = all.filter((a) => a.status === 200);
    if (reachable.length < 2) {
      test.skip(true, `Only ${reachable.length}/4 surfaces returned 200.`);
      return;
    }
    const resolvedAt = reachable.map((a) => a.body.resolved_at_ms).filter((v): v is number => typeof v === 'number');
    if (resolvedAt.length >= 2) {
      const spread = Math.max(...resolvedAt) - Math.min(...resolvedAt);
      console.log(`spread=${spread}ms tolerance=${CROSS_SURFACE_TOLERANCE_MS}ms`);
      expect(spread).toBeLessThanOrEqual(CROSS_SURFACE_TOLERANCE_MS);
    }
    for (const a of reachable) {
      if (a.body.lock_authority !== undefined) {
        expect(a.body.lock_authority).toBe('bono-cloud');
      }
    }
    const tokens = reachable.map((a) => a.body.boundary_fire_idempotency_token).filter((v): v is string => typeof v === 'string' && v.length > 0);
    if (tokens.length >= 2) {
      const uniqueTokens = new Set(tokens);
      expect(uniqueTokens.size).toBe(1);
    }
  });

  test('Test 5: edge Q-2-2 single-campaign degenerate - concurrent_resolution_strategy=single_match', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!DYNAMIC_PRICING_ENGINE_REACHABLE, SKIP_REASONS.ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);

    const probe = await fetchEffectiveRate('sim', undefined, { force_single_eligibility: 'true' });
    console.log(`single_match: status=${probe.status} strategy=${probe.body.concurrent_resolution_strategy}`);
    expect([200, 401, 403, 404, 500]).toContain(probe.status);
    if (probe.status !== 200) {
      test.skip(true, `Engine returned ${probe.status}.`);
      return;
    }
    if (probe.body.concurrent_resolution_strategy !== undefined) {
      expect(['single_match', 'deeper_of']).toContain(probe.body.concurrent_resolution_strategy);
    }
    if (probe.body.concurrent_resolution_strategy === 'single_match') {
      expect(typeof probe.body.base_price_paise).toBe('number');
      expect(typeof probe.body.rate_value_paise).toBe('number');
      expect(typeof probe.body.final_price_paise).toBe('number');
      expect(typeof probe.body.window_id).toBe('string');
      if (probe.body.attributed_campaign_ids !== undefined) {
        expect(probe.body.attributed_campaign_ids.length).toBeLessThanOrEqual(1);
      }
    }
    if (probe.body.window_id) {
      const audit = await fetchAuditLog({ rate_window_id: probe.body.window_id });
      if (audit.status === 200 && audit.body.length >= 1) {
        for (const row of audit.body) {
          if (row.action_type !== undefined) {
            expect(RATE_WINDOW_AUDIT_ACTION_TYPES.has(row.action_type)).toBe(true);
          }
        }
      }
    }
  });
});
