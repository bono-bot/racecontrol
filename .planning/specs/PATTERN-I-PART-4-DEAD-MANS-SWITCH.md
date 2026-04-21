# Pattern I Part 4 — rc-agent Dead-Man's-Switch (DESIGN-SPEC)

**Status:** DESIGN only. No code. No MMA step yet. **Pre-MMA-Step-1.**
**Owner:** James session 2026-04-21.
**Scope:** rc-agent only (`crates/rc-agent/`). No server change, no rc-common change, no frontend change.
**MMA requirement:** This is a cross-process-on-same-machine bridge (rc-agent process lifecycle ↔ RCWatchdog service respawn ↔ `MAINTENANCE_MODE` sentinel). Per CLAUDE.md ("cross-system bridge" standing rule applied conservatively) and per Part 5 precedent: **Step 1 DIAGNOSE + Step 2 PLAN must complete before any code.** Budget target: ≤ $0.10 (same shape as Part 5 Steps 1+2 at $0.097).

## Why this exists

OPEN-PATTERNS.md Pattern I row 139 documents two distinct fingerprints:

1. **Stuck-handshake-forever** (original 2026-04-18 class) — Part 1 (`ws_state.rs` diagnostic endpoint, `92e699f4`) surfaces it; closed in `300e404b` via Pattern I DiD pair (`09acbbe4` + `90b04d71`).
2. **Silent-loop-death** (NEW 2026-04-20 class on Pod 6, `0306fe17`) — the reconnect loop ran normally for 6h, logged 51 successful `Connected and registered as Pod` entries spaced ~10 min apart, then **stopped producing any reconnect-loop output** for 6h while PID stayed alive. Logger wedged OR tokio runtime wedged OR reconnect task panicked silently. Nothing in the current codebase detects this — `/debug/ws-state` would still show a stale `last_register_success` from the final cycle before death, and `/health` returns 200 because the HTTP server lives in a different task that remained responsive.

Part 1/2/3 observe the state. Part 5 compensates on the session-summary side. **Part 4 is the recovery mechanism for the failure class where neither observation nor compensation helps because the reconnect task itself is dead.**

## Goal

rc-agent commits suicide (non-zero exit) if no successful WebSocket register has been observed for a configurable idle window (default N = 15 min). RCWatchdog (`rc-watchdog.exe` Windows service) sees the exit and spawns a fresh rc-agent process in Session 1 via `WTSQueryUserToken` + `CreateProcessAsUser`. A fresh process re-rolls any wedged-runtime state.

## Non-goals

- No stacktrace dump, no gdb attach, no memory snapshot. Too invasive; not proportionate for this class.
- No server-side enforcement. RCWatchdog polling `/debug/ws-state` and killing rc-agent from outside is an alternative mechanism — rejected because it adds a second health-checker to the pod and doubles the surface of "who can kill rc-agent." Keeping the decision inside rc-agent is simpler.
- No change to `MAINTENANCE_MODE` semantics. Part 4 operates below it.

## State model

All state lives on `AppState` (same pattern as Part 5 dedup guard `be664a04` and SN-01 `a13942f2`):

```rust
pub struct DeadManSwitch {
    /// Tokio `Instant` of the most recent successful WS register.
    /// `None` until the first register. Monotonic — unaffected by wall-clock jumps.
    pub last_register_success: Option<tokio::time::Instant>,

    /// Process-start Instant. Used for boot grace window.
    /// Fixed at AppState construction; never reset.
    pub process_started_at: tokio::time::Instant,

    /// Wall-clock SystemTime at process start + each tick — used to detect
    /// suspend/hibernate (Instant should freeze during suspend on Windows,
    /// but we cross-check for belt-and-suspenders).
    pub last_tick_wall_clock: std::sync::Mutex<std::time::SystemTime>,

    /// Count of dead-man exits in the current rc-agent boot epoch, read
    /// from persisted counter file at boot.
    /// Used to defeat restart-storm → MAINTENANCE_MODE lockout.
    pub exits_this_epoch: AtomicU32,
}
```

Persisted file at `C:\RacingPoint\deadman-counter.json`:
```json
{
  "epoch_start_wall": "2026-04-21T12:00:00Z",
  "exit_count": 0,
  "last_exit_wall": null,
  "last_exit_reason": null
}
```
Epoch = 30-min rolling window. Reset on: file absent, epoch_start older than 30 min, or user manual delete (documented in runbook).

## Trigger pipeline

Single background task, launched from `main.rs` right after `AppState` construction, before the WS reconnect loop:

```
tokio::spawn(dead_man_tick_loop(app_state.clone()));

async fn dead_man_tick_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;  // 1-min cadence
        if let Err(e) = dead_man_tick(&state).await {
            // Logging failure is itself a reason to not exit — log at WARN and keep looping.
            tracing::warn!(error = ?e, "dead_man_tick error (continuing)");
        }
    }
}
```

Tick evaluates 6 gates in order; first gate that fails → early return (no exit):

1. **Boot grace.** `state.dead_man.process_started_at.elapsed() < 120s` → return. Fresh process hasn't had time to register yet.

2. **Wall-clock jump detector.** Read `SystemTime::now()`, compare to `last_tick_wall_clock`. If delta > 90s (vs expected 60s ± jitter), assume suspend/hibernate just ended. Reset `last_register_success = None` (so the register-success gate will fail below → no exit this tick), update `last_tick_wall_clock`, return. This is belt-and-suspenders; tokio `Instant` already pauses during Windows suspend on recent versions but the cost of defense is one `SystemTime::now()` call.

3. **No register yet in this boot epoch.** `state.dead_man.last_register_success.is_none()` → return. Don't kill a process that may just be slow to reach the server on cold start. Boot grace catches the first 120s; this catches the next (N - 120s). An rc-agent that has never successfully registered in 15 min likely has a config issue, not a wedged-runtime issue — killing won't help and risks MAINTENANCE_MODE.

4. **Recent register.** `last_register_success.elapsed() < N` → return. N configurable via `rc-agent.toml [dead_man] idle_timeout_secs = 900` (default 900 = 15 min, matching OPEN-PATTERNS.md line 139 proposal).

5. **Restart-storm gate.** Read persisted counter. If `exit_count >= 2` in the current epoch → log ERROR, write JSONL breadcrumb, **do NOT exit this tick** (soft-fail). Rationale: MAINTENANCE_MODE triggers on 3 restarts in 10 min; 2 dead-man exits already means respawns are happening and the root cause is NOT a wedged tokio task (which would not recur). Third exit would permanently lock the pod out. Soft-fail means "wait for staff/automation to investigate" — mesh-intelligence + fleet/health `stuck_session_candidate` (Part 5 Commit 6 `728f9301`) will surface the symptom.

6. **Billing-inactive short-circuit (optional, discuss in MMA).** If `state.heartbeat_status.billing_active.load(Relaxed) == false` for the entire window, we could extend N from 15 min to 60 min — no customer is waiting, silence is cheap. Open question for MMA: does this extension mask real failures that matter for fleet ops (e.g. mesh-intelligence sync stops)? Default: **don't extend**, ship same N regardless.

If all 6 gates pass → **exit path**.

## Exit path

```rust
async fn trigger_dead_man_exit(state: &AppState, reason: &str) {
    // 1. Breadcrumb for post-mortem.
    let _ = tokio::fs::write(
        "C:\\RacingPoint\\RCAGENT_DEADMAN_EXIT.txt",
        format!("{}\nreason={}\nbuild_id={}\npid={}\n",
            chrono::Utc::now().to_rfc3339(),
            reason,
            env!("GIT_HASH"),
            std::process::id()),
    ).await;

    // 2. Persist exit counter (epoch-aware).
    persist_deadman_counter_increment(state).await;

    // 3. JSONL log at ERROR so grep for "deadman_exit" finds it.
    tracing::error!(
        reason = reason,
        exits_this_epoch = state.dead_man.exits_this_epoch.load(Relaxed),
        "dead_man_switch firing — process exit 88"
    );

    // 4. Flush tracing subscriber (best-effort — tracing-subscriber has no
    //    public flush API as of 0.3.x; rely on bounded in-memory buffer.)
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 5. Distinct exit code for audit trail.
    std::process::exit(88);
}
```

**Exit code 88** — distinct from 0 (clean), 1 (panic via default `abort=true`), 0xC000013A (CTRL_CLOSE_EVENT per d616ee10), and any Windows NTSTATUS code. Searchable in telemetry.

## Reset point

Only one writer to `last_register_success`:

```rust
// In reconnect loop, inside the Ok(Ok(ws_stream)) arm, immediately after
// "Connected and registered as Pod #{}" success log:
state.dead_man.last_register_success
    .lock()
    .await
    .replace(tokio::time::Instant::now());
```

Naturally resets the clock every successful register. A loop that wedges AFTER a register will consume the full N idle before firing. A loop that wedges BEFORE the first register is caught by gate 3 (return, don't kill — not our failure class).

Also consider: **hook into `ws_state::record_success`** (from Part 1 `92e699f4`) so the reset point is already-covered code. This is the preferred approach — single writer, already unit-tested, already lives inside the `RwLock`.

## Interaction with existing recovery systems

| System | Interaction | Risk | Mitigation |
|---|---|---|---|
| RCWatchdog service | Respawns rc-agent in Session 1 after exit | Happy-path | None needed |
| `MAINTENANCE_MODE` sentinel | Triggers on 3 restarts in 10 min → permanent block until staff clears | **HIGH** — our 15-min timeout could fire repeatedly | Gate 5 restart-storm: max 2 dead-man exits per 30-min epoch |
| `GRACEFUL_RELAUNCH` sentinel | `RCAGENT_SELF_RESTART` path | No conflict — dead-man is never graceful | None |
| Server `pod_monitor` + WoL auto-wake | Reboots pod if offline > N min | Could respawn a dead pod we want quiet | Dead-man pod will re-register within seconds of RCWatchdog respawn → server sees WS up → no WoL |
| `rc-sentry` watchdog | Independent process; does not restart rc-agent | None | None |
| `SN-01` stuck-ActiveSession valve | Runs inside rc-agent; killed with us | Lose the 15s timer anchor on exit | Fresh process re-derives from server on next register — lock_screen_state synthesises cleanly |
| Part 5 HTTP fallback | Runs inside rc-agent; killed with us | Lose the in-flight probe | T1 fires again on fresh register within seconds; T2 resumes its 5-min cadence |

## Failure modes + mitigations

**FM-1: False-fire during legitimate venue network outage** (all pods dead, upstream LAN/ISP down).
- Consequence: all 8 pods simultaneously suicide and respawn. Storm.
- Mitigation: gate 5 stops at 2 exits per epoch per pod. After outage recovers, all pods re-register cleanly on the fresh process.
- Residual risk: 2 concurrent respawns × 8 pods = 16 rc-agent boots in a short window → server-side load spike. Acceptable (compared to stale overlays).

**FM-2: False-fire during suspend/hibernate** (rare — pods are normally always-on, but possible).
- Consequence: exit on wake if gate 2 misses.
- Mitigation: gate 2 wall-clock jump detector. If tokio `Instant` doesn't pause on Windows (varies by tokio version), the SystemTime cross-check catches it.

**FM-3: Wedged-runtime-with-hang-pre-exit** — exit path itself can't complete (allocator corrupted, tokio executor dead).
- Consequence: `tokio::fs::write` or `persist_deadman_counter_increment` may never return. `std::process::exit(88)` still fires because it's a raw syscall; breadcrumb file may be empty.
- Mitigation: run the exit path with `tokio::time::timeout(Duration::from_secs(5), ...)`; if it times out, call `std::process::exit(88)` immediately, lose the breadcrumb. RCWatchdog respawn still works.

**FM-4: Flapping WS register** — registers every 13 min, resets clock, never fires dead-man, but session state is garbage.
- Consequence: Part 4 does nothing useful in this case.
- Mitigation: Part 4 is not designed for this; Parts 1+3+5 handle it. Document explicitly.

**FM-5: Persisted counter file corrupted or deleted maliciously**.
- Consequence: restart-storm gate ineffective.
- Mitigation: `MAINTENANCE_MODE` (3 restarts in 10 min) is a hard backstop. Acceptable defense-in-depth.

## MMA Step 1 DIAGNOSE — questions for the 5-model consensus

1. Is 15 min the right N? Too short → storm; too long → customer sees frozen overlay longer than needed.
2. Is `ws_state::record_success` the right reset point? Are there other "I successfully talked to the server" signals (heartbeat? pong received?) that should also reset?
3. Is the restart-storm gate (2 exits per 30 min) strong enough? Weak enough?
4. Is gate 2 (wall-clock jump) actually needed on modern tokio/Windows? If tokio `Instant` is guaranteed monotonic-paused, gate 2 is dead code.
5. Should exit be `std::process::exit(88)` or SIGKILL-equivalent via `TerminateProcess`? `exit` runs C runtime destructors which could hang if the hang is in a destructor path.
6. Is the breadcrumb file worth the added complexity vs relying on tracing's JSONL rolling appender?
7. Should FM-1 (fleet-wide simultaneous exit) be mitigated by pod-local random jitter (e.g. ±2 min on N)?
8. Gate 6 billing-inactive extension — include it or not?

## MMA Step 2 PLAN — output shape (after Step 1 converges)

5 models each produce a JSON plan with: file list, function signatures, test plan, rollback strategy, risk score (1-5). Winner selected by highest risk-adjusted score. Same rubric as Part 5 Step 2.

## Test plan (post-MMA)

Unit tests in `dead_man_switch.rs`:

1. `test_boot_grace_blocks_exit` — mock process_started_at 60s ago, assert no exit.
2. `test_no_register_yet_blocks_exit` — `last_register_success=None`, elapsed 30 min, assert no exit.
3. `test_exit_fires_after_idle_timeout` — register at T=0, tick at T=16min, assert exit path called.
4. `test_reset_on_register_success` — register at T=0, register again at T=10min, tick at T=15min, assert no exit (elapsed from second register = 5 min < N).
5. `test_restart_storm_gate` — pre-populate counter file with 2 exits in current epoch, tick meets all other gates, assert exit suppressed + ERROR log.
6. `test_wall_clock_jump_resets_anchor` — advance `last_tick_wall_clock` 120s + tokio `Instant` only 60s, tick, assert `last_register_success` reset.
7. `test_counter_epoch_rollover` — write counter with epoch_start 31 min ago, read counter, assert exit_count=0 returned + file rewritten.

Integration test in `tests/dead_man_integration.rs` (requires tokio runtime + fake time):

8. `test_full_cycle_mock_ws_silence` — spawn `dead_man_tick_loop`, mock `ws_state::record_success` call at T=0, advance tokio time 16 min, intercept `std::process::exit` via `#[cfg(test)] exit_hook` global AtomicBool, assert hook fires with code 88.

## Deploy gate (before any code ships)

**Mandatory prerequisites:**

1. **MMA Step 1 DIAGNOSE** — 5 vendor-diverse models, ≤ $0.10 budget, consensus findings archived to `.planning/specs/mma-part4/FINDINGS-STEP1.md` (mirrors Part 5 structure).
2. **MMA Step 2 PLAN** — 5-plan synth, winner selected, archived to `.planning/specs/mma-part4/PLAN-STEP2.md`.
3. **Part 5 deploy status check.** Part 4 and Part 5 are logically independent but operationally entangled (both live in rc-agent; both deployed together reduces total swap count across fleet by 1). Preferred: bundle. Acceptable: ship alone. **NOT acceptable**: ship Part 4 alone while Part 5 sits un-shipped long enough that the silent-loop-death class recurs (customer sees stuck overlay, Part 4 kills the process, Part 5 would have synthesised SessionEnded — instead the customer sees active_session → blank_screen with no summary).
4. **MMA Step 4 VERIFY** — 3-model adversarial post-code, before deploy. ~$0.50-0.80. This is the gate that's currently also blocking Part 5 deploy.
5. **User auth** — 8-pod atomic swap is destructive. Must be explicit "deploy Part 4 to fleet" from the user, not inferred from "implement Part 4."
6. **Fleet online.** Current state 2026-04-21: 4/8 pods offline (1,2,3,5). Deploy requires 8/8 online OR explicit agreement to deploy-to-online-only + hot-apply-on-reboot.

## Files to be created / modified (when code phase begins)

NEW:
- `crates/rc-agent/src/dead_man_switch.rs` (~180 LOC impl + doctests)
- `crates/rc-agent/tests/dead_man_integration.rs` (~120 LOC)
- `.planning/specs/mma-part4/FINDINGS-STEP1.md` (MMA artifacts)
- `.planning/specs/mma-part4/PLAN-STEP2.md`
- `.planning/specs/mma-part4/responses/*` (5 raw per-model responses)
- `.planning/specs/mma-part4/plans/*` (5 JSON plans)
- `.planning/specs/mma-part4/run-step1.js`
- `.planning/specs/mma-part4/run-step2.js`

MODIFIED:
- `crates/rc-agent/src/app_state.rs` — add `DeadManSwitch` field (~20 LOC)
- `crates/rc-agent/src/main.rs` — spawn `dead_man_tick_loop` at boot (~5 LOC)
- `crates/rc-agent/src/ws_state.rs` — wire reset point into `record_success` (~3 LOC)
- `crates/rc-agent/src/config.rs` — add `[dead_man] idle_timeout_secs` key (~8 LOC)
- `crates/rc-agent/rc-agent.toml` — document default value (~3 LOC)

**Estimated total: ~340 LOC across 5 files.**

## Out of scope

- Server-side "dead-man-recently-exited" flag in `/fleet/health` — separate follow-up (same shape as Part 5 Commit 6 `stuck_session_candidate`).
- Per-pod N variance (some pods idle more than others). Ship constant N first; tune per-pod after 2-4 weeks of production data.
- Stacktrace dump before exit. Would need `backtrace` crate + symbol resolution; out of scope.
- Telemetry export of `exit_count` to server (useful for fleet-wide dashboards). Deferred.

## Cross-refs

- OPEN-PATTERNS.md Pattern I row (line 139 proposal).
- `session_handoff_20260420_pod6_blanking_pattern_i_handoff.md` (Pod 6 silent-loop-death incident).
- Part 5 DESIGN-SPEC for the sibling recovery mechanism.
- `92e699f4` Part 1 — ws_state.rs diagnostic (provides the `last_register_success` anchor we hook into).
- `CLAUDE.md` standing rules on cross-system bridges + MMA mandatory gate.
