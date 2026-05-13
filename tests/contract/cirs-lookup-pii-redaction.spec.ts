/**
 * Layer 7.8 acceptance test: cirs_lookup_handler PII redaction in error paths
 *                            (security-debt-ledger row 7.8 — privacy-debt class)
 *
 * V2-PROGRESS-MAP §7 row 7.8 ("security-debt-ledger | cirs_lookup_handler phone
 * leak via format!('{e}') | Wave 1 billing-engine | YES customer-touching
 * (privacy-debt class) | iter3 = OPEN | iter4 closure moves OPEN -> IN-FLIGHT
 * via this contract test").
 *
 * Source-of-truth (grepped 2026-05-13 IST):
 *   - racecontrol-pr66-deploy/crates/racecontrol/src/api/cirs_lookup.rs L288:
 *       let msg = format!("{e}");
 *     embedded into the HTTP response at L306:
 *       (status, Json(json!({ "error": code, "message": msg }))).into_response()
 *     This is the PII-leak smoking gun. The `e` is a CirsError whose Display
 *     impl (cirs.rs L23-31) interpolates the supplied phone via `{0}`:
 *       #[error("invalid phone: {0}")]      InvalidPhone(String),
 *       #[error("ambiguous phone ... : {0}")] AmbiguousPhone(String),
 *     The String argument is the staff-typed raw input — see cirs.rs L106 / L109
 *     / L120 / L123 / L125 where canonicalize_phone passes `input.to_string()`
 *     verbatim into the error variant. Any 401 / 429 / 4xx response that
 *     traverses this path echoes the raw phone digits straight back to
 *     anyone observing the error body (curl-on-LAN; logs; client-side error
 *     handlers that surface .message to UI). This violates DPDP class 7.8.
 *   - racecontrol-pr66-deploy/crates/racecontrol/src/api/cirs_lookup.rs L199:
 *       let msg = format!("profile_preview_fetch_failed: {e}");
 *     sibling pattern on the 500 path — sqlx::Error Display can include
 *     the bind value column under some sqlx feature flags; defense-in-depth
 *     against future drift covered by Test 1 + Test 2 grepping for digits.
 *   - racecontrol-pr66-deploy/crates/v2-db/src/cirs.rs L23-31 — thiserror::Error
 *     variants Display impl carries the raw input via `{0}`.
 *   - racecontrol-pr66-deploy/crates/racecontrol/src/api/routes.rs L840-844:
 *     POST /api/v1/cirs/lookup is mounted under staff-JWT-gated sub-router
 *     wrapped in auth_rate_limit_layer() (20 burst / 1-per-3s replenish ≈
 *     20/min/IP per security-debt-ledger row 9 closure 2026-05-08 / PR #68).
 *   - racecontrol-pr66-deploy/CLAUDE.md "GDPR erase contract" standing rule
 *     (privacy-debt class anchor; same legal substrate that backs row 1.14
 *     customer_data_delete contract — both are DPDP foundational invariants).
 *
 * Contract under test (5 atomic checks):
 *   T1 (invalid-phone path): POST /api/v1/cirs/lookup with method=phone and
 *       a malformed phone (e.g. "abc123") MUST NOT echo the digits-only
 *       normalization of the supplied input in the error response body.
 *       This is the staff-typo class: even when input is rejected pre-DB,
 *       the rejection reason MUST NOT round-trip the input verbatim.
 *   T2 (lookup-not-found / format-valid path): a phone whose format is
 *       canonicalize-acceptable but driver-not-registered MUST NOT echo the
 *       phone in any 4xx body (the 404 result-shape is structurally safe
 *       today — the message field per cirs_lookup.rs L249-251 is a fixed
 *       string — but the test guards against drift if a future "we couldn't
 *       find X" message ever interpolates the input). F-08 anchor: phone
 *       format valid + record-not-found is still a PII surface if echoed.
 *   T3 (rate-limit / 429 path): a 429 response from the auth_rate_limit_layer
 *       MUST NOT echo the supplied phone. Composes with security-debt-ledger
 *       row 9 (PR #68 rate-limit shipped 2026-05-08) — row 7.8 is the
 *       orthogonal PII-redaction class; both fire on the same surface.
 *   T4 (unauthenticated / 401 path): an unauthenticated POST to /cirs/lookup
 *       MUST NOT echo the supplied phone even pre-auth. Pre-auth path is the
 *       worst-case PII surface — anyone on the LAN with a curl can confirm
 *       a phone exists / doesn't exist if 401 echoes it. require_staff_jwt
 *       middleware (routes.rs L847) rejects before the handler runs, so
 *       this path tests the Axum middleware error-shape envelope.
 *   T5 (success contrast / goldback): a Found response IS allowed to include
 *       the canonicalized phone in primary_phone (cirs_lookup.rs L92, L407)
 *       because the caller has proven staff authorization. Verifies the
 *       redaction is TARGETED (error-path) not BLANKET (response-wide).
 *
 * Skip-coherence: with NO env vars set, ALL 5 tests skip cleanly, ZERO
 * assertion failures. From bono VPS (Hostinger), 192.168.31.0/24 is
 * unreachable by default — CANONICAL_REACHABLE gate handles this.
 *
 * Env vars:
 *   CANONICAL_URL              - Server .23 racecontrol (default http://192.168.31.23:8080)
 *   CANONICAL_REACHABLE        - 'true' to opt into network-reaching tests; default
 *                                unset -> skip. Set 'true' from venue LAN/VPN.
 *   TEST_STAFF_JWT             - staff JWT for POST /api/v1/cirs/lookup (T1, T2, T3, T5)
 *   TEST_PHONE_NOT_REGISTERED  - a phone known to NOT be in the system (T2 lookup-fail)
 *                                Format: 10-digit Indian mobile or +E.164 — must
 *                                pass canonicalize_phone (cirs.rs L99-126).
 *   TEST_PHONE_REGISTERED      - a phone known TO be in the system (T5 success contrast)
 *                                Must resolve to a real ProfilePreview.
 *
 * G4 NOT TESTED in this file (deferred — closure-Phase scope):
 *   - Server log scrape: the audit log itself (`cirs_lookup_audit.input_hash`)
 *     is SHA256-hashed per cirs.rs L57-67 so log-side PII is structurally
 *     redacted; HTTP-response redaction is the open class. A log-scrape
 *     assertion would require shell access; deferred to a sibling test.
 *   - Structural fix verification: this test asserts the CONTRACT (no leak)
 *     but does NOT assert WHICH redaction strategy ships (error-response
 *     sanitization middleware vs. Display impl with redact-by-default vs.
 *     handler-level scrub). Any strategy that passes T1-T4 + preserves T5
 *     is contract-compliant.
 *   - Wallet-engine error paths (the same format!("{e}") anti-pattern likely
 *     exists in billing.rs / wallet.rs error responses — closure-Phase
 *     scope per Wave 1 billing-engine sub-PACT; row 7.8 is cirs-lookup-only).
 *   - L102/L104 14:55 auto-bill beat — sibling PII surface (customer phone
 *     may surface in billing error path on debit-failure). Tracked under
 *     Layer 1.13 auto-bill SLA contract; this file is cirs-lookup scope.
 *   - DPDP customer_data_delete cross-link (Layer 1.14): retention/erase
 *     governed there; this test is error-path redaction only.
 *   - Composes-with Layer 1.18 (UI-SPEC Q-CUST-7 DPDP consent banner —
 *     shipped 2026-05-12 §S-204): consent + redaction are sibling DPDP
 *     invariants; banner is informational, redaction is technical.
 *   - Composes-with Layer 7.7 CSRF (CLOSED 2026-05-12 ~05:37 UTC via
 *     Bearer-only structural protection) and Layer 7.9 rate-limit (CLOSED
 *     via PR #68): row 7.8 is the remaining cirs-lookup-surface security-
 *     debt row from the Wave-1 trio.
 *   - QR / NFC error-paths (cirs_lookup.rs L312-343) — Phase 3 plumbed-disabled;
 *     when activated, same PII class applies to qr_payload / nfc_tag_id
 *     content (V2 customer workflows treats both as PII).
 *
 * V2 doctrine alignment: DPDP / GDPR India compliance is foundational to
 * V2 legal posture. PII in HTTP error responses is observable by anyone
 * with a curl + the route — the digits leak regardless of caller auth state.
 * security-debt-ledger row 7.8 privacy-debt class + Wave 1 billing-engine
 * sub-PACT scope. iter4 closure moves OPEN -> IN-FLIGHT via runtime-testable
 * contract; structural fix (error-response sanitization layer or Display
 * impl redact-by-default) lands as the IN-FLIGHT -> CLOSED transition.
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const TEST_STAFF_JWT = process.env.TEST_STAFF_JWT;
const TEST_PHONE_NOT_REGISTERED = process.env.TEST_PHONE_NOT_REGISTERED;
const TEST_PHONE_REGISTERED = process.env.TEST_PHONE_REGISTERED;

const CIRS_LOOKUP_PATH = '/api/v1/cirs/lookup';

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not set to 'true' - Layer 7.8 requires network reachability to ${CANONICAL_URL}. Set CANONICAL_REACHABLE=true when running from venue LAN or VPN to Server .23. From bono VPS (Hostinger), 192.168.31.0/24 is unreachable by default.`,
  TEST_STAFF_JWT: `TEST_STAFF_JWT not set - Layer 7.8 T1/T2/T3/T5 cannot exercise POST ${CANONICAL_URL}${CIRS_LOOKUP_PATH} without a staff JWT (routes.rs L847 require_staff_jwt). T4 unauth path runs independently. Mint via POST ${CANONICAL_URL}/api/v1/auth/staff-login (or test-mint-staff-jwt in TEST_MODE).`,
  TEST_PHONE_NOT_REGISTERED: `TEST_PHONE_NOT_REGISTERED not set - Layer 7.8 T2 (format-valid-but-not-found path) needs a phone known to PASS cirs.rs L99-126 canonicalize_phone AND NOT match any customers.phone row. Use a 10-digit Indian mobile (auto-prefixed +91) that is reserved-for-testing on the substrate.`,
  TEST_PHONE_REGISTERED: `TEST_PHONE_REGISTERED not set - Layer 7.8 T5 success-contrast cannot fire without a phone that resolves to a real ProfilePreview. The T5 goldback proves redaction is TARGETED (error-path only) not BLANKET (response-wide) — without T5 the contract is half-tested.`,
  PII_REDACTION_NOT_YET_SHIPPED: `Layer 7.8 row OPEN at iter3 -> IN-FLIGHT at iter4. Structural fix has not yet shipped. This test will FAIL until the error-response sanitization (Display impl redact-by-default OR middleware scrub OR handler-level redact) lands. Failure IS the signal that closure work is needed; test is the IN-FLIGHT runtime gate.`,
};

interface CirsLookupPhoneRequest {
  method: 'phone';
  phone: string;
}

interface CirsErrorBody {
  error?: string;
  message?: string;
  [k: string]: unknown;
}

interface ApiRead {
  url: string;
  status: number;
  body: unknown;
  body_text: string;
  read_started_at: number;
  read_finished_at: number;
}

/**
 * Normalize the supplied input to "digits-only" — the contains-check uses
 * this so detection is robust against any echoing strategy (raw "+91..."
 * embedded, raw "98765..." embedded, raw "0987..." embedded). If ANY of
 * these forms appears in the response body, the contract fails.
 */
function digitsOnly(input: string): string {
  return input.replace(/\D/g, '');
}

/**
 * Generate every plausible echo-form of the supplied input. A response
 * leaks if ANY of these substrings appears in the body text:
 *   - raw input verbatim
 *   - digits-only form
 *   - +91-prefixed canonicalization (per cirs.rs L114)
 *   - 0-prefixed STD form (per cirs.rs L116-120 ambiguous-reject)
 *   - trimmed form
 * Any future redaction strategy that asterisks the middle / hashes the
 * value will not contain any of these substrings, so this is the
 * structural detector.
 */
function leakSubstrings(input: string): string[] {
  const trimmed = input.trim();
  const digits = digitsOnly(input);
  const candidates = new Set<string>();
  if (trimmed.length >= 6) candidates.add(trimmed);
  if (digits.length >= 6) candidates.add(digits);
  if (digits.length === 10) {
    candidates.add(`+91${digits}`);
    candidates.add(`91${digits}`);
    candidates.add(`0${digits}`);
  }
  if (digits.length === 11 && digits.startsWith('0')) {
    candidates.add(digits);
    candidates.add(digits.slice(1));
  }
  return Array.from(candidates).filter((s) => s.length >= 6);
}

async function cirsLookupRaw(
  body: unknown,
  jwt: string | null,
): Promise<ApiRead> {
  const url = `${CANONICAL_URL}${CIRS_LOOKUP_PATH}`;
  const started = Date.now();
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (jwt) headers['Authorization'] = `Bearer ${jwt}`;
  const resp = await fetch(url, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
  const finished = Date.now();
  const body_text = await resp.text();
  let parsed: unknown = null;
  try {
    parsed = body_text ? JSON.parse(body_text) : null;
  } catch {
    parsed = null;
  }
  return {
    url,
    status: resp.status,
    body: parsed,
    body_text,
    read_started_at: started,
    read_finished_at: finished,
  };
}

function assertNoLeak(read: ApiRead, supplied: string, testName: string): void {
  const candidates = leakSubstrings(supplied);
  const hits: string[] = [];
  for (const c of candidates) {
    if (read.body_text.includes(c)) hits.push(c);
  }
  console.log(
    `${testName} leak-check status=${read.status} candidates=[${candidates.join('|')}] hits=[${hits.join('|')}] ` +
    `body_preview=${read.body_text.slice(0, 200)}`,
  );
  expect(
    hits,
    `Layer 7.8 PII-redaction contract: response body for status=${read.status} ` +
    `MUST NOT echo any form of supplied phone="${supplied}". Found leak substrings: ` +
    `[${hits.join('|')}]. cirs_lookup.rs L288 format!("{e}") on CirsError variants ` +
    `(cirs.rs L23-31) interpolates raw input via Display {0}. Structural fix: ` +
    `redact-by-default Display impl OR error-response sanitization middleware.`,
  ).toEqual([]);
}

test.describe('Layer 7.8 - cirs_lookup_handler PII redaction (security-debt-ledger row 7.8)', () => {
  test('T1 invalid-phone path - error body MUST NOT echo supplied raw input', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);

    // "abc123" is staff-typo-class: not all-digits, not +-prefixed, length < 10.
    // Routes to cirs.rs L125 InvalidPhone(input.to_string()). The Display
    // impl renders "invalid phone: abc123" and cirs_lookup.rs L288 embeds
    // that verbatim into the 400 response. Test asserts no leak. The
    // assertion accepts any redacted form (asterisked / hashed / dropped
    // message field) as long as the supplied string does not appear.
    const supplied = 'abc123';
    const read = await cirsLookupRaw(
      { method: 'phone', phone: supplied } as CirsLookupPhoneRequest,
      TEST_STAFF_JWT!,
    );

    // Expected status is 400 (BAD_REQUEST per cirs_lookup.rs L279). If the
    // status differs the test still runs the leak check — the contract is
    // about body content, not status. Document the observed status for
    // forward-compat with future error-shape refactors.
    if (read.status !== 400) {
      console.log(`T1 OBSERVATION: status=${read.status} (expected 400 per cirs_lookup.rs L279)`);
    }

    assertNoLeak(read, supplied, 'T1[invalid-phone]');
  });

  test('T2 lookup-not-found path - 404 body MUST NOT echo supplied phone (F-08 class)', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);
    test.skip(!TEST_PHONE_NOT_REGISTERED, SKIP_REASONS.TEST_PHONE_NOT_REGISTERED);

    // Format-valid phone (passes canonicalize_phone) but no customers row
    // matches. Routes to cirs_lookup.rs L228-254 NotFound branch. Today's
    // 404 body is a fixed string (L249-251 "no customer matches the
    // canonicalized phone") — structurally safe. Test guards against
    // drift if any future "we couldn't find <phone>" interpolation lands.
    const supplied = TEST_PHONE_NOT_REGISTERED!;
    const read = await cirsLookupRaw(
      { method: 'phone', phone: supplied } as CirsLookupPhoneRequest,
      TEST_STAFF_JWT!,
    );

    if (read.status !== 404) {
      console.log(`T2 OBSERVATION: status=${read.status} (expected 404 per cirs_lookup.rs L247)`);
    }

    assertNoLeak(read, supplied, 'T2[not-found]');
  });

  test('T3 rate-limit / 429 path - error body MUST NOT echo supplied phone', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);

    // Trigger rate-limit by issuing > 20 requests in rapid succession.
    // The auth_rate_limit_layer (routes.rs L843, ~20 burst / 1-per-3s
    // replenish per security-debt-ledger row 9 / PR #68) returns 429.
    // The tower_governor 429 envelope is independent of the handler —
    // this test asserts the envelope itself doesn't leak. If the layer
    // is not yet wired (older deploy) the test will not see 429 within
    // the burst — handled by an observation log + skip.
    // Sentinel '0000000000' per project no-fake-data rule (passes
    // canonicalize_phone digit-count check, clearly non-production).
    const supplied = '0000000000';
    const N_BURST = 40;
    let observed_429: ApiRead | null = null;
    let first_status: number | null = null;
    for (let i = 0; i < N_BURST; i++) {
      const read = await cirsLookupRaw(
        { method: 'phone', phone: supplied } as CirsLookupPhoneRequest,
        TEST_STAFF_JWT!,
      );
      if (first_status === null) first_status = read.status;
      if (read.status === 429) {
        observed_429 = read;
        break;
      }
    }
    console.log(`T3 burst N=${N_BURST} first_status=${first_status} got_429=${observed_429 !== null}`);

    if (observed_429 === null) {
      console.log(
        `T3 OBSERVATION: rate-limit layer did not emit 429 within ${N_BURST} requests. ` +
        `Possible causes: (a) IP not yet over burst threshold (auth_rate_limit_layer ` +
        `windows; PR #68 closure), (b) layer not wired on this build, (c) test running ` +
        `from inside trusted-IP allow-list. Skip rather than false-pass.`,
      );
      test.skip(true, 'T3 could not trigger 429 — see observation log');
      return;
    }

    assertNoLeak(observed_429, supplied, 'T3[rate-limit]');
  });

  test('T4 unauthenticated / 401 path - error body MUST NOT echo supplied phone', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);

    // No JWT — require_staff_jwt middleware (routes.rs L847) rejects before
    // the handler runs. The 401 envelope is from the Axum middleware layer,
    // not the cirs handler. Test asserts even the middleware reject path
    // does not echo the supplied phone. Worst-case PII surface: anyone on
    // LAN with curl can confirm phone-format-validity if 401 echoes input.
    // require_staff_jwt is the structurally-correct first line of defense
    // (returns 401 before any handler-level processing) — this test
    // verifies that envelope's PII discipline.
    // Sentinel '0000000000' per project no-fake-data rule.
    const supplied = '0000000000';
    const read = await cirsLookupRaw(
      { method: 'phone', phone: supplied } as CirsLookupPhoneRequest,
      null,
    );

    if (read.status !== 401) {
      console.log(`T4 OBSERVATION: status=${read.status} (expected 401 from require_staff_jwt middleware)`);
    }

    assertNoLeak(read, supplied, 'T4[unauthenticated]');
  });

  test('T5 success contrast (goldback) - phone IS allowed in ProfilePreview when caller authorized', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_STAFF_JWT, SKIP_REASONS.TEST_STAFF_JWT);
    test.skip(!TEST_PHONE_REGISTERED, SKIP_REASONS.TEST_PHONE_REGISTERED);

    // Goldback: a Found response IS expected to include the canonical
    // phone in ProfilePreview.primary_phone (cirs_lookup.rs L92, L407).
    // The contrast with T1-T4 proves the redaction is TARGETED at error
    // paths, not BLANKET across the response surface. A regression where
    // the structural fix accidentally redacts the success-path primary_phone
    // would break legitimate staff workflows (POS staff cannot see whose
    // wallet they're about to charge) — T5 guards that regression class.
    const supplied = TEST_PHONE_REGISTERED!;
    const read = await cirsLookupRaw(
      { method: 'phone', phone: supplied } as CirsLookupPhoneRequest,
      TEST_STAFF_JWT!,
    );

    console.log(
      `T5 success-contrast status=${read.status} ` +
      `body_preview=${read.body_text.slice(0, 200)}`,
    );

    // If status is not 200, the seed phone is not actually registered in
    // this environment — observation + skip rather than false-fail.
    if (read.status !== 200) {
      console.log(
        `T5 OBSERVATION: status=${read.status} for TEST_PHONE_REGISTERED. ` +
        `The phone does not resolve in this environment. The goldback contrast ` +
        `cannot be tested without a Found path; seed a real phone or skip.`,
      );
      test.skip(true, 'T5 requires a real Found path - seed TEST_PHONE_REGISTERED in this env');
      return;
    }

    // The success path SHOULD include some form of the phone (the canonical
    // E.164 per cirs_lookup.rs L407 primary_phone field). Assert at least
    // one of the canonical-form substrings is present — this PROVES the
    // redaction is targeted not blanket.
    const candidates = leakSubstrings(supplied);
    const present = candidates.filter((c) => read.body_text.includes(c));
    console.log(`T5 expected-presence candidates=[${candidates.join('|')}] present=[${present.join('|')}]`);

    expect(
      present.length,
      `Layer 7.8 redaction MUST be targeted not blanket. Success-path ProfilePreview ` +
      `for TEST_PHONE_REGISTERED="${supplied}" MUST include some canonical form of ` +
      `the phone in primary_phone (cirs_lookup.rs L92, L407). None found in body. ` +
      `If the structural fix over-redacted, T1-T4 would still pass but T5 fails — ` +
      `that is the regression class T5 guards.`,
    ).toBeGreaterThan(0);
  });
});
