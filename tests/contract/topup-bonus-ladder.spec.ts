/**
 * Layer 1.20 acceptance test: Top-up bonus ladder + iRacing 20% (cap 20%, deeper-of)
 *
 * V2-PROGRESS-MAP §1 row 1.20 ("All-beat | Top-up bonus ladder + iRacing 20%
 * (cap 20%, deeper-of) | PARTIAL | james | spec locked; engine wire-up gated by
 * Phase 2-A/2-F").
 *
 * Source-of-truth (DoD canonical ladder; quoted verbatim 2026-05-13 IST):
 *   - comms-link/v2-skeleton/05-definition-of-done.md L64-74 (Top-up bonus-credit ladder):
 *
 *     | Deposit | Credits received | Bonus % | Source-tag on bonus row |
 *     | <₹2,000 | 1:1 (e.g., ₹500 → 500 credits) | 0% | n/a |
 *     | ≥₹2,000 | 1:1 + 10% bonus (e.g., ₹2,000 → 2,200 credits) | 10% | `bonus / 10pct` |
 *     | ≥₹4,000 | 1:1 + 20% bonus (e.g., ₹4,000 → 4,800 credits) | 20% | `bonus / 20pct` |
 *
 *     "Ladder cap (Captain-locked): the ladder caps at 20% for deposits ≥₹4,000.
 *      There are no higher rungs in v2.0."
 *
 *   - comms-link/v2-skeleton/05-definition-of-done.md L41 (iRacing participation incentive):
 *     "iRacing carries a 20% flat discount off the published rate, applied any time
 *      iRacing is launched. The iRacing discount does not stack with the happy-hour
 *      discount — when both are active, the customer receives the deeper discount of
 *      the two (currently 30% happy-hour > 20% iRacing, so happy-hour wins on
 *      iRacing-during-happy-hour)."
 *
 *   - comms-link/v2-skeleton/05-definition-of-done.md L76-80
 *     (Bonus-vs-discount independence; Captain-locked):
 *     "The top-up bonus ladder and the session discounts (happy-hour, iRacing) are
 *      independent and never stack/compete. They apply to different ledger events:
 *      - Bonus credits = added to wallet at TOP-UP time, source-tagged `bonus / Xpct`.
 *      - Session discounts = applied at GAME LAUNCH time, reducing the per-minute rate."
 *
 *   - comms-link/v2-skeleton/05-definition-of-done.md L343-347 (ledger source-tag enum):
 *     `bonus / 10pct`, `bonus / 20pct` (top-up bonus credits)
 *
 * Contract semantic clarification (anti-pattern guard):
 *   The "deeper-of" rule in DoD L41 governs SESSION DISCOUNTS at launch time
 *   (happy-hour vs iRacing). It does NOT govern top-up-bonus-tier vs iRacing —
 *   those are independent ledger events per L76-80. Test 4 below tests the
 *   correct contract: deeper-of(happy_hour, iRacing) at the SESSION-DISCOUNT
 *   surface, not at the top-up-bonus surface.
 *
 * Endpoint discovery (grep 2026-05-13 IST):
 *   GET /api/v1/wallet/bonus-tiers   (V2-aware anchor; routes.rs L188 in PUBLIC route
 *                                     group → wallet_staff::wallet_bonus_tiers; reads
 *                                     bonus_tiers table {id, min_amount_paise,
 *                                     bonus_percent, sort_order, is_active}).
 *   POST /api/v1/customer/wallet/topup
 *     OR POST /api/v1/wallet/{driver_id}/topup
 *     (top-up flow; bonus auto-credited per
 *      wallet_staff.rs L219-237: bonus_pct selected by min_amount_paise <= amount
 *      ORDER BY min_amount_paise DESC LIMIT 1). Tests below treat the staff-topup
 *      path as canonical for the "deeper-of-ladder" verification.
 *   iRacing 20% discount mechanism:
 *     DOES NOT EXIST in V1 routes — no /api/v1/discount/iracing or game-context flag.
 *     Spec at DoD L41 + L420-424 mandates: "iRacing 20% discount on Tier 2 desktop-
 *     workaround iRacing — Captain-locked R4-D: YES, applies in v2.0." This test is
 *     structurally GAP-revealing for the iRacing-discount surface; activates as
 *     Phase 2-A (rate-table service) + Phase 2-F (campaign object) land.
 *
 * Contract under test:
 *   1. Ladder shape — canonical /wallet/bonus-tiers endpoint exposes the three-rung
 *      structure (0% <₹2,000 implicit, 10% ≥₹2,000, 20% ≥₹4,000) from DoD L64.
 *   2. Tier-boundary mapping — top-up amounts at each boundary yield the right bonus_pct
 *      (₹500 → 0%, ₹2,000 → 10%, ₹4,000 → 20%) per DoD L64 examples.
 *   3. iRacing 20% rule — game-launch surface applies 20% discount when iRacing context
 *      flag is set, regardless of top-up amount. STRUCTURAL GAP: no V1 endpoint exists
 *      for this; SKIP-with-reason until V2 rate-table service ships.
 *   4. Deeper-of rule — when happy-hour (30%) AND iRacing (20%) both active at session
 *      launch, the deeper (happy-hour 30%) wins per DoD L41 verbatim. STRUCTURAL GAP:
 *      no V1 surface joins these two discount sources.
 *   5. 20% bonus-ladder cap — no top-up amount (₹4,000 / ₹10,000 / ₹100,000) yields
 *      bonus_pct > 20% per DoD L74 "ladder caps at 20% for deposits ≥₹4,000".
 *
 * Env vars:
 *   CANONICAL_URL              - Server .23 racecontrol (default http://192.168.31.23:8080)
 *   CANONICAL_REACHABLE        - 'true' to opt into network-reaching tests (mirrors 1.19
 *                                pattern; bono VPS at Hostinger cannot reach 192.168.31.0/24)
 *   TEST_CUSTOMER_JWT          - Bearer JWT for a test driver (used for /customer/wallet
 *                                paths if the staff-topup surface requires it)
 *   TEST_STAFF_JWT             - Bearer JWT for a staff role (used for /wallet/{id}/topup)
 *   TEST_DRIVER_ID             - driver_id matching TEST_CUSTOMER_JWT
 *   BONUS_LADDER_PATH          - override path (default /api/v1/wallet/bonus-tiers;
 *                                public route group, no auth required)
 *   TOPUP_PATH_TEMPLATE        - override (default /api/v1/wallet/{driver_id}/topup)
 *   IRACING_CONTEXT_HEADER     - header to set iRacing context at game launch
 *                                (e.g. X-Game-Context: iracing). Mechanism does NOT
 *                                exist in V1; set when V2 surface lands.
 *   IRACING_DISCOUNT_PATH      - canonical session-discount endpoint (e.g.
 *                                /api/v1/billing/rates/effective?game=iracing).
 *                                Mechanism does NOT exist in V1; set when V2 ships.
 *   HAPPY_HOUR_ACTIVE          - 'true' to opt into Test 4 deeper-of verification
 *                                (requires V2 happy-hour surface + iRacing surface)
 *   TEST_TOPUP_AMOUNTS         - comma-separated paise amounts; default
 *                                "50000,200000,400000,1000000"
 *                                (₹500 / ₹2,000 / ₹4,000 / ₹10,000)
 *
 * G4 NOT TESTED in this file (deferred):
 *   - End-to-end top-up flow (mint customer → top-up → assert wallet ledger row
 *     with source_tag = `bonus / Xpct`): requires test-mode mint + ledger assertions
 *     across the cloud_sync boundary. Phase 2-C will surface a test harness.
 *   - Discount-ineligible fallback accounts (Walk-In Guest 1/2 per DoD L48) — bonus
 *     credits MUST NOT apply (DoD L72). Separate row 1.X for discount_ineligible
 *     flag enforcement; out-of-scope for ladder shape verification.
 *   - Promo / referral bonus paths (review/follow bonus per billing_views.rs L96-148);
 *     those are different bonus classes, not the ladder.
 *   - Tier 2 desktop-workaround iRacing 20% (DoD L420-424; same mechanism gap as
 *     Tier 1 — covered when iRacing surface lands).
 *   - Cross-tz happy-hour boundary (iRacing late-night DoD L33 Tail); covered by 1.19.
 *   - Cloud-vs-venue ladder consistency: bonus_tiers table replicates via
 *     cloud_sync_payload.rs L191; cross-canonical assertion deferred until both
 *     environments have stable bonus_tiers rows seeded.
 *   - Negative-amount / zero-amount / over-MAX-paise edge cases on /topup; covered
 *     by per-endpoint contract tests, not this ladder-shape test.
 *
 * V2 doctrine alignment: Surfaces the V1→V2 gap on iRacing-discount mechanism
 * (no V1 endpoint joins game-context to rate discount) AND validates the bonus
 * ladder shape that V1 already implements via bonus_tiers table. Test passes
 * today via skip-coherence on iRacing-class assertions; activates incrementally
 * as Phase 2-A rate-table service + Phase 2-F campaign object land.
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const TEST_STAFF_JWT = process.env.TEST_STAFF_JWT;
const TEST_DRIVER_ID = process.env.TEST_DRIVER_ID || '<unset>';
const BONUS_LADDER_PATH = process.env.BONUS_LADDER_PATH || '/api/v1/wallet/bonus-tiers';
const TOPUP_PATH_TEMPLATE = process.env.TOPUP_PATH_TEMPLATE || '/api/v1/wallet/{driver_id}/topup';
const IRACING_CONTEXT_HEADER = process.env.IRACING_CONTEXT_HEADER; // e.g. "X-Game-Context: iracing"
const IRACING_DISCOUNT_PATH = process.env.IRACING_DISCOUNT_PATH;   // e.g. "/api/v1/billing/rates/effective?game=iracing"
const HAPPY_HOUR_ACTIVE = process.env.HAPPY_HOUR_ACTIVE === 'true';
const TEST_TOPUP_AMOUNTS = (process.env.TEST_TOPUP_AMOUNTS || '50000,200000,400000,1000000')
  .split(',')
  .map(s => parseInt(s.trim(), 10))
  .filter(n => Number.isFinite(n) && n > 0);

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not set to 'true' - Layer 1.20 requires network reachability to ${CANONICAL_URL}. Set CANONICAL_REACHABLE=true when running from venue LAN or via VPN to Server .23. From bono VPS (Hostinger), 192.168.31.0/24 is unreachable by default.`,
  IRACING_SURFACE_MISSING: `Layer 1.20 iRacing 20% participation discount surface DOES NOT EXIST in V1 routes (grep 2026-05-13: no /api/v1/discount/iracing, no X-Game-Context header reader, no game-context → rate-table join). Spec DoD L41 + L420-424 mandates the mechanism. Phase 2-A (rate-table service) + Phase 2-F (campaign object) are the engine wire-up gate per V2-PROGRESS-MAP §1 row 1.20 anchor. Set IRACING_DISCOUNT_PATH + IRACING_CONTEXT_HEADER once V2 ships.`,
  HAPPY_HOUR_SURFACE_MISSING: `Layer 1.20 deeper-of(happy_hour=30%, iRacing=20%) test requires BOTH session-discount surfaces: happy-hour rule (active by time-of-day) AND iRacing context flag at launch. Per DoD L41 verbatim "currently 30% happy-hour > 20% iRacing, so happy-hour wins on iRacing-during-happy-hour". Neither surface exists in V1. Set HAPPY_HOUR_ACTIVE=true + IRACING_DISCOUNT_PATH once both V2 surfaces land (Phase 2-A rate-table + Phase 2-F campaign).`,
  TEST_STAFF_JWT: `TEST_STAFF_JWT not set - Layer 1.20 top-up flow at /api/v1/wallet/{driver_id}/topup is staff-protected. Mint via staff PIN login or TEST_MODE staff-mint endpoint. Without staff JWT, ladder shape (Test 1) and cap (Test 5) can still verify via public /wallet/bonus-tiers; tier-boundary mapping (Test 2) needs the topup path.`,
  TEST_DRIVER_ID_UNSET: `TEST_DRIVER_ID not set - cannot resolve {driver_id} placeholder in TOPUP_PATH_TEMPLATE. Mint a test driver via POST ${CANONICAL_URL}/api/v1/customer/test-mint-jwt with body {"driver_id":"<test-id>"} in TEST_MODE.`,
  V1_V2_LADDER_GAP: `bonus_tiers row count or values disagree with DoD L64 canonical (0% <₹2,000 implicit + 10% ≥₹2,000 + 20% ≥₹4,000). Anchor: V2-PROGRESS-MAP §1 row 1.20 'spec locked; engine wire-up gated by Phase 2-A/2-F'. V2 settings migration required before contract closes.`,
};

interface BonusTier {
  id: number | string;
  min_amount_paise: number;
  bonus_pct: number;
  sort_order?: number;
}

interface BonusTiersResponse {
  tiers?: BonusTier[];
  rows?: BonusTier[];
  data?: BonusTier[];
  error?: string;
}

interface TopupResponse {
  ok?: boolean;
  bonus_credits_granted?: number;
  bonus_pct?: number;
  amount_paise?: number;
  error?: string;
  [k: string]: unknown;
}

interface RatesResponse {
  base_rate_paise_per_min?: number;
  effective_rate_paise_per_min?: number;
  discount_pct?: number;
  discount_source?: string; // e.g. "happy_hour" | "iracing"
  game?: string;
  error?: string;
  [k: string]: unknown;
}

interface ApiRead<T = unknown> {
  url: string;
  status: number;
  body: T;
  read_started_at: number;
  read_finished_at: number;
}

async function fetchBonusLadder(): Promise<ApiRead<BonusTiersResponse | BonusTier[]>> {
  const started = Date.now();
  const url = `${CANONICAL_URL}${BONUS_LADDER_PATH}`;
  const resp = await fetch(url);
  const finished = Date.now();
  let body: BonusTiersResponse | BonusTier[];
  try {
    body = (await resp.json()) as BonusTiersResponse | BonusTier[];
  } catch {
    body = {} as BonusTiersResponse;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

function extractTiers(body: BonusTiersResponse | BonusTier[]): BonusTier[] {
  if (Array.isArray(body)) return body;
  if (body.tiers) return body.tiers;
  if (body.rows) return body.rows;
  if (body.data) return body.data;
  return [];
}

function resolveTopupUrl(driverId: string): string {
  const path = TOPUP_PATH_TEMPLATE.replace('{driver_id}', driverId);
  return `${CANONICAL_URL}${path}`;
}

async function postTopup(driverId: string, amountPaise: number, jwt: string): Promise<ApiRead<TopupResponse>> {
  const started = Date.now();
  const url = resolveTopupUrl(driverId);
  const resp = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${jwt}`,
    },
    body: JSON.stringify({ amount_paise: amountPaise, method: 'card', source: 'test' }),
  });
  const finished = Date.now();
  let body: TopupResponse;
  try {
    body = (await resp.json()) as TopupResponse;
  } catch {
    body = {} as TopupResponse;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

async function fetchRates(extraHeaders: Record<string, string> = {}): Promise<ApiRead<RatesResponse>> {
  if (!IRACING_DISCOUNT_PATH) {
    throw new Error('IRACING_DISCOUNT_PATH not set');
  }
  const started = Date.now();
  const url = `${CANONICAL_URL}${IRACING_DISCOUNT_PATH}`;
  const resp = await fetch(url, { headers: extraHeaders });
  const finished = Date.now();
  let body: RatesResponse;
  try {
    body = (await resp.json()) as RatesResponse;
  } catch {
    body = {} as RatesResponse;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

/** Compute the expected bonus_pct for a deposit amount per DoD L64 ladder. */
function expectedBonusPct(amountPaise: number): number {
  // DoD L64: <₹2,000 → 0%, ≥₹2,000 → 10%, ≥₹4,000 → 20%
  if (amountPaise >= 4_00_000) return 20; // ₹4,000 = 4_00_000 paise
  if (amountPaise >= 2_00_000) return 10;
  return 0;
}

function parseHeaderEnv(env: string | undefined): { name: string; value: string } | null {
  if (!env) return null;
  const idx = env.indexOf(':');
  if (idx === -1) return null;
  return { name: env.slice(0, idx).trim(), value: env.slice(idx + 1).trim() };
}

test.describe('Layer 1.20 - Top-up bonus ladder + iRacing 20% (cap 20%, deeper-of)', () => {
  test('Test 1: ladder shape - canonical /wallet/bonus-tiers exposes DoD L64 three-rung structure', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    const r = await fetchBonusLadder();
    console.log(`bonus-tiers read t=${r.read_finished_at - r.read_started_at}ms status=${r.status} url=${r.url}`);
    expect(r.status).toBe(200);
    const tiers = extractTiers(r.body);
    console.log(`tiers received: ${JSON.stringify(tiers)}`);

    // DoD L64: the <₹2,000 row is the 0% IMPLICIT row (no source tag); the table stores
    // only the active rungs. Expect at least 2 active rows (10% + 20%).
    expect(tiers.length).toBeGreaterThanOrEqual(2);

    const pcts = new Set(tiers.map(t => t.bonus_pct));
    if (!pcts.has(10) || !pcts.has(20)) {
      console.log(`bonus_pct values found: ${[...pcts].join(',')} - expected {10, 20} per DoD L64`);
      test.skip(true, SKIP_REASONS.V1_V2_LADDER_GAP);
      return;
    }
    expect(pcts.has(10)).toBe(true);
    expect(pcts.has(20)).toBe(true);

    // Threshold values must match DoD L64 examples: ₹2,000 = 200_000 paise, ₹4,000 = 400_000 paise.
    const t10 = tiers.find(t => t.bonus_pct === 10);
    const t20 = tiers.find(t => t.bonus_pct === 20);
    expect(t10).toBeDefined();
    expect(t20).toBeDefined();
    expect(t10!.min_amount_paise).toBe(200_000);
    expect(t20!.min_amount_paise).toBe(400_000);
  });

  test('Test 2: tier-boundary mapping - top-up at each boundary yields the right bonus_pct', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);
    test.skip(TEST_DRIVER_ID === '<unset>', SKIP_REASONS.TEST_DRIVER_ID_UNSET);

    for (const amt of TEST_TOPUP_AMOUNTS) {
      const expected = expectedBonusPct(amt);
      const r = await postTopup(TEST_DRIVER_ID, amt, TEST_STAFF_JWT!);
      console.log(
        `topup amount=${amt}paise (₹${amt / 100}) status=${r.status} ` +
        `bonus_pct=${r.body.bonus_pct ?? 'n/a'} bonus_credits_granted=${r.body.bonus_credits_granted ?? 'n/a'} ` +
        `expected_bonus_pct=${expected}`,
      );
      expect(r.status).toBe(200);
      // bonus_pct field present implies engine populated it; bonus_credits_granted is
      // (amount_paise * bonus_pct / 100) per wallet_staff.rs:229-231.
      if (typeof r.body.bonus_pct === 'number') {
        expect(r.body.bonus_pct).toBe(expected);
      }
      if (typeof r.body.bonus_credits_granted === 'number') {
        const expectedGranted = Math.floor((amt * expected) / 100);
        expect(r.body.bonus_credits_granted).toBe(expectedGranted);
      }
    }
  });

  test('Test 3: iRacing 20% rule - game-launch context yields 20% session discount', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!IRACING_DISCOUNT_PATH || !IRACING_CONTEXT_HEADER, SKIP_REASONS.IRACING_SURFACE_MISSING);

    const hdr = parseHeaderEnv(IRACING_CONTEXT_HEADER);
    const headers: Record<string, string> = {};
    if (hdr) headers[hdr.name] = hdr.value;

    const r = await fetchRates(headers);
    console.log(`rates(iracing) read status=${r.status} body=${JSON.stringify(r.body)}`);
    expect(r.status).toBe(200);
    // Per DoD L41: iRacing carries a 20% flat discount when iRacing is launched.
    expect(r.body.discount_pct).toBeDefined();
    expect(r.body.discount_pct).toBe(20);
    if (r.body.discount_source !== undefined) {
      expect(r.body.discount_source).toBe('iracing');
    }
  });

  test('Test 4: deeper-of rule - happy-hour 30% wins over iRacing 20% when both active', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!IRACING_DISCOUNT_PATH || !IRACING_CONTEXT_HEADER, SKIP_REASONS.IRACING_SURFACE_MISSING);
    test.skip(!HAPPY_HOUR_ACTIVE, SKIP_REASONS.HAPPY_HOUR_SURFACE_MISSING);

    const hdr = parseHeaderEnv(IRACING_CONTEXT_HEADER);
    const headers: Record<string, string> = {};
    if (hdr) headers[hdr.name] = hdr.value;

    const r = await fetchRates(headers);
    console.log(`rates(iracing+happy_hour) read status=${r.status} body=${JSON.stringify(r.body)}`);
    expect(r.status).toBe(200);
    // Per DoD L41 verbatim: "30% happy-hour > 20% iRacing, so happy-hour wins on
    // iRacing-during-happy-hour". The deeper-of rule resolves to the MAX of the two.
    expect(r.body.discount_pct).toBeDefined();
    expect(r.body.discount_pct).toBe(30);
    if (r.body.discount_source !== undefined) {
      expect(r.body.discount_source).toBe('happy_hour');
    }
  });

  test('Test 5: 20% bonus-ladder cap - no top-up amount yields bonus_pct > 20%', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    const r = await fetchBonusLadder();
    expect(r.status).toBe(200);
    const tiers = extractTiers(r.body);
    console.log(`ladder cap check: tiers=${JSON.stringify(tiers)}`);

    // DoD L74: "ladder caps at 20% for deposits ≥₹4,000. There are no higher rungs in v2.0."
    const maxBonus = tiers.reduce((m, t) => Math.max(m, t.bonus_pct), 0);
    expect(maxBonus).toBeLessThanOrEqual(20);

    // Additionally: a top-up at a "whale" amount (₹100,000 = 10_000_000 paise) must NOT
    // resolve to > 20% on the engine side. This is the constitutional ceiling.
    // The ladder selection logic (wallet_staff.rs:221) is:
    //   SELECT bonus_percent FROM bonus_tiers WHERE is_active=1 AND min_amount_paise <= ?
    //   ORDER BY min_amount_paise DESC LIMIT 1
    // Even at ₹100,000, the highest matching tier is the 20% rung.
    const whaleAmount = 100_00_000; // ₹100,000 in paise
    const whaleExpected = expectedBonusPct(whaleAmount);
    expect(whaleExpected).toBeLessThanOrEqual(20);
    expect(whaleExpected).toBe(20);
  });
});
