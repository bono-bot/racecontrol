---
phase: 08-pod-lock-screen-hardening
plan: 03
subsystem: infra
tags: [rust, cargo, release-build, deploy-staging, rc-agent, pod-deploy, static-crt]

# Dependency graph
requires:
  - phase: 08-pod-lock-screen-hardening
    provides: StartupConnecting state (Plan 01) + watchdog-rcagent.bat (Plan 02)
provides:
  - deploy-staging/rc-agent.exe — release binary with Phase 8 lock screen hardening (6.7MB, static CRT)
  - Both deployment artifacts staged and verified for Pod 8 deployment
affects:
  - Pod 8 deployment (next operational step)
  - 09-edge-hardening (pods need updated rc-agent before edge hardening)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Static CRT release build: cargo build -p rc-agent-crate --release with +crt-static via .cargo/config.toml"
    - "Binary size for rc-agent: ~6.7MB (not 15-25MB — plan estimate was based on racecontrol which is larger)"

key-files:
  created:
    - deploy-staging/rc-agent.exe
  modified: []

key-decisions:
  - "Binary size is 6.7MB, not 15-25MB as plan estimated — plan estimate was based on racecontrol (21MB); rc-agent is smaller"
  - "racecontrol test suite has pre-existing compilation failure (non-exhaustive match on AssistChanged/FfbGainChanged/AssistState in ws/mod.rs) — documented in deferred-items.md, does not affect rc-agent build"
  - "Both artifacts also copied to external deploy-staging at C:/Users/bono/racingpoint/deploy-staging/ for HTTP server serving"

patterns-established:
  - "Build and stage pattern: cargo test -p rc-agent-crate + cargo test -p rc-common -> cargo build --release -> cp to deploy-staging"

requirements-completed: [LOCK-01, LOCK-02, LOCK-03]

# Metrics
duration: 15min
completed: 2026-03-14
---

# Phase 8 Plan 03: Build and Stage Release Binary Summary

**6.7MB static-CRT rc-agent release binary with StartupConnecting port readiness + watchdog staged at deploy-staging/ ready for Pod 8 deployment**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-14T00:09:35Z
- **Completed:** 2026-03-14T00:24:00Z
- **Tasks:** 1 of 2 (Task 2 is a human-verify checkpoint)
- **Files modified:** 1 (deploy-staging/rc-agent.exe)

## Accomplishments

- Built release binary with all Phase 8 changes: StartupConnecting state (LOCK-01/LOCK-02), port readiness probe, branded startup page, watchdog-rcagent.bat (LOCK-03)
- Full test suite green: rc-agent 157 tests passed, rc-common 85 tests passed (242 total)
- Binary uses static CRT (+crt-static) — no vcruntime140.dll dependency on pods
- Both artifacts staged in deploy-staging/ and copied to external deploy-staging/ for HTTP server

## Task Commits

Each task was committed atomically:

1. **Task 1: Build rc-agent release binary and stage deployment artifacts** - `533a080` (chore)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `deploy-staging/rc-agent.exe` - Release build of rc-agent with Phase 8 lock screen hardening; static CRT, 6.7MB

## Decisions Made

**Binary size discrepancy:** Plan estimated 15-25MB based on prior builds. Actual rc-agent binary is 6.7MB. The 21MB reference in STATE.md was for racecontrol (racecontrol.exe) not rc-agent. This size is consistent with the external deploy-staging binary (6.6MB from prior build).

**racecontrol test failure:** `cargo test -p racecontrol-crate` fails with pre-existing non-exhaustive match error on `AgentMessage::AssistChanged/FfbGainChanged/AssistState` variants in `ws/mod.rs`. This is documented in 08-01-SUMMARY.md "Issues Encountered" and tracked in deferred-items.md. It does NOT affect the rc-agent binary — rc-agent builds and tests pass cleanly.

**External deploy-staging sync:** Binary and watchdog also copied to `C:\Users\bono\racingpoint\deploy-staging\` which is the directory served by the HTTP server (`python3 -m http.server 9998`) for pod deployment.

## Deviations from Plan

None - plan executed exactly as written. The binary size discrepancy (6.7MB vs 15-25MB estimate) is a documentation issue in the plan, not a deviation — binary is correct and consistent with prior builds.

## Issues Encountered

- racecontrol `cargo test -p racecontrol-crate` fails with pre-existing compilation error (non-exhaustive match on 3 AgentMessage variants in ws/mod.rs). This is a known pre-existing issue from Phase 8 Plan 01, tracked in deferred-items.md. Does not affect rc-agent binary.

## Checkpoint Details

**Task 2 (checkpoint:human-verify)** requires human to:
1. Verify binary: `ls -lh deploy-staging/rc-agent.exe` should show 6.7MB
2. Confirm 242 tests passed (rc-agent 157 + rc-common 85)
3. Review Pod 8 deployment plan (HTTP server + pod-agent commands)
4. Decide on watchdog timing: current 60s interval = ~35s average recovery. If strict 30s needed, stagger a second task at +30s offset.

## User Setup Required

**Pod 8 deployment (operational step, not automated here):**

Start HTTP server on James's PC:
```
python3 -m http.server 9998 --directory /c/Users/bono/racingpoint/deploy-staging --bind 0.0.0.0
```

Deploy rc-agent.exe to Pod 8 (192.168.31.91):
```bash
# Download via pod-agent
curl -s -X POST http://192.168.31.91:8090/exec -H "Content-Type: application/json" \
  -d "{\"cmd\": \"curl -s -o C:\\RacingPoint\\rc-agent-new.exe http://192.168.31.27:9998/rc-agent.exe\"}"

# Kill old, replace, start
curl -s -X POST http://192.168.31.91:8090/exec -H "Content-Type: application/json" \
  -d "{\"cmd\": \"taskkill /F /IM rc-agent.exe\"}"

curl -s -X POST http://192.168.31.91:8090/exec -H "Content-Type: application/json" \
  -d "{\"cmd\": \"move /Y C:\\RacingPoint\\rc-agent-new.exe C:\\RacingPoint\\rc-agent.exe\"}"
```

Deploy watchdog to Pod 8:
```bash
# Download watchdog
curl -s -X POST http://192.168.31.91:8090/exec -H "Content-Type: application/json" \
  -d "{\"cmd\": \"curl -s -o C:\\RacingPoint\\watchdog-rcagent.bat http://192.168.31.27:9998/watchdog-rcagent.bat\"}"

# Create scheduled task
curl -s -X POST http://192.168.31.91:8090/exec -H "Content-Type: application/json" \
  -d "{\"cmd\": \"schtasks /create /TN RCAgentWatchdog /TR C:\\RacingPoint\\watchdog-rcagent.bat /SC MINUTE /MO 1 /RU SYSTEM /RL HIGHEST /F\"}"
```

Then restart Pod 8 and observe: branded "Starting up..." page should appear within 10 seconds of desktop.

## Next Phase Readiness

- deploy-staging/rc-agent.exe ready for deployment to all 8 pods via pod-agent
- Phase 9 (Edge hardening) can proceed after Pod 8 verification confirms no regressions
- Pending Todos: None (LOCK-01, LOCK-02, LOCK-03 all satisfied by Plans 01-03)

---
*Phase: 08-pod-lock-screen-hardening*
*Completed: 2026-03-14*

## Self-Check: PASSED

- deploy-staging/rc-agent.exe: FOUND (6.7MB)
- deploy-staging/watchdog-rcagent.bat: FOUND (560 bytes)
- 08-03-SUMMARY.md: FOUND
- Commit 533a080: FOUND
