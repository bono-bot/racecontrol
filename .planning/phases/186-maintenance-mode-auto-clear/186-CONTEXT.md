# Phase 186: MAINTENANCE_MODE Auto-Clear - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Changes to rc-sentry's MAINTENANCE_MODE handling: JSON payload (reason, timestamp, restart_count), 30-min auto-clear, WOL_SENT immediate-clear, WhatsApp alert on activation. Changes to rc-sentry crate (tier1_fixes.rs) and recovery event reporting.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase. Key constraints from ROADMAP success criteria:
- MAINTENANCE_MODE file becomes JSON with reason, timestamp, restart_count, diagnostic_context fields
- Auto-clear after 30 minutes without manual intervention
- WOL_SENT sentinel triggers immediate clear (WoL from pod_healer breaks the deadlock)
- WhatsApp alert within 60s of MAINTENANCE_MODE activation via POST to /api/v1/fleet/alert
- pod_healer reads MAINTENANCE_MODE JSON via rc-sentry /files before WoL (already done in Phase 185)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tier1_fixes.rs` — MAINTENANCE_FILE constant, RestartTracker with 3-in-10min threshold, existing MAINTENANCE_MODE write logic
- `tier1_fixes.rs` — `post_recovery_event()` and `escalate_to_whatsapp()` from Phase 184
- `watchdog.rs` — main watchdog loop that checks for crash and calls handle_crash()
- `rc-common/recovery.rs` — RecoveryEvent struct

### Established Patterns
- rc-sentry uses pure std (no tokio) — file operations via std::fs
- HTTP via raw TcpStream (same pattern as recovery event reporting)
- Sentinel files at C:\RacingPoint\ — simple file presence checks

### Integration Points
- MAINTENANCE_MODE write in handle_crash() when RestartTracker hits threshold
- WhatsApp alert via escalate_to_whatsapp() already exists (Phase 184)
- Auto-clear can be a check at the start of each watchdog poll cycle
- WOL_SENT sentinel check alongside MAINTENANCE_MODE check

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
