---
phase: 68-pod-switchcontroller
verified: 2026-03-20T09:30:00+05:30
status: human_needed
score: 6/8 must-haves verified (2 require live deploy)
re_verification: false
human_verification:
  - test: "Deploy rc-agent.toml with failover_url to all 8 pods via pendrive"
    expected: "All 8 pods have core.failover_url = \"ws://100.70.177.44:8080/ws/agent\" in C:\\RacingPoint\\rc-agent.toml"
    why_human: "TOML config deployment to physical pods — no code artifact to verify, requires physical/remote deployment action"
  - test: "Pod 8 canary — send SwitchController to Pod 8 via racecontrol WS, observe logs"
    expected: "rc-agent on Pod 8 logs \"[switch] SwitchController received: switching to ws://100.70.177.44:8080/ws/agent\", reconnects to Bono VPS within 15s, rc-agent.exe does NOT restart"
    why_human: "Requires live pod with active WS connection and racecontrol server — cannot simulate reconnect behavior programmatically"
  - test: "self_monitor suppression window — after SwitchController, watch Pod 8 logs for 60s"
    expected: "No RELAUNCH log line in the 60s following a SwitchController; after 60s, if WS is still dead, relaunch fires normally"
    why_human: "Requires real-time log monitoring over a 60s window on a live pod"
  - test: "Switch back to .23 — send SwitchController with primary URL to Pod 8"
    expected: "rc-agent reconnects to ws://192.168.31.23:8080/ws/agent; billing heartbeat resumes; UDP heartbeat ws_connected returns true"
    why_human: "Requires live pod + live racecontrol on .23 to confirm billing heartbeat continuity"
---

# Phase 68: Pod SwitchController Verification Report

**Phase Goal:** Any rc-agent pod can switch its WebSocket target from .23 to Bono's VPS and back at runtime without a process restart, and self_monitor will not fight the intentional switch
**Verified:** 2026-03-20T09:30:00+05:30 (IST)
**Status:** human_needed
**Re-verification:** No — initial verification

## Requirements Coverage

FAIL-01 through FAIL-04 are phase-local requirements defined in ROADMAP.md and 68-RESEARCH.md. They do NOT appear in `.planning/REQUIREMENTS.md` (which covers v10.0/v11.0 feature requirements only). This is expected — failover requirements belong to a separate v12.0/failover milestone that is being built phase by phase. No orphaned requirements.

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| FAIL-01 | 68-01 | rc-agent has failover_url in CoreConfig pointing to Bono's racecontrol via Tailscale | SATISFIED | `failover_url: Option<String>` with `#[serde(default)]` at main.rs:178-179; validate_config extension at main.rs:2868-2875; 3 unit tests pass |
| FAIL-02 | 68-02 | rc-agent WS reconnect loop uses Arc<RwLock<String>> for runtime URL switching | SATISFIED | `active_url: Arc<RwLock<String>>` at main.rs:934; `active_url.read().await.clone()` inside outer loop at main.rs:941; `connect_async(&url)` at main.rs:945 |
| FAIL-03 | 68-01, 68-02 | New SwitchController AgentMessage triggers rc-agent URL switch without process restart | SATISFIED | `SwitchController { target_url: String }` in protocol.rs:403-405; match arm in main.rs:2742-2768 writes RwLock, stores last_switch_ms, sends Close frame, breaks inner loop |
| FAIL-04 | 68-01, 68-02 | self_monitor suppresses relaunch during intentional failover (last_switch_ms guard) | SATISFIED | guard at self_monitor.rs:84-95; `switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000` at line 90; 3 unit tests |

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SwitchController variant exists in CoreToAgentMessage and round-trips through serde | VERIFIED | protocol.rs:403-405; serde test at protocol.rs:2230-2244 |
| 2 | CoreConfig has failover_url: Option<String> with serde(default) | VERIFIED | main.rs:178-179 |
| 3 | validate_config rejects non-ws:// failover_url values | VERIFIED | main.rs:2868-2875; 3 tests at main.rs:3456-3507 |
| 4 | HeartbeatStatus has last_switch_ms: AtomicU64 initialized to 0 | VERIFIED | udp_heartbeat.rs:38, 49; test at udp_heartbeat.rs:174-177 |
| 5 | Reconnect loop reads URL from Arc<RwLock<String>> on each iteration | VERIFIED | main.rs:941 inside outer loop; connect_async receives local `url` clone, not config.core.url |
| 6 | SwitchController handler validates URL, writes RwLock, records last_switch_ms, breaks for reconnect | VERIFIED | main.rs:2742-2768; strict allowlist check (primary+failover only); Close frame sent before break |
| 7 | self_monitor suppresses WS-dead relaunch for 60s after SwitchController | VERIFIED | self_monitor.rs:82-96; "suppressing relaunch" log path confirmed |
| 8 | All 8 pods have failover_url configured in rc-agent.toml (pendrive deploy) | NEEDS HUMAN | Deployment to physical pods — not a code artifact |

**Score:** 7/8 truths verified programmatically (1 requires physical deployment confirmation)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/protocol.rs` | SwitchController { target_url: String } variant in CoreToAgentMessage | VERIFIED | Lines 400-405; serde round-trip test at line 2230 |
| `crates/rc-agent/src/udp_heartbeat.rs` | last_switch_ms: AtomicU64 field on HeartbeatStatus | VERIFIED | Lines 38, 49; AtomicU64 imported; test at line 174 |
| `crates/rc-agent/src/main.rs` | failover_url on CoreConfig + validate_config extension + active_url RwLock + SwitchController match arm | VERIFIED | failover_url:178; validate:2868; active_url:934; handler:2742 |
| `crates/rc-agent/src/self_monitor.rs` | last_switch_ms guard before WS-dead relaunch + log_event pub | VERIFIED | guard:84-95; `pub fn log_event` at line 147 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| main.rs SwitchController handler | udp_heartbeat.rs HeartbeatStatus.last_switch_ms | `heartbeat_status.last_switch_ms.store(now_ms, Ordering::Relaxed)` | WIRED | main.rs:2761 |
| main.rs reconnect loop | active_url Arc<RwLock<String>> | `active_url.read().await.clone()` inside outer loop | WIRED | main.rs:941 |
| self_monitor.rs WS-dead check | udp_heartbeat.rs HeartbeatStatus.last_switch_ms | `status.last_switch_ms.load(Ordering::Relaxed)` | WIRED | self_monitor.rs:84 |
| main.rs SwitchController handler | self_monitor.rs log_event | `self_monitor::log_event(&format!("SWITCH: target={}", target_url))` | WIRED | main.rs:2763 |

### Anti-Patterns Found

No blocker anti-patterns detected. No TODO/FIXME/placeholder comments in modified files. No stub implementations. No empty handlers. The SwitchController handler is fully wired (not just logging or preventDefault equivalent).

### Human Verification Required

#### 1. Pod TOML Deploy — failover_url on all 8 pods

**Test:** Update `C:\RacingPoint\rc-agent.toml` on each pod to include `failover_url = "ws://100.70.177.44:8080/ws/agent"` under the `[core]` section. Restart rc-agent.exe on each pod. Confirm no startup errors in rc-agent logs (config validation will accept the new field).

**Expected:** rc-agent starts cleanly; tracing log shows "Core server: ws://192.168.31.23:8080/ws/agent" (primary unchanged); no validate_config errors.

**Why human:** Physical pendrive deploy or remote exec via rc-agent :8090. No code artifact to verify — this is a runtime configuration step.

#### 2. Pod 8 canary — SwitchController round-trip

**Test:** With Pod 8 running, send `{"type":"switch_controller","data":{"target_url":"ws://100.70.177.44:8080/ws/agent"}}` to rc-agent via racecontrol WS. Observe Pod 8 rc-agent logs.

**Expected:** Log line `[switch] SwitchController received: switching to ws://100.70.177.44:8080/ws/agent` appears; rc-agent sends WS Close frame; reconnect loop picks up new URL; connection to Bono VPS established within 15s; rc-agent.exe PID does NOT change (no restart).

**Why human:** Requires live Pod 8 with active WS connection to racecontrol. Cannot simulate reconnect behavior against a real WS server programmatically.

#### 3. self_monitor suppression window

**Test:** After sending SwitchController to Pod 8, deliberately keep the new URL unreachable for up to 90s. Watch Pod 8 rc-agent logs.

**Expected:** In the first 60s: logs show `[rc-bot] WS dead Xs but SwitchController received Yms ago — suppressing relaunch`. After 60s: if still unreachable, normal `[rc-bot] WebSocket dead Xs — relaunching to reestablish` fires.

**Why human:** Requires real-time log monitoring over a 60s+ window on a live pod with controlled network conditions.

#### 4. Switch back to .23

**Test:** After Pod 8 has switched to Bono VPS, send `{"type":"switch_controller","data":{"target_url":"ws://192.168.31.23:8080/ws/agent"}}` to switch back.

**Expected:** rc-agent reconnects to .23; billing heartbeat UDP packets resume; fleet health dashboard shows Pod 8 ws_connected = true against .23.

**Why human:** Requires live pod + live racecontrol on .23 to confirm billing continuity. Cannot verify state machine correctness of billing_guard after a switch-back without actual session data.

### Gaps Summary

No code gaps. All 8 plan must-haves are implemented and wired. The 4 human verification items are deployment and live-system checks, not code deficiencies. The phase goal is fully implemented in code — the only outstanding work is the pendrive deploy of updated `rc-agent.toml` to all 8 pods and canary verification on Pod 8.

The FAIL-01 through FAIL-04 requirements are not tracked in `.planning/REQUIREMENTS.md` (which covers v10.0/v11.0 milestones). They are phase-local requirements defined in ROADMAP.md and 68-RESEARCH.md. This is a traceability gap in REQUIREMENTS.md but is not a code deficiency — the implementation is complete.

---

_Verified: 2026-03-20T09:30:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
