# Architecture Research

**Domain:** AI-driven recovery integration into rc-sentry / rc-agent / racecontrol
**Researched:** 2026-03-25
**Confidence:** HIGH — based on direct code analysis of all affected crates

---

## Current State Inventory

Before defining the target architecture, the existing recovery actors must be fully mapped. The
codebase already has significant structure that v17.1 must extend, not replace.

### Existing Recovery Actors (as of 2026-03-25)

| Actor | Binary | Location | Scope | Mechanism |
|-------|--------|----------|-------|-----------|
| `rc-sentry watchdog` | `rc-sentry.exe` | Pod :8091 | rc-agent health | 5s HTTP poll, 3-poll hysteresis FSM, `CrashContext` build |
| `rc-sentry tier1_fixes` | `rc-sentry.exe` | Pod :8091 | rc-agent crash | kill zombies, port wait, CLOSE_WAIT clean, config repair, restart via schtasks |
| `rc-sentry debug_memory` | `rc-sentry.exe` | Pod :8091 | crash pattern replay | `debug-memory-sentry.json`, `derive_pattern_key`, instant fix lookup |
| `rc-agent self_monitor` | `rc-agent.exe` | Pod :8090 | self-recovery | 60s check, CLOSE_WAIT threshold, WS dead 5min, PowerShell+DETACHED_PROCESS relaunch |
| `pod_monitor` | `racecontrol.exe` | Server :8080 | heartbeat detection | pure detector, flags `PodStatus::Offline`, delegates ALL repair to pod_healer |
| `pod_healer` | `racecontrol.exe` | Server :8080 | graduated recovery | `PodRecoveryTracker` (Waiting→TierOneRestart→AiEscalation→AlertStaff), cascadeGuard |
| `cascade_guard` | `racecontrol.exe` | Server :8080 | anti-cascade | 60s window, 3 cross-authority actions = 5min pause + WhatsApp alert |
| `rc-watchdog service` | `rc-watchdog.exe` | Pod SYSTEM | rc-agent process | Windows Service, tasklist poll every 5s, session1 spawn on miss |
| `james_monitor` | `rc-watchdog.exe` | James :27 | James-side services | 10 services (ollama, comms-link, go2rtc, racecontrol, kiosk, dashboard...), 4-step graduated AI recovery |
| `RecoveryAuthority` | `rc-common` | shared lib | ownership registry | `ProcessOwnership` map: RcSentry, PodHealer, JamesMonitor registered as exclusive owners |

### Ownership Already Defined in rc-common

`rc_common::recovery::RecoveryAuthority` establishes three registered authorities. The
`ProcessOwnership` registry prevents double-registration with `OwnershipConflict`. The
`RecoveryDecision` / `RecoveryLogger` JSONL pipeline logs every action to
`C:\RacingPoint\recovery-log.jsonl` (pods/server) or `C:\Users\bono\racingpoint\recovery-log.jsonl`
(James).

This means the coordination scaffolding already exists at the type level. v17.1 fills in behavior,
not new infrastructure.

---

## The Conflict Map

Before defining the target architecture, the exact conflicts must be named.

```
Per-pod: Three actors compete for rc-agent.exe recovery authority

  rc-sentry watchdog    rc-agent self_monitor    rc-watchdog service
  (external, :8091)     (internal task,          (SYSTEM svc,
  15s hysteresis +      60s check, WS dead)      tasklist 5s poll,
  tier1 + schtasks                               session1 spawn)
       |                        |                        |
       +------------------------+------------------------+
                                |
                    All three can restart rc-agent
                    independently, with no coordination.
                    Race condition: two processes try to
                    bind port 8090 simultaneously.

Server side: Two actors compete for pod restart decisions

  pod_monitor (detector)  ------>  pod_healer (PodRecoveryTracker)
  (heartbeat timeout,              Waiting -> TierOneRestart ->
  marks pod Offline)               AiEscalation -> AlertStaff
                                          |
                                    cascade_guard
                                    (multi-authority detection
                                    -- but pod_healer never
                                    knows about rc-sentry's
                                    actions on the same pod)

WoL gap: pod_healer cannot distinguish crash vs deliberate shutdown

  Pod offline (MAINTENANCE_MODE active)
    -> pod_healer -> WoL
    -> rc-agent starts
    -> rc-sentry sees MAINTENANCE_MODE -> skips restart
    -> pod stays dead
    -> pod_healer fires WoL again
    -> infinite loop
```

---

## System Overview

### Target Architecture: Single Authority Per Machine

```
+-------------------------------------------------------------------+
|  JAMES (.27)                                                      |
|  james_monitor (rc-watchdog james mode)                           |
|  Authority: JamesMonitor over ollama, comms-link, go2rtc,        |
|             webterm, racecontrol, kiosk, dashboard, tailscale    |
|  NOT an authority over pod internals (that is RcSentry's domain) |
+-------------------------------------------------------------------+
         | monitors racecontrol health via HTTP
         v
+-------------------------------------------------------------------+
|  SERVER (.23) -- racecontrol.exe                                  |
|  pod_monitor: detector only, no actions                           |
|  pod_healer:  PodHealer authority -- server-level recovery ONLY   |
|  cascade_guard: cross-authority anti-cascade                      |
|  Recovery log: C:\RacingPoint\recovery-log.jsonl                  |
+----------------------+--------------------------------------------+
                       | WS + HTTP
       +---------------+----------------+
       v                                v
+------------------+         +------------------+
|  POD N (.xx)     |   ...   |  POD 8 (.91)     |
|                  |         |  (canary)        |
|  rc-sentry       |         |  rc-sentry       |
|  :8091           |         |  :8091           |
|  RcSentry        |         |  RcSentry        |
|  AUTHORITY       |         |  AUTHORITY       |
|                  |         |                  |
|  rc-agent        |         |  rc-agent        |
|  self_monitor    |         |  self_monitor    |
|  yields to       |         |  yields to       |
|  RcSentry        |         |  RcSentry        |
|  :8090           |         |  :8090           |
|                  |         |                  |
|  rc-watchdog     |         |  rc-watchdog     |
|  SYSTEM svc      |         |  SYSTEM svc      |
|  last-resort     |         |  last-resort     |
|  only            |         |  only            |
+------------------+         +------------------+
```

---

## Component Responsibilities

### After v17.1

| Component | Responsibility | Does NOT Do |
|-----------|----------------|-------------|
| `rc-sentry watchdog` | Single recovery authority for rc-agent on the pod. Detects crash via HTTP poll (15s hysteresis). Applies Tier 1 fixes. Checks pattern memory. Queries Ollama if pattern unknown. Restarts via schtasks. Reports to server. | Process inspection (anti-cheat). WoL. Server-level restart. James-side services. |
| `rc-agent self_monitor` | Detects WS-dead / CLOSE_WAIT floods as signals to report to rc-sentry. Writes GRACEFUL_RELAUNCH sentinel before self-restart. Does NOT restart independently after rc-sentry is running. | Acting as recovery authority. Triggering independent restarts (conflicts with rc-sentry). |
| `rc-watchdog.exe (pod mode)` | Last-resort only: catches rc-agent crash if rc-sentry itself has died. Wraps restart in grace window so rc-sentry can run first. Reports crash count to server. | First-responder restarts. AI diagnosis. Pattern memory. Tier 1 fixes. |
| `pod_monitor` | Pure detector. Marks pods Offline on heartbeat timeout. Delegates all repair to pod_healer. | Any restart or WoL decisions. |
| `pod_healer` | Server-level graduated recovery: WoL if pod OS offline, alert staff after N attempts. Receives rc-sentry recovery events via server API to distinguish "rc-agent crashed but pod is alive" from "entire pod is offline". | rc-agent-level restarts (that is rc-sentry's job). Tier 1 fixes on pods (rc-sentry is closer). |
| `cascade_guard` | Detects multi-authority conflict (RcSentry + PodHealer both firing in 60s window). Pauses recovery + alerts Uday. | Deciding who is right in a conflict. |
| `james_monitor` | Monitors James-local and server-level services. Graduated AI recovery for its owned services. | Pod internals. rc-agent restart. |

---

## The Core Design Decisions

### Decision 1: rc-sentry is the single recovery authority per pod

**Rationale:** rc-sentry is the external survivor -- it outlives rc-agent crashes by design. It
already has the full recovery pipeline: watchdog FSM -> tier1_fixes -> debug_memory -> ollama ->
restart_service (verified via health poll).

The `self_monitor` inside rc-agent has a fundamental flaw: it dies with the patient. Its only
remaining role is to write the `GRACEFUL_RELAUNCH` sentinel (already done) and to detect CLOSE_WAIT
floods as an early warning signal. It should NOT act as a restart authority.

**Enforcement:** `self_monitor.rs relaunch_self()` must be gated: only trigger if rc-sentry is
unreachable (`:8091` health check fails). If rc-sentry is up, self_monitor writes a sentinel and
does NOT relaunch -- it trusts rc-sentry to observe the crash and restart cleanly.

### Decision 2: rc-watchdog.exe pod service becomes last-resort fallback

**Current problem:** rc-watchdog polls via `tasklist` every 5s and restarts via
`session::spawn_in_session1()`. This is blind -- it does not know if rc-sentry already fired, does
not write any sentinel, does not check MAINTENANCE_MODE. It will fight rc-sentry.

**Target behavior:** rc-watchdog pod service adds a 30s grace window after any
`C:\RacingPoint\sentry-restart-breadcrumb.txt` write. If rc-sentry has acted in the last 30s,
rc-watchdog skips its restart. It becomes the backstop: if rc-sentry itself crashes (extremely
rare), rc-watchdog picks up.

### Decision 3: pod_healer must know when rc-sentry has handled a crash

**Current problem:** pod_healer's `PodRecoveryTracker` operates on `PodStatus::Offline` only. It
does not know that rc-sentry already restarted rc-agent at the process level. If rc-agent crashes
and restarts in 20s, pod_healer may still fire WoL (30s Waiting -> TierOneRestart).

**Target behavior:** rc-sentry reports successful restarts to the server's recovery API endpoint.
pod_healer checks this log before escalating. If rc-sentry already restarted rc-agent within the
last 60s, pod_healer skips WoL and TierOneRestart (the restart already happened at the right tier).

### Decision 4: MAINTENANCE_MODE must have an auto-clear path

**Current problem:** `enter_maintenance_mode()` writes `C:\RacingPoint\MAINTENANCE_MODE` and nothing
clears it automatically. pod_healer's WoL revives the pod -> rc-agent tries to start -> rc-sentry
sees MAINTENANCE_MODE -> skips restart -> pod stays dead permanently until manual intervention.

**Target behavior:** rc-sentry checks MAINTENANCE_MODE age at every crash event. If it is older than
30 minutes AND the pod has received a WoL (server writes `C:\RacingPoint\WOL_SENT` via rc-sentry
exec before sending WoL), MAINTENANCE_MODE is cleared and recovery is allowed once more.

### Decision 5: spawn success verification is mandatory everywhere

**Current problem (known):** `Command::spawn().is_ok()` is a lie on Windows non-interactive
contexts. rc-sentry's `restart_service()` already implements the correct pattern: `schtasks /Run`
via `run_cmd_sync()` (cmd.exe /C), then polls `:8090/health` for 20s to verify.

**This pattern must not regress.** Any new restart path -- in rc-watchdog, self_monitor, or
pod_healer -- must include a health poll verification step. A restart that returns `success: true`
without a health poll is a known-false positive.

---

## Data Flow: Recovery Pipeline

### Happy path: rc-agent crashes, rc-sentry handles it

```
rc-agent crash
    |
    v  (5s x 3 polls = 15s hysteresis)
rc-sentry watchdog FSM: Healthy -> Suspect(1) -> Suspect(2) -> Crashed
    |
    v
build_crash_context() -- reads startup_log + stderr_log
    |
    +-> GRACEFUL_RELAUNCH sentinel? -> skip escalation counter, clear, restart
    |
    +-> RCAGENT_SELF_RESTART sentinel? -> skip escalation counter, clear, restart
    |
    +-> MAINTENANCE_MODE? -> check age:
    |       age < 30min: skip restart (still in cool-down)
    |       age > 30min AND WOL_SENT file exists: clear both, allow restart
    |
    +-> debug_memory.instant_fix(pattern_key) hit? -> apply known Tier 1 fix, skip Ollama
    |
    +-> Tier 1 fixes: kill_zombies -> wait_for_port -> close_wait -> config_repair -> shader_cache
    |
    +-> Ollama query (James :11434) if pattern unknown -> get RESTART/DIAGNOSE/BLOCK decision
    |
    v
restart_service() via schtasks run_cmd_sync -> poll :8090/health 20s to verify
    |
    +-> verified: record in debug_memory, POST to /api/v1/recovery/events, reset tracker
    +-> not verified: increment tracker -> escalate after 3 failures -> MAINTENANCE_MODE
```

### Conflict path: rc-agent graceful self-restart (self_monitor)

```
self_monitor detects: WS dead 5min OR CLOSE_WAIT flood >= 5 strikes
    |
    +-> rc-sentry reachable (:8091/health)?
    |       YES -> write GRACEFUL_RELAUNCH sentinel, log event, exit rc-agent
    |              rc-sentry observes health drop, sees sentinel, restarts cleanly
    |       NO  -> rc-sentry is down, self_monitor acts as fallback:
    |              PowerShell+DETACHED_PROCESS (existing path, unchanged)
    |
    v
rc-sentry watchdog sees health drop -> reads GRACEFUL_RELAUNCH sentinel
    -> skips escalation counter, clears sentinel, restarts rc-agent
    -> reports to server: action=restart reason=graceful_relaunch authority=rc_sentry
```

### Conflict path: pod_healer vs rc-sentry

```
rc-agent crash on pod-N:
  t=0s:  rc-sentry detects crash (15s hysteresis)
  t=15s: rc-sentry fires tier1 -> restarts rc-agent -> health verified at t=35s
  t=35s: rc-sentry POSTs to /api/v1/recovery/events:
         {pod_id, authority=rc_sentry, action=restart, verified=true, ts}

pod_healer cycle runs every 120s:
  If pod_healer sees pod-N Offline at t=90s:
    Check recovery events for pod-N in last 60s
    -> rc_sentry restarted at t=35s (55s ago, verified=true)
    -> pod is likely recovering -- skip WoL, reset PodRecoveryTracker to Waiting
    -> next cycle: pod should be back online, reset tracker fully

  If pod_healer sees pod-N STILL Offline at t=210s (3min after rc-sentry action):
    Check recovery events: last rc-sentry action at t=35s (175s ago)
    -> rc-sentry attempted and failed (pod offline > 3min since restart)
    -> pod_healer escalates to WoL (entire pod OS may be down, not just rc-agent)
```

### Conflict path: cascade_guard intervention

```
rc-sentry fires on pod-2, pod-3, pod-5 in the same 60s window:
  cascade_guard sees 3 RcSentry actions
  -> same authority burst (all RcSentry) -> NOT a multi-authority cascade
  -> server_startup_recovery exemption: if pods reconnecting after server restart -> no alert

rc-sentry fires on pod-2 + pod_healer fires WoL on pod-2 in same 60s:
  cascade_guard sees RcSentry + PodHealer on same pod
  -> TWO different authorities acting on same target
  -> THIS is a cascade -> pause 5min + WhatsApp alert to Uday
```

---

## New vs Modified Components

### Modified components (no new files unless listed in New section)

| Component | File | Change | Scope |
|-----------|------|--------|-------|
| `rc-sentry/watchdog.rs` | existing | No functional change -- FSM is correct. | NONE |
| `rc-sentry/tier1_fixes.rs` | existing | Add MAINTENANCE_MODE auto-clear logic (age check + WOL_SENT sentinel). | Small |
| `rc-agent/self_monitor.rs` | existing | Gate `relaunch_self()` behind rc-sentry availability check (`:8091/health`). If sentry is up, write sentinel and exit -- do not spawn PowerShell. | Medium |
| `rc-watchdog/service.rs` | existing | Add grace window: read `sentry-restart-breadcrumb.txt` modified time; skip if < 30s old. Also add health poll after spawn. | Small |
| `racecontrol/pod_healer.rs` | existing | Before WoL, query recovery events API for pod. Skip WoL if rc-sentry restarted recently (< 60s, verified=true). Write WOL_SENT sentinel via rc-sentry /exec before WoL. | Medium |
| `racecontrol/cascade_guard.rs` | existing | Already handles same-authority bursts correctly per code review. | NONE |

### New components

| Component | File | Purpose |
|-----------|------|---------|
| Recovery events API | `racecontrol/api/recovery.rs` | `POST /api/v1/recovery/events` -- rc-sentry reports restart actions to server. `GET /api/v1/recovery/events?pod_id=N&since_secs=60` -- pod_healer queries before WoL. |
| Recovery events store | `racecontrol/state.rs` (extension) | `recovery_events: Arc<RwLock<VecDeque<RecoveryEvent>>>` with 200-item cap. In-memory only -- no DB. |

---

## Recommended Project Structure

No new crates required. All changes go into existing files, plus one new API file.

```
crates/
+-- rc-common/src/
|   +-- recovery.rs             # No changes -- types are sufficient
|
+-- rc-sentry/src/
|   +-- watchdog.rs             # No change -- FSM is correct
|   +-- tier1_fixes.rs          # + MAINTENANCE_MODE auto-clear (age check + WOL_SENT file)
|   +-- debug_memory.rs         # No change -- pattern memory is complete
|   +-- main.rs                 # Add: POST recovery event to server after successful restart
|
+-- rc-agent/src/
|   +-- self_monitor.rs         # Gate relaunch_self() behind rc-sentry availability check
|
+-- rc-watchdog/src/
|   +-- service.rs              # + sentry breadcrumb grace window + health poll after spawn
|
+-- racecontrol/src/
    +-- api/
    |   +-- recovery.rs         # NEW: POST + GET /api/v1/recovery/events
    +-- state.rs                # + recovery_events VecDeque in AppState
    +-- pod_healer.rs           # + query recovery events before WoL, write WOL_SENT sentinel
```

---

## Architectural Patterns

### Pattern 1: Sentinel File Coordination

**What:** A zero-size or small text file at a well-known path signals state between processes that
cannot share memory. rc-sentry and rc-agent already use this for `GRACEFUL_RELAUNCH`,
`RCAGENT_SELF_RESTART`, `MAINTENANCE_MODE`, `sentry-restart-breadcrumb.txt`.

**When to use:** Any cross-process state signal that must survive a process crash (cannot use IPC).

**Trade-offs:** Race conditions possible if two processes check and write simultaneously. Mitigated
by: single writer per sentinel, consumer clears after reading (consuming semantics), atomic write
(tmp+rename for JSON files).

**For v17.1:**
- `WOL_SENT` sentinel: written by racecontrol (via rc-sentry /exec) to pod before WoL send. Allows
  tier1_fixes to detect WoL and auto-clear MAINTENANCE_MODE.
- `SENTRY_PRESENT` sentinel: not needed -- self_monitor uses HTTP health check instead (more reliable).

### Pattern 2: Authority-Scoped Recovery with JSONL Audit Log

**What:** `RecoveryDecision` written to JSONL on every action (restart, skip, alert). Single
canonical log per machine. Already mandatory via `RecoveryLogger`.

**For v17.1:** rc-sentry must call `RecoveryLogger` after every restart attempt (success or
failure). pod_healer reads server-side recovery events (in-memory VecDeque, fed from API endpoint)
before escalating.

### Pattern 3: Graduated Response with Pattern Memory

**What:** First try instant fix from memory. Then Tier 1 deterministic. Then AI escalation. Then
staff alert. Each step escalates only if the previous failed.

This pattern already exists in both james_monitor (4 steps) and rc-sentry (3 steps). v17.1 connects
them via the recovery events API so the server knows what pod-level recovery has already occurred.

### Pattern 4: Verified Restart (spawn success != child alive)

**What:** After every restart call, poll the target's health endpoint for up to 20s. Only report
`success: true` after receiving HTTP 200. Already implemented in `tier1_fixes::restart_service()`.

**For v17.1:** rc-watchdog pod service must adopt the same pattern -- currently it fires
`spawn_in_session1()` without verifying. Add poll after spawn.

---

## Data Flow: Recovery Events API

### POST /api/v1/recovery/events (called by rc-sentry after restart)

```json
{
  "pod_id": "pod-3",
  "authority": "rc_sentry",
  "action": "restart",
  "reason": "health_poll_failed_3x",
  "context": "panic:overflow exit:101",
  "verified": true,
  "timestamp": "2026-03-25T10:30:00Z"
}
```

Server: push to recovery_events VecDeque (cap 200), retain last 24h.
Auth: None required -- internal LAN only (same as rc-sentry /exec).
Non-blocking send: if server is unreachable, log warn and continue.

### GET /api/v1/recovery/events?pod_id=pod-3&since_secs=60 (called by pod_healer)

```json
[
  {
    "pod_id": "pod-3",
    "authority": "rc_sentry",
    "action": "restart",
    "verified": true,
    "age_secs": 35
  }
]
```

pod_healer logic:
- Any event with `verified=true` AND `age_secs < 60`: skip WoL, wait next cycle.
- Any event with `verified=false` AND `age_secs < 120`: rc-sentry tried and failed, WoL appropriate.
- No events: pod_healer acts normally (WoL at step 2 of PodRecoveryTracker).

---

## Build Order

The build order is driven by two constraints: shared infrastructure precedes consumers, and the
recovery events API must be deployed to the server before rc-sentry starts reporting to it.

### Phase 1 -- racecontrol: Recovery Events API

Add `api/recovery.rs` with `POST /api/v1/recovery/events` and `GET /api/v1/recovery/events`.
Add `recovery_events: Arc<RwLock<VecDeque<RecoveryEvent>>>` to `AppState`.
Register routes in `main.rs`.

**Crate:** `racecontrol`
**Deploy:** Server rebuild + deploy. This must land BEFORE Phase 2.
**Verification:** `curl -X POST http://192.168.31.23:8080/api/v1/recovery/events -d '{"pod_id":"pod-8","authority":"rc_sentry","action":"restart","reason":"test","verified":true}'` returns 200.

### Phase 2 -- rc-sentry: Report to recovery events API

After `restart_service()` returns (success or failure), POST to the recovery events endpoint.
Non-blocking: `run_cmd_sync` with 3s timeout. Server unreachable -> log warn, continue.
Also: read GRACEFUL_RELAUNCH and RCAGENT_SELF_RESTART sentinels correctly (already done).

**Crate:** `rc-sentry`
**Deploy:** Pod rebuild + fleet deploy. Start from Pod 8 canary.
**Verification:** Kill rc-agent on Pod 8 manually. Confirm event appears in `GET /api/v1/recovery/events?pod_id=pod-8&since_secs=60`.

### Phase 3 -- racecontrol: pod_healer WoL coordination

Modify `run_graduated_recovery()` in `pod_healer.rs`:
- Before WoL: query recovery events API for the pod.
- If rc-sentry restarted within 60s with `verified=true`: skip WoL, stay in Waiting step.
- Before WoL send: write `WOL_SENT` to pod via rc-sentry `/exec` (`echo WOL_SENT > C:\RacingPoint\WOL_SENT`).

**Crate:** `racecontrol`
**Deploy:** Server rebuild + deploy.
**Verification:** Kill rc-agent on Pod 8. Observe: rc-sentry restarts within 20s. pod_healer skips WoL. Check `recovery-log.jsonl` on pod and server.

### Phase 4 -- rc-sentry: MAINTENANCE_MODE auto-clear

In `tier1_fixes::handle_crash()`, when MAINTENANCE_MODE is active: check file age via
`std::fs::metadata(MAINTENANCE_FILE).modified()`. If older than 30min AND
`C:\RacingPoint\WOL_SENT` exists: delete both sentinel files and proceed with restart.

This ends the WoL infinite loop by giving MAINTENANCE_MODE a 30-minute TTL post-WoL.

**Crate:** `rc-sentry`
**Deploy:** Pod rebuild + fleet deploy (can batch with Phase 2 if timing allows).
**Verification:** Simulate MAINTENANCE_MODE scenario: write the file, age it, write WOL_SENT, trigger crash. Confirm MAINTENANCE_MODE is cleared and restart proceeds.

### Phase 5 -- rc-agent: self_monitor coordination

Modify `self_monitor.rs relaunch_self()`:
1. Try TCP connect to `:8091` with 2s timeout.
2. If rc-sentry responds: write `GRACEFUL_RELAUNCH` sentinel, log event, call `std::process::exit(0)`.
   Do NOT spawn PowerShell. rc-sentry will pick up the crash within 15s.
3. If rc-sentry NOT reachable: fall through to existing PowerShell+DETACHED_PROCESS path.
   This is the fallback for when rc-sentry itself has crashed.

**Crate:** `rc-agent`
**Deploy:** Pod rebuild + fleet deploy.
**Verification on Pod 8:**
1. Kill rc-sentry (confirm sentry is down).
2. Kill rc-agent (WS dead simulation). Observe self_monitor fires PowerShell path.
3. Restart rc-sentry.
4. Kill rc-agent again. Observe self_monitor uses sentinel path (no PowerShell spawn, sentry handles restart within 15s).

### Phase 6 -- rc-watchdog: sentry breadcrumb grace window

Modify `service.rs` poll loop:
1. After detecting rc-agent absent: read `C:\RacingPoint\sentry-restart-breadcrumb.txt` modified time.
2. If modified within last 30s: skip restart attempt, log "grace window active", wait one cycle.
3. After `spawn_in_session1()` succeeds: add health poll -- connect to `:8090/health` for up to 15s.
   If no response, log warn. If response: log verified.

**Crate:** `rc-watchdog`
**Deploy:** Pod binary + Windows Service update (requires service stop/start).
**Verification:** Confirm rc-watchdog does not fire within 30s of a rc-sentry restart attempt.

---

## Integration Points with Existing Crates

| Point | Consumer | Provider | Interface |
|-------|----------|----------|-----------|
| Recovery events write | rc-sentry (after restart) | racecontrol REST | `POST /api/v1/recovery/events` (JSON, no auth) |
| Recovery events read | pod_healer (before WoL) | racecontrol state | In-process: `state.recovery_events.read().await` |
| WOL_SENT sentinel write | pod_healer (via rc-sentry exec) | tier1_fixes (reads) | File: `C:\RacingPoint\WOL_SENT` |
| GRACEFUL_RELAUNCH sentinel | self_monitor (writes) | tier1_fixes (reads + clears) | File: `C:\RacingPoint\GRACEFUL_RELAUNCH` -- already working |
| sentry breadcrumb | tier1_fixes (writes) | rc-watchdog (reads) | File: `C:\RacingPoint\sentry-restart-breadcrumb.txt` -- already written |
| Ollama (Tier 3 AI) | rc-sentry ollama.rs | James .27 :11434 | HTTP POST `/api/generate` -- already wired |
| Pattern memory | rc-sentry debug_memory.rs | rc-sentry itself | `C:\RacingPoint\debug-memory-sentry.json` -- already working |
| Recovery JSONL audit | All authorities | RecoveryLogger | Append-only JSONL -- already wired for all 3 authorities |
| cascade_guard | pod_healer | racecontrol AppState | In-process `state.cascade_guard` -- unchanged |

---

## Windows Process Management: Known Constraints

These are production-proven constraints from the codebase. Any code that launches processes must
respect all of them.

| Constraint | Impact | Resolution |
|------------|--------|------------|
| `spawn().is_ok()` != child alive | Silent false restarts | Always poll health endpoint after spawn (20s) |
| Non-interactive context blocks Session 1 launch | `schtasks /Run` via direct `Command::new` silently fails | Route through `run_cmd_sync()` (cmd.exe /C) -- proven working path |
| MAINTENANCE_MODE has no TTL | Permanent pod death after 3 crashes in 10min | Phase 4: add 30min auto-clear on WoL detection |
| DETACHED_PROCESS leaks PowerShell (~90MB/restart) | RAM creep on self_monitor restarts | Phase 5: self_monitor defers to rc-sentry, PowerShell path becomes rare fallback |
| `tasklist /FI "IMAGENAME"` returns empty on filter miss | False "process dead" detection | Prefer TCP health connect over process name scan |
| Windows holds .exe file lock while running | `move /Y` fails on live binary | Rename trick: ren old.exe -> old-bak.exe, ren new.exe -> old.exe, then kill+start |
| `schtasks /Run` fails from SYSTEM context (non-interactive) | rc-watchdog service cannot restart rc-agent directly | `run_cmd_sync()` routes through cmd.exe which has different creation context |

---

## Anti-Patterns

### Anti-Pattern 1: Self-Monitor Acting as Recovery Authority

**What people do:** `self_monitor.rs` calls `relaunch_self()` unconditionally whenever WS is dead or
CLOSE_WAIT floods.

**Why it's wrong:** self_monitor is inside the patient (rc-agent). If rc-sentry is also watching,
both systems independently decide to restart rc-agent at different offsets. rc-sentry's schtasks
path waits 5s for port to clear; self_monitor's PowerShell waits 3s. Both fire, both try to bind
:8090, one fails silently, pod stays dead even though both watchdogs logged "success".

**Do this instead:** self_monitor checks rc-sentry health first (TCP to :8091, 2s timeout). If
sentry is alive, write GRACEFUL_RELAUNCH and exit. Let sentry own the restart.

### Anti-Pattern 2: WoL Without MAINTENANCE_MODE Check

**What people do:** pod_healer sends WoL when pod appears offline, regardless of why.

**Why it's wrong:** A pod in MAINTENANCE_MODE went offline intentionally (3+ crashes in 10min). WoL
revives it. rc-agent starts. rc-sentry sees MAINTENANCE_MODE -> skips restart. Pod goes offline
again immediately. WoL fires again. Infinite loop until manual intervention.

**Do this instead:** Before WoL, query rc-sentry `/files` endpoint to check if MAINTENANCE_MODE
exists. If it does and is recent (< 30min), skip WoL and alert staff instead. Write WOL_SENT first
if WoL is appropriate (allows tier1_fixes to auto-clear on the other side).

### Anti-Pattern 3: Tasklist-Based Process Detection as Primary Signal

**What people do:** rc-watchdog uses `tasklist /FI "IMAGENAME eq rc-agent.exe"` as the only signal
for crash detection.

**Why it's wrong:** `tasklist /FI` returns empty output (not error) when the filter matches nothing
on some Windows configurations. The conservative fallback (`assume_running=true` on error) is
correct but the filter-miss case is the same as "process absent" -- indistinguishable.

**Do this instead:** Prefer HTTP health poll (`/health` endpoint, TCP connect to :8090) as the
ground truth. tasklist is a secondary signal only. The health endpoint is exactly what rc-sentry
watchdog uses and it works reliably.

### Anti-Pattern 4: Non-Interactive Spawn Without Verification

**What people do:** `Command::new("schtasks").args(["/Run", ...]).spawn()` returns Ok -- code
assumes child started.

**Why it's wrong:** On Windows, `spawn()` returning Ok means `CreateProcess` was accepted, not that
the child executed successfully. In non-interactive contexts (SYSTEM service, rc-sentry without
attached console), `schtasks /Run` and `PowerShell Start-Process` silently fail to launch into
Session 1. This is documented in CLAUDE.md standing rules and confirmed by rc-sentry's
DEBUG-RESTART-ISSUE investigation.

**Do this instead:** Use `run_cmd_sync("schtasks /Run /TN StartRCAgent", ...)` (routes through
`cmd.exe /C`), then poll the health endpoint for 20s before declaring success. This is the proven
working pattern already in `tier1_fixes::restart_service()`.

### Anti-Pattern 5: Uncoordinated Parallel Recovery (Standing Rule 10 Violation)

**What people do:** Add a new watchdog or health check without checking what else is watching the
same process.

**Why it's wrong:** Every new recovery actor watching rc-agent adds another independent restart
source. Three actors with different delays and different methods create a race that none of them
can win cleanly. The port :8090 cannot be bound by two processes simultaneously.

**Do this instead:** Before adding any restart logic, check `RecoveryAuthority` ownership map. If
a process is already owned by an authority, new code must either coordinate via sentinel files or
the recovery events API, or act only as a last-resort fallback that yields to the primary owner.

---

## Sources

- Direct code analysis: `crates/rc-sentry/src/{watchdog.rs, tier1_fixes.rs, debug_memory.rs, main.rs}` (2026-03-25)
- Direct code analysis: `crates/rc-agent/src/self_monitor.rs` (2026-03-25)
- Direct code analysis: `crates/racecontrol/src/{pod_healer.rs, pod_monitor.rs, cascade_guard.rs, wol.rs}` (2026-03-25)
- Direct code analysis: `crates/rc-watchdog/src/{main.rs, service.rs, james_monitor.rs}` (2026-03-25)
- Direct code analysis: `crates/rc-common/src/recovery.rs` (2026-03-25)
- `.planning/PROJECT.md` -- v17.1 milestone context, incident history, known constraints
- `CLAUDE.md` standing rules -- `.spawn().is_ok()` warning, MAINTENANCE_MODE silent killer,
  non-interactive context limits, DETACHED_PROCESS PowerShell leak, cascade guard rule (#10)

---

*Architecture research for: v17.1 Watchdog-to-AI Migration*
*Researched: 2026-03-25*
