---
phase: 107-behavior-audit-certificate-procurement
plan: 02
subsystem: docs
tags: [anticheat, compatibility-matrix, procmon, conspit-link, eac, eaac, eos, safe-mode]

# Dependency graph
requires:
  - phase: 107-01
    provides: risk-inventory.md with source file:line behavioral audit (cross-referenced by matrix)
provides:
  - Per-game anti-cheat compatibility matrix (SAFE/UNSAFE/SUSPEND/GATE per subsystem per game)
  - ConspitLink audit template with ProcMon capture procedure
  - TOML safe_mode config preview for Phase 109 implementation
affects:
  - "Phase 108: keyboard hook replacement (UNSAFE verdict drives urgency)"
  - "Phase 109: safe mode state machine (SUSPEND/GATE verdicts drive subsystem list)"
  - "Phase 111: code signing (unsigned binary MEDIUM risk documented)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Ops doc pattern: compatibility matrix rows=subsystems, cols=games, cells=SAFE/UNSAFE/SUSPEND/GATE"
    - "Forward reference pattern: compatibility-matrix.md links risk-inventory.md for traceability chain"

key-files:
  created:
    - docs/anticheat/compatibility-matrix.md
    - docs/anticheat/conspit-link-audit.md
  modified: []

key-decisions:
  - "ConspitLink audit deferred -- template created and ready; ProcMon capture requires physical access to Pod 8"
  - "F1 25 and EA WRC use EAAC (EA Javelin), NOT EAC -- critical distinction for detection behavior"
  - "iRacing uses EOS (Epic Online Services) since May 2024, NOT EAC -- confirmed safe for shared memory SDK reads"
  - "SetWindowsHookEx verdict: UNSAFE for all kernel-level AC games -- Phase 108 must replace before canary testing"
  - "Ollama queries verdict: SUSPEND during all protected sessions due to GPU/VRAM contention visible to EAAC"
  - "AC EVO shared memory verdict: GATE (feature-flagged OFF) until v1.0 release confirms AC situation"

patterns-established:
  - "TOML safe mode config preview in compatibility matrix gives Phase 109 a concrete starting point"
  - "Cross-reference chain: compatibility-matrix.md -> risk-inventory.md -> source file:line"

requirements-completed: [AUDIT-02, AUDIT-04]

# Metrics
duration: 10min
completed: 2026-03-21
---

# Phase 107 Plan 02: Behavior Audit and Certificate Procurement Summary

**Per-game anti-cheat compatibility matrix (17 subsystems x 6 games) plus ConspitLink audit template with ProcMon capture procedure**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-03-21T13:27:29Z
- **Completed:** 2026-03-21T13:37:00Z
- **Tasks:** 2 (Task 1 auto, Task 2 checkpoint:human-action -- template created, capture deferred)
- **Files modified:** 2 created

## Accomplishments
- Created `docs/anticheat/compatibility-matrix.md` covering all 17 rc-agent subsystems across 6 games (F1 25, iRacing, LMU, AC EVO, EA WRC, AC original) with SAFE/UNSAFE/SUSPEND/GATE/N/A verdicts
- Created `docs/anticheat/conspit-link-audit.md` with audit checklist, ProcMon filter setup, and step-by-step capture procedure using Assetto Corsa as test vehicle (no anti-cheat risk)
- Documented TOML safe_mode config preview giving Phase 109 a concrete implementation starting point
- Established cross-reference chain: compatibility matrix -> risk-inventory.md for traceability

## Task Commits

Each task was committed atomically:

1. **Task 1: Create per-game anti-cheat compatibility matrix** - `8ec4a4e` (feat)
2. **Task 2: Create ConspitLink audit template** - `f329291` (feat)

**Plan metadata:** see final docs commit

## Files Created/Modified
- `docs/anticheat/compatibility-matrix.md` - Per-game matrix: 17 subsystems x 6 games, Legend, Key Takeaways, TOML config preview, cross-ref to risk-inventory.md
- `docs/anticheat/conspit-link-audit.md` - ConspitLink audit template: 6-category checklist, ProcMon filters, capture procedure, verdict field (DEFERRED pending capture)

## Decisions Made
- ConspitLink audit deferred: template is ready; ProcMon capture requires physical access to Pod 8 with wheelbase connected. Staff can execute from the document procedure.
- SetWindowsHookEx is UNSAFE (not SUSPEND) for all kernel-level AC games because it must be removed permanently by Phase 108, not just suspended.
- AC EVO treated as SUSPEND/GATE across all subsystems (not SAFE) due to unknown anti-cheat status -- conservative until v1.0.

## Deviations from Plan

None - plan executed exactly as written. ConspitLink audit document was created as a template with DEFERRED verdict as specified in the plan's acceptance criteria (plan explicitly allows "marked as deferred").

## Issues Encountered

None.

## User Setup Required

**Task 2 (ConspitLink audit) requires human action on Pod 8:**

1. Download Sysinternals Process Monitor: https://learn.microsoft.com/en-us/sysinternals/downloads/procmon
2. Run ProcMon as Administrator on Pod 8 (192.168.31.91)
3. Apply filters from `docs/anticheat/conspit-link-audit.md` -- ProcMon Configuration section
4. Launch Assetto Corsa (original) with Conspit Ares 8Nm wheelbase active
5. Drive 2-3 minutes, stop capture, export to CSV
6. Fill in Findings section and set Verdict (SAFE/RISKY/CRITICAL)
7. Run: `signtool verify /pa "C:\path\to\ConspitLink.exe"` for certificate status
8. Update the ConspitLink row in `docs/anticheat/compatibility-matrix.md` with verdict

## Next Phase Readiness

- Phase 107 docs foundation complete: risk inventory (Plan 01) + compatibility matrix + ConspitLink template (Plan 02)
- Phase 108 (keyboard hook replacement): SetWindowsHookEx UNSAFE verdict provides clear go-ahead for replacement
- Phase 109 (safe mode state machine): SUSPEND/GATE verdicts and TOML config preview ready to implement
- ConspitLink row in compatibility matrix will remain "[See ConspitLink Audit]" until audit is executed

---
*Phase: 107-behavior-audit-certificate-procurement*
*Completed: 2026-03-21*
