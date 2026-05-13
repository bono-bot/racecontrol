/**
 * Layer 1.10 acceptance test: Staff at Kiosk launches game (15-20s target)
 *
 * V2-PROGRESS-MAP §1 row 1.10 ("Customer-day | 14:25 | Staff at Kiosk launches
 * game (15-20s target) | NOT-STARTED | james | Joint #3 (Game Launching) full
 * spec; current = no V2 Kiosk staff UI"). iter3 moves row 1.10 NOT-STARTED →
 * IN-FLIGHT via the runtime-testable launch contract: /games/launch is
 * exercise-able today against the V1-inherited code path, so the V2
 * latency/state/error contract for the 14:25 beat can be asserted before
 * the V2 Kiosk staff UI lands.
 *
 * Source-of-truth (DoD canonical day; quoted verbatim 2026-05-13 IST from
 * comms-link/v2-skeleton/05-definition-of-done.md):
 *   - L82:  "14:25 IST — Pod assignment + game launch. Staff at Kiosk .23:
 *           select profile → select pod → select game (Assetto Corsa SP).
 *           Launch dispatches."
 *   - L84:  "Locked invariant (Captain): Kiosk .23 is the ONLY surface that
 *           launches games in v2.0."
 *   - L85:  "PWA does not launch games (deferred to v2.1 per deferral roadmap)."
 *   - L89:  "AC starts on Pod 3 within 15-20 seconds (Captain-locked target —
 *           staff hits Launch on Kiosk → customer is on track in-game). This
 *           is a target, not a guarantee — if a specific game's intrinsic
 *           load (e.g., AC asset streaming, F1 25 anti-cheat init) cannot hit
 *           the window, the target is made game-specific in §3 rather than
 *           universal."
 *   - L260: "Game-launch in-flight failures (the launch-to-on-track 15-20s
 *           window can fail mid-flight):"
 *   - L316: "| **3. Game Launching** | Staff-from-Kiosk launch on Pods 1-8
 *           (sim) + PS5 stations + game-agnostic contract. | [NEEDS CAPTAIN...] |"
 *   - L396: "Full skeleton means: Kiosk launches via SimLauncher → game spawns
 *           within 15-20s target → telemetry flows..."
 *   - L539: "anything beyond 10s on a failure path crosses the §1.2 14:25
 *           launch-target boundary by 2x... Customer's patience tolerance is
 *           at hand; beyond this they form a felt-failure mental-model..."
 *   - CLAUDE.md "Concurrent Session Guard (HTTP 409 upgrade)": "POST
 *     /games/launch: returns HTTP 409 {error:\"game_already_active\", pod_id}
 *     when pod already has a game Launching/Running/Stopping (upgraded from
 *     HTTP 200 + error body)"
 *   - CLAUDE.md Crash Loop Detection 2026-03-26 Pod 6: "Game launch `ok:
 *     true` does NOT mean the agent received the command... GameTracker
 *     stuck in `Launching` permanently, blocking all future launches on that
 *     pod. Short-term: add a 60s timeout on `Launching` state → auto-Error."
 *
 * Contract under test:
 *   POST /api/v1/games/launch MUST accept {pod_id, sim_type} and return
 *   either (a) 200 {ok: true, verified: bool} on dispatch success, or
 *   (b) 409 {error: "game_already_active", pod_id} on concurrent collision.
 *   The Launching → Running transition MUST become observable on
 *   /api/v1/games/pod/{pod_id} within DoD L89 15-20s + 5s harness buffer
 *   (25s). Dispatch round-trip p95 MUST be ≤ 20000ms; soft-warn at 15000ms
 *   (bottom of tolerance band). 409 guard MUST fire on
 *   Launching/Running/Stopping collision (Phase 366 GLD-F-04). A failed
 *   launch (invalid game_key, pod offline) MUST NOT leave GameTracker
 *   stuck in Launching permanently (2026-03-26 Pod 6 incident class) —
 *   either dispatch errors synchronously OR check_launch_timeouts
 *   (billing_game_status_defer.rs:32-34, default 180s) clears the tracker.
 *
 * Endpoint discovery (grep 2026-05-13 IST):
 *   POST /api/v1/games/launch   (CANONICAL; routes.rs:551 → launch_game at
 *     game_launch.rs:67. Body: pod_id, sim_type required; experience_id,
 *     launch_args, force_supersede optional. Returns 200 {ok, verified,
 *     [verification_warning], [warning]}, 400 missing-fields, 401 no-JWT,
 *     409 {error:"game_already_active", pod_id, detail} at
 *     game_launch.rs:398-407.)
 *   GET  /api/v1/games/pod/{pod_id}   (state observation; routes.rs:559 →
 *     pod_game_state at game_state.rs:220-229. Returns {game: GameInfo|null,
 *     state?: "idle"} — game.game_state surfaces "Launching" | "Running" |
 *     "Stopping" | "Error" from active_games.)
 *   GET  /api/v1/games/active   (collection variant; routes.rs:555 →
 *     active_games at game_state.rs:63.)
 *   /api/v1/games/state DOES NOT EXIST as a top-level surface. The "state"
 *     verb ships today as /games/pod/{pod_id} (per-pod read). Override via
 *     STATE_PATH_TEMPLATE env if Joint #3 spec renames.
 *
 * Latency SLA mechanism: dispatch round-trip is bounded by trial-session
 *   SELECT (game_launch.rs:136-148), duration_minutes SELECT
 *   (game_launch.rs:162-180), SEC-01/SEC-02 validation (185-195), and the
 *   per-launch verification poll at game_launch.rs:370-388 ("process not
 *   confirmed running within 20s"). End-to-end wall-clock for "customer on
 *   track" is bounded on the agent side by ac_launcher::launch_ac
 *   (rc-agent/src/ac_launcher.rs:437); V1 optimization at line 836 took
 *   launch_ac() to <1s, AC asset streaming dominates the 15-20s window.
 *
 * Concurrent-launch guard: game_launch.rs:398-407 catches launcher Err
 *   substring "already has a game active" / "game still stopping" → 409.
 *   active_games entries with state in {Launching, Running, Stopping}
 *   block new launches. force_supersede=true bypasses Launching/Running
 *   only (game_launch.rs:103); Stopping waits for completion.
 *
 * Launching-stuck recovery: check_launch_timeouts at
 *   billing_game_status_defer.rs:32-34 (180s default) sweeps
 *   waiting_for_game sessions past threshold; handle_launch_timeouts
 *   consumes the list; billing_game_status.rs:488 removes the GameTracker
 *   so the pod isn't stuck. This guard was added AFTER the 2026-03-26 Pod 6
 *   incident — pre-fix the tracker stayed forever.
 *
 * Env vars:
 *   CANONICAL_URL              - Server .23 racecontrol (default
 *                                http://192.168.31.23:8080)
 *   CANONICAL_REACHABLE        - 'true' to opt into network-reaching tests;
 *                                default unset → skip. Set 'true' from venue
 *                                LAN/VPN; from bono VPS 192.168.31.0/24 is
 *                                unreachable.
 *   TEST_ADMIN_JWT             - staff JWT (POST /games/launch is staff-only
 *                                per DoD L84 Captain-lock). Mint via
 *                                {CANONICAL_URL}/api/v1/auth/staff-login.
 *   TEST_POD_ID                - pod_id body field (e.g. "pod-3" — DoD L89
 *                                canonical example — or UUID from fleet/health)
 *   TEST_SIM_TYPE              - sim_type (default "assetto_corsa", DoD L82
 *                                canonical 14:25 game + only Tier-1 V2.0
 *                                launch path per L402+)
 *   TEST_INVALID_GAME_KEY      - sim_type guaranteed to fail launch
 *                                (default "definitely_not_a_real_game...")
 *   TEST_OFFLINE_POD_ID        - pod registered but ws_connected: false
 *                                (sibling failure mode at game_launch.rs:394)
 *   DISPATCH_PATH              - override (default /api/v1/games/launch)
 *   STATE_PATH_TEMPLATE        - override per-pod state path (default
 *                                /api/v1/games/pod/{pod_id})
 *   LAUNCH_TARGET_MS           - DoD L89 upper bound (default 20000)
 *   LAUNCH_TARGET_WARN_MS      - DoD L89 lower bound of tolerance band
 *                                (default 15000 — soft-fail warning)
 *   STATE_OBSERVE_WINDOW_MS    - Launching→Running observation window
 *                                (default 25000 = target + 5s buffer)
 *   STUCK_LAUNCH_TIMEOUT_MS    - max time a failed launch may leave tracker
 *                                in Launching (default 180000 = BillingConfig
 *                                default at billing_game_status_defer.rs:32-34)
 *
 * G4 NOT TESTED in this file (deferred — Joint #3 full spec scope):
 *   - Kiosk-UI dispatch path (V2 Kiosk staff UI unbuilt per V2-PROGRESS-MAP
 *     row 1.10 baseline; sibling layer-1.10b once UI ships)
 *   - PS5-station launch primitive (DoD L316; different mechanism — CEC/HDMI)
 *   - Tier-2 game launch + Kiosk-selection pricing-attribution invariant
 *     (DoD L297-299 R4-D)
 *   - Desktop-workaround launch path for F1 25/LMU/iRacing/AMS2/rFactor 2
 *     (DoD L402+; billing-continues semantics; layer-1.10c sibling)
 *   - Wallet-debit per-minute rate resolution at launch (DoD L89; tested in
 *     billing/pricing acceptance layer)
 *   - Visual primitives during launch (blanking + loading screens DoD L226+;
 *     pod-OS-level not server-API)
 *   - HUD overlay (composes with Layer 1.11 telemetry)
 *   - Customer-facing 1s ack (DoD L205+ / R4-A — applies to staff Kiosk UI
 *     while the 15-20s backend executes; UI scope, not this file)
 *
 * V2 doctrine alignment: Joint #3 (Game Launching) full spec PENDING per
 * V2-PROGRESS-MAP §1 row 1.10. PWA-self-launch is post-V2.0 (DoD L85 +
 * L584 deferral); V2.0 = staff Kiosk launches only (DoD L84 Captain-lock).
 * The V1 launch path (ac_launcher direct cmd /C launch-ac.bat for MP,
 * direct acs.exe for SP per d616ee10) is V1-inherited; this V2 contract
 * surfaces the LATENCY TARGET (L89 15-20s) and STATE-MACHINE invariants
 * (Launching→Running observable, no-stuck-Launching) WITHOUT coupling to
 * the V1 implementation mechanism. Composes with Layer 1.11 (race +
 * telemetry — Launching→Running is the prerequisite for telemetry to fire)
 * and Layer 1.13 (auto-bill — waiting_for_game >180s feeds
 * cancelled_no_playable, not normal 14:55 finalize).
 *
 * Test posture: SKIPS cleanly when CANONICAL_REACHABLE is unset (default
 * off-venue posture); 5/5 tests dry-run-viable. When env is populated from
 * venue LAN/VPN + TEST_MODE racecontrol, assertions fire against the live
 * Server .23 launch surface.
 */

import { test, expect } from '@playwright/test';

const CANONICAL_URL = process.env.CANONICAL_URL || 'http://192.168.31.23:8080';
const CANONICAL_REACHABLE = process.env.CANONICAL_REACHABLE === 'true';
const TEST_ADMIN_JWT = process.env.TEST_ADMIN_JWT;
const TEST_POD_ID = process.env.TEST_POD_ID;
const TEST_SIM_TYPE = process.env.TEST_SIM_TYPE || 'assetto_corsa';
const TEST_INVALID_GAME_KEY =
  process.env.TEST_INVALID_GAME_KEY || 'definitely_not_a_real_game_v2_layer_1_10';
const TEST_OFFLINE_POD_ID = process.env.TEST_OFFLINE_POD_ID;
const DISPATCH_PATH = process.env.DISPATCH_PATH || '/api/v1/games/launch';
const STATE_PATH_TEMPLATE =
  process.env.STATE_PATH_TEMPLATE || '/api/v1/games/pod/{pod_id}';
const LAUNCH_TARGET_MS = parseInt(process.env.LAUNCH_TARGET_MS || '20000', 10);
const LAUNCH_TARGET_WARN_MS = parseInt(
  process.env.LAUNCH_TARGET_WARN_MS || '15000',
  10,
);
const STATE_OBSERVE_WINDOW_MS = parseInt(
  process.env.STATE_OBSERVE_WINDOW_MS || '25000',
  10,
);
const STUCK_LAUNCH_TIMEOUT_MS = parseInt(
  process.env.STUCK_LAUNCH_TIMEOUT_MS || '180000',
  10,
);

const SKIP_REASONS: Record<string, string> = {
  CANONICAL_REACHABLE: `CANONICAL_REACHABLE not set to 'true' - Layer 1.10 requires network reachability to ${CANONICAL_URL}. Set CANONICAL_REACHABLE=true when running from venue LAN or VPN to Server .23. From bono VPS (Hostinger), 192.168.31.0/24 is unreachable by default.`,
  TEST_ADMIN_JWT: `TEST_ADMIN_JWT not set - Layer 1.10 dispatch endpoint (POST /games/launch) lives in staff_routes and REQUIRES staff JWT. Mint via POST ${CANONICAL_URL}/api/v1/auth/staff-login (or test-mint-staff-jwt in TEST_MODE). DoD L84 Captain-locks Kiosk .23 as the only launch surface; the launch path is staff-only by construction.`,
  TEST_POD_ID: `TEST_POD_ID not set - dispatch requires a pod_id body field per game_launch.rs:72. Use a pod id resolvable on the target (e.g. "pod-3" — DoD L89's canonical 14:25 example pod — or the UUID-style id from /api/v1/fleet/health). Pre-test the pod is online via curl ${CANONICAL_URL}/api/v1/fleet/health.`,
  TEST_OFFLINE_POD_ID: `TEST_OFFLINE_POD_ID not set - the no-agent-connected failure path (game_launch.rs:394 → relay_game_launch_to_venue OR structured error) requires a pod_id that is registered but whose rc-agent is not WS-connected. Pick a pod whose ws_connected: false in /api/v1/fleet/health.`,
  JOINT_3_SPEC_PENDING: `Joint #3 (Game Launching) full spec — pending per V2-PROGRESS-MAP §1 row 1.10. Until the V2 Kiosk staff UI lands and the game-agnostic contract (DoD L316 "Pods 1-8 (sim) + PS5 stations") is ratified, UI-tier and PS5-tier launch contracts can't be exercised. This file asserts the server-API tier only.`,
  POD_6_STUCK_LAUNCH_CLASS: `Failed-launch cleanup invariant requires a deterministic failure-injection surface. The CLAUDE.md 2026-03-26 Pod 6 incident class is what the test guards against — a stuck Launching tracker that blocks all future launches. Today's check_launch_timeouts (180s default, billing_game_status_defer.rs:32-34) clears the tracker; a tighter TEST_MODE timeout helper would make this test fast. Joint #3 spec may add such a helper.`,
};

interface GameLaunchResponse {
  ok?: boolean;
  verified?: boolean;
  verification_warning?: string;
  warning?: string;
  error?: string;
  pod_id?: string;
  detail?: string;
}

interface GameStateInfo {
  game_state?: string;
  sim_type?: string;
  pod_id?: string;
  started_at?: string;
}

interface GameStateResponse {
  game?: GameStateInfo | null;
  state?: string;
  error?: string;
}

interface ApiRead<T> {
  url: string;
  status: number;
  body: T;
  read_started_at: number;
  read_finished_at: number;
}

async function postLaunch(
  jwt: string,
  body: Record<string, unknown>,
): Promise<ApiRead<GameLaunchResponse>> {
  const url = `${CANONICAL_URL}${DISPATCH_PATH}`;
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
  let parsed: GameLaunchResponse;
  try {
    parsed = (await resp.json()) as GameLaunchResponse;
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

async function getPodGameState(
  jwt: string | undefined,
  podId: string,
): Promise<ApiRead<GameStateResponse>> {
  const path = STATE_PATH_TEMPLATE.replace('{pod_id}', encodeURIComponent(podId));
  const url = `${CANONICAL_URL}${path}`;
  const started = Date.now();
  const headers: Record<string, string> = {};
  if (jwt) headers.Authorization = `Bearer ${jwt}`;
  const resp = await fetch(url, { headers });
  const finished = Date.now();
  let parsed: GameStateResponse;
  try {
    parsed = (await resp.json()) as GameStateResponse;
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

function extractGameState(body: GameStateResponse): string | undefined {
  if (body.game && body.game.game_state) return body.game.game_state;
  if (body.state) return body.state;
  return undefined;
}

test.describe('Layer 1.10 - Staff at Kiosk launches game (15-20s target)', () => {
  test('canonical staff-initiated launch endpoint accepts pod_id + sim_type', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_POD_ID, SKIP_REASONS.TEST_POD_ID);

    // Test 1 — Endpoint shape. The minimum body per game_launch.rs:72-94
    // is {pod_id, sim_type}. Missing either yields 400 with
    // {error: "pod_id and sim_type are required"} (game_launch.rs:93).
    // The dispatch returns either 200 (success / verified-warning) or 409
    // (concurrent guard — tested in Test 3) or non-200 structured error.
    const r = await postLaunch(TEST_ADMIN_JWT!, {
      pod_id: TEST_POD_ID,
      sim_type: TEST_SIM_TYPE,
    });
    console.log(
      `launch shape-check t=${r.read_finished_at - r.read_started_at}ms ` +
      `status=${r.status} url=${r.url}`,
    );
    // Accept 200 (fresh dispatch) OR 409 (concurrent — pod already has a game
    // active from a prior test run or live customer). Both prove the surface
    // exists and parses the request body. 401/404/400 would prove a contract
    // break.
    expect([200, 409]).toContain(r.status);

    if (r.status === 200) {
      console.log(
        `launch shape: ok=${r.body.ok} verified=${r.body.verified ?? '<absent>'} ` +
        `verification_warning=${r.body.verification_warning ?? '<absent>'} ` +
        `warning=${r.body.warning ?? '<absent>'}`,
      );
      expect(typeof r.body.ok === 'boolean').toBe(true);
      expect(r.body.ok).toBe(true);
    } else {
      console.log(
        `launch shape (409 concurrent path): error=${r.body.error} ` +
        `pod_id=${r.body.pod_id ?? '<absent>'} ` +
        `detail=${r.body.detail ?? '<absent>'}`,
      );
      expect(r.body.error).toBe('game_already_active');
      expect(typeof r.body.pod_id === 'string').toBe(true);
    }
  });

  test('latency SLA - staff-dispatch round-trip within DoD L89 15-20s target', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_POD_ID, SKIP_REASONS.TEST_POD_ID);

    // Test 2 — Latency SLA. DoD L89 Captain-locks 15-20s as the wall-clock
    // for staff hits Launch → customer on track. The dispatch-ACK (this
    // measurement) is the wire round-trip from POST /games/launch back to
    // 200, NOT the full launch-to-on-track wall-clock — the agent-side game
    // spawn proceeds asynchronously. The 200 includes a per-launch
    // verification poll for up to ~20s (game_launch.rs:370-385), so the
    // observed round-trip can legitimately approach the L89 upper bound.
    // Assertion: hard-fail >20000ms (DoD upper); soft-warn >15000ms (DoD
    // lower-of-tolerance, the bottom of "15-20s" where target acceptance
    // is unambiguous).
    const r = await postLaunch(TEST_ADMIN_JWT!, {
      pod_id: TEST_POD_ID,
      sim_type: TEST_SIM_TYPE,
    });
    const elapsed = r.read_finished_at - r.read_started_at;
    console.log(
      `launch latency-SLA t=${elapsed}ms target=${LAUNCH_TARGET_MS}ms ` +
      `warn_at=${LAUNCH_TARGET_WARN_MS}ms status=${r.status}`,
    );
    // 409 still counts toward latency budget — the concurrent-guard path is
    // a normal response and must be fast (sub-second; it only reads
    // active_games and returns). Per game_launch.rs the 409 path does NOT
    // poll the agent verification window, so 409 expected <1s.
    expect([200, 409]).toContain(r.status);
    if (r.status === 409) {
      // Concurrent-path bound: should be well under the dispatch target.
      // Use the same upper-bound assertion to avoid false-fails on slow
      // network; the soft-warn is the real informative line.
      expect(elapsed).toBeLessThanOrEqual(LAUNCH_TARGET_MS);
    } else {
      expect(elapsed).toBeLessThanOrEqual(LAUNCH_TARGET_MS);
      if (elapsed > LAUNCH_TARGET_WARN_MS) {
        console.log(
          `launch latency soft-warn: dispatch took ${elapsed}ms which is ` +
          `inside the 15-20s tolerance band but above the on-target line. ` +
          `DoD L89 marks this as game-specific tunable if intrinsic load ` +
          `(AC asset streaming, anti-cheat init) cannot hit ≤15s.`,
        );
      }
    }
  });

  test('concurrent-launch guard returns HTTP 409 game_already_active', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_POD_ID, SKIP_REASONS.TEST_POD_ID);

    // Test 3 — Concurrent-launch guard. CLAUDE.md "Concurrent Session Guard
    // (HTTP 409 upgrade)" documents the contract: a second launch on a pod
    // that already has a game Launching/Running/Stopping returns HTTP 409
    // with {error: "game_already_active", pod_id}. game_launch.rs:398-407
    // catches the launcher Err for "already has a game active" / "game
    // still stopping" substrings and emits the 409 body.
    //
    // Pre-conditions: the first launch (call-1) must leave a tracker in
    // Launching or Running state on the pod. If the pod already has an
    // active game from a prior session, call-1 itself returns 409 — that
    // ALSO proves the guard fires; the test accepts either ordering.
    const r1 = await postLaunch(TEST_ADMIN_JWT!, {
      pod_id: TEST_POD_ID,
      sim_type: TEST_SIM_TYPE,
    });
    console.log(
      `409-guard call-1 t=${r1.read_finished_at - r1.read_started_at}ms ` +
      `status=${r1.status} ok=${r1.body.ok ?? '<absent>'} ` +
      `error=${r1.body.error ?? '<absent>'}`,
    );
    expect([200, 409]).toContain(r1.status);

    // Call-2: identical launch, NO force_supersede. game_launch.rs:103
    // force_supersede=true would bypass — we omit it to assert the guard
    // fires. If call-1 was 200 the tracker is now in Launching/Running;
    // call-2 MUST 409. If call-1 was 409 the guard already fired and
    // call-2 will also 409 (the prior tracker is still there).
    const r2 = await postLaunch(TEST_ADMIN_JWT!, {
      pod_id: TEST_POD_ID,
      sim_type: TEST_SIM_TYPE,
    });
    console.log(
      `409-guard call-2 t=${r2.read_finished_at - r2.read_started_at}ms ` +
      `status=${r2.status} ok=${r2.body.ok ?? '<absent>'} ` +
      `error=${r2.body.error ?? '<absent>'} ` +
      `pod_id=${r2.body.pod_id ?? '<absent>'} ` +
      `detail=${r2.body.detail ?? '<absent>'}`,
    );
    expect(r2.status).toBe(409);
    expect(r2.body.error).toBe('game_already_active');
    expect(r2.body.pod_id).toBeDefined();
    // The detail field surfaces the underlying launcher error string (see
    // game_launch.rs:405). The Phase 366 GLD-F-04 upgrade ensures the body
    // is structured rather than a free-form 200 + error field.
    expect(typeof r2.body.pod_id === 'string').toBe(true);
  });

  test('game-state Launching → Running observable on /games/pod/{pod_id} within 25s', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_POD_ID, SKIP_REASONS.TEST_POD_ID);

    // Test 4 — State-machine transition. DoD L89 wall-clock target = 15-20s
    // from staff-Launch to "customer on track in-game". The server-visible
    // proxy for "on track" is GameState::Running on the GameTracker (see
    // game_launch.rs:377 — same field used for the dispatch-time
    // verification flag). The transition Launching → Running is emitted by
    // the agent via WS GameStateUpdate (ws/agent_game.rs:80 + ws/mod.rs:293)
    // and surfaces on GET /games/pod/{pod_id} (game_state.rs:220-229).
    //
    // Observation window: 25s = DoD upper bound 20s + 5s buffer for state
    // propagation + WS jitter + read-side cache.
    //
    // Failure mode under test: if Launching never transitions to Running
    // within the window, the contract is broken — either the agent didn't
    // receive the launch (CLAUDE.md 2026-03-26 "ok: true does NOT mean the
    // agent received the command") or the game spawn failed silently.
    //
    // Note: this test uses force_supersede=true to reset any pre-existing
    // tracker from prior tests so the Launching→Running observation starts
    // from a known baseline. force_supersede only bypasses
    // Launching/Running guard (not Stopping); if the pod is mid-Stopping
    // the launch will queue per game_launch.rs:122-129 wait loop.
    const r = await postLaunch(TEST_ADMIN_JWT!, {
      pod_id: TEST_POD_ID,
      sim_type: TEST_SIM_TYPE,
      force_supersede: true,
    });
    console.log(
      `state-machine launch t=${r.read_finished_at - r.read_started_at}ms ` +
      `status=${r.status} verified=${r.body.verified ?? '<absent>'}`,
    );
    expect([200, 409]).toContain(r.status);

    // Poll /games/pod/{pod_id} until game_state === "Running" or until the
    // observation window expires. Polling cadence 500ms = ~50 reads max in
    // 25s; the read itself is a single in-memory active_games lookup so
    // load is negligible.
    const observationDeadline = Date.now() + STATE_OBSERVE_WINDOW_MS;
    let lastState: string | undefined;
    let observedRunning = false;
    let observedLaunching = false;
    let pollCount = 0;
    while (Date.now() < observationDeadline) {
      const s = await getPodGameState(TEST_ADMIN_JWT, TEST_POD_ID!);
      pollCount++;
      lastState = extractGameState(s.body);
      if (lastState === 'Launching') observedLaunching = true;
      if (lastState === 'Running') {
        observedRunning = true;
        break;
      }
      // Sleep 500ms between polls.
      await new Promise((res) => setTimeout(res, 500));
    }
    console.log(
      `state-machine observation: polls=${pollCount} ` +
      `last_state=${lastState ?? '<absent>'} ` +
      `observed_launching=${observedLaunching} ` +
      `observed_running=${observedRunning} ` +
      `window=${STATE_OBSERVE_WINDOW_MS}ms`,
    );
    // Primary assertion: Running observed within window. If the pod is
    // offline / agent disconnected, this surfaces as observed_running=false
    // (the contract failure). The test reports what was seen rather than
    // labeling PASS/FAIL (H3 observations-not-verdicts).
    expect(observedRunning).toBe(true);
  });

  test('failed launch returns explicit error without leaving GameTracker stuck in Launching', async () => {
    test.skip(!CANONICAL_REACHABLE, SKIP_REASONS.CANONICAL_REACHABLE);
    test.skip(!TEST_ADMIN_JWT, SKIP_REASONS.TEST_ADMIN_JWT);
    test.skip(!TEST_POD_ID, SKIP_REASONS.TEST_POD_ID);

    // Test 5 — Failed-launch cleanup. CLAUDE.md 2026-03-26 Pod 6 incident:
    // a launch that fails to reach the agent (WS dropped between queue and
    // delivery, or invalid game_key, or pod offline) MUST NOT leave the
    // GameTracker stuck in Launching permanently. Pre-fix: tracker stayed
    // forever, blocking ALL future launches with "already has a game
    // active". Post-fix: either dispatch returns a structured error
    // synchronously (game_launch.rs:409 generic Err path), OR the
    // background sweep at billing_game_status_defer::check_launch_timeouts
    // (default 180s) clears the tracker.
    //
    // Two failure-injection paths available:
    //   (a) TEST_INVALID_GAME_KEY — sim_type that no agent will accept
    //   (b) TEST_OFFLINE_POD_ID — pod registered but not WS-connected
    //
    // We use (a) by default because it's deterministic and doesn't depend
    // on an offline pod being available. If (b) is set, we run that path
    // instead.
    const useOfflinePodPath = !!TEST_OFFLINE_POD_ID;
    const failedPodId = useOfflinePodPath ? TEST_OFFLINE_POD_ID! : TEST_POD_ID!;
    const failedSimType = useOfflinePodPath ? TEST_SIM_TYPE : TEST_INVALID_GAME_KEY;
    console.log(
      `failed-launch path: ${useOfflinePodPath ? 'offline-pod' : 'invalid-game-key'} ` +
      `pod_id=${failedPodId} sim_type=${failedSimType}`,
    );

    const r = await postLaunch(TEST_ADMIN_JWT!, {
      pod_id: failedPodId,
      sim_type: failedSimType,
      force_supersede: true,
    });
    console.log(
      `failed-launch dispatch t=${r.read_finished_at - r.read_started_at}ms ` +
      `status=${r.status} ok=${r.body.ok ?? '<absent>'} ` +
      `error=${r.body.error ?? '<absent>'} ` +
      `verified=${r.body.verified ?? '<absent>'} ` +
      `verification_warning=${r.body.verification_warning ?? '<absent>'}`,
    );

    // Acceptable outcomes for the dispatch:
    //   - 200 {ok: false, error: ...}                  (generic Err path)
    //   - 200 {ok: true, verified: false,
    //          verification_warning: "Game launch command sent but process
    //          not confirmed running within 20s. Check pod manually."}
    //          (the verification-warning path at game_launch.rs:384-388 —
    //          the agent never confirmed Running)
    //   - 400 {error: "pod_id and sim_type are required"}  (if the
    //          invalid-game path triggers a body-validation reject earlier
    //          — we use a non-empty string so this should not fire, but
    //          assert tolerantly)
    //   - 4xx/5xx structured error
    //   - 409 game_already_active (if a prior test left a tracker; not the
    //          target of this test but valid behavior)
    //
    // The key invariant under test is: AFTER the failed dispatch, the
    // GameTracker MUST NOT remain in Launching state forever. We assert
    // by polling /games/pod/{pod_id} for up to STUCK_LAUNCH_TIMEOUT_MS
    // and confirming the state eventually leaves "Launching" (either
    // clears to idle, transitions to Error, or — if the failed launch
    // was a no-op that never created a tracker — was never Launching to
    // begin with).
    const stuckDeadline = Date.now() + STUCK_LAUNCH_TIMEOUT_MS;
    let lastState: string | undefined;
    let stuckObservations = 0;
    let nonLaunchingObservations = 0;
    // Cadence: 1s poll for up to 180s = up to 180 reads. Cheap on the
    // server (single in-memory lookup).
    while (Date.now() < stuckDeadline) {
      const s = await getPodGameState(TEST_ADMIN_JWT, failedPodId);
      lastState = extractGameState(s.body);
      if (lastState === 'Launching') {
        stuckObservations++;
      } else {
        nonLaunchingObservations++;
        // Once we see a non-Launching state, the cleanup invariant is
        // satisfied — we can stop polling.
        break;
      }
      await new Promise((res) => setTimeout(res, 1000));
    }
    console.log(
      `failed-launch cleanup: last_state=${lastState ?? '<absent>'} ` +
      `stuck_polls=${stuckObservations} ` +
      `non_launching_polls=${nonLaunchingObservations} ` +
      `window=${STUCK_LAUNCH_TIMEOUT_MS}ms`,
    );

    // Assertion: at end of window, the tracker is NOT in Launching state.
    // The pod_6_stuck_launch_class invariant is the negation of "tracker
    // stuck in Launching permanently".
    expect(lastState).not.toBe('Launching');

    // Secondary observation: the dispatch response itself should have
    // included SOME indication of failure (either ok: false, or
    // verified: false + verification_warning, or non-200 status, or 409).
    // Strict assertion-by-status here would over-couple; we log the
    // observed combination and assert at minimum the failure surfaced
    // (not a silent ok: true verified: true response).
    if (r.status === 200) {
      const silentSuccess =
        r.body.ok === true &&
        r.body.verified === true &&
        !r.body.verification_warning &&
        !r.body.error;
      console.log(
        `failed-launch response classification: silent_success=${silentSuccess}`,
      );
      // A silent-success response on a deliberately failed launch would
      // indicate the contract is broken (the 2026-03-26 "ok: true does
      // NOT mean agent received it" class). Assert against it.
      expect(silentSuccess).toBe(false);
    }
  });
});
