---
phase: 06-diagnosis
plan: 01
subsystem: infra
tags: [rc-agent, edge-browser, pod-agent, diagnostics, registry]

# Dependency graph
requires: []
provides:
  - "DIAG-01: rc-agent log status and error patterns for all 8 pods"
  - "DIAG-03: Edge browser version and kiosk-relevant registry settings for all 8 pods"
  - "Root cause: rc-core unreachable on .23:8080 causes all pods to show disconnected lock screen"
  - "Edge policy gap: StartupBoost + BackgroundMode not disabled on any pod (all default-enabled)"
affects:
  - phase 7 (rc-core auto-start + server IP pinning)
  - phase 8 (lock screen WebSocket reconnection)
  - phase 9 (Edge startup boost + background mode remediation)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "pod-agent /exec for remote diagnostics — write JSON payload to file, curl -d @file"
    - "rc-agent debug server at 127.0.0.1:18924/status reports lock_screen_state"

key-files:
  created:
    - ".planning/phases/06-diagnosis/06-FINDINGS.md"
  modified: []

key-decisions:
  - "DIAG-01: Only Pod 8 has a persistent log file — Pods 1-7 stdout not redirected, no log to read"
  - "DIAG-01: Pod 3 debug server confirms lock_screen_state=disconnected — rc-core unreachable is root cause"
  - "DIAG-01: Pod 8 shows 5 UDP port conflicts (10048) and watchdog crash loop — Pod 8 has stale rc-agent instance"
  - "DIAG-03: All 8 pods on Edge 145.0.3800.97 — consistent fleet, single remediation script covers all"
  - "DIAG-03: StartupBoost and BackgroundMode not set = default-enabled on all 8 pods — all need Phase 9 remediation"
  - "DIAG-03: EdgeUpdate service STOPPED but not DISABLED on all pods — can auto-start, needs sc config fix in Phase 9"

patterns-established:
  - "Registry 'not set' means default-enabled, NOT disabled — must explicitly set 0 to disable Edge features"
  - "Port 10048 (AddrInUse) on UDP ports = stale rc-agent instance still holding sockets"

requirements-completed:
  - DIAG-01
  - DIAG-03

# Metrics
duration: 15min
completed: 2026-03-13
---

# Phase 6 Plan 01: Diagnostic Data Collection Summary

**rc-agent log analysis + Edge registry baseline for all 8 pods reveals rc-core unreachable as universal lock screen root cause and confirms all 8 pods need Phase 9 Edge policy remediation**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-13T09:01:35Z
- **Completed:** 2026-03-13T09:16:00Z
- **Tasks:** 2 of 2
- **Files modified:** 1 (06-FINDINGS.md created)

## Accomplishments

- Established DIAG-01: Pod 3 debug server confirms lock_screen_state=disconnected; Pod 8 full log shows rc-core WebSocket timeout to ws://192.168.31.23:8080/ws/agent with exponential backoff 0-6+ attempts
- Established DIAG-03: All 8 pods on Edge 145.0.3800.97; StartupBoost and BackgroundMode not set (default-enabled) on all pods; EdgeUpdate service STOPPED but not DISABLED
- Identified that rc-core unreachable on .23:8080 is the single systemic root cause — all pods will show disconnected lock screen until Phase 7 fixes server auto-start

## Task Commits

Each task was committed atomically:

1. **Task 1 + Task 2: DIAG-01 logs + DIAG-03 Edge settings (both in 06-FINDINGS.md)** - `216341c` (feat)

**Plan metadata:** (pending — this summary commit)

## Files Created/Modified

- `.planning/phases/06-diagnosis/06-FINDINGS.md` - Pod diagnostic data: rc-agent log status, debug server responses, Pod 8 full log analysis, Edge version table, registry policy baseline for all 8 pods

## Decisions Made

- Pod 8 alone has rc-agent-log.txt — the start-rcagent.bat on Pods 1-7 does not redirect stdout. No log to collect means Phase 6-02 RDP session is the only way to get live logs from those pods.
- Pod 8 watchdog crash loop: pod-agent v0.4.0 panics on port 10048 (already running). Pod 8 should be upgraded to v0.5.0 which handles this gracefully.
- All 8 pods need the same Phase 9 remediation — no pod is an exception.
- EdgeUpdate service: STOPPED is not safe — it can re-enable itself. Must set `sc config EdgeUpdate start= disabled` in Phase 9.

## Deviations from Plan

None - plan executed exactly as written. Data was pre-collected by orchestrator and verified present in 06-FINDINGS.md. No curl commands needed to re-run.

## Issues Encountered

None. Both DIAG-01 and DIAG-03 sections were complete with all 8 pods' data. Verification passed (grep -c confirmed section markers and 8-row tables).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **Phase 7 (rc-core auto-start):** Blocked on DIAG-02/DIAG-04 (server MAC + port audit) — Plan 06-02 must complete first before Phase 7 can begin
- **Phase 9 (Edge remediation):** Ready to plan — all data collected. Script: set StartupBoost=0, BackgroundMode=0 in HKLM\SOFTWARE\Policies\Microsoft\Edge; disable EdgeUpdate and edgeupdate services; deploy to all 8 pods
- **Critical prerequisite for Phase 7:** Server MAC address for DHCP reservation still pending (DIAG-02/04)
- **Pod 8 specific:** Upgrade pod-agent to v0.5.0 to fix watchdog crash loop on port 10048

---
*Phase: 06-diagnosis*
*Completed: 2026-03-13*
