/**
 * Layer 2 Wave 2 Phase 2-E acceptance test: Combo Offer Primitive
 *
 * V2-PROGRESS-MAP §2 / Wave 2 — Phase 2-E "Combo Offer Primitive"
 *   ("All-beat | Cross-surface package primitive (sim+cafe / buy-X-get-Y /
 *    fixed-price bundles) | NOT-STARTED | bono | PACT-DRAFT present;
 *    engine wire-up gated by Wave 2 ship time + Phase 2-A rate-table +
 *    Phase 2-F campaign object FILE events").
 *
 * Anchors:
 *   - PACT-DRAFT pointer:
 *       /root/comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-e-combo-offer-primitive.md
 *       (608 lines, status DRAFT-AWAITING-WAVE-2-SHIP-TIME)
 *   - PACT-DRAFT AMENDMENT-PROPOSAL pointer:
 *       /root/comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-e-combo-offer-primitive.AMENDMENT-PROPOSAL-20260512.md
 *       (Q-2E-1..8 disposition table; 4 LOAD-BEARING marked)
 *   - V-LBAC §S-218 multi-agent orchestration iter3 close-anchor
 *     (composes-with §S-215 iter1 + §S-216 iter2)
 *   - V2-MASTER-STATE §S-84 GAP-2 (Captain auth 5 V2.0 gap closures 2026-05-07)
 *   - V2-MASTER-STATE §S-214.6 — 4 LOAD-BEARING invariants for Phase 2-E:
 *       composability / pricing precedence / audit-log / V1 ladder migration
 *
 * V1 → V2 NEW-MECHANISM-CLASS gap (grep 2026-05-13 IST):
 *   V1 substrate has `cafe_promos` table at
 *     racecontrol/crates/racecontrol/src/cafe_promos.rs (346 lines) —
 *     `promo_type IN ('combo', 'happy_hour', 'gaming_bundle')` — but this
 *     primitive is CAFE-ONLY (single-surface; column shape inadequate for
 *     cross-surface sim+ps5+cafe+voucher bundles).
 *
 *   V1 does NOT have:
 *     - `combo_offers` table (cross-surface bundle primary key)
 *     - `combo_line_items` table (per-surface entitlement breakdown)
 *     - `combo_redemptions` table (conversion-tracking surface)
 *     - GET /v2/combos/eligible endpoint (eligibility check at booking)
 *     - `combo_override` field on rate-resolution return shape
 *     - 6-action_type combo lifecycle audit-log enum
 *       (combo_created / combo_applied / combo_voucher_issued /
 *        combo_voucher_redeemed / combo_voucher_expired / combo_cancelled)
 *
 *   Phase 2-E is therefore a NEW-MECHANISM-CLASS for V2 (parallel to the
 *   row-1.20 iRacing-discount gap and row-1.13 finalize-endpoint gap surfaced
 *   in iter2). The cafe_promos surface is V1-era cafe-scoped and SHOULD remain
 *   as the cafe-only path; Phase 2-E ships a SEPARATE cross-surface engine
 *   layered atop Phase 2-A `rate_table.resolve_rate()` per Q-2E-2a.
 *
 *   §S-146 V1↔V2 RCA disposition (when this PACT-DRAFT FILEs):
 *     - cafe_promos → KEEP (V1-anchor for cafe-scoped messaging-layer per §S-104)
 *     - combo_offers → NEW (V2 substrate; layered atop rate_table)
 *     - Stacking gate: Q-2E-1a flat-combo, NO recursion (V2.0 ship simplicity)
 *
 * ANTI-PATTERN GUARD — Combo offer composability rule (DoD L76-80 + Q-2E-1a + Q-2E-4a):
 *   Combo offers STACK with session discounts (happy-hour, iRacing) via deeper-of
 *   per Phase 2-A rate_table.resolve_rate(), BUT are INDEPENDENT from the
 *   top-up bonus ladder per DoD L76-80 "bonus-vs-discount independence".
 *   Never mix the two surfaces:
 *     - Combo discount applies to SESSION-CHARGE surface at bill-render time
 *       (deeper-of vs other session discounts; Q-2E-2a `combo_override` field)
 *     - Top-up bonus applies to WALLET-CREDIT surface at top-up time
 *       (different ledger event; source_tag `bonus / Xpct`; never reduces
 *        wallet bonus_balance via combo path)
 *   Mirrors topup-bonus-ladder.spec.ts anti-pattern guard pattern.
 *
 * 4 LOAD-BEARING + 1 edge case (5 tests; each maps to a Q-2E disposition):
 *   Test 1 (LOAD-BEARING composability; Q-2E-1a + DoD L76-80):
 *     Combo offer stacks with HAPPY-HOUR session discount via deeper-of rule
 *     BUT is INDEPENDENT from TOP-UP BONUS ladder; wallet bonus_balance never
 *     reduced by combo path.
 *   Test 2 (LOAD-BEARING pricing precedence; Q-2E-2a `combo_override`):
 *     When MULTIPLE combo offers apply, deepest-discount-wins (NOT first-match);
 *     ties broken by combo_offer_id ordering for determinism.
 *   Test 3 (LOAD-BEARING audit-log; Q-2E-3b 6-event lifecycle + §S-158 doctrine):
 *     Every combo-offer activation writes an audit_log row carrying
 *     {combo_offer_id, session_id, customer_id, base_price, combo_discount,
 *      final_price, applied_at, deeper_of_breakdown}; action_type ∈ 6-event enum.
 *   Test 4 (LOAD-BEARING V1 ladder migration; Q-2E-4a Wallet-Framing-C):
 *     Pre-existing customer with V1 wallet bonus-tier credit retains that
 *     credit; combo offer cannot reduce wallet bonus_balance (combo applies to
 *     SESSION-CHARGE surface, NOT wallet surface).
 *   Test 5 (edge; Q-2E-1a degenerate):
 *     Degenerate single-item combo offer (combo_size=1) treated as straight
 *     discount; documented behavior; equivalent to a session-discount entry
 *     in rate_table (Q-2E-1a flat-combo collapse path).
 *
 * Env vars:
 *   CANONICAL_URL              - Server .23 racecontrol (default http://192.168.31.23:8080)
 *   CANONICAL_REACHABLE        - 'true' to opt into network-reaching tests (bono VPS at
 *                                Hostinger cannot reach 192.168.31.0/24; default unset → skip)
 *   TEST_ADMIN_JWT             - admin/staff JWT for combo-offer admin endpoints
 *                                (POST/PUT /v2/combos/* and admin audit-log reads)
 *   TEST_CUSTOMER_ID           - customer_id for combo-eligibility + redemption flows
 *   TEST_SESSION_ID            - session_id for combo-application bill-render flow
 *   COMBO_OFFER_ENGINE_REACHABLE - 'true' to opt into engine assertions. Default unset →
 *                                  skip ALL combo-engine-touching assertions because
 *                                  Phase 2-E PACT-DRAFT is NOT YET FILED; engine wire-up
 *                                  gates on Wave 2 ship time + Phase 2-A + Phase 2-F.
 *   COMBO_ELIGIBLE_PATH        - override path (default /v2/combos/eligible)
 *   COMBO_APPLY_PATH           - override path (default /v2/combos/apply)
 *   AUDIT_LOG_PATH             - override path (default /api/v1/audit-log)
 *   WALLET_BALANCE_PATH        - override path template
 *                                (default /api/v1/wallet/{customer_id}/balance)
 *   HAPPY_HOUR_ACTIVE          - 'true' if happy-hour window is active during test run
 *                                (Test 1 composability requires it to verify deeper-of)
 *
 * G4 NOT TESTED in this file (deferred):
 *   - Live engine wire-up — Phase 2-E PACT-DRAFT is unfiled; engine deferred to
 *     Wave 2 ship time per V2-MASTER-STATE §S-84 GAP-2 + §S-86 timeline
 *     (~2026-06-04 to 2026-06-18 estimate)
 *   - Live audit-log assertion against real combo activations (Test 3 asserts
 *     SHAPE of audit row when COMBO_OFFER_ENGINE_REACHABLE='true'; live row
 *     content depends on engine running + a real activation in the window)
 *   - Concurrent-stack race — two customers redeeming the same Sub-class C
 *     group combo within the active window; covered by Wave 2 group-pricing
 *     PACT when it lands (Q-2E-1a defers Sub-class C to V2.1+)
 *   - Multi-customer combo eligibility — Sub-class C 4-friends bundle profile
 *     resolution; gates on 4-profiles-per-family V2 customer workflows §3
 *   - Cafe-voucher state machine (issued → partially_redeemed → fully_redeemed
 *     → expired); covered by cafe-module sub-PACT at Wave 2
 *   - Combo cancellation cascade (combo_cancelled action_type); composes-with
 *     Phase 2-F Q-2F-3b cancellation cascade
 *   - GST-inclusive revenue-recognition split (§S-101 doctrine); Wave 4 §4.3
 *     `total_revenue_net` ingestion contract
 *   - Sub-class C group-pricing per-head allocation (V2.1+ deferred)
 *   - Phase 2-F campaign-attribution co-location (Q-2E-6a; co-located column
 *     `campaign_attributions.combo_id`)
 *   - HARD-NEVER channel allow-list enforcement (Q-2E-7a; covered by combo-
 *     display sub-PACT)
 *
 * V2 doctrine alignment: Surfaces the V1→V2 NEW-MECHANISM-CLASS gap on the
 * cross-surface combo-offer primitive (no V1 engine exists; cafe_promos is
 * cafe-scoped and NOT a substitute) AND encodes the 4 LOAD-BEARING invariants
 * per §S-214.6. Test passes today via skip-coherence; activates when the
 * Phase 2-E PACT-DRAFT FILEs + engine wires up at Wave 2 ship time.
 *
 * Multi-agent orchestration iter3 (V-LBAC §S-218):
 *   Authored by bono sub-agent under §S-218 close-anchor; iter3 composes-with
 *   §S-215 iter1 (3 acceptance tests 1.14/1.16/1.19) + §S-216 iter2 (3 more
 *   1.11/1.13/1.20). This file lands the Phase 2-E primitive acceptance test
 *   ahead of FILE event — substrate-prep so Wave 2 author starts from a clean
 *   anti-pattern-guarded contract surface.
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const TEST_ADMIN_JWT = process.env.TEST_ADMIN_JWT;
const TEST_CUSTOMER_ID = process.env.TEST_CUSTOMER_ID;
const TEST_SESSION_ID = process.env.TEST_SESSION_ID;
const COMBO_OFFER_ENGINE_REACHABLE = process.env.COMBO_OFFER_ENGINE_REACHABLE === 'true';
const COMBO_ELIGIBLE_PATH = process.env.COMBO_ELIGIBLE_PATH || '/v2/combos/eligible';
const COMBO_APPLY_PATH = process.env.COMBO_APPLY_PATH || '/v2/combos/apply';
const AUDIT_LOG_PATH = process.env.AUDIT_LOG_PATH || '/api/v1/audit-log';
const WALLET_BALANCE_PATH_TEMPLATE =
  process.env.WALLET_BALANCE_PATH || '/api/v1/wallet/{customer_id}/balance';
const HAPPY_HOUR_ACTIVE = process.env.HAPPY_HOUR_ACTIVE === 'true';

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not set to 'true' - Phase 2-E acceptance test requires network reachability to ${CANONICAL_URL}. Set CANONICAL_REACHABLE=true when running from venue LAN or via VPN to Server .23. From bono VPS (Hostinger), 192.168.31.0/24 is unreachable by default.`,
  COMBO_ENGINE_MISSING: `COMBO_OFFER_ENGINE_REACHABLE not set to 'true' - Phase 2-E combo offer engine DOES NOT EXIST in V1 substrate (grep 2026-05-13: no /v2/combos/eligible, no /v2/combos/apply, no combo_offers / combo_line_items / combo_redemptions tables, no combo_override field on rate-resolution shape; V1 cafe_promos is cafe-scoped single-surface and NOT a substitute). PACT-DRAFT-phase-2-e-combo-offer-primitive.md is unfiled (DRAFT-AWAITING-WAVE-2-SHIP-TIME); engine wire-up gates on Wave 2 ship time + Phase 2-A rate-table FILE + Phase 2-F campaign object FILE per V2-MASTER-STATE §S-84 + §S-86. Set COMBO_OFFER_ENGINE_REACHABLE=true once V2 engine lands.`,
  TEST_ADMIN_JWT: `TEST_ADMIN_JWT not set - Phase 2-E admin endpoints (POST/PUT /v2/combos/*, GET /api/v1/audit-log) are staff-JWT gated. Mint via staff PIN login or TEST_MODE staff-mint endpoint.`,
  TEST_CUSTOMER_ID: `TEST_CUSTOMER_ID not set - combo eligibility + wallet-balance flows require an existing customer record. Mint via POST ${CANONICAL_URL}/api/v1/customer/test-mint-jwt in TEST_MODE.`,
  TEST_SESSION_ID: `TEST_SESSION_ID not set - combo-application bill-render flow requires an active session_id. Start a session via POST /api/v1/billing/sessions then export TEST_SESSION_ID=<returned_id>.`,
  HAPPY_HOUR_INACTIVE: `HAPPY_HOUR_ACTIVE not set to 'true' - Test 1 composability (combo + happy-hour deeper-of) requires happy-hour window active. Re-run during a happy-hour window OR seed test-mode happy-hour via Phase 2-A rate-table service when it ships.`,
};

interface ComboLineItem {
  surface: 'sim' | 'ps5' | 'cafe' | 'voucher';
  item_kind: string;
  item_quantity: number;
  unit_unit_paise?: number;
  cafe_class?: 'A' | 'B' | 'C' | null;
}

interface ComboOffer {
  combo_id: string;
  combo_kind: 'bundle' | 'buy_x_get_y' | 'group';
  display_name: string;
  fixed_price_paise: number | null;
  active_starts_at: string;
  active_ends_at: string;
  line_items?: ComboLineItem[];
}

interface ComboApplyResponse {
  combo_offer_id?: string;
  session_id?: string;
  base_price_paise?: number;
  combo_discount_paise?: number;
  final_price_paise?: number;
  deeper_of_breakdown?: {
    happy_hour_pct?: number;
    iracing_pct?: number;
    combo_pct?: number;
    winning_source?: string;
    winning_pct?: number;
  };
  applied_at?: string;
  audit_log_id?: string;
  error?: string;
  [k: string]: unknown;
}

interface AuditLogRow {
  id?: string | number;
  action_type?: string;
  combo_offer_id?: string;
  session_id?: string;
  customer_id?: string;
  base_price?: number;
  combo_discount?: number;
  final_price?: number;
  applied_at?: string;
  deeper_of_breakdown?: unknown;
  [k: string]: unknown;
}

interface WalletBalance {
  customer_id?: string;
  balance_paise?: number;
  bonus_balance_paise?: number;
  [k: string]: unknown;
}

interface ApiRead<T = unknown> {
  url: string;
  status: number;
  body: T;
  read_started_at: number;
  read_finished_at: number;
}

// The 6-event combo-lifecycle audit-log enum per Q-2E-3b (PACT-DRAFT
// AMENDMENT-PROPOSAL §3 Q-2E-3 recommendation).
const COMBO_AUDIT_ACTION_TYPES = new Set<string>([
  'combo_created',
  'combo_applied',
  'combo_voucher_issued',
  'combo_voucher_redeemed',
  'combo_voucher_expired',
  'combo_cancelled',
]);

async function fetchEligibleCombos(customerId: string): Promise<ApiRead<ComboOffer[]>> {
  const started = Date.now();
  const url = `${CANONICAL_URL}${COMBO_ELIGIBLE_PATH}?customer_id=${encodeURIComponent(customerId)}`;
  const headers: Record<string, string> = {};
  if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
  const resp = await fetch(url, { headers });
  const finished = Date.now();
  let body: ComboOffer[];
  try {
    const parsed = (await resp.json()) as ComboOffer[] | { offers?: ComboOffer[] };
    body = Array.isArray(parsed) ? parsed : (parsed.offers ?? []);
  } catch {
    body = [];
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

async function applyCombo(
  comboId: string,
  sessionId: string,
  customerId: string,
): Promise<ApiRead<ComboApplyResponse>> {
  const started = Date.now();
  const url = `${CANONICAL_URL}${COMBO_APPLY_PATH}`;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
  const resp = await fetch(url, {
    method: 'POST',
    headers,
    body: JSON.stringify({ combo_offer_id: comboId, session_id: sessionId, customer_id: customerId }),
  });
  const finished = Date.now();
  let body: ComboApplyResponse;
  try {
    body = (await resp.json()) as ComboApplyResponse;
  } catch {
    body = {} as ComboApplyResponse;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

async function fetchAuditLog(filters: Record<string, string>): Promise<ApiRead<AuditLogRow[]>> {
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

async function fetchWalletBalance(customerId: string): Promise<ApiRead<WalletBalance>> {
  const started = Date.now();
  const path = WALLET_BALANCE_PATH_TEMPLATE.replace('{customer_id}', encodeURIComponent(customerId));
  const url = `${CANONICAL_URL}${path}`;
  const headers: Record<string, string> = {};
  if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
  const resp = await fetch(url, { headers });
  const finished = Date.now();
  let body: WalletBalance;
  try {
    body = (await resp.json()) as WalletBalance;
  } catch {
    body = {} as WalletBalance;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

test.describe('Layer 2 W2 Phase 2-E - Combo Offer Primitive', () => {
  test('Test 1: composability - combo stacks with happy-hour via deeper-of BUT independent from top-up bonus ladder (DoD L76-80 + Q-2E-1a + Q-2E-4a)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!COMBO_OFFER_ENGINE_REACHABLE, SKIP_REASONS.COMBO_ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_CUSTOMER_ID, SKIP_REASONS.TEST_CUSTOMER_ID);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);
    test.skip(!HAPPY_HOUR_ACTIVE, SKIP_REASONS.HAPPY_HOUR_INACTIVE);

    // Snapshot wallet bonus_balance BEFORE combo apply.
    const walletBefore = await fetchWalletBalance(TEST_CUSTOMER_ID!);
    console.log(`wallet before: status=${walletBefore.status} body=${JSON.stringify(walletBefore.body)}`);
    expect(walletBefore.status).toBe(200);
    const bonusBefore = walletBefore.body.bonus_balance_paise ?? 0;

    // Discover an eligible combo for this customer.
    const eligible = await fetchEligibleCombos(TEST_CUSTOMER_ID!);
    console.log(`eligible combos: status=${eligible.status} count=${eligible.body.length}`);
    expect(eligible.status).toBe(200);
    expect(eligible.body.length).toBeGreaterThanOrEqual(1);

    const combo = eligible.body[0];
    const applyResp = await applyCombo(combo.combo_id, TEST_SESSION_ID!, TEST_CUSTOMER_ID!);
    console.log(`combo apply: status=${applyResp.status} body=${JSON.stringify(applyResp.body)}`);
    expect(applyResp.status).toBe(200);
    expect(applyResp.body.combo_offer_id).toBe(combo.combo_id);
    expect(applyResp.body.deeper_of_breakdown).toBeDefined();

    // Composability: deeper-of resolves to MAX(combo_pct, happy_hour_pct, iracing_pct).
    // Winning source must be ONE of the session-discount sources, never `bonus_ladder`.
    const breakdown = applyResp.body.deeper_of_breakdown!;
    expect(breakdown.winning_source).toBeDefined();
    expect(breakdown.winning_source).not.toBe('bonus_ladder');
    expect(['combo', 'happy_hour', 'iracing']).toContain(breakdown.winning_source);
    if (
      typeof breakdown.combo_pct === 'number' &&
      typeof breakdown.happy_hour_pct === 'number'
    ) {
      const maxPct = Math.max(breakdown.combo_pct, breakdown.happy_hour_pct, breakdown.iracing_pct ?? 0);
      expect(breakdown.winning_pct).toBe(maxPct);
    }

    // Independence: wallet bonus_balance MUST NOT decrease across combo apply.
    // Combo applies to SESSION-CHARGE surface, NEVER to wallet bonus surface.
    const walletAfter = await fetchWalletBalance(TEST_CUSTOMER_ID!);
    expect(walletAfter.status).toBe(200);
    const bonusAfter = walletAfter.body.bonus_balance_paise ?? 0;
    console.log(`wallet bonus: before=${bonusBefore} after=${bonusAfter}`);
    expect(bonusAfter).toBe(bonusBefore);
  });

  test('Test 2: pricing precedence - multiple combos resolve via deepest-discount-wins; combo_offer_id ordering tiebreak (Q-2E-2a)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!COMBO_OFFER_ENGINE_REACHABLE, SKIP_REASONS.COMBO_ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_CUSTOMER_ID, SKIP_REASONS.TEST_CUSTOMER_ID);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    const eligible = await fetchEligibleCombos(TEST_CUSTOMER_ID!);
    console.log(`eligible combos for precedence test: count=${eligible.body.length}`);
    if (eligible.body.length < 2) {
      console.log('Need >=2 eligible combos to verify precedence; current scenario insufficient.');
      test.skip(true, 'Pricing-precedence test requires >=2 simultaneously eligible combos. Seed two overlapping combos via admin /v2/combos POST then re-run.');
      return;
    }

    // The apply endpoint MUST select the deepest-discount combo when caller
    // does not pin a specific combo_id. Q-2E-2a `combo_override` field on
    // rate-resolution return shape carries the selected combo_id.
    const headers: Record<string, string> = { 'Content-Type': 'application/json' };
    if (TEST_ADMIN_JWT) headers.Authorization = `Bearer ${TEST_ADMIN_JWT}`;
    const resp = await fetch(`${CANONICAL_URL}${COMBO_APPLY_PATH}`, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        // No combo_offer_id; engine selects via deepest-discount.
        session_id: TEST_SESSION_ID,
        customer_id: TEST_CUSTOMER_ID,
        select_mode: 'deepest_discount',
      }),
    });
    const body = (await resp.json()) as ComboApplyResponse;
    console.log(`deepest-discount selection: status=${resp.status} body=${JSON.stringify(body)}`);
    expect(resp.status).toBe(200);
    expect(body.combo_offer_id).toBeDefined();

    // The selected combo MUST have the maximum combo_discount_paise of all
    // eligible combos (NOT the first-match in eligibility list).
    // Tiebreaker: lexicographic combo_offer_id ascending for determinism.
    const selectedId = body.combo_offer_id!;
    const breakdown = body.deeper_of_breakdown;
    expect(breakdown).toBeDefined();
    expect(typeof breakdown!.combo_pct).toBe('number');

    // Apply each candidate independently and confirm selectedId is max-discount.
    const candidates = eligible.body.slice(0, 5); // bounded
    const discounts: Array<{ id: string; pct: number }> = [];
    for (const c of candidates) {
      const r = await applyCombo(c.combo_id, TEST_SESSION_ID!, TEST_CUSTOMER_ID!);
      const pct = r.body.deeper_of_breakdown?.combo_pct ?? 0;
      discounts.push({ id: c.combo_id, pct });
    }
    const maxPct = Math.max(...discounts.map(d => d.pct));
    const winners = discounts.filter(d => d.pct === maxPct).sort((a, b) => a.id.localeCompare(b.id));
    console.log(`candidates: ${JSON.stringify(discounts)} expected_winner=${winners[0]?.id} actual=${selectedId}`);
    // Engine MUST pick the lexicographically-first combo at max pct (deterministic tiebreaker).
    expect(selectedId).toBe(winners[0].id);
  });

  test('Test 3: audit-log - combo activation writes audit row with full lifecycle shape (Q-2E-3b + §S-158)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!COMBO_OFFER_ENGINE_REACHABLE, SKIP_REASONS.COMBO_ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_CUSTOMER_ID, SKIP_REASONS.TEST_CUSTOMER_ID);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    const eligible = await fetchEligibleCombos(TEST_CUSTOMER_ID!);
    expect(eligible.status).toBe(200);
    expect(eligible.body.length).toBeGreaterThanOrEqual(1);
    const combo = eligible.body[0];

    const applyResp = await applyCombo(combo.combo_id, TEST_SESSION_ID!, TEST_CUSTOMER_ID!);
    expect(applyResp.status).toBe(200);
    const appliedAt = applyResp.body.applied_at;
    expect(appliedAt).toBeDefined();

    // Read back the audit-log row(s) for this combo activation.
    const audit = await fetchAuditLog({
      action_type: 'combo_applied',
      combo_offer_id: combo.combo_id,
      session_id: TEST_SESSION_ID!,
    });
    console.log(`audit rows: status=${audit.status} count=${audit.body.length}`);
    expect(audit.status).toBe(200);
    expect(audit.body.length).toBeGreaterThanOrEqual(1);

    const row = audit.body[0];
    console.log(`audit row: ${JSON.stringify(row)}`);

    // Q-2E-3b: action_type MUST be one of the 6 combo lifecycle events.
    expect(row.action_type).toBeDefined();
    expect(COMBO_AUDIT_ACTION_TYPES.has(row.action_type!)).toBe(true);

    // §S-158 doctrine: audit row carries full shape for forensic reconstruction.
    expect(row.combo_offer_id).toBe(combo.combo_id);
    expect(row.session_id).toBe(TEST_SESSION_ID);
    expect(row.customer_id).toBe(TEST_CUSTOMER_ID);
    expect(typeof row.base_price).toBe('number');
    expect(typeof row.combo_discount).toBe('number');
    expect(typeof row.final_price).toBe('number');
    expect(row.applied_at).toBeDefined();
    expect(row.deeper_of_breakdown).toBeDefined();

    // Invariant: final_price = base_price - combo_discount (rounding-safe ≥ 0).
    const computed = (row.base_price ?? 0) - (row.combo_discount ?? 0);
    expect(row.final_price).toBe(computed);
    expect((row.final_price ?? 0) >= 0).toBe(true);
  });

  test('Test 4: V1 ladder migration - pre-existing wallet bonus-tier credit retained; combo cannot reduce wallet bonus_balance (Q-2E-4a + Wallet-Framing-C)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!COMBO_OFFER_ENGINE_REACHABLE, SKIP_REASONS.COMBO_ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_CUSTOMER_ID, SKIP_REASONS.TEST_CUSTOMER_ID);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    // Pre-existing V1 wallet bonus-tier credit invariant: customer has a
    // non-zero bonus_balance_paise from a prior top-up at the 10pct or 20pct
    // rung (per row 1.20 ladder shape verified in topup-bonus-ladder.spec.ts).
    const walletBefore = await fetchWalletBalance(TEST_CUSTOMER_ID!);
    expect(walletBefore.status).toBe(200);
    const bonusBefore = walletBefore.body.bonus_balance_paise ?? 0;
    console.log(`wallet bonus_balance before combo: ${bonusBefore} paise`);
    if (bonusBefore === 0) {
      console.log('Customer has zero bonus_balance; ladder-migration invariant requires a prior top-up at 10pct/20pct rung.');
      test.skip(true, 'V1 ladder migration test requires TEST_CUSTOMER_ID with non-zero bonus_balance_paise. Run a >=₹2000 top-up first via Layer 1.20 setup OR pick a customer who already has ladder credit.');
      return;
    }

    // Apply combo and confirm: (a) combo applies to session, (b) wallet
    // bonus_balance is UNCHANGED post-apply (combo operates on SESSION surface,
    // not WALLET surface; bonus-vs-discount independence per DoD L76-80).
    const eligible = await fetchEligibleCombos(TEST_CUSTOMER_ID!);
    expect(eligible.status).toBe(200);
    expect(eligible.body.length).toBeGreaterThanOrEqual(1);
    const combo = eligible.body[0];

    const applyResp = await applyCombo(combo.combo_id, TEST_SESSION_ID!, TEST_CUSTOMER_ID!);
    expect(applyResp.status).toBe(200);
    expect(applyResp.body.combo_offer_id).toBe(combo.combo_id);
    expect(applyResp.body.combo_discount_paise).toBeDefined();
    expect((applyResp.body.combo_discount_paise ?? 0) > 0).toBe(true);

    const walletAfter = await fetchWalletBalance(TEST_CUSTOMER_ID!);
    expect(walletAfter.status).toBe(200);
    const bonusAfter = walletAfter.body.bonus_balance_paise ?? 0;
    console.log(`wallet bonus_balance after combo: ${bonusAfter} paise (delta=${bonusAfter - bonusBefore})`);
    // Combo MUST NOT decrement bonus_balance. Equal or greater allowed (e.g.
    // a concurrent top-up could raise it; but combo path itself must never
    // reduce it).
    expect(bonusAfter).toBeGreaterThanOrEqual(bonusBefore);
  });

  test('Test 5: degenerate single-item combo (combo_size=1) treated as straight session discount (Q-2E-1a flat-combo collapse)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!COMBO_OFFER_ENGINE_REACHABLE, SKIP_REASONS.COMBO_ENGINE_MISSING);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_CUSTOMER_ID, SKIP_REASONS.TEST_CUSTOMER_ID);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    // Q-2E-1a: combos are flat (no recursion). A combo_offer with exactly ONE
    // line_item is a degenerate edge — engine MUST treat as a straight session
    // discount (equivalent to a rate_table session-discount row) and the
    // audit-log + deeper_of_breakdown must still emit a normal combo path
    // (not a different code path).
    const eligible = await fetchEligibleCombos(TEST_CUSTOMER_ID!);
    expect(eligible.status).toBe(200);

    // Find a degenerate combo (one line_item) in the eligibility set.
    const degenerate = eligible.body.find(c => (c.line_items?.length ?? 0) === 1);
    if (!degenerate) {
      console.log(`no degenerate combo (combo_size=1) in eligibility set of ${eligible.body.length}`);
      test.skip(true, 'Edge test requires at least one eligible combo with exactly 1 line_item. Seed a "single-item combo" via admin /v2/combos POST then re-run.');
      return;
    }

    const applyResp = await applyCombo(degenerate.combo_id, TEST_SESSION_ID!, TEST_CUSTOMER_ID!);
    console.log(`degenerate combo apply: status=${applyResp.status} body=${JSON.stringify(applyResp.body)}`);
    expect(applyResp.status).toBe(200);
    expect(applyResp.body.combo_offer_id).toBe(degenerate.combo_id);
    expect(applyResp.body.deeper_of_breakdown).toBeDefined();
    expect(applyResp.body.deeper_of_breakdown!.combo_pct).toBeDefined();

    // Documented behavior: a 1-item combo's combo_pct is the same as if it
    // were defined as a rate_table session_discount. The engine path emits
    // through the combo audit-log (action_type=combo_applied), NOT a
    // session_discount path. This proves Q-2E-1a's "no special-case for
    // size=1" design — flat-combo collapse is uniform.
    const audit = await fetchAuditLog({
      combo_offer_id: degenerate.combo_id,
      session_id: TEST_SESSION_ID!,
    });
    expect(audit.body.length).toBeGreaterThanOrEqual(1);
    expect(audit.body[0].action_type).toBe('combo_applied');
  });
});
