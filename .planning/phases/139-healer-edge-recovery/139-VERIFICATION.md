---
phase: 139-healer-edge-recovery
verified: 2026-03-22T11:30:00+05:30
status: human_needed
score: 7/7 automated must-haves verified
re_verification: false
human_verification:
  - test: "Deploy 139-02 binaries to Pod 8. With pod idle (no billing), observe the lock screen HTTP probe fail (e.g., kill Edge manually on the pod). Wait for the pod_healer cycle. Check server logs for 'dispatching ForceRelaunchBrowser' and the pod rc-agent logs for 'ForceRelaunchBrowser received -- relaunching Edge lock screen'. Then verify the lock screen URL is responsive via HTTP within 30 seconds."
    expected: "Lock screen returns HTTP 200 within 30 seconds of healer action. Server logs show ForceRelaunchBrowser sent. Agent logs show close_browser + launch_browser called."
    why_human: "Requires live pod hardware, actual Edge browser processes, network probe timing, and real WS message delivery -- cannot be verified by static code analysis."
  - test: "With a billing session active on a pod, trigger a lock screen HTTP failure. Confirm the server logs warn 'billing active -- skipping relaunch' and does NOT call close_browser/launch_browser on the agent."
    expected: "No lock screen disruption. Server logs 'billing active -- no relaunch dispatched'. Session continues uninterrupted."
    why_human: "Requires live billing session and real healer cycle to confirm the guard fires correctly in production conditions."
---

# Phase 139: Healer Edge Recovery Verification Report

**Phase Goal:** The racecontrol pod healer can trigger a full Edge relaunch on any pod via a new WS protocol message -- no SSH, no exec endpoint, just the existing WebSocket connection
**Verified:** 2026-03-22T11:30:00+05:30
**Status:** human_needed (all automated checks passed; 2 runtime behaviors need live-pod confirmation)
**Re-verification:** No -- initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `CoreToAgentMessage::ForceRelaunchBrowser` exists and serializes as snake_case via serde | VERIFIED | Line 483 of protocol.rs; serde tag+content + rename_all=snake_case on enum (lines 275-276). Test `test_force_relaunch_browser_roundtrip` passes. |
| 2 | Rule 2 in pod_healer.rs dispatches ForceRelaunchBrowser via agent_senders when WS connected + no billing | VERIFIED | Lines 213-241 of pod_healer.rs. Reads agent_senders, checks has_active_billing, pushes `relaunch_lock_screen` HealAction. Test `relaunch_lock_screen_dispatched_when_ws_connected_no_billing` passes. |
| 3 | Rule 2 skips dispatch (logs warn) when billing is active | VERIFIED | Lines 217-225 of pod_healer.rs. `if has_active_billing { tracing::warn!(...); issues.push(...); }`. Test `relaunch_not_dispatched_when_billing_active` passes. |
| 4 | `execute_heal_action` handles "relaunch_lock_screen" without a shell command | VERIFIED | Lines 582-607 of pod_healer.rs. Early-return arm sends `ForceRelaunchBrowser` via WS and returns before reaching the `cmd` match block. |
| 5 | rc-agent handles `ForceRelaunchBrowser` in ws_handler.rs before the catch-all | VERIFIED | Line 961 of ws_handler.rs. Arm is at line 961; catch-all `other =>` is at line 983. Correct ordering confirmed. |
| 6 | Handler calls close_browser then launch_browser; gated on billing_active | VERIFIED | Lines 964-980 of ws_handler.rs. `billing_active.load(Relaxed)` guard; false branch calls `state.lock_screen.close_browser()` then `state.lock_screen.launch_browser()`. |
| 7 | All three crate test suites pass for this feature | VERIFIED | `test_force_relaunch_browser_roundtrip` (rc-common): ok. `relaunch_lock_screen_action_string` (racecontrol): ok. `force_relaunch_browser_variant_exists` (rc-agent): ok. |

**Score:** 7/7 automated truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/protocol.rs` | `ForceRelaunchBrowser { pod_id: String }` variant + snake_case serde | VERIFIED | Line 483. Serde attributes confirmed at lines 275-276. Roundtrip test at lines 2410-2420. |
| `crates/racecontrol/src/pod_healer.rs` | `relaunch_lock_screen` HealAction + Rule 2 WS dispatch + execute_heal_action arm | VERIFIED | Rule 2 at lines 213-241. execute_heal_action arm at lines 582-607. 4 new unit tests at lines 998-1044. |
| `crates/rc-agent/src/ws_handler.rs` | `ForceRelaunchBrowser` match arm before catch-all | VERIFIED | Arm at line 961, catch-all at line 983. Billing guard at line 964. close_browser+launch_browser at lines 974-975. Test at lines 993-1005. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `pod_healer.rs` Rule 2 | `state.agent_senders` | `agent_senders.read().await` + `sender.send(CoreToAgentMessage::ForceRelaunchBrowser)` | WIRED | Lines 584-598. Pattern: read senders, get by pod_id, send with match Ok/Err. No unwrap. |
| `ws_handler.rs` ForceRelaunchBrowser arm | `state.lock_screen.close_browser()` + `state.lock_screen.launch_browser()` | `state.heartbeat_status.billing_active.load(Relaxed)` guard | WIRED | Lines 964-975. Guard fires first; if not billing, calls both methods in sequence. |
| Protocol serialization | WS wire format | serde tag+content snake_case on `CoreToAgentMessage` | WIRED | `{"type":"force_relaunch_browser","data":{"pod_id":"..."}}` — confirmed by roundtrip test. |

---

### Requirements Coverage

REQUIREMENTS.md does not list HEAL-01/02/03 with standalone definitions (no grep hits). Requirements are defined implicitly in the ROADMAP.md success criteria and PLAN must_haves. Coverage per PLAN frontmatter:

| Requirement | Source Plan | Description (from PLAN) | Status | Evidence |
|-------------|------------|-------------------------|--------|---------|
| HEAL-01 | 139-01-PLAN.md | ForceRelaunchBrowser WS protocol variant in rc-common | SATISFIED | Variant at protocol.rs:483; serde roundtrip test passes |
| HEAL-02 | 139-01-PLAN.md | pod_healer Rule 2 WS dispatch + execute_heal_action arm | SATISFIED | Lines 213-241 and 582-607 of pod_healer.rs; unit tests pass |
| HEAL-03 | 139-02-PLAN.md | rc-agent ForceRelaunchBrowser handler + billing guard | SATISFIED | Lines 961-981 of ws_handler.rs; billing guard confirmed; test passes |

Note: 139-02-PLAN.md is not marked `[x]` in ROADMAP.md (still shows `[ ]`). However, the code is fully implemented and tests pass. This is a documentation oversight — the implementation is real.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/racecontrol/src/pod_healer.rs` | 146 | `.unwrap()` on ping result | Info | Pre-existing, not introduced by this phase. Not in new code paths. |

No TODO, FIXME, placeholder, empty handler, or return-null patterns found in the three modified files within the new code regions.

---

### Human Verification Required

#### 1. Full Round-Trip Lock Screen Recovery on Live Pod

**Test:** Deploy updated racecontrol.exe and rc-agent.exe to Pod 8. With no active billing session, simulate lock screen HTTP failure (kill msedge.exe on the pod). Wait one pod_healer cycle. Observe server logs for `dispatching ForceRelaunchBrowser` and agent logs for `ForceRelaunchBrowser received -- relaunching Edge lock screen`. Then probe the lock screen URL via HTTP.

**Expected:** Lock screen returns HTTP 200 within 30 seconds of the healer action. Server and agent logs confirm the full message path. Edge browser process is running on the pod after recovery.

**Why human:** Requires live pod hardware, actual Edge browser processes, real WS message delivery, and timed HTTP probe. Static analysis cannot verify the 30-second SLA or confirm Edge actually relaunches.

#### 2. Billing Guard Blocks Relaunch During Active Session

**Test:** Start a billing session on Pod 8. Simulate lock screen HTTP failure while session is active. Observe healer logs. Confirm the session is NOT disrupted.

**Expected:** Server logs `billing active -- no relaunch dispatched`. No `ForceRelaunchBrowser` sent. Billing session continues normally. Agent lock screen is not touched.

**Why human:** Requires a live billing session running concurrently with a healer cycle. Cannot verify the timing interaction or session continuity programmatically.

---

### Gaps Summary

No automated gaps. All 7 must-have truths are verified in the codebase:

- The complete server-to-pod chain is wired: healer detects (Rule 2) -> queues HealAction -> execute_heal_action sends WS message -> agent handles and calls close_browser + launch_browser.
- Billing guard is present at both ends: server skips dispatch if billing active; agent skips relaunch if billing active.
- No SSH, no exec endpoint -- the entire path uses the existing WebSocket connection as required by the phase goal.
- No unwrap in new code; match-based error handling throughout.

Two human tests remain to confirm live runtime behavior (30s recovery SLA and billing guard under real load).

---

_Verified: 2026-03-22T11:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
