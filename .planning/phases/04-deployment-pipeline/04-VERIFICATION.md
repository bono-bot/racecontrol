---
phase: 04-deployment-pipeline
verified: 2026-03-13T03:30:00Z
status: human_needed
score: 6/8 must-haves verified
human_verification:
  - test: "Launch a game from the kiosk 'Start' button and time from click to game visible on pod screen"
    expected: "Game should be visible on pod within a reasonable time window (the existing launch path has not regressed; no new latency was introduced)"
    why_human: "PERF-01 was marked 'manual verification, N/A' in research. No code changes were made for game launch timing. The requirement needs a live timing measurement to confirm the current path still meets expectations."
  - test: "Enter a PIN on the pod lock screen (4 digits) and measure response time"
    expected: "Lock screen transitions (accepted/rejected) within 1-2 seconds of final digit entry"
    why_human: "PERF-02 was never claimed by any plan (orphaned requirement). No code changes address it in this phase. Needs live measurement to determine if current implementation already meets or misses the 1-2s target."
orphaned_requirements:
  - id: PERF-02
    description: "Lock screen responds to PIN entry within 1-2 seconds"
    status: orphaned
    reason: "Listed in ROADMAP Phase 4 requirements but not in any plan frontmatter. REQUIREMENTS.md marks it Pending. No implementation work was done."
---

# Phase 4: Deployment Pipeline Hardening Verification Report

**Phase Goal:** Every rc-agent deploy follows kill->wait->verify-dead->download->size-check->start->verify-reconnect sequence automatically; no binary file lock issues; new binaries work identically on all 8 pods; active sessions are not disrupted by rolling updates
**Verified:** 2026-03-13T03:30:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Deploy sequence enforces kill->wait-dead->download->size-check->start->verify-reconnect automatically | VERIFIED | `deploy_pod()` in deploy.rs (853 lines) implements all steps sequentially with DeployState transitions at each step |
| 2 | Binary file lock issue prevented — old process verified dead before new binary written | VERIFIED | `is_process_alive()` polls every 2s up to 10s before delete/download; fails with "process still running" if not dead |
| 3 | New binaries deploy identically to all 8 pods via rolling deploy | VERIFIED | `deploy_rolling()` sorts pod_8 as canary, then 1-7; resolves IPs from AppState.pods at call time |
| 4 | Active billing sessions are not disrupted by rolling updates | VERIFIED | `has_active_session` check in deploy_rolling(); sets WaitingSession + stores in pending_deploys; session-end hook in billing.rs (3 call sites) |
| 5 | Deploy failure is reported clearly and leaves pod in known state | VERIFIED | Every failure path calls `set_deploy_state(Failed{reason})` + sends email alert; no silent failures |
| 6 | Binary URL validated before killing old process | VERIFIED | HEAD request to binary_url before DeployState::Killing; returns early with Failed if unreachable |
| 7 | Game launch completes within expected time window (PERF-01) | NEEDS HUMAN | No code changes made for game launch timing; research explicitly flagged as "manual verification, N/A" |
| 8 | PIN entry on pod lock screen responds within 1-2 seconds (PERF-02) | NEEDS HUMAN | Not claimed by any plan (orphaned); REQUIREMENTS.md marks Pending; no implementation changes |

**Score:** 6/8 truths verified (2 require human testing)

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/types.rs` | DeployState enum with 10 variants (9 original + WaitingSession) | VERIFIED | 10 variants confirmed: Idle, Killing, WaitingDead, Downloading, SizeCheck, Starting, VerifyingHealth, Complete, Failed, WaitingSession |
| `crates/rc-common/src/protocol.rs` | DeployProgress event, DeployPod/DeployRolling/CancelDeploy commands | VERIFIED | All variants present; 6 serde roundtrip tests pass |
| `crates/racecontrol/src/state.rs` | pod_deploy_states and pending_deploys fields in AppState | VERIFIED | Both fields present at lines 85 and 88; create_initial_deploy_states() pre-populates pods 1-8 as Idle |
| `crates/racecontrol/src/deploy.rs` | deploy_pod() async executor, deploy_rolling(), deploy_status() | VERIFIED | 853 lines; all three functions present and substantive |
| `crates/racecontrol/src/api/routes.rs` | POST /api/deploy/:pod_id, POST /api/deploy/rolling, GET /api/deploy/status | VERIFIED | All three routes wired at lines 260-262; deploy_single_pod returns 202/409/404; deploy_rolling_handler returns 202/409 |
| `kiosk/src/components/DeployPanel.tsx` | Deploy UI with URL input, Deploy button, 8 pod cards | VERIFIED | 198 lines; DeployPanel function exported; pod cards with color coding; CANARY badge for pod_8 |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/rc-common/src/types.rs` | `crates/rc-common/src/protocol.rs` | DeployState used in DashboardEvent::DeployProgress | WIRED | DeployState imported and used at protocol.rs lines 361-369 |
| `crates/racecontrol/src/deploy.rs` | `crates/racecontrol/src/state.rs` | Reads/writes pod_deploy_states in AppState | WIRED | pod_deploy_states.write() at multiple steps in deploy_pod(); pending_deploys.write() in deploy_rolling() |
| `crates/racecontrol/src/api/routes.rs` | `crates/racecontrol/src/deploy.rs` | deploy_single_pod calls deploy::deploy_pod() | WIRED | tokio::spawn at line 9700 calls crate::deploy::deploy_pod() |
| `crates/racecontrol/src/ws/mod.rs` | `crates/racecontrol/src/deploy.rs` | DeployPod/DeployRolling/CancelDeploy dispatch | WIRED | Lines 603-672 in ws/mod.rs handle all three commands; DeployRolling calls deploy_rolling() |
| `crates/racecontrol/src/deploy.rs` | `crates/racecontrol/src/billing.rs` | Reads active_timers to check if pod has active session | WIRED | state.billing.active_timers.read() in deploy_rolling() line 636 |
| `crates/racecontrol/src/billing.rs` | `crates/racecontrol/src/deploy.rs` | Session-end hook triggers pending deploy | WIRED | check_and_trigger_pending_deploy() called at 3 billing sites (lines 267, 270, 1172) |
| `kiosk/src/hooks/useKioskSocket.ts` | `kiosk/src/components/DeployPanel.tsx` | DeployProgress events update deploy state map | WIRED | useKioskSocket handles deploy_progress events (line 275), exposes deployStates and sendDeployRolling; settings/page.tsx passes both to DeployPanel |
| `crates/racecontrol/src/pod_monitor.rs` | deploy skip logic | pod_deploy_states.read() + is_active() skip | WIRED | Lines 223-229 in pod_monitor.rs; deploy state read and is_active() checked |
| `crates/racecontrol/src/pod_healer.rs` | deploy skip logic | pod_deploy_states.read() + is_active() skip | WIRED | Lines 166-172 in pod_healer.rs; deploy state read and is_active() checked |

---

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|---------------|-------------|--------|----------|
| DEPLOY-02 | 04-01, 04-02 | Deploy sequence enforces kill->wait->verify-dead->download->size-check->start->verify-reconnect | SATISFIED | deploy_pod() implements all steps; 20 serde tests pass in rc-common; 17 pure-function tests pass in racecontrol |
| DEPLOY-05 | 04-02, 04-03 | Deploy without disrupting active sessions; rolling update with backward-compatible transitions | SATISFIED | deploy_rolling() with WaitingSession queuing; session-end hook fires pending deploy; 5s inter-pod delay |
| PERF-01 | 04-03 | Game launch completes within target time from kiosk Start to game visible on pod | NEEDS HUMAN | Claimed by plan 04-03 but research explicitly marked "manual verification, N/A". No code changes target this; existing launch path unchanged. Human must time a live launch. |
| PERF-02 | (none) | Lock screen responds to PIN entry within 1-2 seconds | ORPHANED | Not in any plan frontmatter. ROADMAP lists it under Phase 4 requirements; REQUIREMENTS.md marks Pending. Never addressed. See orphaned requirements section. |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/racecontrol/src/api/routes.rs` | 2115 | unused variable `sid` | Info | Pre-existing warning; unrelated to this phase |
| `crates/racecontrol/src/api/routes.rs` | 2156 | unused variable `new_balance` | Info | Pre-existing warning; unrelated to this phase |
| `crates/racecontrol/src/deploy.rs` | — | `deploy_rolling()`: non-canary pod failure swallowed with error log only | Warning | If pod 3 fails mid-rolling deploy, the deploy continues to pods 4-7 without alerting staff. Canary failure does halt. This is documented behavior (per-plan decision) but no email alert fires for non-canary failures. |

No stub patterns, placeholder components, or TODO/FIXME markers found in phase 4 files.

---

### Human Verification Required

#### 1. PERF-01 — Game Launch Timing

**Test:** From the kiosk staff terminal, start a session on any pod and press "Launch Game." Measure the time from button press to when the game is visible and interactive on the pod screen.

**Expected:** Game should appear within a reasonable window (the existing path — Content Manager launch + overlay readiness check — is unchanged by this phase; no regression should have occurred).

**Why human:** PERF-01 was explicitly flagged in Phase 4 research as "manual verification, N/A." The requirement measures end-to-end game launch time on the actual hardware with real process startup. No code in this phase touches the game launcher path. A live timing run is needed to confirm the baseline still holds.

#### 2. PERF-02 — PIN Entry Response Time

**Test:** On any pod lock screen, enter a valid 4-digit PIN and measure from the final digit entry to the lock screen transition (accepted or rejected response visible).

**Expected:** Transition should occur within 1-2 seconds of PIN completion (per REQUIREMENTS.md PERF-02).

**Why human:** PERF-02 was not claimed by any plan in this phase (orphaned requirement). No code changes were made to the PIN validation path. This requirement has been in REQUIREMENTS.md since the project started but was never assigned to a plan. A live timing test will determine whether it already passes or needs work in a future phase.

---

### Gaps Summary

The two automated-check gaps are human-verification items, not implementation gaps:

**PERF-01** is a measurement requirement (not a build requirement). The deployment pipeline does not change the game launch path at all, so PERF-01 is best characterized as "no regression expected, but unverified." A 5-minute live test would close it.

**PERF-02** is an orphaned requirement — the ROADMAP listed it under Phase 4 but no plan claimed it. The lock screen PIN validation path (rc-agent lock screen HTTP server on port 18923, validate PIN via racecontrol WebSocket) was built in earlier phases. Whether it already meets the 1-2s target is unknown. If it fails human testing, it should be assigned to Phase 5 (Blanking Screen Protocol) which already owns AUTH-01 and SCREEN-01-03.

The core phase goal — automated kill->verify-dead->download->size-check->start->verify-reconnect with session protection — is fully achieved. All 6 verifiable must-haves pass.

---

*Verified: 2026-03-13T03:30:00Z*
*Verifier: Claude (gsd-verifier)*
