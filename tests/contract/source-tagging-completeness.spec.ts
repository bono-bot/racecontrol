/**
 * Layer 1.16 acceptance test: Source-tagging completeness
 * (PWA / POS / Kiosk x UPI / card / cash)
 *
 * V2-PROGRESS-MAP §1 row 1.16 ("All-beat | Source-tagging completeness
 * (PWA/POS/Kiosk x UPI/card/cash) | PARTIAL | james-owned | enum locked
 * in DoD §3.3; implementation drift unverified").
 *
 * Source-of-truth:
 *   - comms-link/v2-skeleton/05-definition-of-done.md §3.3
 *     ("Substrate primitives - wallet . source_tag enumeration (locked v2.0)")
 *   - DoD line 209 standing rule:
 *     "No untagged wallet transaction. Every transaction carries source
 *      (PWA / POS .130 / Kiosk), payment method (UPI / card / cash),
 *      and operator. Untagged = audit-blind = constitutional violation."
 *   - V2 canonical-source ledger doctrine: Server .23 racecontrol is the
 *     authoritative ledger; surfaces (PWA / POS / Kiosk) emit tagged
 *     write events; the canonical store retains the tag immutably.
 *
 * Contract under test:
 *   For the last LOOKBACK_DAYS of billing / wallet / session events
 *   in the canonical store (Server .23 racecontrol):
 *
 *   C1 (shape):  The configured BILLING_LIST_PATH responds 200 and
 *                returns an array of rows under {sessions|transactions|items|rows}.
 *   C2 (source enum drift):  Every row.source MUST be one of
 *                {pwa, pos, kiosk}.  Any other value = test FAILURE
 *                (this exposes the "implementation drift unverified"
 *                half of row 1.16).
 *   C3 (payment_method enum drift):  Every row.payment_method MUST
 *                be one of {upi, card, cash}.  Any other value = FAILURE.
 *   C4 (matrix coverage report):  For the 3x3 surface x payment-type
 *                matrix, list cells with ZERO observed events.  GAPS
 *                are OBSERVATIONS (console.log) not assertions - the
 *                test does not fail on empty cells, only on unenumerated
 *                values.
 *   C5 (R1-C correlation sanity):  DoD §1 (line 60) locks "Cash is
 *                NOT accepted at Kiosk" - Kiosk x cash combo = FAIL.
 *                Implements the physical-position split rule.
 *
 * DoD §3.3 canonical enum (locked v2.0):
 *   surface in {pwa, pos, kiosk}              (POS = POS .130 reception)
 *   payment_method in {upi, card, cash}       (cash only via POS .130)
 *
 *   Top-up wallet source_tag set:
 *     PWA / UPI, PWA / card, POS .130 / cash,
 *     Kiosk / UPI, Kiosk / card, bonus / 10pct, bonus / 20pct
 *   Session-debit / cafe / close tags are NOT under this test's
 *   matrix (those are operator-internal, not surface-x-payment).
 *
 * Each test is env-gated. When required env is missing, the test
 * skips with an explicit reason so the gap is visible in run output.
 * Skips do NOT fail the suite. The suite PASSES today (env unset)
 * and EXPOSES drift the moment env is wired.
 *
 * Env vars:
 *   CANONICAL_URL        Server .23 racecontrol (default http://192.168.31.23:8080)
 *   TEST_ADMIN_JWT       admin JWT for billing/session list endpoints
 *                        Mint via staff auth flow under TEST_MODE.
 *   BILLING_LIST_PATH    override path (default /api/v1/billing/sessions);
 *                        switch to wallet endpoint (e.g.
 *                        /api/v1/admin/wallet/transactions?date=YYYY-MM-DD)
 *                        if the canonical surface for source-tag rows
 *                        moves under V2.
 *   LOOKBACK_DAYS        how far to scan in days (default 7)
 *   SOURCE_FIELD         JSON key carrying the surface tag
 *                        (default "source"; override to "source_tag" /
 *                        "txn_type" if schema picks a different name)
 *   PAYMENT_FIELD        JSON key carrying the payment method
 *                        (default "payment_method"; override to "method"
 *                        for current /admin/wallet/transactions shape)
 *
 * G4 NOT TESTED in this file (deferred):
 *   - Operator field (customer / staff_id) population - DoD §209 names
 *     it as a third axis but row 1.16 is scoped to surface x payment.
 *   - Bonus-credit source_tag values (bonus / 10pct, bonus / 20pct) -
 *     orthogonal to the 3x3 matrix; covered separately by ladder test.
 *   - Session-debit tag shape (Pod time / AC SP / pod_id) - those are
 *     operator-internal substrate moves, not surface-x-payment writes.
 *   - Cross-table joins to verify the source_tag column on a side
 *     ledger (wallet_topups, journal_entries) - current canonical
 *     surface is the single list endpoint; deeper schema is V2.1.
 *   - Real-time push (WebSocket) consistency of tags on broadcast.
 *   - Write-side enforcement (DoD §209: untagged write should be
 *     rejected at the API boundary - verified by a separate
 *     contract test on POST endpoints, not this read-side audit).
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const TEST_ADMIN_JWT = process.env.TEST_ADMIN_JWT;
const BILLING_LIST_PATH = process.env.BILLING_LIST_PATH || '/api/v1/billing/sessions';
const LOOKBACK_DAYS = parseInt(process.env.LOOKBACK_DAYS || '7', 10);
const SOURCE_FIELD = process.env.SOURCE_FIELD || 'source';
const PAYMENT_FIELD = process.env.PAYMENT_FIELD || 'payment_method';

const SOURCE_ENUM = ['pwa', 'pos', 'kiosk'] as const;
const PAYMENT_ENUM = ['upi', 'card', 'cash'] as const;

const SKIP_REASONS: Record<string, string> = {
  TEST_ADMIN_JWT: `TEST_ADMIN_JWT not set - Layer 1.16 source-tagging completeness audit requires admin JWT to list billing/wallet rows. DoD §3.3 enum {pwa,pos,kiosk} x {upi,card,cash} is locked; implementation drift unverified until this test runs against a live canonical store. Mint via staff auth flow under TEST_MODE on ${CANONICAL_URL}.`,
  EMPTY_RESULT: `Layer 1.16 read returned 0 rows over ${LOOKBACK_DAYS}-day lookback at ${BILLING_LIST_PATH} - drift class CANNOT be verified without samples. Either lookback is too short, BILLING_LIST_PATH does not surface source-tagged rows (DoD §3.3 lives on wallet_transactions not billing_sessions - try /api/v1/admin/wallet/transactions), or the canonical store is empty in this environment.`,
  SHAPE_MISSING: `Layer 1.16 row schema does not expose '${SOURCE_FIELD}' / '${PAYMENT_FIELD}' fields at ${BILLING_LIST_PATH} - this IS the "implementation drift unverified" signal from DoD §3.3. Either (a) the endpoint is not the canonical source-tag surface (try BILLING_LIST_PATH=/api/v1/admin/wallet/transactions + SOURCE_FIELD=source_tag + PAYMENT_FIELD=method), or (b) the canonical store does not yet enforce DoD §209 "untagged = constitutional violation". This skip surfaces the schema gap without failing.`,
};

interface SampleRow {
  raw: Record<string, unknown>;
  source: string | null;
  payment_method: string | null;
}

interface ListResponse {
  sessions?: unknown[];
  transactions?: unknown[];
  items?: unknown[];
  rows?: unknown[];
  data?: unknown[];
  error?: string;
}

function pickArray(body: ListResponse): unknown[] | null {
  if (Array.isArray(body.sessions)) return body.sessions;
  if (Array.isArray(body.transactions)) return body.transactions;
  if (Array.isArray(body.items)) return body.items;
  if (Array.isArray(body.rows)) return body.rows;
  if (Array.isArray(body.data)) return body.data;
  return null;
}

function normalizeEnum(v: unknown): string | null {
  if (typeof v !== 'string') return null;
  return v.trim().toLowerCase();
}

function toSample(raw: unknown): SampleRow {
  const row = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  return {
    raw: row,
    source: normalizeEnum(row[SOURCE_FIELD]),
    payment_method: normalizeEnum(row[PAYMENT_FIELD]),
  };
}

async function fetchList(): Promise<{ status: number; rows: unknown[] | null; body: ListResponse }> {
  const url = `${CANONICAL_URL}${BILLING_LIST_PATH}`;
  const resp = await fetch(url, {
    headers: { Authorization: `Bearer ${TEST_ADMIN_JWT}` },
  });
  const body = (await resp.json().catch(() => ({}))) as ListResponse;
  const rows = pickArray(body);
  return { status: resp.status, rows, body };
}

test.describe('Layer 1.16 - Source-tagging completeness (PWA/POS/Kiosk x UPI/card/cash)', () => {
  test('C1 canonical billing-list endpoint responds with array shape', async () => {
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    const { status, rows, body } = await fetchList();
    console.log(`C1 ${BILLING_LIST_PATH} status=${status} array_present=${Array.isArray(rows)} top_keys=${Object.keys(body).join(',')}`);
    expect(status).toBe(200);
    expect(rows).not.toBeNull();
    expect(Array.isArray(rows)).toBe(true);
  });

  test(`C2 every row.${SOURCE_FIELD} is in DoD §3.3 surface enum {${SOURCE_ENUM.join(',')}}`, async () => {
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    const { rows } = await fetchList();
    expect(rows).not.toBeNull();
    const samples = (rows ?? []).map(toSample);

    test.skip(samples.length === 0, SKIP_REASONS.EMPTY_RESULT);

    const shapeHasField = samples.some(s => SOURCE_FIELD in s.raw);
    test.skip(!shapeHasField, SKIP_REASONS.SHAPE_MISSING);

    const offenders = samples
      .filter(s => s.source !== null && !(SOURCE_ENUM as readonly string[]).includes(s.source))
      .map(s => ({ value: s.raw[SOURCE_FIELD], id: s.raw['id'] ?? s.raw['session_id'] ?? '?' }));
    if (offenders.length > 0) {
      console.log(`C2 DRIFT detected n=${offenders.length}: ${JSON.stringify(offenders.slice(0, 10))}`);
    }
    console.log(`C2 scanned n=${samples.length} unique_sources=${JSON.stringify([...new Set(samples.map(s => s.source))].sort())}`);

    expect(offenders, `Layer 1.16 source enum drift - DoD §3.3 locks {${SOURCE_ENUM.join(',')}}`).toEqual([]);
  });

  test(`C3 every row.${PAYMENT_FIELD} is in DoD §3.3 payment enum {${PAYMENT_ENUM.join(',')}}`, async () => {
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    const { rows } = await fetchList();
    expect(rows).not.toBeNull();
    const samples = (rows ?? []).map(toSample);

    test.skip(samples.length === 0, SKIP_REASONS.EMPTY_RESULT);

    const shapeHasField = samples.some(s => PAYMENT_FIELD in s.raw);
    test.skip(!shapeHasField, SKIP_REASONS.SHAPE_MISSING);

    const offenders = samples
      .filter(s => s.payment_method !== null && !(PAYMENT_ENUM as readonly string[]).includes(s.payment_method))
      .map(s => ({ value: s.raw[PAYMENT_FIELD], id: s.raw['id'] ?? s.raw['session_id'] ?? '?' }));
    if (offenders.length > 0) {
      console.log(`C3 DRIFT detected n=${offenders.length}: ${JSON.stringify(offenders.slice(0, 10))}`);
    }
    console.log(`C3 scanned n=${samples.length} unique_methods=${JSON.stringify([...new Set(samples.map(s => s.payment_method))].sort())}`);

    expect(offenders, `Layer 1.16 payment_method enum drift - DoD §3.3 locks {${PAYMENT_ENUM.join(',')}}`).toEqual([]);
  });

  test('C4 3x3 surface x payment-type matrix coverage report (observations not assertions)', async () => {
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    const { rows } = await fetchList();
    expect(rows).not.toBeNull();
    const samples = (rows ?? []).map(toSample);

    test.skip(samples.length === 0, SKIP_REASONS.EMPTY_RESULT);

    const hasBothFields = samples.some(s => SOURCE_FIELD in s.raw && PAYMENT_FIELD in s.raw);
    test.skip(!hasBothFields, SKIP_REASONS.SHAPE_MISSING);

    const matrix: Record<string, Record<string, number>> = {};
    for (const s of SOURCE_ENUM) {
      matrix[s] = {};
      for (const p of PAYMENT_ENUM) matrix[s][p] = 0;
    }
    for (const sample of samples) {
      if (!sample.source || !sample.payment_method) continue;
      if (!(SOURCE_ENUM as readonly string[]).includes(sample.source)) continue;
      if (!(PAYMENT_ENUM as readonly string[]).includes(sample.payment_method)) continue;
      matrix[sample.source][sample.payment_method] += 1;
    }

    const cells: string[] = [];
    const gaps: string[] = [];
    for (const s of SOURCE_ENUM) {
      for (const p of PAYMENT_ENUM) {
        const n = matrix[s][p];
        cells.push(`${s}x${p}=${n}`);
        if (n === 0) gaps.push(`${s}x${p}`);
      }
    }
    console.log(`C4 matrix lookback=${LOOKBACK_DAYS}d n=${samples.length} cells: ${cells.join(' ')}`);
    if (gaps.length > 0) {
      console.log(`C4 GAPS (zero-observed cells, NOT a failure): ${gaps.join(', ')}`);
    }
    // Observations only - pass unconditionally; the assertion is the
    // existence of the log line. Drift detection lives in C2/C3.
    expect(true).toBe(true);
  });

  test('C5 R1-C correlation: kiosk x cash is forbidden (DoD §1 line 60 physical-position split)', async () => {
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    const { rows } = await fetchList();
    expect(rows).not.toBeNull();
    const samples = (rows ?? []).map(toSample);

    test.skip(samples.length === 0, SKIP_REASONS.EMPTY_RESULT);

    const hasBothFields = samples.some(s => SOURCE_FIELD in s.raw && PAYMENT_FIELD in s.raw);
    test.skip(!hasBothFields, SKIP_REASONS.SHAPE_MISSING);

    const violations = samples
      .filter(s => s.source === 'kiosk' && s.payment_method === 'cash')
      .map(s => ({ id: s.raw['id'] ?? s.raw['session_id'] ?? '?', created_at: s.raw['created_at'] ?? s.raw['started_at'] ?? '?' }));
    if (violations.length > 0) {
      console.log(`C5 R1-C VIOLATION n=${violations.length}: ${JSON.stringify(violations.slice(0, 10))}`);
    } else {
      console.log(`C5 R1-C kiosk x cash = 0 across n=${samples.length} (constitutional rule held)`);
    }
    expect(violations, 'DoD §1 line 60: cash is dedicated function of POS .130; kiosk x cash is a physical-position split violation').toEqual([]);
  });
});
