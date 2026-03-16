---
phase: 03-websocket-resilience
verified: 2026-03-13T00:00:00Z
status: passed
score: 10/10 must-haves verified
re_verification: false
human_verification:
  - test: "Observe kiosk header during a game launch on any pod"
    expected: "Header stays green throughout shader compilation (10-30s). No 'Disconnected' flash."
    why_human: "Requires live CPU spike condition that cannot be simulated with static analysis"
  - test: "Check racecontrol logs 30 minutes after startup"
    expected: "tracing::debug or tracing::warn messages containing 'WS round-trip' every ~30s per connected agent"
    why_human: "Round-trip measurement fires on intervals; requires live run to confirm logging"
---

# Phase 3: WebSocket Resilience Verification Report

**Phase Goal:** WebSocket connections survive game launch CPU spikes through ping/pong keepalive; rc-agent reconnects automatically after any drop; kiosk debounces brief absences so staff never see a spurious "Disconnected" during normal game launch; WebSocket round-trips stay fast.
**Verified:** 2026-03-13
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | racecontrol sends WS ping frames every 15s to keep TCP alive during CPU spikes | VERIFIED | `ping_interval = interval(Duration::from_secs(15))` with `MissedTickBehavior::Skip` in `ws/mod.rs:65-66`. `Message::Ping(vec![].into())` sent at line 92. |
| 2 | racecontrol sends app-level Ping every 30s and warns on round-trip >200ms | VERIFIED | `measure_interval = interval(Duration::from_secs(30))` at line 68. `CoreToAgentMessage::Ping { id }` sent at line 99-104. `tracing::warn!` at line 358 when elapsed_ms > 200. |
| 3 | Existing agent message handling is unaffected | VERIFIED | All pre-existing match arms (Register, Heartbeat, Telemetry, LapCompleted, etc.) intact. 83 racecontrol tests + 13 integration tests all pass. |
| 4 | rc-agent reconnects within 1s for attempts 0-2 (fast retry) | VERIFIED | `reconnect_delay_for_attempt(attempt: u32)` at line 1284. Returns `Duration::from_secs(1)` when `attempt < 3`. Tests confirm: attempt 0, 1, 2 all return 1s. |
| 5 | After attempt 3, rc-agent uses exponential backoff capped at 30s | VERIFIED | Formula `(attempt - 2).min(5)` as exponent. Tests verify: attempt 3→2s, 4→4s, 5→8s, 6→16s, 7→30s, 100→30s. |
| 6 | rc-agent responds to CoreToAgentMessage::Ping with AgentMessage::Pong carrying same id | VERIFIED | Match arm at `main.rs:1232`. Creates `AgentMessage::Pong { id }` and sends via `ws_tx`. |
| 7 | reconnect_attempt resets to 0 only on successful connect_async | VERIFIED | `reconnect_attempt = 0` at line 433 (inside `Ok(Ok(stream))` arm only). All 4 delay sites increment counter on failure. |
| 8 | Kiosk header stays green for WS drops under 15s | VERIFIED | `onclose` at line 282: sets 15s timer only if `disconnectTimerRef.current === null`. `setConnected(false)` deferred inside the 15s callback. |
| 9 | Reconnecting within 15s cancels the disconnect timer | VERIFIED | `onopen` at line 69-72: clears `disconnectTimerRef.current` before `setConnected(true)`. |
| 10 | Only the pod card whose data changed re-renders | VERIFIED | `export const KioskPodCard = React.memo(function KioskPodCard(...))` at `KioskPodCard.tsx:76`. Default shallow equality. `setPods` uses Map copy pattern preserving object references for unchanged pods. |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/protocol.rs` | `Ping`/`Pong` variants on `CoreToAgentMessage`/`AgentMessage` | VERIFIED | `CoreToAgentMessage::Ping { id: u64 }` at line 180; `AgentMessage::Pong { id: u64 }` at line 46. Both have roundtrip serde tests. |
| `crates/racecontrol/src/ws/mod.rs` | WS keepalive ping + app-level round-trip measurement | VERIFIED | `ping_interval` at line 65; `measure_interval` at line 68; `tokio::select!` at line 76 with 3 arms; `pending_ping Arc<Mutex>` at line 58. |
| `crates/rc-agent/src/main.rs` | Fast-then-backoff reconnect + Ping handler | VERIFIED | `reconnect_attempt: u32` counter at line 421; `reconnect_delay_for_attempt()` function at line 1284; `CoreToAgentMessage::Ping` match arm at line 1232. |
| `kiosk/src/hooks/useKioskSocket.ts` | 15s disconnect debounce via useRef timer | VERIFIED | `disconnectTimerRef` at line 39 (`useRef`, not `useState`); timer guard at line 282; cleanup in useEffect at line 305. |
| `kiosk/src/components/KioskPodCard.tsx` | React.memo wrapper | VERIFIED | `export const KioskPodCard = React.memo(function KioskPodCard(...))` at line 76. No custom comparator. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `ws/mod.rs` | `protocol.rs` | `CoreToAgentMessage::Ping` sent in measure_interval arm | VERIFIED | Line 99: `CoreToAgentMessage::Ping { id: ping_id }` serialized and sent as WS text. |
| `ws/mod.rs` | `protocol.rs` | `AgentMessage::Pong` received and measured | VERIFIED | Line 350: `AgentMessage::Pong { id }` match arm reads `pending_ping`, computes elapsed, logs warn/debug. |
| `rc-agent/main.rs` | `protocol.rs` | `CoreToAgentMessage::Ping` deserialized, `AgentMessage::Pong` sent | VERIFIED | Line 1232: `CoreToAgentMessage::Ping { id }` match arm; creates `AgentMessage::Pong { id }` at line 1233, sends via `ws_tx`. |
| `useKioskSocket.ts` | `KioskPodCard.tsx` | `connected` state and `pods` Map drive re-render | VERIFIED | `setConnected` in debounce timer (line 284); `setPods` with Map copy (line 91-95) provides reference-stable unchanged pods to React.memo. |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CONN-01 | 03-01 | WS ping/pong keepalive prevents drops during game launch CPU spikes | SATISFIED | 15s `Message::Ping` WS frames in `ws/mod.rs`; `MissedTickBehavior::Skip` prevents burst. Serde tests pass. |
| CONN-02 | 03-03 | Kiosk debounces disconnect — only shows "Disconnected" after 15s+ confirmed absence | SATISFIED | `disconnectTimerRef` with 15s `setTimeout` in `useKioskSocket.ts`. `onopen` cancels timer. Cleanup on unmount. |
| CONN-03 | 03-02 | rc-agent reconnects automatically with short backoff on WS drop | SATISFIED | `reconnect_delay_for_attempt()` with 1s fast retries for attempts 0-2, then 2s/4s/8s/16s/30s. All 4 reconnect sites updated. 3 unit tests pass. |
| PERF-03 | 03-01 | WS command round-trip stays under threshold | SATISFIED | 30s measurement ping; `Arc<Mutex<Option<(u64, Instant)>>>` shared state; `tracing::warn!` at >200ms. Agent responds with Pong. |
| PERF-04 | 03-03 | Kiosk UI interactions feel instant — no unnecessary re-renders | SATISFIED | `React.memo` on `KioskPodCard`; default shallow equality correct given Map copy pattern in `setPods`. |

**Note on REQUIREMENTS.md stale status:** The REQUIREMENTS.md file still marks CONN-01, CONN-03, and PERF-03 as `[ ]` (unchecked) with traceability status "Pending". These are implemented and verified — the file was not updated after Phase 3 completed. This is an info-level documentation gap, not a blocker. CONN-02 and PERF-04 are correctly marked `[x]`.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/rc-agent/src/main.rs` | 1288 | Deviation from plan formula: `(attempt - 2).min(5)` vs plan's `(attempt - 3).min(4)` | Info | Intentional deviation, documented in SUMMARY. Produces **correct** behavior per tests. Attempt 3→2s, 7→30s confirmed. |

No blocker anti-patterns. No stubs, no TODO/FIXME comments in modified files, no empty implementations, no manual WS-level Pong sends (RFC 6455 compliant).

### Human Verification Required

#### 1. Kiosk Disconnect Debounce Under Live CPU Spike

**Test:** Launch AC on any pod, observe kiosk staff screen header during shader compilation (first 10-30s).
**Expected:** Header connection indicator stays green throughout. No flash to "Disconnected" state.
**Why human:** Static analysis confirms the 15s debounce logic is correct. Actual CPU spike behavior requires a live game launch to produce the condition being guarded against.

#### 2. WS Round-Trip Latency Logging

**Test:** Start racecontrol, connect a pod agent, wait 30 minutes. Check racecontrol logs for `WS round-trip` entries.
**Expected:** `tracing::debug!("WS round-trip: Xms (pod_N)")` every ~30 seconds per connected agent. `tracing::warn!` if any round-trip exceeds 200ms.
**Why human:** The measurement interval fires asynchronously after full startup; cannot confirm logging occurs without a running system.

### Gaps Summary

No gaps. All 10 observable truths verified. All 5 artifacts confirmed substantive and wired. All 5 requirements satisfied with implementation evidence. All tests pass: 35 rc-common, 83+13 racecontrol, 47 rc-agent (including 3 new reconnect_delay tests). racecontrol builds cleanly.

One minor documentation gap: REQUIREMENTS.md traceability table shows CONN-01, CONN-03, PERF-03 as "Pending" — should be updated to "Complete" to reflect Phase 3 completion.

---

_Verified: 2026-03-13_
_Verifier: Claude (gsd-verifier)_
