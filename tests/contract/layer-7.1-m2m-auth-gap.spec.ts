/**
 * Layer 7.1 acceptance test: racecontrol direct M2M paths — PACT-026 §A carve-outs 1+2+3.
 *
 * V-LBAC §S-219 iter4 bono-leg · sub-class FALSE-SUCCESS-REPORT recovery + untracked-file
 * live-sync clobber recovery (re-Written from main-thread context after working-tree wipe).
 *
 * V2-PROGRESS-MAP Layer 7 row 7.1 (auth-gap):
 *   "racecontrol direct M2M paths (PACT-026 §A 1+2+3 carve-outs) — class=auth-gap;
 *    closure_phase=Post-V2.0-AUTH-Sprint; customer-impact=NO (staff-internal)."
 *
 * Anchors:
 *   - PACT-20260504-026 §A NO-direct-heart-narrow-carve-out (3 classes 1+2+3)
 *   - security-debt-ledger row 1: closure_phase=Post-V2.0-AUTH-Sprint;
 *     closure_constraints "must-hot-swap; dual-validate-then-enforce"
 *   - Captain Q2 2026-05-05 verbatim "Open by default but flagged to close later"
 *   - V-LBAC §S-219 iter4 (composes-with james §S-219 rows 7.3+7.6+7.8)
 *
 * 3-carve-out class taxonomy (PACT-026 §A §2.2 verbatim):
 *   Class 1 - Diagnostic reads (MI/HALO read-only health probes)
 *   Class 2 - Emergency control commands (rc-agent restart, pod-state reset)
 *   Class 3 - Time-critical telemetry writes (sub-second lap-time, pod-state events)
 *   NOT carved-out (HARD): customer billing / wallet / staff actions / commercial
 *   config / pricing rule changes.
 *
 * V1 substrate empirical (2026-05-13):
 *   routes.rs L167-217 public_routes() registers GET /fleet/health, POST /sentry/crash,
 *   POST /fleet/blocked-start, POST /fleet/alert without auth.
 *   fleet_alert.rs L46-61 has PARTIAL ENFORCEMENT: `if Some(key) && !empty` guard.
 *   survival.rs L227-230 is the correct closed-state reference (mandatory X-Service-Key).
 *
 * ANTI-PATTERN GUARD: V2 ships these 3 M2M classes WITH V1 trust intact per
 * Open-by-Default-Flagged-to-Close doctrine. Tests document gap, NOT enforce auth
 * pre-AUTH-Sprint. env-gate M2M_AUTH_ENFORCEMENT_ACTIVE controls observation vs assertion.
 *
 * Closure-phase target (Post-V2.0-AUTH-Sprint):
 *   - Canonical X-Service-Key extended to all carve-out classes
 *   - Drop conditional guard; fail-closed on missing config
 *   - Rotation primitive via config_push WS channel
 *   - audit_log row on every M2M call
 *   - Per-machine key allowlist
 *
 * 5 tests:
 *   T1 - Class 1 GET /fleet/health: OPEN today; 401-when-absent under enforcement
 *   T2 - Class 2 POST /sentry/crash + /fleet/blocked-start: OPEN; 401-when-absent
 *   T3 - Class 3 POST /fleet/alert: PARTIAL today; MANDATORY under closure-phase
 *   T4 - audit-trail invariant: every M2M call emits audit_log row
 *   T5 - service-key rotation invariant: rotation invalidates prior tokens
 *
 * Per CGP H3 "observations not verdicts": pre-enforcement branches use permissive
 * expect([...]).toContain(probe.status) with explicit console.log of observed status.
 *
 * G4 NOT TESTED (deferred):
 *   - Live AUTH-Sprint cutover (rotation + dual-validate + cutover)
 *   - Per-machine key allowlist
 *   - Audit_log row content beyond shape
 *   - F2 Feature Flag emergency-mode auto-toggle
 *   - MI auto-detection ≥3-pods-unreachable-60s threshold
 *   - Direct-heart-carve-out-audit.jsonl separate retention class
 *   - DPDP retention class for Class 1+2 events
 *   - PACT-022a panic-recovery wrapper round-trip
 *   - T2 /fleet/blocked-start positive 200 path (WhatsApp-alert noise)
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const M2M_AUTH_ENFORCEMENT_ACTIVE =
  process.env.M2M_AUTH_ENFORCEMENT_ACTIVE === 'true';
const TEST_VALID_SERVICE_KEY = process.env.TEST_VALID_SERVICE_KEY;
const TEST_PRIOR_SERVICE_KEY = process.env.TEST_PRIOR_SERVICE_KEY;
const TEST_AUDIT_LOG_REACHABLE = process.env.TEST_AUDIT_LOG_REACHABLE === 'true';
const AUDIT_LOG_PATH = process.env.AUDIT_LOG_PATH || '/api/v1/audit-log';

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not set - Layer 7.1 requires network reachability to ${CANONICAL_URL}.`,
  M2M_AUTH_NOT_ACTIVE: `M2M_AUTH_ENFORCEMENT_ACTIVE not set - PACT-026 §A carve-outs OPEN by default per security-debt-ledger row 1 (closure_phase=Post-V2.0-AUTH-Sprint).`,
  VALID_KEY_MISSING: `TEST_VALID_SERVICE_KEY not set - positive-case assertions cannot run without a known-good sentry_service_key value.`,
  PRIOR_KEY_MISSING: `TEST_PRIOR_SERVICE_KEY not set - T5 requires pre-rotation key value.`,
  AUDIT_LOG_MISSING: `TEST_AUDIT_LOG_REACHABLE not set - T4 requires audit_log substrate wired into M2M paths.`,
};

interface ProbeRead {
  url: string;
  method: 'GET' | 'POST';
  status: number;
  body_excerpt: string;
  read_started_at: number;
  read_finished_at: number;
}

interface AuditLogRow {
  id?: string | number;
  source_machine?: string;
  endpoint?: string;
  ts?: string;
  outcome?: string;
  [k: string]: unknown;
}

async function probe(
  method: 'GET' | 'POST',
  path: string,
  opts: { headers?: Record<string, string>; body?: unknown } = {},
): Promise<ProbeRead> {
  const started = Date.now();
  const url = `${CANONICAL_URL}${path}`;
  const init: RequestInit = {
    method,
    headers: opts.headers ?? {},
  };
  if (method === 'POST' && opts.body !== undefined) {
    init.body = typeof opts.body === 'string' ? opts.body : JSON.stringify(opts.body);
    (init.headers as Record<string, string>)['Content-Type'] =
      (init.headers as Record<string, string>)['Content-Type'] || 'application/json';
  }
  const resp = await fetch(url, init);
  const finished = Date.now();
  let body_excerpt = '';
  try {
    const text = await resp.text();
    body_excerpt = text.slice(0, 200);
  } catch {
    body_excerpt = '<no body>';
  }
  return {
    url,
    method,
    status: resp.status,
    body_excerpt,
    read_started_at: started,
    read_finished_at: finished,
  };
}

async function fetchAuditLog(
  filters: Record<string, string>,
): Promise<{ status: number; body: AuditLogRow[]; url: string }> {
  const qs = new URLSearchParams(filters).toString();
  const url = `${CANONICAL_URL}${AUDIT_LOG_PATH}?${qs}`;
  const headers: Record<string, string> = {};
  if (TEST_VALID_SERVICE_KEY) headers['X-Service-Key'] = TEST_VALID_SERVICE_KEY;
  const resp = await fetch(url, { headers });
  let body: AuditLogRow[] = [];
  try {
    const parsed = (await resp.json()) as AuditLogRow[] | { rows?: AuditLogRow[] };
    body = Array.isArray(parsed) ? parsed : (parsed.rows ?? []);
  } catch {
    body = [];
  }
  return { url, status: resp.status, body };
}

test.describe('Layer 7.1 - racecontrol direct M2M paths (PACT-026 §A 1+2+3 carve-outs)', () => {
  test('T1: Class 1 - diagnostic reads (GET /api/v1/fleet/health) - OPEN today; 401 under enforcement', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);

    const probeNoKey = await probe('GET', '/api/v1/fleet/health');
    console.log(`T1 probe-no-key: status=${probeNoKey.status} url=${probeNoKey.url}`);

    if (!M2M_AUTH_ENFORCEMENT_ACTIVE) {
      expect([200, 404, 500, 502, 503]).toContain(probeNoKey.status);
      console.log(`T1 observation: M2M_AUTH_ENFORCEMENT_ACTIVE not set; Class 1 OPEN status=${probeNoKey.status}.`);
      return;
    }

    expect(probeNoKey.status).toBe(401);
    test.skip(!TEST_VALID_SERVICE_KEY, SKIP_REASONS.VALID_KEY_MISSING);
    const probeWithKey = await probe('GET', '/api/v1/fleet/health', {
      headers: { 'X-Service-Key': TEST_VALID_SERVICE_KEY! },
    });
    expect(probeWithKey.status).toBe(200);
  });

  test('T2: Class 2 - emergency control (POST /api/v1/sentry/crash) - OPEN today; 401 under enforcement', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);

    const crashBody = {
      pod_id: 'pod-test-7.1',
      resolution_tier: 'observation',
      escalated: false,
      restart_count: 0,
      summary: 'Layer 7.1 acceptance test probe',
    };

    const probeNoKey = await probe('POST', '/api/v1/sentry/crash', { body: crashBody });
    console.log(`T2 probe-no-key: status=${probeNoKey.status}`);

    if (!M2M_AUTH_ENFORCEMENT_ACTIVE) {
      expect([200, 400, 422, 500, 502, 503]).toContain(probeNoKey.status);
      console.log(`T2 observation: Class 2 OPEN status=${probeNoKey.status}.`);
      return;
    }

    expect(probeNoKey.status).toBe(401);

    const blockedStartProbe = await probe('POST', '/api/v1/fleet/blocked-start', {
      body: { hostname: 'test-7.1', reason: 'acceptance test' },
    });
    expect(blockedStartProbe.status).toBe(401);

    test.skip(!TEST_VALID_SERVICE_KEY, SKIP_REASONS.VALID_KEY_MISSING);
    const probeWithKey = await probe('POST', '/api/v1/sentry/crash', {
      headers: { 'X-Service-Key': TEST_VALID_SERVICE_KEY! },
      body: crashBody,
    });
    expect([200, 202]).toContain(probeWithKey.status);
  });

  test('T3: Class 3 - telemetry writes (POST /api/v1/fleet/alert) - PARTIAL today; MANDATORY closure', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);

    const alertBody = {
      pod_id: 'pod-test-7.1',
      message: 'Layer 7.1 acceptance test probe',
      severity: 'info',
    };

    const probeNoKey = await probe('POST', '/api/v1/fleet/alert', { body: alertBody });
    console.log(`T3 probe-no-key: status=${probeNoKey.status}`);

    if (!M2M_AUTH_ENFORCEMENT_ACTIVE) {
      expect([401, 202, 429, 500, 502, 503]).toContain(probeNoKey.status);
      console.log(`T3 observation: Class 3 PARTIAL-enforcement status=${probeNoKey.status}.`);
      return;
    }

    expect(probeNoKey.status).toBe(401);
    test.skip(!TEST_VALID_SERVICE_KEY, SKIP_REASONS.VALID_KEY_MISSING);
    const probeWithKey = await probe('POST', '/api/v1/fleet/alert', {
      headers: { 'X-Service-Key': TEST_VALID_SERVICE_KEY! },
      body: alertBody,
    });
    expect([202, 429]).toContain(probeWithKey.status);
  });

  test('T4: audit-trail invariant - every M2M call emits audit_log row {source_machine, endpoint, ts, outcome}', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_AUDIT_LOG_REACHABLE, SKIP_REASONS.AUDIT_LOG_MISSING);
    test.skip(!M2M_AUTH_ENFORCEMENT_ACTIVE, SKIP_REASONS.M2M_AUTH_NOT_ACTIVE);
    test.skip(!TEST_VALID_SERVICE_KEY, SKIP_REASONS.VALID_KEY_MISSING);

    const t0 = Date.now();
    const headers = { 'X-Service-Key': TEST_VALID_SERVICE_KEY! };

    const c1 = await probe('GET', '/api/v1/fleet/health', { headers });
    const c2 = await probe('POST', '/api/v1/sentry/crash', {
      headers,
      body: {
        pod_id: 'pod-test-7.1',
        resolution_tier: 'observation',
        escalated: false,
        restart_count: 0,
        summary: 'T4 audit probe',
      },
    });
    const c3 = await probe('POST', '/api/v1/fleet/alert', {
      headers,
      body: { pod_id: 'pod-test-7.1', message: 'T4 audit probe', severity: 'info' },
    });
    console.log(`T4 probes: c1=${c1.status} c2=${c2.status} c3=${c3.status}`);

    const sinceIso = new Date(t0).toISOString();
    for (const ep of ['/api/v1/fleet/health', '/api/v1/sentry/crash', '/api/v1/fleet/alert']) {
      const audit = await fetchAuditLog({ endpoint: ep, since: sinceIso });
      expect(audit.status).toBe(200);
      const row = audit.body.find((r) => r.endpoint === ep);
      if (ep === '/api/v1/fleet/alert' && c3.status === 429) continue;
      expect(row).toBeDefined();
      expect(row!.source_machine).toBeDefined();
      expect(typeof row!.source_machine).toBe('string');
      expect(row!.endpoint).toBe(ep);
      expect(row!.ts).toBeDefined();
      expect(row!.outcome).toBeDefined();
      expect(['success', 'reject', 'rate_limited']).toContain(row!.outcome);
    }
  });

  test('T5: service-key rotation invariant - rotation invalidates prior tokens', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!M2M_AUTH_ENFORCEMENT_ACTIVE, SKIP_REASONS.M2M_AUTH_NOT_ACTIVE);
    test.skip(!TEST_VALID_SERVICE_KEY, SKIP_REASONS.VALID_KEY_MISSING);
    test.skip(!TEST_PRIOR_SERVICE_KEY, SKIP_REASONS.PRIOR_KEY_MISSING);

    const probePrior = await probe('GET', '/api/v1/fleet/health', {
      headers: { 'X-Service-Key': TEST_PRIOR_SERVICE_KEY! },
    });
    expect(probePrior.status).toBe(401);

    const probeCurrent = await probe('GET', '/api/v1/fleet/health', {
      headers: { 'X-Service-Key': TEST_VALID_SERVICE_KEY! },
    });
    expect(probeCurrent.status).toBe(200);

    const probePriorClass2 = await probe('POST', '/api/v1/sentry/crash', {
      headers: { 'X-Service-Key': TEST_PRIOR_SERVICE_KEY! },
      body: {
        pod_id: 'pod-test-7.1',
        resolution_tier: 'observation',
        escalated: false,
        restart_count: 0,
        summary: 'T5 rotation probe',
      },
    });
    expect(probePriorClass2.status).toBe(401);
  });
});
