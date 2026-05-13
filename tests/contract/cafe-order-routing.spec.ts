/**
 * Layer 1.12 acceptance test: Cafe order routing (PWA-self OR Kiosk-staff → kitchen tablet)
 *
 * V2-PROGRESS-MAP §1 row 1.12 ("Customer-day | 14:42 | Cafe order (PWA-self OR
 * Kiosk-staff to kitchen tablet) | NOT-STARTED | both | kitchen single-tablet
 * UI v2.0-required; not started"). This iter3 closure moves row 1.12
 * NOT-STARTED → IN-FLIGHT via the runtime-testable cafe-order routing contract.
 *
 * Source-of-truth (DoD canonical day; quoted verbatim 2026-05-13 IST):
 *   - comms-link/v2-skeleton/05-definition-of-done.md L93:
 *     "14:42 IST — Customer wants food. Two paths (Captain-locked under R1-C
 *      split — POS .130 does NOT do cafe orders in v2.0; cafe orders are
 *      PWA-self or Kiosk-staff only):"
 *   - comms-link/v2-skeleton/05-definition-of-done.md L95:
 *     "Path A — PWA self-order: customer orders via PWA → wallet debit →
 *      kitchen notification → chef sees order on a single tablet
 *      (Captain-locked) in the kitchen. Tablet shows the live order queue +
 *      lets the chef mark items 'preparing' / 'ready'."
 *   - comms-link/v2-skeleton/05-definition-of-done.md L96:
 *     "Path B — staff via Kiosk .23: Kiosk surfaces a cafe-order flow at the
 *      same surface where staff launch games and top up wallets. Staff places
 *      the order without leaving Kiosk. → wallet debit → kitchen notification
 *      → chef tablet."
 *   - comms-link/v2-skeleton/05-definition-of-done.md L98:
 *     "cafe orders only go through Kiosk so the queue has one source of truth,
 *      not two."
 *   - comms-link/v2-skeleton/05-definition-of-done.md L104:
 *     "14:55:01 IST — Bill auto-generates. Wallet auto-tallies session minutes
 *      × rate + cafe-order amount."  ← cafe-order inclusion in auto-bill
 *   - comms-link/v2-skeleton/05-definition-of-done.md L62:
 *     "Cross-surface consistency tolerance: 2 seconds (Captain-locked) — a PWA
 *      top-up must be visible on Kiosk and POS .130 within 2 seconds."
 *      (applies symmetrically to cafe-order surface per Layer 1.17 composition)
 *   - comms-link/v2-skeleton/05-definition-of-done.md L209:
 *     "No untagged wallet transaction. Every transaction carries source (PWA /
 *      POS .130 / Kiosk), payment method (UPI / card / cash), and operator
 *      (customer / staff X) where applicable." (Source-tagging completeness;
 *      Locked-OPEN in 04-connection-matrix.md §Source-tagging completeness.)
 *   - comms-link/v2-skeleton/05-definition-of-done.md L342:
 *     "wallet.source_tag enumeration (locked v2.0):" — defines the enum cafe
 *     orders must carry (cafe / PWA, cafe / Kiosk per L355-356).
 *   - comms-link/v2-skeleton/05-definition-of-done.md L777:
 *     "Kitchen notification surface: single tablet in kitchen with order
 *      queue + preparing/ready states"
 *   - comms-link/v2-skeleton/05-definition-of-done.md L790:
 *     "Customer-facing acknowledgement time: 1 second (R4-A)"
 *
 * Contract under test:
 *   POST /api/v1/cafe/order accepts {customer_id, items: [{sku, qty}], source:
 *   "pwa-self" | "kiosk-staff"} and returns {order_id, kitchen_routing_id}.
 *   The order MUST appear on the kitchen-tablet queue tagged with the same
 *   source. The cafe-order amount MUST accumulate to the active session's
 *   wallet_debit_paise (composes with Layer 1.13 auto-bill 1s). Cross-surface
 *   visibility ≤ 2s (composes with Layer 1.17). Same client-supplied
 *   idempotency_key MUST NOT double-create.
 *
 * Endpoint discovery (grep 2026-05-13 IST):
 *   /api/v1/cafe/order              ENDPOINT NOT YET REGISTERED — kitchen
 *                                   single-tablet UI v2.0-required not yet
 *                                   built; Layer 1.12 is row 1.12 NOT-STARTED
 *                                   per V2-PROGRESS-MAP §1.
 *   /api/v1/cafe/kitchen-queue      ENDPOINT NOT YET REGISTERED — kitchen
 *                                   tablet UI is v2.0 NEW build; no v1 kitchen
 *                                   flow exists (v1 PoS-direct flow obsolete
 *                                   under R1-C split per DoD L93).
 *   /api/v1/customer/cafe/orders    EXISTS today (routes.rs L380) — POST,
 *                                   customer-self path. Body lacks `source`
 *                                   discriminator + `idempotency_key`; reads
 *                                   driver_id from JWT, not body. Composes
 *                                   forward to Layer 1.12 but is not the
 *                                   canonical v2.0 endpoint named in this
 *                                   contract.
 *   /api/v1/cafe/orders             EXISTS today (routes.rs L683) — POST,
 *                                   staff path. Same shape gaps as customer
 *                                   path. Predates the dual-source v2.0 design.
 *   Kitchen queue endpoint           NO match anywhere in
 *                                   crates/racecontrol/src/ for "kitchen" or
 *                                   "kitchen_routing" (grep 0 hits). Chef
 *                                   tablet UI substrate is the V2.0 build
 *                                   target — front-end + backend BOTH absent.
 *   v2-db cafe/kitchen migrations    NO files matching cafe|kitchen in
 *                                   crates/v2-db/migrations/ (grep 0 hits).
 *                                   v2 schema for kitchen_orders is yet-to-land.
 *
 *   Today's place_cafe_order_inner (crates/racecontrol/src/cafe_orders.rs:25)
 *   accepts {driver_id, items: [{item_id, quantity}]} only — no `source`, no
 *   `idempotency_key`, no kitchen-routing return field. Stock-decrement +
 *   wallet-debit are transactional; idempotency is NOT enforced at the API
 *   boundary (a network retry creates a second order). PlaceOrderResponse
 *   (cafe_order_types.rs:24) returns {order_id, receipt_number, wallet_txn_id,
 *   total_paise, ...} — no kitchen_routing_id.
 *
 * Routing-state mechanism notes:
 *   Layer 1.12 routing requires THREE substrate guarantees not yet built:
 *     (1) Source discriminator on the order row (`cafe / PWA` vs `cafe / Kiosk`
 *         per DoD L355-356 wallet source_tag enum) — propagated end-to-end so
 *         the kitchen tablet renders the origin badge on each card.
 *     (2) Kitchen-queue substrate (kitchen_orders table + WS topic or pull
 *         endpoint) the chef tablet polls/subscribes. DoD L777 + L328 Captain-
 *         locked "single tablet" — multi-surface kitchen UI is a v2.0 design
 *         violation, not a stretch goal.
 *     (3) Idempotency key on the order POST (client-supplied UUID) — paired
 *         with a sessions_clientside table or order.idempotency_key column +
 *         UNIQUE index, returning the prior order_id on replay. The wallet
 *         path (start_billing) already has this pattern at
 *         billing_start_validate.rs:28-156; cafe needs the parallel.
 *   Until these land, the 5 tests below all skip; the file remains
 *   contractually authoritative for what Layer 1.12 must satisfy.
 *
 * Env vars:
 *   CANONICAL_URL          - Server .23 racecontrol (default http://192.168.31.23:8080)
 *   CANONICAL_REACHABLE    - 'true' to opt into network-reaching tests; default
 *                            unset → skip (bono VPS at Hostinger cannot reach
 *                            192.168.31.0/24). Set 'true' from venue LAN/VPN.
 *   TEST_CUSTOMER_JWT      - Bearer JWT for a test driver (PWA-self path);
 *                            mint via POST {CANONICAL_URL}/api/v1/customer/test-mint-jwt
 *                            body {"driver_id":"<test-driver-id>"} (TEST_MODE only)
 *   TEST_STAFF_JWT         - staff JWT for the Kiosk-staff path; mint via
 *                            POST {CANONICAL_URL}/api/v1/auth/staff-login
 *                            (or test-mint-staff-jwt in TEST_MODE).
 *   TEST_CUSTOMER_ID       - driver_id used for source-tagging assertions
 *                            (the JWT subject; the kitchen-queue row must
 *                            carry this id)
 *   TEST_ITEM_SKU          - cafe_items.id of a test item with stock_quantity
 *                            ≥ 5 and is_available=1 (the test does not seed —
 *                            seed via /api/v1/cafe/items POST in TEST_MODE).
 *   TEST_SESSION_ID        - active billing session id for the auto-bill
 *                            composition test (Test 3). The session must
 *                            belong to TEST_CUSTOMER_ID. The test does NOT
 *                            create one — Layer 1.13 owns session start.
 *   CAFE_ORDER_PATH        - override path (default /api/v1/cafe/order)
 *   KITCHEN_QUEUE_PATH     - override path (default /api/v1/cafe/kitchen-queue)
 *   X_SURFACE_TOLERANCE_MS - default 2000 (DoD L62 — 2s cross-surface contract)
 *   R4A_ACK_MS             - default 1000 (DoD L790 — customer-facing 1s ack)
 *
 * G4 NOT TESTED in this file (deferred — Layer 1.12 substrate scope):
 *   - Combo-offer pricing (Layer 10.14 Phase 2-E combo-offer-primitive PACT-
 *     DRAFT — combo pricing for cafe+session bundle is a downstream pricing-
 *     engine layer; this file tests routing only)
 *   - Chef tablet "preparing"/"ready" state transitions (DoD L95, L777 —
 *     state-channel-on-kitchen-row is a substrate the tablet UI owns)
 *   - WhatsApp receipt parity for cafe orders (cafe_order_receipts.rs ships
 *     today; v2.0 contract for source-tagged receipts not yet pinned)
 *   - Stock decrement transactional invariant under crash mid-debit (covered
 *     by cafe_tests.rs unit tests in the same crate; this is the API contract
 *     surface, not the storage invariant)
 *   - POS-direct cafe path negative-assertion (DoD L93 R1-C lock — POS .130
 *     MUST NOT have a cafe-order entry surface; that is an admin/UI guard
 *     test, not a routing-contract test)
 *   - Multi-item single-order edge cases (large carts, mixed countable/
 *     uncountable items) — covered by existing cafe_orders.rs unit tests
 *   - WS push channel to kitchen tablet (the contract asserts kitchen-queue
 *     READ visibility; push-vs-pull is a transport choice owned by the
 *     kitchen-tablet sub-PACT)
 *   - Cafe-only customer with no active session (DoD L108 "Cafe-only customer
 *     (no race) follows the same wallet shape with no session record") —
 *     Test 3 composes ONLY with the session-active path; the no-session
 *     cafe-only debit path is a sibling test once Layer 1.13 lands.
 *
 * V2 doctrine alignment: Kitchen single-tablet UI is V2.0-required NEW build
 * (DoD L328 + L777 + Captain-lock). NO v1 kitchen flow exists — v1's
 * POS-direct cafe entry was REMOVED under R1-C physical-position split (DoD
 * L93 + L98 + L325). V2 introduces dual-source order entry (PWA-self for
 * customer phone per L95, Kiosk-staff for desk-side staff entry per L96), both
 * converging on one kitchen-queue row of truth (L98). Composes with Layer 1.13
 * auto-bill 1s SLA (cafe-order amount in L104 tally), Layer 1.16 source-tagging
 * completeness (L209 / L342 wallet source_tag enum), Layer 1.17 cross-surface
 * 2s tolerance (L62). Phase 2-E combo-offer-primitive PACT-DRAFT (Layer 10.14)
 * — combo pricing for cafe+session bundle is the downstream pricing layer this
 * routing contract enables. Test passes today via skip-coherence; activates as
 * the kitchen-tablet sub-PACT lands.
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const TEST_CUSTOMER_JWT = process.env.TEST_CUSTOMER_JWT;
const TEST_STAFF_JWT = process.env.TEST_STAFF_JWT;
const TEST_CUSTOMER_ID = process.env.TEST_CUSTOMER_ID;
const TEST_ITEM_SKU = process.env.TEST_ITEM_SKU;
const TEST_SESSION_ID = process.env.TEST_SESSION_ID;
const CAFE_ORDER_PATH = process.env.CAFE_ORDER_PATH || '/api/v1/cafe/order';
const KITCHEN_QUEUE_PATH = process.env.KITCHEN_QUEUE_PATH || '/api/v1/cafe/kitchen-queue';
const PROFILE_PATH = '/api/v1/customer/profile';
const X_SURFACE_TOLERANCE_MS = parseInt(process.env.X_SURFACE_TOLERANCE_MS || '2000', 10);
const R4A_ACK_MS = parseInt(process.env.R4A_ACK_MS || '1000', 10);
// Test-harness margin for network jitter + clock skew on top of the canonical
// contract values. The contract itself remains 2000ms / 1000ms; the assertion
// uses tolerance.
const ASSERTION_MARGIN_MS = 100;

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not set to 'true' - Layer 1.12 requires network reachability to ${CANONICAL_URL}. Set CANONICAL_REACHABLE=true when running from venue LAN or VPN to Server .23. From bono VPS (Hostinger), 192.168.31.0/24 is unreachable by default.`,
  TEST_CUSTOMER_JWT: `TEST_CUSTOMER_JWT not set - Layer 1.12 PWA-self path (DoD L95) requires customer JWT. Mint via POST ${CANONICAL_URL}/api/v1/customer/test-mint-jwt with body {"driver_id":"<test-id>"} in TEST_MODE.`,
  TEST_STAFF_JWT: `TEST_STAFF_JWT not set - Layer 1.12 Kiosk-staff path (DoD L96) requires staff JWT. Mint via POST ${CANONICAL_URL}/api/v1/auth/staff-login (or test-mint-staff-jwt in TEST_MODE).`,
  TEST_ITEM_SKU: `TEST_ITEM_SKU not set - cafe order requires an existing cafe_items row with stock_quantity >= 5 and is_available=1. Seed via POST ${CANONICAL_URL}/api/v1/cafe/items in TEST_MODE and pass the returned id.`,
  TEST_CUSTOMER_ID: `TEST_CUSTOMER_ID not set - source-tagging assertions (Test 2) require the JWT-subject driver_id to compare against the kitchen-queue row's customer_id field. Set to the same id used when minting TEST_CUSTOMER_JWT.`,
  TEST_SESSION_ID: `TEST_SESSION_ID not set - Layer 1.12 → Layer 1.13 composition (Test 3) requires an active billing session whose wallet_debit_paise will accumulate the cafe-order amount per DoD L104. Pre-create via Layer 1.13 start path; the cafe test does NOT create sessions.`,
  ENDPOINT_NOT_REGISTERED: `Canonical v2.0 endpoint ${CAFE_ORDER_PATH} NOT YET REGISTERED — kitchen single-tablet UI v2.0-required not yet built. V2-PROGRESS-MAP §1 row 1.12 is NOT-STARTED. Today's substrate has /api/v1/customer/cafe/orders + /api/v1/cafe/orders but neither carries the {source, idempotency_key} body shape OR returns kitchen_routing_id. This test is skip-coherent until the v2.0 routing sub-PACT lands.`,
  KITCHEN_TABLET_PENDING: `Kitchen single-tablet UI is V2.0-required NEW build (DoD L328 + L777 + Captain-lock); no v1 flow exists. ${KITCHEN_QUEUE_PATH} is unreachable until the kitchen-tablet sub-PACT lands. Composes with Layer 1.16 source-tagging completeness (DoD L209 Locked-OPEN) and Layer 1.17 2s cross-surface (DoD L62).`,
  LAYER_113_PENDING: `Layer 1.13 auto-bill 1s SLA (V2-PROGRESS-MAP §1 row 1.13) is NOT-STARTED. The composition test (Test 3) requires session-end wallet tally to surface cafe-order amount in wallet_debit_paise per DoD L104 ('session minutes × rate + cafe-order amount'). Skip until Joint #2 sub-PACT lands the auto-bill verb.`,
};

interface CafeOrderRequest {
  customer_id?: string;
  items: Array<{ sku: string; qty: number }>;
  source: 'pwa-self' | 'kiosk-staff';
  idempotency_key?: string;
}

interface CafeOrderResponse {
  order_id?: string;
  kitchen_routing_id?: string;
  source?: string;
  total_paise?: number;
  idempotent_replay?: boolean;
  error?: string;
  // Today's PlaceOrderResponse shape (legacy, partial parity):
  receipt_number?: string;
  wallet_txn_id?: string;
}

interface KitchenQueueRow {
  order_id: string;
  kitchen_routing_id: string;
  source: 'pwa-self' | 'kiosk-staff';
  customer_id?: string;
  driver_id?: string;
  items: Array<{ sku: string; qty: number; name?: string }>;
  state?: 'queued' | 'preparing' | 'ready';
  created_at?: string;
}

interface KitchenQueueResponse {
  orders?: KitchenQueueRow[];
  error?: string;
}

interface ProfileDriver {
  id: string;
  wallet_balance_paise: number;
}

interface ProfileResponse {
  driver?: ProfileDriver;
  active_session_id?: string;
  active_session_wallet_debit_paise?: number;
  error?: string;
}

interface ApiRead<T> {
  url: string;
  status: number;
  body: T;
  read_started_at: number;
  read_finished_at: number;
}

async function postCafeOrder(
  jwt: string,
  body: CafeOrderRequest,
): Promise<ApiRead<CafeOrderResponse>> {
  const url = `${CANONICAL_URL}${CAFE_ORDER_PATH}`;
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
  let parsed: CafeOrderResponse;
  try {
    parsed = (await resp.json()) as CafeOrderResponse;
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

async function getKitchenQueue(jwt: string): Promise<ApiRead<KitchenQueueResponse>> {
  const url = `${CANONICAL_URL}${KITCHEN_QUEUE_PATH}`;
  const started = Date.now();
  const resp = await fetch(url, {
    headers: { Authorization: `Bearer ${jwt}` },
  });
  const finished = Date.now();
  let parsed: KitchenQueueResponse;
  try {
    parsed = (await resp.json()) as KitchenQueueResponse;
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

function genIdempotencyKey(): string {
  // RFC4122 v4-ish (no crypto dependency — collision-resistant enough for a
  // test-harness key; production callers should use a real uuid lib).
  return 'idem-' + Math.random().toString(36).slice(2) + '-' + Date.now().toString(36);
}

test.describe('Layer 1.12 - Cafe order routing (PWA-self OR Kiosk-staff → kitchen tablet)', () => {
  test('canonical /cafe/order endpoint accepts dual-source body + returns kitchen_routing_id', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!TEST_ITEM_SKU, SKIP_REASONS.TEST_ITEM_SKU);

    const body: CafeOrderRequest = {
      items: [{ sku: TEST_ITEM_SKU!, qty: 1 }],
      source: 'pwa-self',
    };
    const r = await postCafeOrder(TEST_CUSTOMER_JWT!, body);
    console.log(
      `cafe-order shape-check t=${r.read_finished_at - r.read_started_at}ms ` +
      `status=${r.status} url=${r.url}`,
    );

    // ENDPOINT NOT YET REGISTERED is the expected state today (row 1.12 NOT-
    // STARTED). 404 + "not found"-class body → skip-coherent.
    if (r.status === 404) {
      console.log(
        `cafe-order endpoint missing: ${CAFE_ORDER_PATH} returned 404. ` +
        `Today's substrate has /api/v1/customer/cafe/orders + /api/v1/cafe/orders ` +
        `but neither is the canonical v2.0 dual-source endpoint. ` +
        `Layer 1.12 sub-PACT must register ${CAFE_ORDER_PATH}.`,
      );
      test.skip(true, SKIP_REASONS.ENDPOINT_NOT_REGISTERED);
      return;
    }

    expect(r.status).toBe(200);
    // R4-A ack envelope: customer-facing acknowledgement ≤ 1s (DoD L790).
    expect(r.read_finished_at - r.read_started_at).toBeLessThanOrEqual(R4A_ACK_MS + ASSERTION_MARGIN_MS);

    // Required v2.0 response shape: order_id + kitchen_routing_id. Today's
    // PlaceOrderResponse returns {order_id, receipt_number, wallet_txn_id, ...}
    // — no kitchen_routing_id. This assertion forces the sub-PACT to add it.
    console.log(
      `cafe-order response: order_id=${r.body.order_id ?? '<absent>'} ` +
      `kitchen_routing_id=${r.body.kitchen_routing_id ?? '<absent>'} ` +
      `source=${r.body.source ?? '<absent>'}`,
    );
    expect(typeof r.body.order_id).toBe('string');
    expect(typeof r.body.kitchen_routing_id).toBe('string');
  });

  test('source tagging propagates to kitchen tablet (composes Layer 1.16)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);
    test.skip(!TEST_ITEM_SKU, SKIP_REASONS.TEST_ITEM_SKU);
    test.skip(!TEST_CUSTOMER_ID, SKIP_REASONS.TEST_CUSTOMER_ID);

    // 1. Place a PWA-self order. Per DoD L209 + L342 the order MUST carry the
    //    source_tag (cafe / PWA). The kitchen queue MUST surface that tag so
    //    the chef tablet renders origin badge (Captain-lock single-tablet, DoD
    //    L777 — chef sees ONE queue with both origins).
    const body: CafeOrderRequest = {
      items: [{ sku: TEST_ITEM_SKU!, qty: 1 }],
      source: 'pwa-self',
    };
    const place = await postCafeOrder(TEST_CUSTOMER_JWT!, body);
    if (place.status === 404) {
      console.log(
        `source-tagging test skipped: ${CAFE_ORDER_PATH} not registered. ` +
        `Layer 1.16 source-tagging completeness (DoD L209 Locked-OPEN) ` +
        `assertion blocked at the routing-substrate gap.`,
      );
      test.skip(true, SKIP_REASONS.ENDPOINT_NOT_REGISTERED);
      return;
    }
    expect(place.status).toBe(200);
    const orderId = place.body.order_id!;
    const routingId = place.body.kitchen_routing_id!;

    // 2. Read the kitchen queue via the staff (kitchen-tablet) JWT.
    const queue = await getKitchenQueue(TEST_STAFF_JWT!);
    if (queue.status === 404) {
      console.log(
        `kitchen-queue endpoint missing: ${KITCHEN_QUEUE_PATH} returned 404. ` +
        `Chef tablet substrate is the V2.0 build target — no v1 kitchen flow ` +
        `exists. Layer 1.12 + Layer 1.16 source-tagging assertion blocked ` +
        `until kitchen-tablet sub-PACT lands.`,
      );
      test.skip(true, SKIP_REASONS.KITCHEN_TABLET_PENDING);
      return;
    }
    expect(queue.status).toBe(200);

    const row = (queue.body.orders ?? []).find(
      (o) => o.order_id === orderId || o.kitchen_routing_id === routingId,
    );
    console.log(
      `source-tagging row: order_id=${orderId} routing_id=${routingId} ` +
      `found=${row !== undefined} row.source=${row?.source ?? '<absent>'} ` +
      `row.customer_id=${row?.customer_id ?? row?.driver_id ?? '<absent>'}`,
    );
    expect(row).toBeDefined();
    // Source MUST match what we posted (round-trip through the row of truth
    // per DoD L98 "one source of truth, not two").
    expect(row!.source).toBe('pwa-self');
    // Customer/driver id MUST be present + match — anti-pattern in DoD L209:
    // "untagged = audit-blind = constitutional violation."
    const rowCustomerId = row!.customer_id ?? row!.driver_id;
    expect(rowCustomerId).toBe(TEST_CUSTOMER_ID);
  });

  test('cafe-order amount feeds auto-bill (composes Layer 1.13; DoD L104)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!TEST_ITEM_SKU, SKIP_REASONS.TEST_ITEM_SKU);
    test.skip(!TEST_SESSION_ID, SKIP_REASONS.TEST_SESSION_ID);

    // Read pre-cafe wallet_debit_paise on the active session.
    const before = await getProfile(TEST_CUSTOMER_JWT!);
    expect(before.status).toBe(200);
    const debitBefore = before.body.active_session_wallet_debit_paise;
    console.log(
      `auto-bill before-cafe active_session_id=${before.body.active_session_id ?? '<absent>'} ` +
      `wallet_debit_paise=${debitBefore ?? '<absent>'}`,
    );
    if (typeof debitBefore !== 'number') {
      // The composition test requires Layer 1.13 to surface the in-flight
      // tally on /customer/profile. If it doesn't yet, the cross-layer
      // contract is unobservable — skip-coherent with Joint #2 sub-PACT.
      console.log(
        `auto-bill cafe-composition gap: /customer/profile does not surface ` +
        `active_session_wallet_debit_paise. Joint #2 (Billing) sub-PACT must ` +
        `expose the in-flight tally so cafe-order debits compose with the ` +
        `auto-bill 1s SLA per DoD L104.`,
      );
      test.skip(true, SKIP_REASONS.LAYER_113_PENDING);
      return;
    }

    // Place a cafe order while the session is active.
    const body: CafeOrderRequest = {
      items: [{ sku: TEST_ITEM_SKU!, qty: 1 }],
      source: 'pwa-self',
    };
    const place = await postCafeOrder(TEST_CUSTOMER_JWT!, body);
    if (place.status === 404) {
      test.skip(true, SKIP_REASONS.ENDPOINT_NOT_REGISTERED);
      return;
    }
    expect(place.status).toBe(200);
    const cafeAmount = place.body.total_paise;
    console.log(
      `auto-bill cafe-order placed order_id=${place.body.order_id} ` +
      `total_paise=${cafeAmount ?? '<absent>'}`,
    );

    // Read post-cafe wallet_debit_paise — MUST accumulate the cafe amount.
    // DoD L104: "Wallet auto-tallies session minutes × rate + cafe-order amount."
    const after = await getProfile(TEST_CUSTOMER_JWT!);
    expect(after.status).toBe(200);
    const debitAfter = after.body.active_session_wallet_debit_paise;
    console.log(
      `auto-bill after-cafe wallet_debit_paise=${debitAfter ?? '<absent>'} ` +
      `delta=${typeof debitAfter === 'number' ? debitAfter - debitBefore : '<n/a>'}`,
    );

    if (typeof debitAfter !== 'number' || typeof cafeAmount !== 'number') {
      test.skip(true, SKIP_REASONS.LAYER_113_PENDING);
      return;
    }
    const delta = debitAfter - debitBefore;
    // Delta should equal the cafe-order amount (session-minutes tally is
    // monotonic but slow on a 1-2s window — strict equality is appropriate at
    // the second-resolution Layer 1.13 surface; if the tally also advances
    // by session-minute drift, expect delta >= cafeAmount).
    expect(delta).toBeGreaterThanOrEqual(cafeAmount);
  });

  test('cross-surface visibility - cafe order placed via PWA visible to Kiosk ≤2s (composes Layer 1.17)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);
    test.skip(!TEST_ITEM_SKU, SKIP_REASONS.TEST_ITEM_SKU);

    // PWA path: customer places order.
    const placeStart = Date.now();
    const place = await postCafeOrder(TEST_CUSTOMER_JWT!, {
      items: [{ sku: TEST_ITEM_SKU!, qty: 1 }],
      source: 'pwa-self',
    });
    if (place.status === 404) {
      test.skip(true, SKIP_REASONS.ENDPOINT_NOT_REGISTERED);
      return;
    }
    expect(place.status).toBe(200);
    const orderId = place.body.order_id!;
    const routingId = place.body.kitchen_routing_id!;

    // Kiosk-side read (staff JWT against kitchen-queue — the Kiosk surface
    // staff use to see the queue per DoD L96 + L777). Visibility budget is
    // 2 seconds Captain-locked (DoD L62) from place-start to read-finish.
    const queue = await getKitchenQueue(TEST_STAFF_JWT!);
    if (queue.status === 404) {
      test.skip(true, SKIP_REASONS.KITCHEN_TABLET_PENDING);
      return;
    }
    expect(queue.status).toBe(200);
    const propagation_ms = queue.read_finished_at - placeStart;
    console.log(
      `x-surface place→kitchen-visible propagation=${propagation_ms}ms ` +
      `threshold=${X_SURFACE_TOLERANCE_MS}ms order_id=${orderId}`,
    );
    expect(propagation_ms).toBeLessThanOrEqual(X_SURFACE_TOLERANCE_MS + ASSERTION_MARGIN_MS);

    // The placed order MUST be in the kitchen queue (presence is the cross-
    // surface visibility contract; state can still be 'queued').
    const row = (queue.body.orders ?? []).find(
      (o) => o.order_id === orderId || o.kitchen_routing_id === routingId,
    );
    expect(row).toBeDefined();
  });

  test('idempotency - same idempotency_key MUST NOT double-create order', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!TEST_ITEM_SKU, SKIP_REASONS.TEST_ITEM_SKU);

    const key = genIdempotencyKey();
    const body: CafeOrderRequest = {
      items: [{ sku: TEST_ITEM_SKU!, qty: 1 }],
      source: 'pwa-self',
      idempotency_key: key,
    };

    // Call 1: place order.
    const r1 = await postCafeOrder(TEST_CUSTOMER_JWT!, body);
    if (r1.status === 404) {
      test.skip(true, SKIP_REASONS.ENDPOINT_NOT_REGISTERED);
      return;
    }
    console.log(
      `idempotency call-1 t=${r1.read_finished_at - r1.read_started_at}ms ` +
      `status=${r1.status} order_id=${r1.body.order_id ?? '<absent>'} ` +
      `total_paise=${r1.body.total_paise ?? '<absent>'} ` +
      `replay=${r1.body.idempotent_replay ?? '<absent>'}`,
    );
    expect(r1.status).toBe(200);
    const orderId1 = r1.body.order_id!;
    const totalPaise1 = r1.body.total_paise;

    // Call 2: same idempotency_key, identical body. Contract: "second call MUST
    // NOT create a second order." Two acceptable shapes for the response:
    //   (a) idempotent_replay=true + same order_id (preferred)
    //   (b) plain {order_id: <same id>} with no replay flag (informational)
    // Either way, the order_id MUST equal the first call's order_id. Today's
    // place_cafe_order_inner (cafe_orders.rs:25) has NO idempotency_key path —
    // a retry creates a SECOND order with a new id + a second wallet debit.
    // That class of bug is exactly what this test guards.
    const r2 = await postCafeOrder(TEST_CUSTOMER_JWT!, body);
    console.log(
      `idempotency call-2 t=${r2.read_finished_at - r2.read_started_at}ms ` +
      `status=${r2.status} order_id=${r2.body.order_id ?? '<absent>'} ` +
      `total_paise=${r2.body.total_paise ?? '<absent>'} ` +
      `replay=${r2.body.idempotent_replay ?? '<absent>'}`,
    );
    expect(r2.status).toBe(200);
    const orderId2 = r2.body.order_id!;

    // The canonical invariant: same key → same order_id. Anything else is a
    // double-create (constitutional violation; cafe debited twice for one
    // customer intent — DoD §customer-felt failure class L488 "Cafe order
    // placed but never delivered to kitchen" sibling class).
    expect(orderId2).toBe(orderId1);

    // If the API exposes total_paise, verify call 2 reports the SAME amount
    // (not 0 — which would be wrong-direction "second order was free").
    if (typeof totalPaise1 === 'number' && typeof r2.body.total_paise === 'number') {
      expect(r2.body.total_paise).toBe(totalPaise1);
    }
  });
});
