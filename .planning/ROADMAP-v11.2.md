# Roadmap: v11.2 RC Sentry AI Debugger

**Milestone:** v11.2 RC Sentry AI Debugger
**Goal:** When rc-agent crashes, rc-sentry diagnoses WHY, applies deterministic fixes, queries Ollama for unknown patterns, and restarts with context — instead of blind restarts.
**Created:** 2026-03-21 IST
**Phases:** 101–105 (5 phases)
**Requirements:** 23 v11.2 requirements, 100% mapped

---

## Phases

- [ ] **Phase 101: rc-common Types Foundation** — SentryCrashReport and CrashDiagResult structs in rc-common unblock compilation in both rc-sentry and racecontrol
- [ ] **Phase 102: Watchdog Core** — Health poll FSM (5s interval, N=3 hysteresis) + post-crash log reading wired into rc-sentry background thread
- [ ] **Phase 103: Tier 1 Fixes and Escalation FSM** — Deterministic fix functions (zombie kill, port wait, socket clean, config repair, shader cache) + restart cooldown backoff + maintenance mode escalation
- [ ] **Phase 104: Pattern Memory and Ollama Integration** — debug-memory.json read/write with atomic writes + blocking std TCP Ollama query (fire-and-forget with 45s timeout)
- [ ] **Phase 105: Server Endpoint, bat File, and Fleet Rollout** — racecontrol crash report endpoint + FleetHealthStore extension + stderr capture bat update + Pod 8 canary deploy then full fleet

---

## Phase Details

### Phase 101: rc-common Types Foundation
**Goal:** SentryCrashReport and CrashDiagResult compile in rc-common so both rc-sentry and racecontrol can import them without circular dependencies or missing types.
**Depends on:** Nothing (compiler prerequisite)
**Requirements:** FLEET-01
**Success Criteria** (what must be TRUE):
  1. `cargo build -p rc-common` succeeds with SentryCrashReport and CrashDiagResult present in rc-common/src/types.rs
  2. rc-sentry and racecontrol can reference these types without a compiler error
  3. Existing rc-common tests remain green after the addition
**Plans:** TBD

### Phase 102: Watchdog Core
**Goal:** rc-sentry has a background std::thread that polls localhost:8090/health every 5 seconds, declares a crash after 3 consecutive failures, and reads startup_log + stderr log to extract crash context — all without touching any process inspection API.
**Depends on:** Phase 101
**Requirements:** DETECT-01, DETECT-02, DETECT-05
**Success Criteria** (what must be TRUE):
  1. rc-sentry spawns a watchdog thread at startup; existing 6 endpoints continue responding normally during watchdog operation
  2. A simulated rc-agent crash (taskkill via /exec) is declared crashed only after 15s of consecutive poll failures (3 polls), not on the first missed poll
  3. After crash declaration, the watchdog reads C:\RacingPoint\startup_log and rc-agent-stderr.log and populates a CrashContext struct with panic message, exit code, and last phase
  4. Watchdog transitions: Healthy → Suspect(1) → Suspect(2) → Suspect(3) → Crashed are observable in crash-sentry.log
  5. No WinAPI process inspection calls (OpenProcess, CreateToolhelp32Snapshot) appear in the binary; health polling uses only std::net::TcpStream connect
**Plans:** TBD

### Phase 103: Tier 1 Fixes and Escalation FSM
**Goal:** After crash detection, rc-sentry applies a deterministic fix sequence (kill zombie, wait for port clearance, clean sockets, repair config, clear shader cache) then restarts rc-agent — and enters maintenance mode after 3 restarts within 10 minutes.
**Depends on:** Phase 102
**Requirements:** FIX-01, FIX-02, FIX-03, FIX-04, FIX-05, FIX-06, ESC-01, ESC-02
**Success Criteria** (what must be TRUE):
  1. After crash detection, rc-agent zombie process is killed (taskkill by name) and port 8090 is confirmed clear before rc-agent is restarted — confirmed via crash-sentry.log entries showing kill → port-wait → restart sequence
  2. `cargo test -p rc-sentry` passes with all Tier 1 fix functions mocked via #[cfg(test)] guards — no real taskkill or netsh commands fire during test run
  3. If rc-agent crashes 3 times within 10 minutes, rc-sentry stops restarting and writes a MAINTENANCE_MODE entry to crash-sentry.log; subsequent health poll failures do not trigger additional restart attempts
  4. Restart intervals follow EscalatingBackoff from rc-common: 5s → 15s → 30s → 60s → 5min — observable in crash-sentry.log timestamps
  5. Config repair (missing rc-agent.toml or start-rcagent.bat) is applied before restart when crash log indicates a config-related exit
**Plans:** TBD

### Phase 104: Pattern Memory and Ollama Integration
**Goal:** rc-sentry reads debug-memory.json on crash to replay known fixes instantly, and fires a blocking std::net::TcpStream POST to Ollama on James .27:11434 for unknown patterns — never blocking the restart path beyond 45 seconds, and gracefully skipping if Ollama is unreachable.
**Depends on:** Phase 103
**Requirements:** MEM-01, MEM-02, MEM-03, LLM-01, LLM-02, LLM-03
**Success Criteria** (what must be TRUE):
  1. When a crash matches a pattern key in debug-memory.json, the known fix is applied without querying Ollama — verified by crash-sentry.log showing "pattern hit" and no outbound connection to .27:11434
  2. After a successful fix and verified rc-agent restart, the fix result is written back to debug-memory.json via atomic tmp+rename — the file is not zero-bytes if process is killed mid-write
  3. When no pattern match exists, a blocking POST to Ollama fires on a separate std::thread; rc-agent is restarted immediately with Tier 1 fixes while Ollama runs in parallel
  4. If Ollama is unreachable (connect timeout 5s) or exceeds 45s read timeout, the watchdog logs "ollama_skip" and continues with restart — rc-agent restart latency stays under 10 seconds regardless of Ollama state
  5. Pattern keys are derived from crash log content (not SimType/exit_code) and are stable across identical crash patterns from the same log content
**Plans:** TBD

### Phase 105: Server Endpoint, bat File, and Fleet Rollout
**Goal:** racecontrol accepts crash reports from rc-sentry pods, staff can see last crash diagnosis per pod in the fleet dashboard, start-rcagent.bat redirects stderr to rc-agent-stderr.log, and the full pipeline is validated on Pod 8 before rolling to all 8 pods.
**Depends on:** Phase 104, Phase 101 (racecontrol imports SentryCrashReport)
**Requirements:** FLEET-02, FLEET-03, ESC-03, DETECT-03, DETECT-04
**Success Criteria** (what must be TRUE):
  1. `curl -X POST http://192.168.31.23:8080/api/v1/sentry/crash` with a valid SentryCrashReport JSON body returns HTTP 200 and the report appears in FleetHealthStore.last_sentry_crash for the correct pod
  2. Fleet health dashboard shows last_sentry_crash data (pod, crash time, fix applied, restart verified) for any pod that has reported a crash in the current session
  3. After ESC-01 escalation threshold is crossed, an email alert reaches Uday and Bono with crash diagnostics — confirmed by checking email receipt
  4. C:\RacingPoint\rc-agent-stderr.log exists and contains panic output after a test rc-agent crash on Pod 8 — self_heal.rs START_SCRIPT_CONTENT matches the updated bat file exactly
  5. Full end-to-end pipeline on Pod 8 confirmed: crash → 15s declare → Tier 1 fixes → restart → health verified → fleet report received at server — then identical validation on all remaining 7 pods
**Plans:** TBD

---

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 101 - rc-common Types Foundation | 0/? | Not started | - |
| 102 - Watchdog Core | 0/? | Not started | - |
| 103 - Tier 1 Fixes and Escalation FSM | 0/? | Not started | - |
| 104 - Pattern Memory and Ollama Integration | 0/? | Not started | - |
| 105 - Server Endpoint, bat File, and Fleet Rollout | 0/? | Not started | - |

---

## Requirement Coverage

| Requirement | Phase | Rationale |
|-------------|-------|-----------|
| FLEET-01 | 101 | Compiler prerequisite — types must exist before any import |
| DETECT-01 | 102 | Health poll is the watchdog trigger mechanism |
| DETECT-02 | 102 | Hysteresis is part of the FSM that declares crashes |
| DETECT-05 | 102 | Log reading runs immediately after crash is declared |
| FIX-01 | 103 | Zombie kill is Step 1 of the Tier 1 fix sequence |
| FIX-02 | 103 | Port wait follows zombie kill in the same sequence |
| FIX-03 | 103 | Socket cleanup is a Tier 1 fix applied pre-restart |
| FIX-04 | 103 | Config repair is a Tier 1 fix applied pre-restart |
| FIX-05 | 103 | Shader cache clear is a Tier 1 fix for GPU crash patterns |
| FIX-06 | 103 | Test guards belong with the fix functions they protect |
| ESC-01 | 103 | Escalation FSM is the restart loop guard in the fix pipeline |
| ESC-02 | 103 | EscalatingBackoff controls restart cooldown in the same loop |
| MEM-01 | 104 | DebugMemory struct and file I/O are the memory layer |
| MEM-02 | 104 | Pattern matching runs at start of each crash analysis cycle |
| MEM-03 | 104 | Fix result write-back closes the learning loop after restart |
| LLM-01 | 104 | Ollama POST is the Tier 3 query fired on unknown patterns |
| LLM-02 | 104 | Fire-and-forget restart path requires the Ollama thread design |
| LLM-03 | 104 | Graceful skip on unreachable Ollama is part of the same function |
| FLEET-02 | 105 | Server endpoint must be deployed before end-to-end test |
| FLEET-03 | 105 | Dashboard extension accompanies the endpoint in racecontrol |
| ESC-03 | 105 | Email alert on escalation fires via the server after FLEET-02 |
| DETECT-03 | 105 | bat file update is a deploy step validated in canary phase |
| DETECT-04 | 105 | self_heal.rs update accompanies the bat file change |

**Coverage: 23/23 v11.2 requirements mapped. No orphans.**

---

## Key Constraints

- Anti-cheat safety: HTTP health polling only via std::net::TcpStream — no OpenProcess, CreateToolhelp32Snapshot, or any WinAPI on game PIDs
- Pure std in rc-sentry: std::thread + std::net only — no tokio, no reqwest, no async runtime
- All Tier 1 fix functions must have #[cfg(test)] guards returning mock results — cargo test must never fire real taskkill/netsh
- Tier 1 sequence order: kill zombie first → wait 500ms → read logs → apply other fixes → restart (log timing race prevention)
- Ollama query is always fire-and-forget — restart latency target under 10 seconds regardless of Ollama state
- Pod 8 canary first for all deployment changes

---

*Roadmap created: 2026-03-21 IST*
*Next step: `/gsd:plan-phase 101`*
