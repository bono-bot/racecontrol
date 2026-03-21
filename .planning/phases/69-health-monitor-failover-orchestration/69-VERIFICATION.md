---
phase: 69-health-monitor-failover-orchestration
verified: 2026-03-21T08:15:00+05:30
status: gaps_found
score: 8/10 must-haves verified
re_verification: false
gaps:
  - truth: "Uday receives an email and WhatsApp notification within 2 minutes of failover completing, stating which URL pods switched to"
    status: failed
    reason: "notify_failover command is not registered in shared/exec-protocol.js COMMAND_REGISTRY. When James sends exec_request with command='notify_failover', Bono's ExecHandler returns exitCode=-1 with 'Unknown command: notify_failover'. Uday receives no notification via the primary failover path."
    artifacts:
      - path: "C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js"
        issue: "Sends exec_request with command='notify_failover' (line 209) but this command is not in COMMAND_REGISTRY"
      - path: "C:/Users/bono/racingpoint/comms-link/shared/exec-protocol.js"
        issue: "COMMAND_REGISTRY has activate_failover and racecontrol_health but no notify_failover entry"
    missing:
      - "Add notify_failover to COMMAND_REGISTRY in shared/exec-protocol.js with appropriate binary/args to send WhatsApp via Evolution API (or sendEvolutionText directly from failover-orchestrator.js using env vars on James)"

  - truth: "Uday receives an email and WhatsApp notification within 2 minutes of failover completing, stating which URL pods switched to"
    status: failed
    reason: "In the Bono secondary watchdog path (venue power outage scenario), bono/index.js line 354 calls alertManager.handleNotification({text}) but AlertManager has no handleNotification method — only handleJamesDown and handleRecovery. This throws TypeError at runtime, caught by the outer try/catch at line 356, silently discarding the notification."
    artifacts:
      - path: "C:/Users/bono/racingpoint/comms-link/bono/index.js"
        issue: "Line 354: alertManager.handleNotification({ text }) — AlertManager has no handleNotification method; causes TypeError caught silently"
      - path: "C:/Users/bono/racingpoint/comms-link/bono/alert-manager.js"
        issue: "AlertManager exposes handleJamesDown() and handleRecovery() — no handleNotification method"
    missing:
      - "Replace alertManager.handleNotification({ text }) with sendEvolutionText({...}) directly, or add a handleNotification method to AlertManager"

  - truth: "Uday receives an email and WhatsApp notification within 2 minutes of failover completing, stating which URL pods switched to"
    status: failed
    reason: "Success criterion 5 requires 'email AND WhatsApp notification'. No email notification is implemented in any failover path — neither failover-orchestrator.js nor bono/index.js watchdog sends an email. ROADMAP success criterion explicitly states 'email and WhatsApp'."
    artifacts:
      - path: "C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js"
        issue: "No email notification — only a failed WhatsApp path via notify_failover exec_request"
      - path: "C:/Users/bono/racingpoint/comms-link/bono/index.js"
        issue: "No email notification in watchdog — only broken WhatsApp via handleNotification"
    missing:
      - "Implement email notification on failover, either via Bono's send_email.js path or a new notify_failover command that sends both WhatsApp and email"

human_verification:
  - test: "End-to-end failover timing validation"
    expected: "When server .23 is powered off, failover fires exactly at 60s (not sooner due to AC-launch CPU spike, not later) and all 8 pods show connected to Bono's VPS WebSocket within 30s of broadcast"
    why_human: "Requires physically powering off .23 and timing the sequence; cannot be verified by static analysis"
  - test: "Split-brain guard pod behavior"
    expected: "If .23 is still LAN-reachable from a specific pod when SwitchController arrives, that pod stays on .23; once .23 goes unreachable the pod accepts the next SwitchController"
    why_human: "Requires physical network partition testing across actual pod hardware"
---

# Phase 69: Health Monitor & Failover Orchestration Verification Report

**Phase Goal:** James automatically detects when .23 is unreachable, waits to confirm it is not a transient AC-launch CPU spike, then coordinates with Bono to promote cloud racecontrol as primary and switch all pods — with Uday notified
**Verified:** 2026-03-21T08:15:00+05:30 (IST)
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | James's health probe loop runs every 5s, status visible without manual intervention | VERIFIED | health-monitor.js: PROBE_INTERVAL_MS=5_000, setInterval calls #probe(), state_change events logged |
| 2 | Failover fires only after continuous 60s outage — 3s AC-launch CPU spike does NOT trigger | VERIFIED | DOWN_THRESHOLD=12 x 5s = 60s; single cycleOk=true resets consecutiveFailures to 0 |
| 3 | After failover fires: all 8 pods connected to Bono's VPS within 30s of SwitchController broadcast | VERIFIED (partial) | failover_broadcast endpoint iterates agent_senders and sends SwitchController; rc-agent handles switch — timing verification needs human |
| 4 | A pod that still has .23 reachable does NOT honor SwitchController until LAN probe confirms .23 down | VERIFIED | rc-agent main.rs: split_brain_probe probes 192.168.31.23:8090/ping with 2s timeout before acting |
| 5 | Uday receives email AND WhatsApp notification within 2 minutes of failover completing | FAILED | notify_failover not in COMMAND_REGISTRY; alertManager.handleNotification does not exist; no email path |

**Score:** 4/5 truths fully verified (truth 3 needs human for timing; truth 5 fails automated checks)

---

## Required Artifacts

### Plan 69-01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `comms-link/james/health-monitor.js` | HealthMonitor class with hysteresis FSM | VERIFIED | 187 lines, exports HealthMonitor, DOWN_THRESHOLD=12, PROBE_INTERVAL_MS=5000, correct FSM |
| `comms-link/james/failover-orchestrator.js` | Failover orchestration sequence | VERIFIED | 241 lines, exports FailoverOrchestrator, full 7-step sequence implemented |

### Plan 69-02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/api/routes.rs` | POST /api/v1/failover/broadcast endpoint | VERIFIED | Route registered, handler auth-protected, iterates agent_senders, returns sent/total JSON |
| `crates/rc-agent/src/main.rs` | Split-brain guard in SwitchController handler | VERIFIED | split_brain_probe created once before outer loop, guards with 2s timeout HTTP probe |

### Plan 69-03 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `comms-link/bono/index.js` | Secondary watchdog timer in james_down handler | VERIFIED | httpProbe helper, secondaryWatchdogTimer, 255s delay, Tailscale probe, pm2 start, broadcast POST |

---

## Key Link Verification

### Plan 69-01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| james/health-monitor.js | james/failover-orchestrator.js | EventEmitter 'server_down' event | WIRED | index.js line 595: `healthMonitor.on('server_down', () => failoverOrchestrator.initiateFailover())` |
| james/failover-orchestrator.js | comms-link exec_request | `activate_failover` command | WIRED | failover-orchestrator.js line 115: `command: 'activate_failover'` — command IS in COMMAND_REGISTRY |
| james/failover-orchestrator.js | cloud racecontrol /api/v1/failover/broadcast | httpPost to cloud endpoint | WIRED | failover-orchestrator.js line 171: `'http://100.70.177.44:8080/api/v1/failover/broadcast'` |

### Plan 69-02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| routes.rs failover_broadcast | state.agent_senders | iterate senders, send SwitchController | WIRED | routes.rs lines 11861-11870: agent_senders.iter() + CoreToAgentMessage::SwitchController |
| rc-agent SwitchController handler | http://192.168.31.23:8090/ping | reqwest GET with 2s timeout | WIRED | main.rs lines 2590-2597: split_brain_probe.get("http://192.168.31.23:8090/ping").send() |

### Plan 69-03 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| bono/index.js james_down handler | http://100.71.226.83:8090/ping | Tailscale probe after 5-min timer | WIRED | bono/index.js line 302: `httpProbe('http://100.71.226.83:8090/ping', 5000)` |
| bono/index.js watchdog | localhost:8080/api/v1/failover/broadcast | local HTTP POST after pm2 start | WIRED | bono/index.js line 343: `httpPost('http://localhost:8080/api/v1/failover/broadcast', ...)` |

### Critical Broken Link

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| james/failover-orchestrator.js | Uday WhatsApp notification | exec_request command='notify_failover' | NOT_WIRED | `notify_failover` absent from shared/exec-protocol.js COMMAND_REGISTRY — Bono's ExecHandler rejects with "Unknown command: notify_failover" |
| bono/index.js watchdog | Uday WhatsApp notification | alertManager.handleNotification() | NOT_WIRED | AlertManager has no `handleNotification` method — only `handleJamesDown` and `handleRecovery`. Line 354 throws TypeError, caught by outer try/catch. |

---

## Requirements Coverage

The requirement IDs HLTH-01 through ORCH-04 are defined only in ROADMAP.md (not in REQUIREMENTS.md). They are inferred from plan-to-success-criterion mapping:

| Requirement | Source Plan | Inferred Meaning | Status | Evidence |
|-------------|-------------|-----------------|--------|---------|
| HLTH-01 | 69-01 | Health probe loop every 5s | SATISFIED | HealthMonitor PROBE_INTERVAL_MS=5000 |
| HLTH-02 | 69-01 | 60s hysteresis, single success resets counter | SATISFIED | DOWN_THRESHOLD=12, consecutiveFailures=0 on cycleOk |
| HLTH-03 | 69-01 | Tailscale fallback only when both LAN probes fail | SATISFIED | `lanOk ? false : await #httpGet(TAILSCALE_URL)` |
| HLTH-04 | 69-03 | Bono secondary watchdog for venue power outage | SATISFIED (mechanics) | 255s timer, dual-condition gate, pm2 start, broadcast POST |
| ORCH-01 | 69-01 | James sends activate_failover to Bono before broadcasting | SATISFIED | exec_request step 2 precedes broadcast step 5 |
| ORCH-02 | 69-02 | Cloud racecontrol broadcast endpoint sends SwitchController | SATISFIED | POST /api/v1/failover/broadcast iterates agent_senders |
| ORCH-03 | 69-02 | rc-agent split-brain guard verifies .23 unreachable before switching | SATISFIED | split_brain_probe with 2s timeout |
| ORCH-04 | 69-01 | Uday notified after failover | BLOCKED | notify_failover not in COMMAND_REGISTRY; WhatsApp path broken in both primary and watchdog paths; no email notification |

**REQUIREMENTS.md cross-reference:** HLTH-01 through ORCH-04 are NOT in REQUIREMENTS.md — they exist only in ROADMAP.md Phase 69 section and plan frontmatter. This is an orphaned requirement set (defined outside the canonical requirements document). No traceability rows exist in REQUIREMENTS.md for any Phase 69 requirement IDs.

---

## Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `comms-link/james/failover-orchestrator.js:209` | `command: 'notify_failover'` — command not in COMMAND_REGISTRY | Blocker | Uday receives no notification via primary failover path |
| `comms-link/bono/index.js:354` | `alertManager.handleNotification({ text })` — method does not exist on AlertManager | Blocker | TypeError thrown at runtime (inside try/catch — silently caught); Uday receives no notification via Bono watchdog |
| `comms-link/bono/index.js:142` | `alertManager?.handleNotification?.({ text })` — same missing method, with optional chain | Warning | notifyFn for exec tier 'notify' is silently a no-op instead of sending a WhatsApp |

---

## Human Verification Required

### 1. End-to-End Failover Timing

**Test:** Power off server .23 and observe when failover fires
**Expected:** Failover fires after exactly 60s of continuous failure (not 57s, not 75s); a 3s power hiccup followed by recovery does NOT trigger failover
**Why human:** Requires physical hardware — cannot verify timing by static code analysis

### 2. Pod Reconnect Timing After Broadcast

**Test:** Observe pod WebSocket reconnect dashboards after failover broadcast
**Expected:** All 8 pods (that cannot reach .23) are connected to Bono's VPS within 30s of SwitchController broadcast
**Why human:** Requires live network observation; static analysis cannot verify 30s timing

### 3. Split-Brain Guard Behavior

**Test:** Partition network so Pod 1 can still reach .23 while Pods 2-8 cannot; trigger failover
**Expected:** Pods 2-8 switch to Bono's VPS; Pod 1 stays on .23 and logs "split-brain guard: .23 still reachable"
**Why human:** Requires physical network manipulation across actual pod hardware

---

## Gaps Summary

The core failover mechanics are fully implemented and correct: HealthMonitor probes with proper 60s hysteresis, split-brain guard protects against false switches, the broadcast endpoint sends SwitchController to all connected pods, and the Bono secondary watchdog correctly handles the venue power outage edge case.

The single failing area is **Uday notification** — all three delivery paths are broken:

1. **Primary path (James -> Bono via exec_request):** `notify_failover` command does not exist in `shared/exec-protocol.js` COMMAND_REGISTRY. Bono's ExecHandler will reject it with "Unknown command: notify_failover" and return exitCode=-1.

2. **Bono watchdog path:** `alertManager.handleNotification()` is called but `AlertManager` has no such method (it has `handleJamesDown` and `handleRecovery`). A TypeError is thrown at line 354 of bono/index.js, caught by the enclosing try/catch, and silently swallowed.

3. **Email notification:** The ROADMAP success criterion explicitly requires "email AND WhatsApp notification." Neither failover path sends an email. Only WhatsApp is attempted, and both WhatsApp paths are broken.

The root cause for gaps 1 and 2 is that `notify_failover` is a command invented during Phase 69 that was never added to COMMAND_REGISTRY, and `handleNotification` was used without checking AlertManager's actual API.

**Fix scope:** Small and contained — add `notify_failover` to COMMAND_REGISTRY in shared/exec-protocol.js, fix the `alertManager.handleNotification` call in bono/index.js, and add email notification to at least one failover path.

---

_Verified: 2026-03-21T08:15:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
