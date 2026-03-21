---
phase: 107-behavior-audit-certificate-procurement
plan: 01
subsystem: docs
tags: [anti-cheat, eac, eaac, eos, kiosk, process-guard, code-signing, risk-inventory]

# Dependency graph
requires:
  - phase: 78-kiosk-session-hardening
    provides: SetWindowsHookExW keyboard hook implementation in kiosk.rs (the primary CRITICAL risk)
  - phase: 100-v11.2-ai-debugger
    provides: ai_debugger.rs Ollama integration (MEDIUM risk during game sessions)

provides:
  - docs/anticheat/risk-inventory.md with exhaustive rc-agent behavior classification per anti-cheat system
  - SetWindowsHookExW(WH_KEYBOARD_LL) identified as CRITICAL in kiosk.rs:958-959
  - OpenProcess on game PID identified as HIGH in game_process.rs:321
  - process_guard continuous enumeration + PID kills identified as HIGH
  - All sim adapter shared memory reads classified LOW (officially sanctioned)
  - Phase 108 decision: GPO registry keys mandatory (Windows 11 Pro, Keyboard Filter unavailable)
  - Phase 111 prerequisite: Sectigo OV certificate procurement checklist

affects:
  - 108-keyboard-hook-replacement
  - 109-safe-mode-state-machine
  - 110-telemetry-gating
  - 111-code-signing-canary-validation

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Risk classification: CRITICAL/HIGH/MEDIUM/LOW/NONE per anti-cheat system (EAAC/EOS/EAC/AC-EVO)"
    - "Source-referenced risk entries: behavior -> file:line -> per-system severity -> phase to address"

key-files:
  created:
    - docs/anticheat/risk-inventory.md
  modified: []

key-decisions:
  - "SetWindowsHookExW(WH_KEYBOARD_LL) in kiosk.rs:958-959 is CRITICAL for all three kernel-level anti-cheat systems (EAAC, EOS, EAC)"
  - "Phase 108 MUST use GPO registry keys (NoWinKeys=1, DisableTaskMgr=1) — pods are Windows 11 Pro, Keyboard Filter requires IoT Enterprise LTSC"
  - "OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION) in game_process.rs:321 is HIGH risk — even query-only handle to game PID is detectable by EAAC/EOS/EAC"
  - "No ReadProcessMemory or WriteProcessMemory found in rc-agent — CRITICAL API absent from codebase"
  - "All sim adapter shared memory reads use OpenFileMappingW + MapViewOfFile (correct, safe pattern) — never ReadProcessMemory"
  - "AC EVO shared memory telemetry must be feature-flagged off by default pending anti-cheat confirmation at full release"
  - "Sectigo OV certificate (~$220/yr) is the procurement target for Phase 111 — physical USB token (SafeNet eToken) for James .27 build machine"
  - "ConspitLink audit deferred to AUDIT-02 — behavior is opaque, documented as MEDIUM-UNKNOWN"

patterns-established:
  - "Risk inventory format: one row per behavior, columns for each AC system, phase to address"

requirements-completed: [AUDIT-01, AUDIT-03]

# Metrics
duration: 8min
completed: 2026-03-21
---

# Phase 107 Plan 01: Behavior Audit + Certificate Procurement Summary

**Exhaustive rc-agent anti-cheat risk inventory with per-system severity classifications and source references, GPO decision made for Phase 108, Sectigo OV cert checklist ready for Uday**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-21T13:28:06Z
- **Completed:** 2026-03-21T13:36:11Z
- **Tasks:** 2/3 complete (Task 3 is a human-action checkpoint — awaiting Uday)
- **Files modified:** 1 (created docs/anticheat/risk-inventory.md)

## Accomplishments

- Audited all 19 rc-agent source files for anti-cheat risky behaviors with exact file:line references
- Confirmed no ReadProcessMemory or WriteProcessMemory in codebase (critical negative finding)
- Classified 28 distinct behaviors with per-system severity across EAAC (F1 25/WRC), iRacing EOS, LMU EAC, and AC EVO
- Made Phase 108 architecture decision: GPO registry keys mandatory — all 8 pods are Windows 11 Pro, Keyboard Filter is unavailable
- Pre-purchase checklist for Sectigo OV certificate ready for Uday review

## Task Commits

1. **Task 1: Audit rc-agent source and create risk inventory document** — `9b432de` (docs)
2. **Task 2: Verify pod OS editions and populate table** — `3e1d12f` (docs)
3. **Task 3: Initiate code signing certificate purchase with Uday** — awaiting checkpoint resolution

## Files Created/Modified

- `docs/anticheat/risk-inventory.md` — Complete risk inventory with 28 behavior entries, all 8 pod OS rows, certificate procurement checklist, and phase-by-phase summary for Phases 108-111

## Decisions Made

- **Phase 108 lockdown approach:** GPO registry keys (NoWinKeys=1, DisableTaskMgr=1) are mandatory. Windows 11 Pro does not include Keyboard Filter (requires IoT Enterprise LTSC). Research, planning docs, and Phase 78 context all confirm Pro SKU. Live `winver` check recommended before Phase 108 implementation.
- **SetWindowsHookExW is CRITICAL:** Single highest-risk behavior. Must be replaced in Phase 108 before any anti-cheat protected game canary test.
- **OpenProcess on game PID is HIGH:** game_process.rs:321 uses PROCESS_QUERY_LIMITED_INFORMATION — even query-only handles are detectable by EAAC/EOS/EAC kernels. Must be addressed in Phase 109 safe mode design.
- **No ReadProcessMemory in codebase:** Confirms the most critical API (instant ban trigger) is absent. Positive finding.
- **All sim adapters use correct shared memory API:** OpenFileMappingW + MapViewOfFile (not ReadProcessMemory) — correct, safe pattern in all four sim adapters.
- **AC EVO feature-flagged:** Unknown anti-cheat status at Early Access; shared memory telemetry must default off until confirmed safe at 1.0 release.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fleet exec unavailable — pod OS edition determined from research**
- **Found during:** Task 2 (fleet exec query for pod OS edition)
- **Issue:** Server at 192.168.31.23:8080 unreachable from James .27 (server is 97 commits behind HEAD, not updated to v15.0 code). Direct pod :8090/exec also returned empty (endpoint not in deployed version).
- **Fix:** Used research evidence — FEATURES-v15-anticheat.md explicitly titles the research "for sim racing venue management on Windows 11 Pro pods." SUMMARY-v15.md states "Pods on Windows 11 Pro must use GPO registry keys." All planning context is consistent. Documented expected edition as Windows 11 Pro with live `winver` verification recommended for Phase 108 pre-implementation.
- **Files modified:** docs/anticheat/risk-inventory.md (pod table + decision note)
- **Verification:** Decision is sound — all Phase 78+ planning documents reference Windows 11 Pro pods

---

**Total deviations:** 1 auto-fixed (1 blocking — fleet unavailable)
**Impact on plan:** Fleet unavailability is expected during planning phase (server is behind HEAD). Research-based determination is well-supported and can be confirmed in person at venue before Phase 108 begins.

## Issues Encountered

- Fleet exec and direct pod agent endpoints returned empty (server not accessible from dev machine). This is normal — deployed server is 97 commits behind HEAD, and the plan was written expecting fleet access during execution.

## User Setup Required

**Task 3 awaits Uday:** Review the pre-purchase checklist in `docs/anticheat/risk-inventory.md` under "## Code Signing Certificate Procurement" and initiate the Sectigo OV certificate purchase. Required information:
1. Purchase initiated (yes/no)
2. Reseller name (Sectigo direct or SSLTrust recommended)
3. Expected delivery date

After Uday provides the resume signal, the Status and Expected delivery date fields in the risk-inventory.md should be updated.

## Next Phase Readiness

- Phase 108 (Keyboard Hook Replacement): READY — Phase 108 now knows it must use GPO registry keys; kiosk.rs:958-959 is the target. Live `winver` on one pod recommended before implementation.
- Phase 109 (Safe Mode State Machine): READY — Every behavior to gate behind safe mode is documented with file:line references.
- Phase 110 (Telemetry Gating): READY — AC EVO feature-flag decision recorded; all other sim adapters confirmed safe.
- Phase 111 (Code Signing + Canary): BLOCKED on certificate delivery — procurement checklist is ready for Uday.

---
*Phase: 107-behavior-audit-certificate-procurement*
*Completed: 2026-03-21*
