/**
 * Layer 1.17 acceptance test: Cross-surface consistency (2s tolerance)
 *
 * V2-PROGRESS-MAP §1 row 1.17 ("All-beat | Cross-surface consistency
 * (2s tolerance) | NOT-STARTED | both | acceptance test missing").
 *
 * Source-of-truth:
 *   - comms-link/v2-skeleton/05-definition-of-done.md §1.2 (canonical day)
 *   - racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md
 *   - V2 canonical-source ledger doctrine: Server .23 racecontrol is the
 *     authoritative store for customer state; surfaces (PWA / POS / Kiosk)
 *     are read replicas that MUST reflect canonical within 2s propagation.
 *
 * Contract under test:
 *   For a given customer C and the canonical record on Server .23
 *   (racecontrol /api/v1/customer/profile), every deployed surface S
 *   serving customer-facing UI (PWA / POS / Kiosk) MUST return the same
 *   stable fields for C, with a read-propagation tolerance of <= 2 seconds.
 *
 * Stable fields compared (volatile fields like updated_at are excluded):
 *   - driver.id
 *   - driver.customer_id
 *   - driver.name
 *   - driver.wallet_balance_paise   <- primary cross-surface concern
 *   - driver.total_laps
 *   - driver.registration_completed
 *
 * Surface-deployment status (Layer 1 row map at 2026-05-12):
 *   1.5  PWA   DONE     https://v2.racingpoint.cloud (Bono VPS pm2 racingpoint-web-v2)
 *   1.6  POS   BLOCKED  Layer 1.6 - depends on V2 staff session-cookie auth + JWT
 *   1.10 Kiosk NOT-STARTED  Layer 1.10 - V2 Kiosk staff UI does not exist yet
 *
 * Each surface check is env-gated. When the corresponding env var is undefined,
 * the check skips with an explicit reason so the gap is visible in the run
 * output. The test PASSES when all deployed surfaces match canonical within
 * the tolerance; skips do not fail the suite.
 *
 * Env vars:
 *   CANONICAL_URL      - Server .23 racecontrol (default http://192.168.31.23:8080)
 *   PWA_URL            - racingpoint.cloud PWA root (e.g. https://v2.racingpoint.cloud)
 *   POS_URL            - POS PC web app root (e.g. http://192.168.31.130:3200)
 *   KIOSK_URL          - Kiosk root (e.g. http://192.168.31.23:3300)
 *   TEST_CUSTOMER_JWT  - Bearer JWT for a test driver
 *                        Mint via: POST {CANONICAL_URL}/api/v1/customer/test-mint-jwt
 *                        body: {"driver_id":"<test-driver-id>"} (TEST_MODE only)
 *   TEST_DRIVER_ID     - driver_id matching TEST_CUSTOMER_JWT (used for assertion)
 *   READ_TOLERANCE_MS  - read-propagation tolerance in ms (default 2000)
 *
 * G4 NOT TESTED in this file (deferred):
 *   - Real-time push consistency via WebSocket subscriptions
 *   - Cross-surface write propagation (write on PWA, read on POS within 2s)
 *   - Surface-to-surface direct comparison (we compare each -> canonical only)
 *   - Auth-failure paths on each surface (covered by per-surface contract tests)
 *   - Schema-drift detection across surfaces (covered by openapi.yaml conformance)
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const PWA_URL = process.env.PWA_URL;
const POS_URL = process.env.POS_URL;
const KIOSK_URL = process.env.KIOSK_URL;
const TEST_CUSTOMER_JWT = process.env.TEST_CUSTOMER_JWT;
const TEST_DRIVER_ID = process.env.TEST_DRIVER_ID || '<unset>';
const READ_TOLERANCE_MS = parseInt(process.env.READ_TOLERANCE_MS || '2000', 10);

const SKIP_REASONS: Record<string, string> = {
  PWA_URL: `PWA_URL not set - Layer 1.5 frontpage is DONE on Bono VPS but customer-state proxy not under test until env is wired`,
  POS_URL: `POS_URL not set - Layer 1.6 POS V2 customer state is BLOCKED on staff session-cookie auth (PACT-001 Phase 1 cookie SHIPPED; V2 staff JWT pending)`,
  KIOSK_URL: `KIOSK_URL not set - Layer 1.10 V2 Kiosk staff UI is NOT-STARTED (Joint #3 Game Launching full spec pending)`,
  TEST_CUSTOMER_JWT: `TEST_CUSTOMER_JWT not set - mint via POST ${CANONICAL_URL}/api/v1/customer/test-mint-jwt with body {"driver_id":"<test-id>"} in TEST_MODE`,
};

const PROFILE_PATH = '/api/v1/customer/profile';

interface ProfileDriver {
  id: string;
  customer_id: string | null;
  name: string;
  wallet_balance_paise: number;
  total_laps: number;
  registration_completed: boolean;
}

interface ProfileResponse {
  driver?: ProfileDriver;
  error?: string;
}

interface SurfaceRead {
  surface: string;
  url: string;
  status: number;
  body: ProfileResponse;
  read_started_at: number;
  read_finished_at: number;
}

async function fetchProfile(baseUrl: string, jwt: string, surface: string): Promise<SurfaceRead> {
  const started = Date.now();
  const resp = await fetch(`${baseUrl}${PROFILE_PATH}`, {
    headers: { Authorization: `Bearer ${jwt}` },
  });
  const finished = Date.now();
  const body = (await resp.json()) as ProfileResponse;
  return {
    surface,
    url: `${baseUrl}${PROFILE_PATH}`,
    status: resp.status,
    body,
    read_started_at: started,
    read_finished_at: finished,
  };
}

function stableFields(r: ProfileResponse) {
  if (!r.driver) return null;
  return {
    id: r.driver.id,
    customer_id: r.driver.customer_id,
    name: r.driver.name,
    wallet_balance_paise: r.driver.wallet_balance_paise,
    total_laps: r.driver.total_laps,
    registration_completed: r.driver.registration_completed,
  };
}

async function compareSurfaceToCanonical(
  surfaceName: string,
  surfaceUrl: string,
  jwt: string,
  toleranceMs: number,
): Promise<void> {
  const canonical = await fetchProfile(CANONICAL_URL, jwt, 'canonical');
  const surface = await fetchProfile(surfaceUrl, jwt, surfaceName);

  console.log(
    `${surfaceName} read t=${surface.read_finished_at - surface.read_started_at}ms ` +
    `status=${surface.status} ` +
    `delta_from_canonical=${surface.read_finished_at - canonical.read_started_at}ms`,
  );

  expect(canonical.status).toBe(200);
  expect(surface.status).toBe(200);

  const cFields = stableFields(canonical.body);
  const sFields = stableFields(surface.body);
  expect(cFields).not.toBeNull();
  expect(sFields).not.toBeNull();
  expect(sFields).toEqual(cFields);

  // Tolerance: surface read finishes within toleranceMs of canonical
  // read STARTING. Catches stale-cache + slow-proxy classes both.
  const propagation_ms = surface.read_finished_at - canonical.read_started_at;
  expect(propagation_ms).toBeLessThanOrEqual(toleranceMs);
}

test.describe('Layer 1.17 - Cross-surface consistency (2s tolerance)', () => {
  test('canonical surface (Server .23 racecontrol) responds with profile shape', async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    const r = await fetchProfile(CANONICAL_URL, TEST_CUSTOMER_JWT!, 'canonical');
    console.log(`canonical read t=${r.read_finished_at - r.read_started_at}ms status=${r.status}`);
    expect(r.status).toBe(200);
    expect(r.body.driver).toBeDefined();
    const fields = stableFields(r.body);
    expect(fields).not.toBeNull();
    expect(typeof fields!.id).toBe('string');
    expect(typeof fields!.wallet_balance_paise).toBe('number');
    expect(typeof fields!.total_laps).toBe('number');
    expect(typeof fields!.registration_completed).toBe('boolean');
    if (TEST_DRIVER_ID !== '<unset>') {
      expect(fields!.id).toBe(TEST_DRIVER_ID);
    }
  });

  test(`PWA: matches canonical state within tolerance`, async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!PWA_URL, SKIP_REASONS.PWA_URL);
    await compareSurfaceToCanonical('PWA', PWA_URL!, TEST_CUSTOMER_JWT!, READ_TOLERANCE_MS);
  });

  test(`POS: matches canonical state within tolerance`, async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!POS_URL, SKIP_REASONS.POS_URL);
    await compareSurfaceToCanonical('POS', POS_URL!, TEST_CUSTOMER_JWT!, READ_TOLERANCE_MS);
  });

  test(`Kiosk: matches canonical state within tolerance`, async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!KIOSK_URL, SKIP_REASONS.KIOSK_URL);
    await compareSurfaceToCanonical('Kiosk', KIOSK_URL!, TEST_CUSTOMER_JWT!, READ_TOLERANCE_MS);
  });

  test('cross-surface read times converge within tolerance window', async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);

    const deployed = [
      { name: 'PWA', url: PWA_URL },
      { name: 'POS', url: POS_URL },
      { name: 'Kiosk', url: KIOSK_URL },
    ].filter((s): s is { name: string; url: string } => Boolean(s.url));

    test.skip(
      deployed.length < 2,
      `<2 deployed surfaces (have ${deployed.length}: ${deployed.map(s => s.name).join(',') || 'none'}). Test activates when >=2 surface URL env vars are set.`,
    );

    const reads = await Promise.all(
      deployed.map(s => fetchProfile(s.url, TEST_CUSTOMER_JWT!, s.name)),
    );
    const finishes = reads.map(r => r.read_finished_at);
    const spread = Math.max(...finishes) - Math.min(...finishes);
    console.log(
      `convergence spread=${spread}ms across ${deployed.map(s => s.name).join(',')}`,
    );
    expect(spread).toBeLessThanOrEqual(READ_TOLERANCE_MS);

    // All deployed surfaces agree on stable fields
    const allFields = reads.map(r => stableFields(r.body));
    for (let i = 1; i < allFields.length; i++) {
      expect(allFields[i]).toEqual(allFields[0]);
    }
  });
});
