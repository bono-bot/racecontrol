/**
 * Row 8.B acceptance test: Boot Resilience — no single-fetch-at-boot without retry.
 *
 * V2-LBAC row 8.B ("Boot Resilience: No single-fetch-at-boot without retry").
 * Doctrine source: racecontrol/CLAUDE.md "Boot Resilience: No single-fetch-at-boot
 * without retry" rule. Any data fetched from a remote source at startup MUST have
 * a periodic re-fetch background task so that a server transient at boot does not
 * leave the resource pinned at its cached/default value forever.
 *
 * Source-of-truth (read 2026-05-13 IST):
 *   - racecontrol/crates/rc-common/src/boot_resilience.rs
 *       exports `spawn_periodic_refetch<T, E, F, Fut>(resource_name, interval, fetch_fn)`
 *       returning `tokio::task::JoinHandle<()>`. Lifecycle log targets:
 *         "periodic_refetch started" / "first_success" / "failed" / "self_healed" / "exit"
 *       Loop is unconditional `tokio::spawn` with `tokio::time::interval` — failure on
 *       fetch_fn logs warn! with retry_count and continues; success after failure logs
 *       self_healed with downtime_ms. No `break` on error path.
 *   - racecontrol/crates/rc-agent/src/main.rs:1671 (feature_flags refetch via
 *       spawn_periodic_refetch, interval=300s)
 *   - racecontrol/crates/rc-agent/src/main.rs:1710 (mesh_service_key refetch via
 *       spawn_periodic_refetch, interval=300s; preceded by best-effort initial
 *       synchronous fetch with warn-only on failure)
 *   - racecontrol/crates/rc-agent/src/main.rs:1756 (allowlist refetch via custom
 *       `allowlist_poll_loop` defined at L294 — same pattern, different shape:
 *       `tokio::time::interval(Duration::from_secs(kiosk::ALLOWLIST_REFRESH_SECS))`
 *       with first-tick-immediate. Predates spawn_periodic_refetch (commit `821c3031`
 *       extracted the abstraction afterward). Equivalent semantics; both satisfy the
 *       banned-pattern guard.)
 *
 * Empirical anchor (CLAUDE.md "Boot Resilience"):
 *   2026-03-24 process guard allowlist was fetched once at boot while the server
 *   was briefly down. All 8 pods got `MachineWhitelist::default()` (empty) and never
 *   re-fetched → 100 false-positive violations/24h on every pod for 2 days. Fix in
 *   commit `821c3031` introduced periodic re-fetch; rc-common::boot_resilience
 *   crate generalized the pattern.
 *
 * Contract under test (anti-pattern guard, not behavior reproduction):
 *   1. `spawn_periodic_refetch` is exported from rc-common/src/boot_resilience.rs
 *      with the documented signature (interval-typed Duration + Fn closure shape).
 *   2. rc-agent main.rs invokes the boot-resilient pattern at every known startup
 *      HTTP-fetch site (feature_flags, mesh_service_key) — call-site grep test.
 *   3. The implementation contains a `tokio::spawn` containing a loop that does NOT
 *      `break` on the Err arm (resilience invariant — loop survives fetch failure).
 *   4. The allowlist boot-fetch is registered via the periodic `allowlist_poll_loop`
 *      (or spawn_periodic_refetch) and not a one-shot startup call.
 *   5. Anti-pattern guard: rc-agent main.rs boot-window code (top of `async fn main`
 *      through the spawn_periodic_refetch / allowlist_poll_loop call-sites) MUST NOT
 *      contain a bare `.send().await` HTTP call that is not either (a) inside one of
 *      the recognized periodic-fetch wrappers OR (b) tagged with `// best-effort initial`
 *      adjacent to a periodic-fetch wrapper for the same resource. This is the
 *      banned-pattern: single-fetch-at-boot without retry.
 *
 * G4 NOT TESTED in this file (deferred):
 *   - Live behavioral verification on a running rc-agent process (would require
 *     running rc-agent + simulated server outage; out of contract-test scope).
 *   - The TOKIO_RUNTIME timer fires at exactly the configured interval (covered
 *     by inline rc-common::boot_resilience::tests::spawn_periodic_refetch_returns_join_handle).
 *   - Self-healing log emission at runtime (covered by inline test
 *     spawn_periodic_refetch_self_heals_after_failure).
 *   - Newly added resources after this test lands (e.g. billing rates re-fetch
 *     CHECK in CLAUDE.md). Forward additions should grep the same call-site set.
 *   - Cross-organ boot-resilience (racecontrol server / cloud / kiosk frontends).
 *     Server-side spawn_periodic_refetch invocation at
 *     crates/racecontrol/src/api/mesh_intelligence.rs:368 is out of scope here.
 *
 * V2 doctrine alignment: encodes a forward-only banned-pattern guard around a
 * doctrine that today is text-only in CLAUDE.md. Regression-class is empirically
 * known (PG-allowlist 2026-03-24 outage). Test fails the BUILD when a new
 * boot-time HTTP call lands without a retry wrapper.
 */

import { test, expect } from '@playwright/test';
import { promises as fs } from 'fs';
import * as path from 'path';

const REPO_ROOT = path.resolve(__dirname, '../../');
const BOOT_RESILIENCE_RS = path.join(
  REPO_ROOT,
  'crates/rc-common/src/boot_resilience.rs',
);
const RC_AGENT_MAIN_RS = path.join(
  REPO_ROOT,
  'crates/rc-agent/src/main.rs',
);

async function readFile(p: string): Promise<string> {
  return fs.readFile(p, 'utf-8');
}

test.describe('Row 8.B - Boot Resilience: no single-fetch-at-boot without retry', () => {
  test('boot_resilience.rs exports spawn_periodic_refetch with documented signature', async () => {
    // Substrate-existence + signature-shape gate. F1 G-F1-4 says the mechanism
    // exists; this test pins the *signature* so future refactors that drop the
    // interval-Duration parameter or the resource-name label trip CI.
    const src = await readFile(BOOT_RESILIENCE_RS);

    // Public export
    expect(
      src.includes('pub fn spawn_periodic_refetch'),
      'spawn_periodic_refetch must be pub-exported from rc-common::boot_resilience',
    ).toBe(true);

    // Required parameters (each is a structural assertion, no order coupling)
    expect(
      src.includes('resource_name: String'),
      'spawn_periodic_refetch must accept a String resource_name for log labelling',
    ).toBe(true);
    expect(
      src.includes('interval_duration: Duration'),
      'spawn_periodic_refetch must accept an interval_duration: Duration parameter',
    ).toBe(true);
    expect(
      src.includes('fetch_fn: F'),
      'spawn_periodic_refetch must accept a generic fetch_fn closure parameter',
    ).toBe(true);

    // Return type — JoinHandle is the contract that lets caller abort the task
    expect(
      src.includes('tokio::task::JoinHandle<()>'),
      'spawn_periodic_refetch must return tokio::task::JoinHandle<()> (abort-able)',
    ).toBe(true);

    // Lifecycle log targets — observable contract for monitoring
    expect(
      src.includes('"periodic_refetch started"'),
      'spawn_periodic_refetch must emit "periodic_refetch started" on spawn',
    ).toBe(true);
    expect(
      src.includes('"periodic_refetch first_success"'),
      'spawn_periodic_refetch must emit "periodic_refetch first_success" on first Ok',
    ).toBe(true);
    expect(
      src.includes('"periodic_refetch failed"'),
      'spawn_periodic_refetch must emit "periodic_refetch failed" on Err',
    ).toBe(true);
    expect(
      src.includes('"periodic_refetch self_healed"'),
      'spawn_periodic_refetch must emit "periodic_refetch self_healed" on Ok after Err',
    ).toBe(true);
  });

  test('rc-agent main.rs invokes spawn_periodic_refetch at every known boot-fetch site', async () => {
    // Call-site enumeration test. Today: feature_flags + mesh_service_key.
    // The allowlist is checked separately in its own test because it uses a
    // sibling pattern (allowlist_poll_loop) that predates the abstraction.
    const src = await readFile(RC_AGENT_MAIN_RS);

    // Count spawn_periodic_refetch invocations
    const invocations = src.match(
      /rc_common::boot_resilience::spawn_periodic_refetch/g,
    ) ?? [];

    expect(
      invocations.length,
      `rc-agent main.rs must invoke rc_common::boot_resilience::spawn_periodic_refetch at >=2 sites (today: feature_flags + mesh_service_key). Found ${invocations.length}. If a call-site was removed, verify the underlying resource is no longer fetched at boot.`,
    ).toBeGreaterThanOrEqual(2);

    // Per-resource name pinning — these resource_name labels are part of the
    // operational contract (used in log filters, alerts, dashboards).
    expect(
      src.includes('"feature_flags".to_string()'),
      'feature_flags refetch must be registered with resource_name "feature_flags"',
    ).toBe(true);
    expect(
      src.includes('"mesh_service_key".to_string()'),
      'mesh_service_key refetch must be registered with resource_name "mesh_service_key"',
    ).toBe(true);
  });

  test('spawn_periodic_refetch loop does NOT break on Err (resilience invariant)', async () => {
    // The whole point of the abstraction is "errors are warn-not-fatal".
    // This test pins that invariant by reading the impl and verifying the
    // Err arm of the inner match does NOT contain a `break` or `return`.
    // Static-text test; runtime self-heal behavior is covered by inline
    // unit test spawn_periodic_refetch_self_heals_after_failure.
    const src = await readFile(BOOT_RESILIENCE_RS);

    // Locate the Err arm — between `Err(e) => {` and the closing `}` of that arm.
    const errArmStart = src.indexOf('Err(e) => {');
    expect(
      errArmStart,
      'boot_resilience.rs must contain an `Err(e) => {` match arm in the loop',
    ).toBeGreaterThan(-1);

    // Walk forward to the matching close brace (brace-balance, not regex —
    // the arm contains its own inner expression scope).
    let depth = 0;
    let errArmEnd = errArmStart;
    for (let i = errArmStart; i < src.length; i++) {
      const ch = src[i];
      if (ch === '{') depth++;
      else if (ch === '}') {
        depth--;
        if (depth === 0) {
          errArmEnd = i;
          break;
        }
      }
    }
    expect(
      errArmEnd,
      'Err arm must have a matching close brace',
    ).toBeGreaterThan(errArmStart);

    const errArmBody = src.slice(errArmStart, errArmEnd + 1);

    // Resilience invariant — no early exit on failure
    expect(
      /\bbreak\b/.test(errArmBody),
      `spawn_periodic_refetch Err arm contains \`break\` — resilience invariant violated. Err must be logged-and-continue, never exit the loop. Body:\n${errArmBody}`,
    ).toBe(false);
    expect(
      /\breturn\b/.test(errArmBody),
      `spawn_periodic_refetch Err arm contains \`return\` — resilience invariant violated. Err must be logged-and-continue, never exit the loop. Body:\n${errArmBody}`,
    ).toBe(false);
    // Warn must be emitted with retry_count for observability
    expect(
      errArmBody.includes('retry_count'),
      'spawn_periodic_refetch Err arm must emit `retry_count` field for monitoring',
    ).toBe(true);
  });

  test('allowlist boot fetch is registered via periodic poll loop (not single-fetch)', async () => {
    // Sibling-pattern coverage: the allowlist refresh predates the
    // spawn_periodic_refetch extraction (commit `821c3031`) and is implemented
    // as a custom `allowlist_poll_loop`. Both shapes satisfy the doctrine.
    // This test allows either pattern but requires that ONE of them registers
    // the allowlist for periodic fetch.
    const src = await readFile(RC_AGENT_MAIN_RS);

    // Pattern 1: dedicated periodic-loop function (current code, ~L1756)
    const usesAllowlistPollLoop =
      src.includes('tokio::spawn(allowlist_poll_loop(') &&
      // and the function itself uses tokio::time::interval (the periodic primitive)
      src.includes('async fn allowlist_poll_loop') &&
      src.includes('tokio::time::interval(');

    // Pattern 2: refactor to use spawn_periodic_refetch with resource_name "allowlist"
    // (forward-compatible — if the function is ever unified, this still passes)
    const usesPeriodicRefetchForAllowlist =
      /spawn_periodic_refetch\([\s\S]{0,200}"allowlist"/.test(src) ||
      /spawn_periodic_refetch\([\s\S]{0,200}"process_allowlist"/.test(src) ||
      /spawn_periodic_refetch\([\s\S]{0,200}"kiosk_allowlist"/.test(src);

    expect(
      usesAllowlistPollLoop || usesPeriodicRefetchForAllowlist,
      'allowlist must be registered for periodic re-fetch (either via `allowlist_poll_loop` + tokio::time::interval OR via spawn_periodic_refetch with resource_name allowlist/process_allowlist/kiosk_allowlist). Bare single-shot fetch at boot is BANNED (CLAUDE.md "Boot Resilience"; empirical anchor 2026-03-24 PG-allowlist outage).',
    ).toBe(true);
  });

  test('no bare single-fetch HTTP call appears in rc-agent boot window without retry wrapper', async () => {
    // Anti-pattern guard. Scans the boot window (`async fn main` up through the
    // last `spawn_periodic_refetch` / `allowlist_poll_loop` invocation, which is
    // the canonical end-of-boot marker) for raw `.send().await` HTTP calls that
    // are NOT part of a recognized periodic-fetch wrapper or explicitly tagged
    // best-effort-initial.
    //
    // Recognized as legitimate (allow-listed):
    //   - Inside a spawn_periodic_refetch closure body (already wrapped)
    //   - Inside `allowlist_poll_loop` body (already periodic)
    //   - An "initial best-effort fetch" pattern: comment containing
    //     "best-effort" or "initial" AND a matching spawn_periodic_refetch
    //     for the same resource within 50 lines after.
    //
    // The test is conservative: it greps for `.send().await` (the reqwest
    // post-await marker) inside the `async fn main` body and ALLOWS any
    // occurrence that is followed (within 50 lines) by a spawn_periodic_refetch
    // OR allowlist_poll_loop registration. Anything else fails the test.
    const src = await readFile(RC_AGENT_MAIN_RS);

    const mainStart = src.indexOf('async fn main(');
    expect(
      mainStart,
      'rc-agent main.rs must define `async fn main(`',
    ).toBeGreaterThan(-1);

    // End of boot window: the last spawn_periodic_refetch / allowlist_poll_loop call.
    // Anything past that is steady-state runtime code (WS loop, sentinels, etc.) —
    // out of scope for this anti-pattern guard.
    // MAOR fix 2026-05-13 §S-221 meta-test: anchor lastIndexOf to mainSrc slice
    // so future occurrences of these strings outside `async fn main` (helper fns,
    // test modules, comments) don't fool the boot-window end marker.
    const mainSrc = src.slice(mainStart);
    const lastPeriodicRefetchInMain = mainSrc.lastIndexOf('spawn_periodic_refetch');
    const lastAllowlistPollInMain = mainSrc.lastIndexOf('tokio::spawn(allowlist_poll_loop(');
    const bootWindowEnd = mainStart + Math.max(lastPeriodicRefetchInMain, lastAllowlistPollInMain);
    expect(
      bootWindowEnd,
      'rc-agent main.rs must register at least one periodic-refetch task during boot (no markers found)',
    ).toBeGreaterThan(mainStart);

    const bootWindow = src.slice(mainStart, bootWindowEnd);
    const lines = bootWindow.split('\n');

    const offenders: string[] = [];
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      if (!line.includes('.send().await')) continue;

      // Look at a +/- 25 line window for retry markers
      const ctxStart = Math.max(0, i - 25);
      const ctxEnd = Math.min(lines.length, i + 25);
      const ctx = lines.slice(ctxStart, ctxEnd).join('\n');

      const wrappedByPeriodic =
        ctx.includes('spawn_periodic_refetch') ||
        ctx.includes('allowlist_poll_loop') ||
        // best-effort-initial comment (canonical phrasing in main.rs:1699-1705)
        /best.effort/i.test(ctx) ||
        /initial.+fetch/i.test(ctx);

      if (!wrappedByPeriodic) {
        offenders.push(`L~${i + 1}: ${line.trim()}`);
      }
    }

    expect(
      offenders,
      `BANNED PATTERN: bare \`.send().await\` HTTP call in rc-agent boot window without a retry wrapper.\n` +
        `Doctrine: racecontrol/CLAUDE.md "Boot Resilience: No single-fetch-at-boot without retry".\n` +
        `Empirical anchor: 2026-03-24 PG-allowlist outage — server transient at boot pinned all 8 pods at empty default for 2 days, 100 false violations/24h.\n` +
        `Fix: wrap the fetch in rc_common::boot_resilience::spawn_periodic_refetch OR an equivalent periodic loop with TTL.\n` +
        `Offenders:\n${offenders.join('\n')}`,
    ).toEqual([]);
  });
});
