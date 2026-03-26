---
phase: 212-detection-expansion
plan: 01
subsystem: infra
tags: [bash, jq, detection, auto-detect, cascade, config-drift, bat-drift, log-anomaly]

requires:
  - phase: 211-safe-scheduling-foundation
    provides: auto-detect.sh 6-step pipeline with BUGS_FOUND/RESULT_DIR/LOG_FILE env contracts
  - phase: 210-audit-protocol-v3
    provides: bat-scanner.sh with bat_scan_pod_json function

provides:
  - scripts/cascade.sh — DET-07 sourcing framework with _emit_finding helper and run_all_detectors orchestrator
  - scripts/detectors/detect-config-drift.sh — DET-01 rc-agent.toml drift detection via safe_remote_exec
  - scripts/detectors/detect-bat-drift.sh — DET-02 bat file checksum drift via bat_scan_pod_json
  - scripts/detectors/detect-log-anomaly.sh — DET-03 ERROR/PANIC log count with venue-aware thresholds
  - auto-detect.sh step 4 wired to source cascade.sh and call run_all_detectors before 4a block

affects:
  - 212-02 (DET-04/05/06 detectors sourced by same cascade.sh)
  - 213-healing (findings.json written by _emit_finding feeds auto-fix engine)
  - bono-auto-detect.sh (inherits same cascade.sh sourcing pattern)

tech-stack:
  added: []
  patterns:
    - "Detector module pattern: standalone bash script with detect_*() function, set -u + set -o pipefail (no set -e), export -f at end"
    - "_emit_finding(category, severity, pod_ip, message): appends JSON to findings.json array, logs WARN"
    - "cascade.sh sourced (not exec'd) inside run_cascade_check() — inherits BUGS_FOUND/RESULT_DIR/SCRIPT_DIR from auto-detect.sh"
    - "Detector existence check: [[ $(type -t detect_*) == 'function' ]] before calling — safe when Phase 212-02 files not yet present"

key-files:
  created:
    - scripts/cascade.sh
    - scripts/detectors/.gitkeep
    - scripts/detectors/detect-config-drift.sh
    - scripts/detectors/detect-bat-drift.sh
    - scripts/detectors/detect-log-anomaly.sh
  modified:
    - scripts/auto-detect.sh (run_cascade_check wired to source cascade.sh + call run_all_detectors)

key-decisions:
  - "DET-01 uses safe_remote_exec :8090 (not SCP) to read rc-agent.toml — pods run rc-agent not racecontrol, SCP auth unreliable on Windows pods"
  - "Bat drift (DET-02) delegates to existing bat_scan_pod_json — thin wrapper avoids duplicating checksum logic"
  - "Log anomaly (DET-03) uses UTC date for JSONL filename (Pitfall 4) — logs roll at UTC midnight not IST midnight"
  - "cascade.sh places DETECTOR_FINDINGS=0 outside run_all_detectors so it persists across all detector calls"
  - "run_all_detectors accumulates BUGS_FOUND += DETECTOR_FINDINGS after all 6 detectors run — single accumulation point"

patterns-established:
  - "Pattern: Each detector script is self-contained — sources its own deps (e.g. bat-scanner.sh), handles missing files gracefully"
  - "Pattern: _emit_finding increments DETECTOR_FINDINGS itself — callers do NOT need to manually increment"
  - "Pattern: findings.json initialized to [] by cascade.sh before any detector runs (Pitfall 5 prevention)"

requirements-completed: [DET-01, DET-02, DET-03, DET-07]

duration: 3min
completed: 2026-03-26
---

# Phase 212 Plan 01: Detection Expansion Summary

**Cascade detection framework (cascade.sh) with _emit_finding helper wired into auto-detect.sh step 4, plus 3 detector scripts: rc-agent.toml config drift (DET-01), bat file checksum drift (DET-02), and venue-aware ERROR/PANIC log anomaly detection (DET-03)**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-03-26T07:21:06Z
- **Completed:** 2026-03-26T07:24:23Z
- **Tasks:** 2 of 2
- **Files modified:** 5 created, 1 modified

## Accomplishments

- Created `scripts/cascade.sh` (DET-07): `_emit_finding` JSON helper writes to `findings.json` array, `run_all_detectors` orchestrates 6 detectors with existence checks, `DETECTOR_FINDINGS` accumulator feeds back to `BUGS_FOUND`
- Created 3 detector scripts: DET-01 reads `rc-agent.toml` via `safe_remote_exec` checking `ws_connect_timeout>=600` and `pod_number` presence; DET-02 wraps `bat_scan_pod_json` for checksum drift; DET-03 counts ERROR/PANIC in UTC-dated JSONL with venue-aware thresholds
- Wired cascade.sh into `auto-detect.sh` `run_cascade_check()` before the 4a block — existing build-drift and comms-link checks still run unchanged after detection completes

## Task Commits

1. **Task 1: cascade.sh framework and _emit_finding helper** - `2756ed86` (feat)
2. **Task 2: DET-01/02/03 detector scripts** - `71442a9a` (feat)

## Files Created/Modified

- `scripts/cascade.sh` — DET-07 framework: DETECTOR_FINDINGS accumulator, findings.json init, sources 6 detectors with existence check, run_all_detectors orchestrator
- `scripts/detectors/.gitkeep` — marks directory for git tracking
- `scripts/detectors/detect-config-drift.sh` — DET-01: reads rc-agent.toml via safe_remote_exec :8090, banner guard, ws_connect_timeout>=600, pod_number key check
- `scripts/detectors/detect-bat-drift.sh` — DET-02: sources bat-scanner.sh, calls bat_scan_pod_json, emits P2 on DRIFT per pod
- `scripts/detectors/detect-log-anomaly.sh` — DET-03: UTC date for JSONL filename, venue-aware thresholds (open=10/closed=2), P1 if >50
- `scripts/auto-detect.sh` — added DET-07 integration block in run_cascade_check() before 4a

## Decisions Made

- Used `safe_remote_exec` (not SCP) for DET-01 config drift — pods run rc-agent, not racecontrol; file is `rc-agent.toml` not `racecontrol.toml`; SCP auth unreliable on Windows pods per CONTEXT.md research decision
- DET-02 is a thin wrapper around `bat_scan_pod_json` — avoids duplicating checksum logic already proven in Phase 210 audit
- DET-03 uses UTC date for JSONL filename per Pitfall 4 (logs are UTC-dated, not IST-dated)
- `_emit_finding` increments `DETECTOR_FINDINGS` directly so callers don't double-count

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Phase 212-02 can immediately add DET-04/05/06 scripts to `scripts/detectors/` and they will be auto-sourced by `cascade.sh` (existence-check pattern already in place)
- `findings.json` accumulates across all detectors — Phase 213 auto-fix engine can read it for WhatsApp cooldown gating
- `bono-auto-detect.sh` should be updated to also source `cascade.sh` (out of scope for this plan)

---
*Phase: 212-detection-expansion*
*Completed: 2026-03-26*
