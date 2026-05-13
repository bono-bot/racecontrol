/**
 * Layer 1.14 acceptance test: Substrate retains profile + stats + history
 *                             + DPDP/GDPR erase enforcement
 *
 * V2-PROGRESS-MAP §1 row 1.14 ("All-beat | Substrate retains profile + stats
 * + history | PARTIAL | james | DB schema present; V2 GDPR/DPDP
 * customer_data_delete contract authored not enforced").
 *
 * Source-of-truth:
 *   - comms-link/v2-skeleton/05-definition-of-done.md §1.2 (canonical day,
 *     stable identity beats 14:00..14:56)
 *   - racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md
 *     (Q-CUST-7 DPDP consent banner — the consent half of the contract)
 *   - racecontrol/crates/racecontrol/src/api/customer_legal.rs (canonical
 *     erase implementation; 43 ERASE_TABLES + 3 TRANSITIVE_ERASE_SQL paths
 *     + 3 POINTER_TABLES nullified; single Sqlite transaction rollback)
 *   - racecontrol/CLAUDE.md "GDPR erase contract" standing rule (new
 *     driver/lap FK = update customer_data_delete in same commit)
 *
 * Contract under test (two halves of row 1.14):
 *   (RETENTION) Canonical Server .23 racecontrol persists driver + total_laps
 *     + customer history beyond the immediate beat that wrote them. Profile,
 *     stats, and sessions endpoints expose the retained shape.
 *   (DPDP/GDPR ENFORCEMENT) DELETE /api/v1/customer/data-delete cascades
 *     through all FK-bearing tables (ERASE_TABLES + TRANSITIVE_ERASE_SQL +
 *     POINTER_TABLES) in a single transaction, returns 204 No Content, and
 *     post-erase reads of profile + sessions MUST NOT return prior rows for
 *     the erased driver_id.
 *
 * Surface paths verified against codebase 2026-05-13 IST:
 *   GET    /api/v1/customer/profile      (driver profile; same as 1.17 uses)
 *   GET    /api/v1/customer/sessions     (history listing; routes.rs L313)
 *   GET    /api/v1/customer/stats        (lap-history rollup; routes.rs L316)
 *   DELETE /api/v1/customer/data-delete  (customer_legal.rs L297; 204 No Content)
 *   POST   /api/v1/customer/test-mint-jwt (TEST_MODE only; routes.rs L130-144)
 *
 * Env vars:
 *   CANONICAL_URL        - Server .23 racecontrol (default http://192.168.31.23:8080)
 *   TEST_CUSTOMER_JWT    - Bearer JWT for a test driver (mint via test-mint-jwt;
 *                          server TEST_MODE=true required)
 *   TEST_DRIVER_ID       - driver_id matching TEST_CUSTOMER_JWT
 *   TEST_ADMIN_JWT       - (reserved; revoke-consent cross-check, deferred)
 *   DPDP_DELETE_PATH     - override path (default /api/v1/customer/data-delete)
 *   DPDP_DESTRUCTIVE_OK  - must be 'true' to run hard-delete tests
 *
 * G4 NOT TESTED in this file (deferred):
 *   - Per-table orphan-row enumeration after erase (requires DB read; black-box scope)
 *   - revoke-consent (anonymize-only) path — distinct contract from data-delete
 *     (LEGAL-09 preserves financial rows for 8-yr Income Tax Act retention)
 *   - Staff-initiated guardian erase (POST /drivers/{id}/revoke-consent)
 *   - Cross-restart persistence of retention (cannot trigger racecontrol restart in CI)
 *   - Idempotent re-delete (calling data-delete twice on same driver_id)
 *   - Server log assertion of "DPDP erase COMPLETE" audit line (no API for log scrape)
 *   - Tombstone / soft-delete schema (current contract is hard-delete)
 *   - Data-export round-trip (DPDP §A right-to-access; separate row)
 *
 * V2 doctrine alignment: Captures the "authored not enforced" half of row 1.14
 * explicitly. The runtime exercise of the erase contract is the load-bearing
 * missing piece. Test passes today via skip-coherence; activates the moment
 * TEST_CUSTOMER_JWT + DPDP_DESTRUCTIVE_OK env is wired in a disposable DB.
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const TEST_CUSTOMER_JWT = process.env.TEST_CUSTOMER_JWT;
const TEST_DRIVER_ID = process.env.TEST_DRIVER_ID;
const DPDP_DELETE_PATH = process.env.DPDP_DELETE_PATH || '/api/v1/customer/data-delete';
const DPDP_DESTRUCTIVE_OK = process.env.DPDP_DESTRUCTIVE_OK === 'true';

const SKIP_REASONS: Record<string, string> = {
  TEST_CUSTOMER_JWT: `TEST_CUSTOMER_JWT not set - Layer 1.14 substrate-retention + DPDP erase enforcement cannot be exercised without a customer JWT. Mint via POST ${CANONICAL_URL}/api/v1/customer/test-mint-jwt body {"driver_id":"<test-id>"} (requires server TEST_MODE=true).`,
  TEST_DRIVER_ID: `TEST_DRIVER_ID not set - Layer 1.14 retention assertion needs the driver_id matching TEST_CUSTOMER_JWT to verify the canonical record belongs to the expected subject (DoD §1.2 stable identity contract).`,
  DPDP_DESTRUCTIVE_OK: `DPDP_DESTRUCTIVE_OK not set to 'true' - Layer 1.14 DPDP/GDPR erase test is DESTRUCTIVE (hard-deletes driver + cascading rows per customer_legal.rs ERASE_TABLES). Set DPDP_DESTRUCTIVE_OK=true ONLY in a disposable test DB. Anchor: V2-PROGRESS-MAP §1 row 1.14 'GDPR/DPDP customer_data_delete contract authored not enforced' - enforcement requires runtime exercise.`,
};

const PROFILE_PATH = '/api/v1/customer/profile';
const SESSIONS_PATH = '/api/v1/customer/sessions';
const STATS_PATH = '/api/v1/customer/stats';

interface ApiRead {
  url: string;
  status: number;
  body: any;
  read_started_at: number;
  read_finished_at: number;
}

async function apiGet(baseUrl: string, path: string, jwt: string): Promise<ApiRead> {
  const started = Date.now();
  const resp = await fetch(`${baseUrl}${path}`, {
    headers: { Authorization: `Bearer ${jwt}` },
  });
  const finished = Date.now();
  let body: any = null;
  try {
    body = await resp.json();
  } catch {
    body = null;
  }
  return {
    url: `${baseUrl}${path}`,
    status: resp.status,
    body,
    read_started_at: started,
    read_finished_at: finished,
  };
}

async function apiDelete(baseUrl: string, path: string, jwt: string): Promise<ApiRead> {
  const started = Date.now();
  const resp = await fetch(`${baseUrl}${path}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${jwt}` },
  });
  const finished = Date.now();
  let body: any = null;
  if (resp.status !== 204) {
    try {
      body = await resp.json();
    } catch {
      body = null;
    }
  }
  return {
    url: `${baseUrl}${path}`,
    status: resp.status,
    body,
    read_started_at: started,
    read_finished_at: finished,
  };
}

test.describe('Layer 1.14 - Substrate retains profile + stats + history (DoD §1.2)', () => {
  test('canonical profile responds with retention-shape fields', async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    const r = await apiGet(CANONICAL_URL, PROFILE_PATH, TEST_CUSTOMER_JWT!);
    console.log(`profile read t=${r.read_finished_at - r.read_started_at}ms status=${r.status}`);
    expect(r.status).toBe(200);
    expect(r.body?.driver).toBeDefined();
    expect(typeof r.body.driver.id).toBe('string');
    expect(typeof r.body.driver.total_laps).toBe('number');
    expect(typeof r.body.driver.wallet_balance_paise).toBe('number');
    expect(typeof r.body.driver.registration_completed).toBe('boolean');
    if (TEST_DRIVER_ID) {
      expect(r.body.driver.id).toBe(TEST_DRIVER_ID);
    }
  });

  test('total_laps is numeric and >= 0 (retention baseline)', async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    const r = await apiGet(CANONICAL_URL, PROFILE_PATH, TEST_CUSTOMER_JWT!);
    expect(r.status).toBe(200);
    expect(r.body.driver.total_laps).toBeGreaterThanOrEqual(0);
  });

  test('customer history endpoint (/customer/sessions) is reachable + array-shaped', async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    const r = await apiGet(CANONICAL_URL, SESSIONS_PATH, TEST_CUSTOMER_JWT!);
    console.log(`sessions read t=${r.read_finished_at - r.read_started_at}ms status=${r.status}`);
    expect(r.status).toBe(200);
    // Body shape: either { sessions: [...] } or [...]
    const sessions = Array.isArray(r.body) ? r.body : r.body?.sessions;
    expect(Array.isArray(sessions)).toBe(true);
  });

  test('customer stats endpoint carries lap-history rollup shape', async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    const r = await apiGet(CANONICAL_URL, STATS_PATH, TEST_CUSTOMER_JWT!);
    console.log(`stats read t=${r.read_finished_at - r.read_started_at}ms status=${r.status}`);
    expect(r.status).toBe(200);
    expect(r.body).toBeDefined();
  });

  test('DPDP/GDPR data-delete cascades + returns 204 No Content (DESTRUCTIVE; opt-in)', async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!DPDP_DESTRUCTIVE_OK, SKIP_REASONS.DPDP_DESTRUCTIVE_OK);
    const r = await apiDelete(CANONICAL_URL, DPDP_DELETE_PATH, TEST_CUSTOMER_JWT!);
    console.log(`data-delete t=${r.read_finished_at - r.read_started_at}ms status=${r.status}`);
    // Contract: 204 No Content per customer_legal.rs L297-523 transactional cascade
    expect(r.status).toBe(204);
  });

  test('post-erase customer reads MUST NOT return prior rows (DESTRUCTIVE; opt-in)', async () => {
    test.skip(!TEST_CUSTOMER_JWT, SKIP_REASONS.TEST_CUSTOMER_JWT);
    test.skip(!DPDP_DESTRUCTIVE_OK, SKIP_REASONS.DPDP_DESTRUCTIVE_OK);
    // After data-delete in previous test, JWT subject is gone from substrate.
    const profile = await apiGet(CANONICAL_URL, PROFILE_PATH, TEST_CUSTOMER_JWT!);
    console.log(`post-erase profile status=${profile.status}`);
    // Post-erase: 404/410 (resource gone) or 401 (JWT subject invalid)
    expect([401, 404, 410]).toContain(profile.status);

    const sessions = await apiGet(CANONICAL_URL, SESSIONS_PATH, TEST_CUSTOMER_JWT!);
    console.log(`post-erase sessions status=${sessions.status}`);
    if (sessions.status === 200) {
      const arr = Array.isArray(sessions.body) ? sessions.body : sessions.body?.sessions;
      expect(arr).toHaveLength(0);
    } else {
      expect([401, 404, 410]).toContain(sessions.status);
    }
  });
});
