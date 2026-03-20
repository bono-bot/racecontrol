---
phase: 54-structured-logging-error-rate-alerting
plan: 02
subsystem: infra
tags: [tracing, tracing-subscriber, tracing-appender, json, structured-logging, rc-agent]

# Dependency graph
requires:
  - phase: 54-01
    provides: "json feature flag added to workspace tracing-subscriber dependency"
provides:
  - "rc-agent emits rc-agent-YYYY-MM-DD.jsonl files with timestamp, level, message, target, pod_id"
  - "Daily rotation via RollingFileAppender::builder() with DAILY rotation"
  - "30-day log cleanup on startup via cleanup_old_logs()"
  - "pod_id injected into all logs via info_span! entered after config load"
  - "stdout layer stays plain text; file layer uses .json() for structured output"
affects: [55-netdata-fleet-deploy, 56-whatsapp-alerting]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Tracing init deferred to after config load — pod_id available for span context"
    - "Pre-init logging via eprintln! (crash_recovery, config errors before tracing ready)"
    - "Two-layer tracing: stdout (plain text) + file (JSON), same registry"
    - "RollingFileAppender::builder() pattern for configurable daily-rotating JSONL files"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Tracing init moved after config load so pod_id (config.pod.number) is available for info_span! — pre-init messages use eprintln!"
  - "cleanup_old_logs() deletes .jsonl and .log files older than 30 days — called before tracing init, no tracing dependency"
  - "Stdout layer kept as plain text (human-readable); only file layer uses .json() for fleet-wide jq filtering"
  - "JSONL filename pattern: rc-agent-YYYY-MM-DD.jsonl (prefix=rc-agent-, suffix=jsonl, DAILY rotation)"
  - "Cargo.toml json feature was already added by Plan 54-01 (wave 1 parallel) — not re-added here"

patterns-established:
  - "Pod-scoped logging: enter info_span!(pod_id) after config load; all subsequent logs carry pod_id in span context"

requirements-completed: [MON-02]

# Metrics
duration: 12min
completed: 2026-03-20
---

# Phase 54 Plan 02: rc-agent Structured JSON Logging Summary

**rc-agent writes daily-rotating rc-agent-YYYY-MM-DD.jsonl files with pod_id field injected via span, enabling jq-based fleet-wide log aggregation across all 8 pods**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-20T08:45:00Z
- **Completed:** 2026-03-20T08:57:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Moved tracing initialization after config load so pod_id (config.pod.number) is available for structured log tagging
- Added `cleanup_old_logs()` function that removes .jsonl and .log files older than 30 days, called before tracing init on every startup
- Replaced `rolling::never()` + plain text file layer with `RollingFileAppender::builder()` using `Rotation::DAILY`, prefix `rc-agent-`, suffix `jsonl`
- Added JSON file layer with `.json()` — stdout layer stays human-readable plain text
- Entered `info_span!("rc-agent", pod_id = %pod_id_str)` after tracing init — all subsequent logs carry pod_id in span context
- Replaced pre-init `tracing::warn!`/`tracing::error!` calls with `eprintln!` (crash_recovery, config load error)

## Task Commits

Each task was committed atomically:

1. **Task 1: Reorder rc-agent init, add JSON file layer + pod_id span** - `0c42b1a` (feat)

## Files Created/Modified

- `crates/rc-agent/src/main.rs` - Restructured init order, added cleanup_old_logs(), new JSON rolling file layer, pod_id span

## Decisions Made

- Tracing init deferred to after config load — the few pre-init messages (banner, crash_recovery, self_heal) use `eprintln!` since tracing is not yet initialized. This is acceptable as per plan specification.
- Stdout layer kept as plain text — only file layer uses `.json()` to avoid breaking existing stdout log parsing
- Cargo.toml "json" feature was already added by Plan 54-01 (wave 1 parallel execution confirmed via git log)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Replaced pre-init tracing::error! in config load error branch**

- **Found during:** Task 1 (init reorder)
- **Issue:** `tracing::error!("Config error: {}", e)` inside the `load_config()` Err arm runs before tracing is initialized (tracing init now placed after config load)
- **Fix:** Replaced with `eprintln!("[rc-agent] Config error: {}", e)` — consistent with plan's stated approach for pre-init logging
- **Files modified:** `crates/rc-agent/src/main.rs`
- **Verification:** cargo check passes; no tracing calls before registry().init()
- **Committed in:** 0c42b1a (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (missing critical — pre-init tracing call)
**Impact on plan:** Required for correctness — tracing::error! before init would panic. Consistent with plan's stated eprintln! approach.

## Issues Encountered

- Python string replacement failed on Windows path backslashes in comments (`C:\RacingPoint\rc-agent.log`) — resolved by switching to line-based replacement. No impact on correctness.

## Next Phase Readiness

- rc-agent will emit `rc-agent-YYYY-MM-DD.jsonl` on next pod deploy with structured JSON including pod_id field
- Ready for Phase 55 (Netdata fleet deploy) and Phase 56 (WhatsApp alerting + weekly report)
- Fleet log investigation pattern: `jq 'select(.fields.pod_id == "pod_3")' rc-agent-*.jsonl`

---
*Phase: 54-structured-logging-error-rate-alerting*
*Completed: 2026-03-20*
