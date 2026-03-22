---
phase: 141-warn-log-scanner
plan: "02"
subsystem: infra
tags: [rust, pod-healer, warn-scanner, ai-escalation, deduplication, ai-suggestions]

# Dependency graph
requires:
  - phase: 141-01
    provides: scan_warn_logs() with threshold + cooldown, warn_scanner_last_escalated in AppState

provides:
  - escalate_warn_surge() function in pod_healer.rs with deduplication and AI query
  - scan_warn_logs() now calls escalate_warn_surge() when threshold breached and cooldown clear
  - AI suggestions stored in ai_suggestions table with source='warn_scanner', pod_id='server'

affects:
  - pod_healer.rs (escalate_warn_surge added, placeholder replaced)
  - ai_suggestions table (new entries with source=warn_scanner)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "WARN deduplication: HashMap<String,usize> keyed on fields.message, fallback to truncated raw line"
    - "Frequency-sorted cap: sort by count desc, truncate to 20 unique messages before AI send"
    - "Server-level AI event: pod_id='server', sim_type='server' for non-pod escalations"
    - "query_ai() reuse: same path as escalate_to_ai() with source='warn_scanner'"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/pod_healer.rs

key-decisions:
  - "141-02: escalate_warn_surge() directly persists to ai_suggestions via sqlx — query_ai() does not persist server-level events so explicit INSERT is required"
  - "141-02: fields.message fallback is line.chars().take(120) — avoids storing giant raw JSONL in HashMap keys"
  - "141-02: context string uses WARN_THRESHOLD constant (not hardcoded 50) so threshold changes propagate automatically"

requirements-completed:
  - WARN-03

# Metrics
duration: 10min
completed: "2026-03-22"
---

# Phase 141 Plan 02: WARN Log Scanner AI Escalation Summary

**WARN surge deduplication and AI escalation via escalate_warn_surge() — groups identical WARN messages by frequency, builds compact context, calls query_ai(source="warn_scanner"), persists to ai_suggestions with pod_id="server"**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-22T11:50:00+05:30
- **Completed:** 2026-03-22T12:00:00+05:30
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- Replaced `let _ = warn_lines` placeholder in `scan_warn_logs()` with `escalate_warn_surge(state, warn_count, warn_lines).await`
- Implemented `escalate_warn_surge()`: HashMap deduplication on fields.message, sort by frequency desc, cap at 20 unique messages
- Builds formatted context string: total count + unique type count + grouped messages + threshold value
- Calls `query_ai()` with `source="warn_scanner"` matching plan spec
- Persists AI response to ai_suggestions with pod_id="server", sim_type="server" (server-level event)
- Zero `.unwrap()` in new code; all errors via `if let Ok`, `and_then`, `unwrap_or_else`, `match`
- 418 tests pass; 1 pre-existing config test failure unrelated to phase 141

## Task Commits

1. **Task 1: Implement escalate_warn_surge() with deduplication and AI escalation** - `62c7443` (feat)
2. **Task 2: Full test build smoke-check and LOGBOOK update** - `361d48b` (feat)

## Files Created/Modified

- `crates/racecontrol/src/pod_healer.rs` - Added escalate_warn_surge() (106 lines), replaced placeholder call site

## Decisions Made

- `escalate_warn_surge()` does an explicit sqlx INSERT into ai_suggestions rather than relying on `query_ai()` to persist — `query_ai()` persists with pod_id context but server-level events need pod_id="server" which requires explicit binding
- Fallback for unparseable JSONL lines is `line.chars().take(120)` — safe truncation prevents giant keys in the dedup HashMap
- The context string references `WARN_THRESHOLD` constant directly — if threshold is tuned later, the AI prompt updates automatically

## Deviations from Plan

None - plan executed exactly as written.

## Test Results

- `cargo build -p racecontrol-crate`: clean, zero errors
- `cargo test -p racecontrol-crate`: 418 passed, 1 pre-existing failure
  - `config::tests::config_fallback_preserved_when_no_env_vars` — pre-existing; config.rs untouched by phase 141; last modified in commit 68b4c81 (unrelated phase)

## Structural Verification

- `scan_warn_logs` defined at line 896, called at line 133 (heal_all_pods)
- `escalate_warn_surge` defined at line 985, called at line 975 (scan_warn_logs)
- `warn_scanner_last_escalated` in state.rs at lines 182 + 237
- `source='warn_scanner'` at lines 1052 and 1066 of pod_healer.rs
- No new `.unwrap()` — only pre-existing at line 149 (ping response, unrelated)

## Next Phase Readiness

- Phase 141 complete: WARN scanner foundation (plan 01) + deduplication/AI escalation (plan 02) fully wired
- AI receives grouped WARN context when threshold breached, stores suggestion in ai_suggestions

---
*Phase: 141-warn-log-scanner*
*Completed: 2026-03-22*
