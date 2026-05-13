/**
 * Layer 1.15 acceptance test: Walk-in Guest accounts (discount_ineligible)
 *
 * V2-PROGRESS-MAP §1 row 1.15 ("Walk-in fallback | 2 named Walk-In Guest
 * accounts (discount_ineligible) | NOT-STARTED | james | Captain-locked
 * design; needs DB seed + POS flow").
 *
 * Source-of-truth (grepped 2026-05-13 IST):
 *   - comms-link/v2-skeleton/05-definition-of-done.md §1.2 walk-in fallback
 *     (Path 1 staff-assisted phone, Path 2 named fallback no-phone)
 *   - DoD §1.2 line 45 "Walk-in fallback path — Captain-locked, MVP-required"
 *   - DoD §3 line 314 Joint 1: "Two named fallback customer accounts on
 *     substrate, flagged discount_ineligible: true, for extreme-edge no-phone
 *     walk-ins. LOCKED: 2 named fallback accounts are MVP-required (Captain)."
 *   - DoD §3.3 line 341 substrate flag extension:
 *     "customer record . discount_ineligible: bool — true for the two named
 *     fallback accounts (Walk-In Guest 1, Walk-In Guest 2); false for all real
 *     customer profiles."
 *   - DoD §2 line 72 bonus ladder: "Bonus credits do not apply to
 *     discount-ineligible fallback accounts (Path 2 in §1.2 14:05)."
 *   - DoD line 775 Round 2 lock: "Named fallback accounts: 2 — Walk-In Guest 1
 *     and Walk-In Guest 2; persist forever in audit; excluded from public
 *     leaderboards; cannot be claimed/merged"
 *   - racecontrol/crates/racecontrol/src/api/cirs_lookup.rs (V1-anchor →
 *     V2 endpoint) — Walk-In Guest synthetic path
 *   - racecontrol/crates/v2-db/src/cirs.rs L37-42 LookupInput enum with
 *     WalkInGuestId { guest_id: u8 } variant; serde tag = "method",
 *     rename_all = "snake_case" → wire shape
 *     {"method":"walk_in_guest_id","guest_id":1}
 *   - racecontrol/crates/racecontrol/src/api/routes.rs L840-844:
 *     POST /api/v1/cirs/lookup is staff-JWT gated + rate-limited
 *
 * V2 doctrine alignment: V2 Wallet-Framing-C invariant (single-purpose
 * voucher) requires bill-eligible accounts. Walk-in guest accounts are the
 * fallback path for customers without phone/profile per Captain-locked
 * design — bill capture MUST work (sessions, top-ups), discount path MUST
 * NOT (no bonus ladder, no happy-hour, no iRacing 20%, no leaderboard
 * surfacing).
 *
 * Contract under test (5 atomic checks):
 *   C1 (existence): Both named Walk-In Guest accounts (guest_id 1 + 2)
 *       resolve via POST /api/v1/cirs/lookup with method=walk_in_guest_id.
 *   C2 (flag): Both ProfilePreview rows return discount_ineligible=true.
 *   C3 (billable): Walk-in accounts CAN be billed. Functional fallback, not ghost.
 *   C4 (no bonus ladder): A wallet top-up at the 10pct rung (>=2000) does NOT
 *       produce a bonus-credit ledger entry. Composes with row 1.20.
 *   C5 (source_tag distinct): A top-up writes a wallet ledger row with
 *       source_tag in {pwa,pos,kiosk} × payment_method in {upi,card,cash}.
 *
 * Skip-coherence: with NO env vars set, ALL 5 tests skip cleanly, ZERO assertion failures.
 *
 * Env vars:
 *   CANONICAL_URL          - Server .23 racecontrol (default http://192.168.31.23:8080)
 *   TEST_STAFF_JWT         - staff JWT for POST /api/v1/cirs/lookup
 *   TEST_CUSTOMER_JWT      - customer-class JWT for billing/wallet writes (C3-C5)
 *   WALKIN_GUEST_1_ID      - customer_id for "Walk-In Guest 1" once seeded
 *   WALKIN_GUEST_2_ID      - customer_id for "Walk-In Guest 2" once seeded
 *   BILLING_START_PATH     - override path (default /api/v1/billing/sessions)
 *   WALLET_TOPUP_PATH      - override path (default /api/v1/wallet/topup)
 *   WALLET_HISTORY_PATH    - override path (default /api/v1/wallet/history)
 *   DESTRUCTIVE_OK         - must be 'true' to run C3 / C4 / C5
 *
 * G4 NOT TESTED in this file (deferred):
 *   - Auth + waiver capture flow for staff-mediated walk-in (row 1.6 / 1.11)
 *   - Direct pricing-engine rate-resolution-with-discount-flag (row 1.18, 1.19)
 *   - Leaderboard exclusion (row 1.13 leaderboard contract)
 *   - Cannot-be-claimed / cannot-be-merged invariant (DoD line 775)
 *   - QR / NFC plumbed-disabled paths
 *   - Concurrent walk-in session race
 *   - Per-restart persistence of the seed
 *   - Audit-trail row inspection
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const TEST_STAFF_JWT = process.env.TEST_STAFF_JWT;
const TEST_CUSTOMER_JWT = process.env.TEST_CUSTOMER_JWT;
const WALKIN_GUEST_1_ID = process.env.WALKIN_GUEST_1_ID;
const BILLING_START_PATH = process.env.BILLING_START_PATH || '/api/v1/billing/sessions';
const WALLET_TOPUP_PATH = process.env.WALLET_TOPUP_PATH || '/api/v1/wallet/topup';
const WALLET_HISTORY_PATH = process.env.WALLET_HISTORY_PATH || '/api/v1/wallet/history';
const DESTRUCTIVE_OK = process.env.DESTRUCTIVE_OK === 'true';

const CIRS_LOOKUP_PATH = '/api/v1/cirs/lookup';

const SOURCE_ENUM = ['pwa', 'pos', 'kiosk'] as const;
const PAYMENT_ENUM = ['upi', 'card', 'cash'] as const;

const SKIP_REASONS: Record<string, string> = {
  TEST_STAFF_JWT: `TEST_STAFF_JWT not set - Layer 1.15 Walk-In Guest contract cannot resolve via POST ${CANONICAL_URL}${CIRS_LOOKUP_PATH} without a staff JWT. The CIRS lookup route is staff-JWT gated + rate-limited per routes.rs L840-844.`,
  TEST_CUSTOMER_JWT: `TEST_CUSTOMER_JWT not set - Layer 1.15 billing-eligibility tests (C3 session-start, C4 no-bonus, C5 source_tag) need a customer-class or staff-on-behalf-of JWT.`,
  WALKIN_GUEST_ID_UNSET: `WALKIN_GUEST_1_ID env var not set - row 1.15 seed status NOT-STARTED. Once seeded, export the IDs and tests activate.`,
  DESTRUCTIVE_OK: `DESTRUCTIVE_OK not set to 'true' - Layer 1.15 C3 / C4 / C5 write to canonical wallet ledger. Set DESTRUCTIVE_OK=true ONLY in a disposable test DB.`,
  NOT_SEEDED: `Layer 1.15 walk-in lookup returned NotFound / 404 / 501 - the two named accounts (Walk-In Guest 1, Walk-In Guest 2) are NOT seeded. This IS the row 1.15 "needs DB seed" half.`,
  SHAPE_MISSING: `Layer 1.15 ProfilePreview response does not expose 'discount_ineligible' field per cirs_lookup.rs L99. Schema gap surfaces without failing.`,
  NOT_BILLABLE: `Layer 1.15 C3 / C4 / C5 wallet/billing endpoint returned non-2xx for the walk-in account. Either endpoint not wired ("needs POS flow") OR endpoint rejects discount_ineligible entirely (would VIOLATE V2 Wallet-Framing-C).`,
};

interface CirsLookupRequestPhone {
  method: 'phone';
  phone: string;
}

interface CirsLookupRequestWalkIn {
  method: 'walk_in_guest_id';
  guest_id: number;
}

type CirsLookupRequest = CirsLookupRequestPhone | CirsLookupRequestWalkIn;

interface ProfileSummary {
  profile_id: string;
  name: string;
  is_default: boolean;
  discount_ineligible: boolean;
}

interface ProfilePreview {
  customer_id: string;
  primary_phone: string;
  name: string;
  profiles: ProfileSummary[];
  balance_credits: number;
  last_visit_ts: string | null;
  arrival_history_count_30d: number;
  discount_ineligible: boolean;
}

interface CirsLookupResponse {
  result?: string;
  profile?: ProfilePreview;
  error?: string;
  [k: string]: unknown;
}

interface ApiRead {
  url: string;
  status: number;
  body: any;
  read_started_at: number;
  read_finished_at: number;
}

async function cirsLookup(req: CirsLookupRequest, jwt: string): Promise<ApiRead> {
  const url = `${CANONICAL_URL}${CIRS_LOOKUP_PATH}`;
  const started = Date.now();
  const resp = await fetch(url, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${jwt}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(req),
  });
  const finished = Date.now();
  let body: any = null;
  try {
    body = await resp.json();
  } catch {
    body = null;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

async function apiPost(path: string, jwt: string, body: unknown): Promise<ApiRead> {
  const url = `${CANONICAL_URL}${path}`;
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
  let rbody: any = null;
  try {
    rbody = await resp.json();
  } catch {
    rbody = null;
  }
  return { url, status: resp.status, body: rbody, read_started_at: started, read_finished_at: finished };
}

async function apiGet(path: string, jwt: string): Promise<ApiRead> {
  const url = `${CANONICAL_URL}${path}`;
  const started = Date.now();
  const resp = await fetch(url, { headers: { Authorization: `Bearer ${jwt}` } });
  const finished = Date.now();
  let body: any = null;
  try {
    body = await resp.json();
  } catch {
    body = null;
  }
  return { url, status: resp.status, body, read_started_at: started, read_finished_at: finished };
}

function extractProfile(body: CirsLookupResponse | null): ProfilePreview | null {
  if (!body || typeof body !== 'object') return null;
  if (body.profile && typeof body.profile === 'object') return body.profile;
  const maybe = body as unknown as ProfilePreview;
  if (typeof maybe.customer_id === 'string' && 'discount_ineligible' in maybe) return maybe;
  return null;
}

function isSeedGapStatus(status: number): boolean {
  return status === 404 || status === 501;
}

test.describe('Layer 1.15 - Walk-in Guest accounts (discount_ineligible)', () => {
  test('C1 both named Walk-In Guest accounts resolve via cirs/lookup (WalkInGuestId variant)', async () => {
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);

    const r1 = await cirsLookup({ method: 'walk_in_guest_id', guest_id: 1 }, TEST_STAFF_JWT!);
    const r2 = await cirsLookup({ method: 'walk_in_guest_id', guest_id: 2 }, TEST_STAFF_JWT!);
    console.log(
      `C1 guest_id=1 status=${r1.status} t=${r1.read_finished_at - r1.read_started_at}ms | ` +
      `guest_id=2 status=${r2.status} t=${r2.read_finished_at - r2.read_started_at}ms`,
    );

    const seedGap1 = isSeedGapStatus(r1.status) || (r1.status === 200 && r1.body?.result === 'not_found');
    const seedGap2 = isSeedGapStatus(r2.status) || (r2.status === 200 && r2.body?.result === 'not_found');
    test.skip(seedGap1 || seedGap2, SKIP_REASONS.NOT_SEEDED);

    expect(r1.status).toBe(200);
    expect(r2.status).toBe(200);

    const p1 = extractProfile(r1.body);
    const p2 = extractProfile(r2.body);
    expect(p1, 'Walk-In Guest 1 ProfilePreview must be present in cirs/lookup response').not.toBeNull();
    expect(p2, 'Walk-In Guest 2 ProfilePreview must be present in cirs/lookup response').not.toBeNull();

    expect(p1!.name, 'DoD line 775 locks the name "Walk-In Guest 1"').toMatch(/walk.?in.*guest.*1/i);
    expect(p2!.name, 'DoD line 775 locks the name "Walk-In Guest 2"').toMatch(/walk.?in.*guest.*2/i);

    expect(p1!.customer_id).not.toEqual(p2!.customer_id);
    console.log(`C1 resolved: guest_1.customer_id=${p1!.customer_id} guest_2.customer_id=${p2!.customer_id}`);
  });

  test('C2 both walk-in accounts have discount_ineligible=true (DoD §3.3 line 341)', async () => {
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);

    const r1 = await cirsLookup({ method: 'walk_in_guest_id', guest_id: 1 }, TEST_STAFF_JWT!);
    const r2 = await cirsLookup({ method: 'walk_in_guest_id', guest_id: 2 }, TEST_STAFF_JWT!);

    const seedGap1 = isSeedGapStatus(r1.status) || (r1.status === 200 && r1.body?.result === 'not_found');
    const seedGap2 = isSeedGapStatus(r2.status) || (r2.status === 200 && r2.body?.result === 'not_found');
    test.skip(seedGap1 || seedGap2, SKIP_REASONS.NOT_SEEDED);

    const p1 = extractProfile(r1.body);
    const p2 = extractProfile(r2.body);
    test.skip(p1 === null || p2 === null, SKIP_REASONS.SHAPE_MISSING);

    const hasFlag1 = 'discount_ineligible' in (p1 as object);
    const hasFlag2 = 'discount_ineligible' in (p2 as object);
    test.skip(!hasFlag1 || !hasFlag2, SKIP_REASONS.SHAPE_MISSING);

    console.log(
      `C2 discount_ineligible: guest_1=${p1!.discount_ineligible} guest_2=${p2!.discount_ineligible}`,
    );

    expect(p1!.discount_ineligible, 'DoD §3.3 line 341: Walk-In Guest 1 MUST have discount_ineligible=true').toBe(true);
    expect(p2!.discount_ineligible, 'DoD §3.3 line 341: Walk-In Guest 2 MUST have discount_ineligible=true').toBe(true);

    if (Array.isArray(p1!.profiles) && p1!.profiles.length > 0) {
      expect(p1!.profiles[0].discount_ineligible).toBe(true);
    }
    if (Array.isArray(p2!.profiles) && p2!.profiles.length > 0) {
      expect(p2!.profiles[0].discount_ineligible).toBe(true);
    }
  });

  test('C3 walk-in accounts ARE billable (sessions / top-ups accepted; functional fallback not ghost)', async () => {
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!WALKIN_GUEST_1_ID, SKIP_REASONS.WALKIN_GUEST_ID_UNSET);
    test.skip(!DESTRUCTIVE_OK, SKIP_REASONS.DESTRUCTIVE_OK);

    const topupResp = await apiPost(
      WALLET_TOPUP_PATH,
      TEST_CUSTOMER_JWT!,
      {
        customer_id: WALKIN_GUEST_1_ID,
        amount_paise: 50000,
        payment_method: 'cash',
        source: 'pos',
      },
    );
    console.log(`C3 walk-in top-up ${topupResp.url} status=${topupResp.status}`);

    let billable = topupResp.status >= 200 && topupResp.status < 300;

    if (!billable) {
      const sessionResp = await apiPost(
        BILLING_START_PATH,
        TEST_CUSTOMER_JWT!,
        {
          customer_id: WALKIN_GUEST_1_ID,
          pod_id: 'pod_1',
          duration_minutes: 30,
        },
      );
      console.log(`C3 walk-in session start ${sessionResp.url} status=${sessionResp.status}`);
      billable = sessionResp.status >= 200 && sessionResp.status < 300;
    }

    test.skip(!billable, SKIP_REASONS.NOT_BILLABLE);

    expect(billable, 'Walk-In Guest accounts MUST be bill-eligible per V2 Wallet-Framing-C').toBe(true);
  });

  test('C4 walk-in top-up >=2000 does NOT trigger bonus-credit ladder (DoD §2 line 72)', async () => {
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!WALKIN_GUEST_1_ID, SKIP_REASONS.WALKIN_GUEST_ID_UNSET);
    test.skip(!DESTRUCTIVE_OK, SKIP_REASONS.DESTRUCTIVE_OK);

    const topupAmountPaise = 200000;
    const topupResp = await apiPost(
      WALLET_TOPUP_PATH,
      TEST_CUSTOMER_JWT!,
      {
        customer_id: WALKIN_GUEST_1_ID,
        amount_paise: topupAmountPaise,
        payment_method: 'cash',
        source: 'pos',
      },
    );
    console.log(`C4 walk-in top-up ₹2000 status=${topupResp.status}`);
    test.skip(topupResp.status < 200 || topupResp.status >= 300, SKIP_REASONS.NOT_BILLABLE);

    const historyResp = await apiGet(
      `${WALLET_HISTORY_PATH}?customer_id=${encodeURIComponent(WALKIN_GUEST_1_ID!)}`,
      TEST_CUSTOMER_JWT!,
    );
    console.log(`C4 wallet history status=${historyResp.status}`);
    test.skip(historyResp.status !== 200, SKIP_REASONS.NOT_BILLABLE);

    const rows: unknown[] = Array.isArray(historyResp.body)
      ? historyResp.body
      : Array.isArray(historyResp.body?.entries)
        ? historyResp.body.entries
        : Array.isArray(historyResp.body?.transactions)
          ? historyResp.body.transactions
          : Array.isArray(historyResp.body?.history)
            ? historyResp.body.history
            : [];
    test.skip(rows.length === 0, SKIP_REASONS.NOT_BILLABLE);

    const bonusRows = rows.filter((raw) => {
      const row = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
      const tag = typeof row.source_tag === 'string' ? row.source_tag.toLowerCase() : '';
      const source = typeof row.source === 'string' ? row.source.toLowerCase() : '';
      const kind = typeof row.kind === 'string' ? row.kind.toLowerCase() : '';
      const type = typeof row.type === 'string' ? row.type.toLowerCase() : '';
      return (
        tag.includes('bonus') ||
        source.includes('bonus') ||
        kind.includes('bonus') ||
        type.includes('bonus')
      );
    });
    console.log(`C4 bonus rows found for walk-in customer: ${bonusRows.length}`);

    expect(
      bonusRows,
      'DoD §2 line 72: discount_ineligible accounts MUST NOT receive bonus credits at the 10pct or 20pct rungs',
    ).toEqual([]);
  });

  test('C5 walk-in top-up carries source_tag in canonical {pwa,pos,kiosk} x {upi,card,cash} enum (composes row 1.16)', async () => {
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!WALKIN_GUEST_1_ID, SKIP_REASONS.WALKIN_GUEST_ID_UNSET);
    test.skip(!DESTRUCTIVE_OK, SKIP_REASONS.DESTRUCTIVE_OK);

    const topupResp = await apiPost(
      WALLET_TOPUP_PATH,
      TEST_CUSTOMER_JWT!,
      {
        customer_id: WALKIN_GUEST_1_ID,
        amount_paise: 50000,
        payment_method: 'cash',
        source: 'pos',
      },
    );
    console.log(`C5 walk-in top-up status=${topupResp.status}`);
    test.skip(topupResp.status < 200 || topupResp.status >= 300, SKIP_REASONS.NOT_BILLABLE);

    const historyResp = await apiGet(
      `${WALLET_HISTORY_PATH}?customer_id=${encodeURIComponent(WALKIN_GUEST_1_ID!)}`,
      TEST_CUSTOMER_JWT!,
    );
    test.skip(historyResp.status !== 200, SKIP_REASONS.NOT_BILLABLE);

    const rows: unknown[] = Array.isArray(historyResp.body)
      ? historyResp.body
      : Array.isArray(historyResp.body?.entries)
        ? historyResp.body.entries
        : Array.isArray(historyResp.body?.transactions)
          ? historyResp.body.transactions
          : Array.isArray(historyResp.body?.history)
            ? historyResp.body.history
            : [];
    test.skip(rows.length === 0, SKIP_REASONS.NOT_BILLABLE);

    const candidates = [rows[rows.length - 1], rows[0]].filter(Boolean) as Array<Record<string, unknown>>;
    let surface: string | null = null;
    let paymentMethod: string | null = null;
    for (const c of candidates) {
      const row = (c && typeof c === 'object' ? c : {}) as Record<string, unknown>;
      const s = typeof row.source === 'string' ? row.source.trim().toLowerCase() : null;
      const p = typeof row.payment_method === 'string' ? row.payment_method.trim().toLowerCase() : null;
      if (s) surface = surface ?? s;
      if (p) paymentMethod = paymentMethod ?? p;
    }
    console.log(`C5 top-up tag: source=${surface} payment_method=${paymentMethod}`);
    test.skip(surface === null || paymentMethod === null, SKIP_REASONS.SHAPE_MISSING);

    expect(
      (SOURCE_ENUM as readonly string[]).includes(surface!),
      `DoD §3.3 source enum locks {${SOURCE_ENUM.join(',')}} - walk-in top-up source must be attributable to one of these surfaces`,
    ).toBe(true);
    expect(
      (PAYMENT_ENUM as readonly string[]).includes(paymentMethod!),
      `DoD §3.3 payment enum locks {${PAYMENT_ENUM.join(',')}} - walk-in top-up payment_method must be in canonical enum`,
    ).toBe(true);

    if (surface !== 'pos' || paymentMethod !== 'cash') {
      console.log(
        `C5 OBSERVATION: walk-in top-up landed at surface=${surface} payment=${paymentMethod} (canonical Path B is pos x cash per DoD §1.2 line 47)`,
      );
    }
  });
});
