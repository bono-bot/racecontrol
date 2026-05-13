/**
 * Layer 1.13 acceptance test: Auto-bill in 1s (billing-finalize latency SLA)
 *
 * V2-PROGRESS-MAP §1 row 1.13 ("Customer-day | Auto-bill in 1s | NOT-STARTED |
 * james | Joint #2 (Billing) full spec — wallet-engine sub-PACT pending").
 *
 * Source-of-truth (DoD canonical day; quoted verbatim 2026-05-13 IST):
 *   - comms-link/v2-skeleton/05-definition-of-done.md L102:
 *     "14:55 IST — Session ends (timer expired; or customer pressed "end")."
 *   - comms-link/v2-skeleton/05-definition-of-done.md L104:
 *     "14:55:01 IST — Bill auto-generates. Wallet auto-tallies session minutes ×
 *      rate + cafe-order amount. No manual staff bill creation. Customer pays
 *      via wallet balance. Bill record on substrate, payment-state-having."
 *   - comms-link/v2-skeleton/05-definition-of-done.md L205:
 *     "10. No surface where the customer waits without seeing what's happening.
 *      Customer-facing surfaces (PWA, kiosk visible to customer, leaderboard
 *      display) acknowledge action within 1 second (Captain-locked) and show
 *      progress for any operation that exceeds it."
 *   - comms-link/v2-skeleton/05-definition-of-done.md L790:
 *     "Customer-facing acknowledgement time: 1 second (R4-A)"
 *   - racecontrol/CLAUDE.md "Billing and Rates": 30min/₹700, 60min/₹900,
 *     5min free trial, 10s idle threshold.
 *
 * Contract under test:
 *   The billing-finalize round-trip (race-end signal → wallet-debit + receipt-
 *   emit) MUST complete within 1 second (DoD L102 / L104 — 14:55 → 14:55:01).
 *   The wallet-debited state MUST be observable on /customer/profile within
 *   the same 1s envelope (composes with Layer 1.17 cross-surface contract).
 *   The finalize MUST be idempotent: a second call on the same session MUST
 *   NOT double-debit (F-05 RCA anti-pattern; see anchor below).
 *   Failed finalize MUST NOT leave a partial debit (transactional invariant).
 *
 * Endpoint discovery (grep 2026-05-13 IST):
 *   POST /api/v1/billing/{id}/stop  (CANONICAL today; staff_routes.rs L524
 *                                    → stop_billing → end_billing_session_public
 *                                    in billing.rs; idempotent_replay returned
 *                                    via session status check at billing_session.rs:248).
 *   POST /api/v1/billing/{id}/agent-shutdown  (pod-side trigger; routes.rs L237
 *                                              → agent_shutdown_handler; idempotent
 *                                              per billing_recovery.rs:18 "if the
 *                                              session was already ended, returns
 *                                              Ok with refund_paise=0").
 *   /api/v1/billing/finalize  DOES NOT EXIST. The "finalize" verb is shipped
 *                             today as {id}/stop (staff) + {id}/agent-shutdown
 *                             (pod) — Joint #2 sub-PACT pending may rename/
 *                             consolidate.
 *
 * Idempotency mechanism (grep 2026-05-13 IST):
 *   Body-level `idempotency_key` SUPPORTED on start_billing (billing_start_validate
 *   .rs:28-156: SELECT id, wallet_debit_paise FROM billing_sessions WHERE
 *   idempotency_key = ? → idempotent_replay flag) and on refund/issue_refund
 *   (billing_invoice.rs:244-271: same key→replay pattern).
 *   stop_billing handler ACCEPTS but does not honor idempotency_key
 *   (billing_session.rs:252 "informational, currently — future use") —
 *   idempotency on stop relies on session.status guard (billing_session.rs:248
 *   "{ ok: true, idempotent_replay: true }" when already ended).
 *   F-05 ANTI-PATTERN ANCHOR: end_billing_session() UPDATEs and SELECTs
 *   wallet_debit_paise in the same scope (root-cause-analysis-F05-2026-03-28.md:
 *   "UPDATE writes Rs.375 → SELECT reads Rs.375" instead of pre-UPDATE value).
 *   This test does not yet exercise the partial-refund-on-early-end calc,
 *   but its idempotency check IS the canonical first guard for that class:
 *   a double-finalize that re-runs the UPDATE→SELECT will double-debit if
 *   the column-snapshot fix is not yet in place.
 *
 * Env vars:
 *   CANONICAL_URL          - Server .23 racecontrol (default http://192.168.31.23:8080)
 *   CANONICAL_REACHABLE    - 'true' to opt into network-reaching tests; default
 *                            unset → skip (bono VPS at Hostinger cannot reach
 *                            192.168.31.0/24). Set 'true' from venue LAN/VPN.
 *   TEST_CUSTOMER_JWT      - Bearer JWT for a test driver (read-side checks);
 *                            mint via POST {CANONICAL_URL}/api/v1/customer/test-mint-jwt
 *                            body {"driver_id":"<test-driver-id>"} (TEST_MODE only)
 *   TEST_ADMIN_JWT         - staff JWT to trigger /billing/{id}/stop (server-side
 *                            finalize). Staff routes require Authorization header.
 *   TEST_SESSION_ID        - active billing session id for finalize subject
 *                            (the test does NOT create one — start-session is
 *                            out of scope; this contract isolates END latency)
 *   FINALIZE_PATH          - override path (default /api/v1/billing/{TEST_SESSION_ID}/stop
 *                            — set if Joint #2 sub-PACT renames to /finalize)
 *   SLA_TOLERANCE_MS       - default 1000 (the "1s" contract; ±50ms test
 *                            tolerance accepted by test assertion margin)
 *
 * G4 NOT TESTED in this file (deferred — Joint #2 sub-PACT scope):
 *   - Wallet-engine sub-PACT (PACT-001 PACT-DRAFT pending; canonical wallet-debit
 *     state machine HOLD/RELEASE/CAPTURE/OVERFLOW not yet ratified — referenced
 *     in §S-N msg=36347 Phase 2-C AMPLIFIER-ASK)
 *   - F-05 partial-refund-on-early-end calc invariant (wallet_debit_paise
 *     UPDATE→SELECT column-snapshot fix). This file tests idempotency only.
 *   - Race-end signal → finalize trigger linkage (pod telemetry → server
 *     orchestration; Joint #2 spec defines who signals when)
 *   - Cafe-order amount inclusion in auto-bill total (DoD L104 "session
 *     minutes × rate + cafe-order amount" — cafe pricing live in Phase 2-E
 *     combo offer primitive AMPLIFIER-ASK msg=36349)
 *   - Concurrent-finalize race (two staff stop the same session simultaneously
 *     — current guard is session.status check, NOT a row-lock or SELECT FOR
 *     UPDATE; concurrent-finalize is an F-05 sibling class)
 *   - Receipt-emit substrate destination + retention (Joint #2 sub-PACT)
 *   - Cross-surface receipt visibility (POS / Kiosk / PWA receipt views —
 *     Phase 2-G customer transition overlay AMPLIFIER-ASK msg=36349)
 *   - Pod-side agent-shutdown alternate finalize path (this file tests the
 *     staff-initiated /billing/{id}/stop only; pod-initiated is sibling)
 *
 * V2 doctrine alignment: Surfaces the V1→V2 gap on auto-bill canonical
 * (V1 has /billing/{id}/stop but no /billing/finalize verb; no 1s SLA assertion
 * in V1 contract tests; no sub-1s end-to-end pipeline verification). Test
 * passes today via skip-coherence; activates as Joint #2 sub-PACT lands.
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const TEST_CUSTOMER_JWT = process.env.TEST_CUSTOMER_JWT;
const TEST_ADMIN_JWT = process.env.TEST_ADMIN_JWT;
const TEST_SESSION_ID = process.env.TEST_SESSION_ID;
const FINALIZE_PATH =
  process.env.FINALIZE_PATH ||
  (TEST_SESSION_ID ? `/api/v1/billing/${TEST_SESSION_ID}/stop` : '/api/v1/billing/{TEST_SESSION_ID}/stop');
const PROFILE_PATH = '/api/v1/customer/profile';
const SLA_TOLERANCE_MS = parseInt(process.env.SLA_TOLERANCE_MS || '1000', 10);
// Allow ±50ms test-harness tolerance on top of the 1s contract for network jitter +
// timer resolution — the contract itself remains 1000ms; the assertion uses tolerance.
const ASSERTION_MARGIN_MS = 50;

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not set to 'true' - Layer 1.13 requires network reachability to ${CANONICAL_URL}. Set CANONICAL_REACHABLE=true when running from venue LAN or VPN to Server .23. From bono VPS (Hostinger), 192.168.31.0/24 is unreachable by default.`,
  TEST_ADMIN_JWT: `TEST_ADMIN_JWT not set - Layer 1.13 finalize endpoint (POST /billing/{id}/stop) lives in staff_routes and REQUIRES staff JWT. Mint via POST ${CANONICAL_URL}/api/v1/auth/staff-login (or test-mint-staff-jwt in TEST_MODE). Joint #2 sub-PACT may relax this to allow agent-shutdown / system finalize paths but staff path is canonical today.`,
  TEST_CUSTOMER_JWT: `TEST_CUSTOMER_JWT not set - profile read-back (Test 4) cannot observe wallet_balance_paise without driver JWT. Mint via POST ${CANONICAL_URL}/api/v1/customer/test-mint-jwt with body {"driver_id":"<test-id>"} in TEST_MODE.`,
  TEST_SESSION_ID: `TEST_SESSION_ID not set - Layer 1.13 SLA test is END-of-session latency only; the test does NOT create a session. Pre-create an active billing session (POST /billing/start with test driver) and pass the returned id as TEST_SESSION_ID. Joint #2 sub-PACT may add a TEST_MODE seed-session helper.`,
  JOINT_2_SPEC_PENDING: `Joint #2 (Billing) full spec — wallet-engine sub-PACT pending (PACT-001 PACT-DRAFT in comms-link/.planning/draft-pacts/). Until wallet HOLD/RELEASE/CAPTURE/OVERFLOW state machine is ratified (msg=36347 Phase 2-C AMPLIFIER-ASK), the failed-finalize cleanup invariant cannot be exercised against canonical contract.`,
  F05_REFUND_CALC: `F-05 RCA partial-refund-on-early-end UPDATE→SELECT column-snapshot fix is out of scope for this file. Idempotency is tested (the first guard); the refund-calc class is deferred to a dedicated Layer 1.13 sub-test once Joint #2 spec lands.`,
};

interface FinalizeResponse {
  ok?: boolean;
  idempotent_replay?: boolean;
  session_id?: string;
  driver_id?: string;
  debit_paise?: number;
  wallet_debit_paise?: number;
  refund_paise?: number;
  finalized_at?: string;
  ended_at?: string;
  status?: string;
  error?: string;
}

interface ProfileDriver {
  id: string;
  wallet_balance_paise: number;
}

interface ProfileResponse {
  driver?: ProfileDriver;
  error?: string;
}

interface ApiRead<T> {
  url: string;
  status: number;
  body: T;
  read_started_at: number;
  read_finished_at: number;
}

async function postFinalize(
  jwt: string,
  body: Record<string, unknown> = {},
): Promise<ApiRead<FinalizeResponse>> {
  const url = `${CANONICAL_URL}${FINALIZE_PATH}`;
  const started = Date.now();
  const resp = await fetch(url, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${jwt}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  const finished = Date.now();
  let parsed: FinalizeResponse;
  try {
    parsed = (await resp.json()) as FinalizeResponse;
  } catch {
    parsed = { error: 'non-json-response' };
  }
  return {
    url,
    status: resp.status,
    body: parsed,
    read_started_at: started,
    read_finished_at: finished,
  };
}

async function getProfile(jwt: string): Promise<ApiRead<ProfileResponse>> {
  const url = `${CANONICAL_URL}${PROFILE_PATH}`;
  const started = Date.now();
  const resp = await fetch(url, {
    headers: { Authorization: `Bearer ${jwt}` },
  });
  const finished = Date.now();
  let parsed: ProfileResponse;
  try {
    parsed = (await resp.json()) as ProfileResponse;
  } catch {
    parsed = { error: 'non-json-response' };
  }
  return {
    url,
    status: resp.status,
    body: parsed,
    read_started_at: started,
    read_finished_at: finished,
  };
}

function extractDebitPaise(body: FinalizeResponse): number | undefined {
  if (typeof body.debit_paise === 'number') return body.debit_paise;
  if (typeof body.wallet_debit_paise === 'number') return body.wallet_debit_paise;
  return undefined;
}

function extractFinalizedAt(body: FinalizeResponse): string | undefined {
  return body.finalized_at ?? body.ended_at;
}

test.describe('Layer 1.13 - Auto-bill in 1s (billing-finalize latency SLA)', () => {
  test('finalize endpoint exists + responds with expected shape', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    const r = await postFinalize(TEST_ADMIN_JWT!);
    console.log(
      `finalize shape-check t=${r.read_finished_at - r.read_started_at}ms ` +
      `status=${r.status} url=${r.url}`,
    );
    expect(r.status).toBe(200);

    // Canonical (today): { ok: true, idempotent_replay?: bool } on stop_billing
    // success or replay (billing_session.rs:248 + :424). The Joint #2 sub-PACT
    // may add explicit finalized_at + debit_paise to this surface; this test
    // asserts the shape that EITHER today's contract OR the Joint #2 spec
    // satisfies (assertion uses presence checks, not exact equality).
    expect(r.body).toBeDefined();
    const sessionIdReturned = r.body.session_id;
    const driverIdReturned = r.body.driver_id;
    const debitReturned = extractDebitPaise(r.body);
    const finalizedAtReturned = extractFinalizedAt(r.body);

    // Today's stop_billing returns { ok: true } only; Joint #2 spec adds the
    // richer shape. We log what we see + assert ok-flag is at minimum present.
    console.log(
      `finalize shape: ok=${r.body.ok} idempotent_replay=${r.body.idempotent_replay} ` +
      `session_id=${sessionIdReturned ?? '<absent>'} ` +
      `driver_id=${driverIdReturned ?? '<absent>'} ` +
      `debit_paise=${debitReturned ?? '<absent>'} ` +
      `finalized_at=${finalizedAtReturned ?? '<absent>'}`,
    );
    expect(typeof r.body.ok === 'boolean' || typeof r.body.status === 'string').toBe(true);
  });

  test('latency SLA - finalize round-trip completes within 1s (1000ms ±50ms)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    const r = await postFinalize(TEST_ADMIN_JWT!);
    const elapsed = r.read_finished_at - r.read_started_at;
    console.log(
      `finalize latency-SLA t=${elapsed}ms threshold=${SLA_TOLERANCE_MS}ms ` +
      `margin=${ASSERTION_MARGIN_MS}ms status=${r.status}`,
    );
    expect(r.status).toBe(200);
    // DoD L102 → L104: 14:55 → 14:55:01 = 1 second contract (Captain-locked).
    // Assertion uses contract + small harness margin for network jitter.
    expect(elapsed).toBeLessThanOrEqual(SLA_TOLERANCE_MS + ASSERTION_MARGIN_MS);
  });

  test('idempotency - second finalize MUST NOT double-debit (F-05 anti-pattern guard)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    // Call 1: finalize (or replay-finalize if already finalized; the test
    // does not require a freshly-active session — the invariant under test
    // is "two calls produce one debit", which holds even if call 1 is itself
    // a replay).
    const r1 = await postFinalize(TEST_ADMIN_JWT!);
    console.log(
      `idempotency call-1 t=${r1.read_finished_at - r1.read_started_at}ms ` +
      `status=${r1.status} ok=${r1.body.ok} replay=${r1.body.idempotent_replay}`,
    );
    expect(r1.status).toBe(200);
    const debit1 = extractDebitPaise(r1.body);

    // Call 2: identical finalize on the same session_id. The contract is
    // "second call MUST NOT debit again". Today's stop_billing returns
    // { ok: true, idempotent_replay: true } via session.status guard
    // (billing_session.rs:248). Joint #2 sub-PACT may strengthen this to
    // an explicit idempotency_key check on stop (currently informational
    // per billing_session.rs:252).
    const r2 = await postFinalize(TEST_ADMIN_JWT!);
    console.log(
      `idempotency call-2 t=${r2.read_finished_at - r2.read_started_at}ms ` +
      `status=${r2.status} ok=${r2.body.ok} replay=${r2.body.idempotent_replay}`,
    );
    expect(r2.status).toBe(200);
    const debit2 = extractDebitPaise(r2.body);

    // The canonical invariant: total debit after 2 calls == total debit after 1 call.
    // If the API exposes debit_paise on the response, we can assert directly.
    // Otherwise we rely on the explicit idempotent_replay flag OR a profile read-back
    // (covered in the wallet-state test below).
    if (typeof debit1 === 'number' && typeof debit2 === 'number') {
      expect(debit2).toBe(debit1);
    } else if (typeof r2.body.idempotent_replay === 'boolean') {
      expect(r2.body.idempotent_replay).toBe(true);
    } else {
      // Joint #2 spec pending — neither field present means the API doesn't
      // yet expose enough information to make a strong idempotency assertion
      // at this layer. The test surfaces the GAP rather than passing falsely.
      console.log(
        `idempotency gap: neither debit_paise nor idempotent_replay exposed on response. ` +
        `Joint #2 sub-PACT must define this surface.`,
      );
      test.skip(true, SKIP_REASONS.JOINT_2_SPEC_PENDING);
      return;
    }
  });

  test('wallet-state read after finalize reflects debit within 1s (composes 1.17)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    // 1. Read profile before finalize (baseline wallet balance)
    const before = await getProfile(TEST_CUSTOMER_JWT!);
    expect(before.status).toBe(200);
    const balanceBefore = before.body.driver?.wallet_balance_paise;
    console.log(
      `wallet-state before-finalize balance=${balanceBefore} ` +
      `read_t=${before.read_finished_at - before.read_started_at}ms`,
    );

    // 2. Finalize
    const finalize = await postFinalize(TEST_ADMIN_JWT!);
    expect(finalize.status).toBe(200);

    // 3. Read profile immediately after finalize — within 1s envelope
    const after = await getProfile(TEST_CUSTOMER_JWT!);
    expect(after.status).toBe(200);
    const balanceAfter = after.body.driver?.wallet_balance_paise;

    // Latency budget: finalize-start → wallet-state-visible MUST be ≤1s
    const propagation_ms = after.read_finished_at - finalize.read_started_at;
    console.log(
      `wallet-state after-finalize balance=${balanceAfter} ` +
      `propagation=${propagation_ms}ms threshold=${SLA_TOLERANCE_MS}ms`,
    );
    expect(propagation_ms).toBeLessThanOrEqual(SLA_TOLERANCE_MS + ASSERTION_MARGIN_MS);

    // If finalize.body exposes debit_paise, verify wallet decreased by that amount.
    // Replay-class call-1 case: debit_paise may be 0 (already finalized) → balance unchanged.
    const debit = extractDebitPaise(finalize.body);
    if (typeof balanceBefore === 'number' && typeof balanceAfter === 'number') {
      const delta = balanceBefore - balanceAfter;
      console.log(`wallet delta=${delta} debit_paise=${debit ?? '<absent>'}`);
      if (typeof debit === 'number') {
        // Wallet decrement equals reported debit. Strict equality — under-debit
        // hides leakage; over-debit means double-charge. Composes with F-05 RCA.
        expect(delta).toBe(debit);
      } else {
        // No debit_paise on response — assert at minimum non-increase (idempotent
        // or fresh-finalize both yield delta ≥ 0).
        expect(delta).toBeGreaterThanOrEqual(0);
      }
    } else {
      console.log(
        `wallet_balance_paise not present on profile.driver — Joint #2 sub-PACT ` +
        `must guarantee this field for cross-surface wallet-state contract.`,
      );
      test.skip(true, SKIP_REASONS.JOINT_2_SPEC_PENDING);
      return;
    }
  });

  test('failed-finalize cleanup - no partial debit persists on transaction failure', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);

    // Failure-path test: requires either (a) a TEST_MODE injection point
    // that forces end_billing_session() to fail mid-transaction, or
    // (b) a known-failing session id (e.g. a session whose pricing_tier
    // was deleted, or whose wallet has insufficient balance to cover the
    // computed debit). Neither is canonical today — Joint #2 sub-PACT
    // must define the failure-injection surface.
    //
    // Until then, this test is structurally GAP-revealing. The assertion
    // would be: balance_before == balance_after when finalize returns
    // non-200 (no partial debit). F-05 RCA anchors why this matters:
    // a half-applied UPDATE that commits the write but rolls back the
    // refund-calc leaves the customer charged the gross instead of net.
    console.log(
      `failed-finalize cleanup: failure-injection surface not yet defined. ` +
      `Joint #2 sub-PACT must add TEST_MODE handler or document a known-failing ` +
      `session class to exercise transactional invariant.`,
    );
    test.skip(true, SKIP_REASONS.JOINT_2_SPEC_PENDING);
  });
});
