---
phase: 193-auto-fix-notifications-and-results-management
plan: 02
subsystem: infra
tags: [bash, notifications, comms-link, whatsapp, audit, websocket]

# Dependency graph
requires:
  - phase: 192-intelligence-layer-delta-engine
    provides: audit-summary.json and delta.json written to RESULT_DIR by generate_report and compute_delta
  - phase: 193-auto-fix-notifications-and-results-management plan 01
    provides: results.sh with finalize_results producing run-meta.json and audit-summary.json

provides:
  - audit/lib/notify.sh with send_notifications() entry point
  - Bono WS notification channel via comms-link send-message.js
  - Bono INBOX.md persistent record channel with git push
  - WhatsApp Uday notification via Bono relay Evolution API
  - Off-by-default --notify gate (NOTIFY env var)
  - Delta summary in notification text when previous run exists

affects:
  - audit/audit.sh (now sources notify.sh and calls send_notifications)
  - 193-03 and later plans that build on the full audit pipeline

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Failure-safe notification pattern: all channels wrapped in subshells, always return 0
    - Off-by-default feature gate via env var (NOTIFY=true to enable)
    - Temp file JSON pattern for curl payloads (bash string escaping safety)
    - Subshell isolation for external tool calls (node, git, curl)

key-files:
  created:
    - audit/lib/notify.sh
  modified:
    - audit/audit.sh

key-decisions:
  - "NOTIFY=false by default — notifications only fire when operator explicitly passes --notify flag"
  - "UDAY_WHATSAPP from env var — number not hardcoded, skip with warning if unset"
  - "Subshell isolation for all three channels — one channel failure cannot affect others or abort audit"
  - "15s timeout on WS and WhatsApp relay calls — prevents notification blocking a long audit"
  - "notify.sh wired into audit.sh after intelligence layer — report already complete before notifications fire"

patterns-established:
  - "Failure-safe channel pattern: wrap in subshell, pipe failures to true, always return 0"
  - "IST timestamp header for INBOX.md: TZ=Asia/Kolkata date format matching standing rule"

requirements-completed: [NOTF-01, NOTF-02, NOTF-03, NOTF-04, NOTF-05]

# Metrics
duration: 8min
completed: 2026-03-25
---

# Phase 193 Plan 02: Notification Engine Summary

**Failure-safe three-channel notification engine (Bono WS + INBOX.md + WhatsApp Uday) gated behind --notify flag with delta summary inclusion**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-25T16:10:00+05:30
- **Completed:** 2026-03-25T16:18:00+05:30
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Created audit/lib/notify.sh with send_notifications() entry point callable from audit.sh
- Implemented three notification channels: Bono WS (send-message.js), Bono INBOX.md (git push), WhatsApp relay (Evolution API)
- All channels are failure-safe: subshell isolation, 13 `return 0` statements, no channel can abort the audit
- Off-by-default gate: NOTIFY=false unless --notify flag is passed (NOTF-04)
- Delta summary appended when delta.json exists with has_previous=true, including "REGRESSIONS DETECTED" warning (NOTF-05)
- Wired notify.sh into audit.sh: source block added after report.sh, send_notifications() called after Intelligence Layer

## Task Commits

Each task was committed atomically:

1. **Task 1: Create lib/notify.sh with Bono dual-channel and WhatsApp relay** - `c93f805e` (feat)

**Plan metadata:** (added with SUMMARY.md commit)

## Files Created/Modified
- `audit/lib/notify.sh` - Notification engine with all three channels and send_notifications() entry point
- `audit/audit.sh` - Added source block for notify.sh and send_notifications() call after Intelligence Layer

## Decisions Made
- NOTIFY env var checked as first line of send_notifications() — any notification work only happens when explicitly requested by operator
- UDAY_WHATSAPP must include country code 91 — env var, not hardcoded, skip with warning if unset
- All channels in subshells with `|| true` guards — one broken channel (comms-link unavailable, relay down) cannot cascade to others
- 15s timeout on node send-message.js and curl relay calls — bounded execution time
- notify.sh wired immediately after generate_report in the Intelligence Layer, so audit-summary.json is complete before any notification fires

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Wired notify.sh into audit.sh**
- **Found during:** Task 1 post-implementation check
- **Issue:** notify.sh was created but not sourced in audit.sh and send_notifications() was never called — the notification engine would exist but never run
- **Fix:** Added source block for notify.sh after report.sh in audit.sh, and added send_notifications() call in new Notification Layer section after Intelligence Layer complete
- **Files modified:** audit/audit.sh
- **Verification:** `bash -n audit/audit.sh` passes; grep confirms source and call present
- **Committed in:** c93f805e (included in task commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 - missing critical wiring)
**Impact on plan:** Essential for the notification engine to actually run. No scope creep.

## Issues Encountered
None — plan executed cleanly with one auto-fix for missing wiring.

## User Setup Required

The notification engine requires environment variables at runtime:
- `COMMS_PSK` — comms-link pre-shared key (from CLAUDE.md standing rules)
- `COMMS_URL` — WebSocket URL (e.g. `ws://srv1422716.hstgr.cloud:8765`)
- `UDAY_WHATSAPP` — Uday's WhatsApp number with country code (e.g. `919059833001`)

Without these env vars, channels will emit warnings and skip gracefully. The audit run is not affected.

## Next Phase Readiness
- notify.sh is complete and wired into audit.sh
- Notification engine fires after Intelligence Layer (after generate_report and compute_delta)
- All three channels verified: syntax clean, functions declared, acceptance criteria pass
- Ready for 193-03 and subsequent plans in the auto-fix and results management phase

---
*Phase: 193-auto-fix-notifications-and-results-management*
*Completed: 2026-03-25*
