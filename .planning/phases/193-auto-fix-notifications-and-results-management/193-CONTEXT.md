# Phase 193: Auto-Fix, Notifications, and Results Management - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Build lib/fixes.sh with approved-fix whitelist and is_pod_idle() gate (never touch active billing sessions). Build lib/notify.sh with comms-link WS relay + INBOX.md dual-channel for Bono and WhatsApp via Bono relay Evolution API for Uday. Add --commit flag to git-commit results. Wire all into audit.sh as the final pipeline step.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Auto-fix is OFF by default — requires --auto-fix flag
- is_pod_idle() gate: check fleet health API for active billing sessions on pod — SKIP fix if active
- OTA_DEPLOYING sentinel: skip fix if pod is mid-deploy
- MAINTENANCE_MODE sentinel: one of the fixable items (clear stale sentinel)
- Approved fixes whitelist (safe, reversible):
  - Clear stale MAINTENANCE_MODE sentinel
  - Kill orphan powershell.exe processes
  - Restart rc-sentry if not responding
  - Clear stale lock files
- Per-fix audit log: append to fixes.jsonl in RESULT_DIR with before/after state
- Notifications via --notify flag (off by default, failure does NOT abort audit):
  - Bono: `cd comms-link && COMMS_PSK="..." node send-message.js "text"` + append to INBOX.md + git push
  - Uday WhatsApp: via Bono relay Evolution API (curl to Bono VPS)
- --commit flag: `git add audit/results/... && git commit -m "audit: ..."` after run
- Comms-link path: C:/Users/bono/racingpoint/comms-link
- COMMS_PSK and COMMS_URL from env vars (never hardcoded)
- WhatsApp phone numbers: staff=7075778180 (via Bono VPS Evolution API)
- Uday phone: usingh@racingpoint.in contact

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `audit/audit.sh` — entry point with --auto-fix, --notify, --commit flags already parsed (lines 100-108)
- `audit/lib/core.sh` — emit_result, emit_fix (fix audit trail to fixes.jsonl)
- `audit/lib/results.sh` — finalize_results, already wired into audit.sh
- `audit/lib/report.sh` — generate_report, already wired into audit.sh
- `comms-link/send-message.js` — Bono messaging tool
- Fleet health API: GET http://192.168.31.23:8080/api/v1/fleet/health

### Established Patterns
- All libs sourced conditionally: `if [ -f "$SCRIPT_DIR/lib/X.sh" ]; then source ...; fi`
- All function calls guarded: `declare -f func_name >/dev/null 2>&1 && func_name`
- emit_fix already in core.sh for fix audit trail
- Environment vars for secrets (AUDIT_PIN, COMMS_PSK, COMMS_URL)

### Integration Points
- fixes.sh called after phase execution, before report generation
- notify.sh called after report generation (needs summary data)
- git commit called as last step before exit
- audit.sh already has AUTO_FIX, NOTIFY, COMMIT variables exported

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the implementation. Bono notification uses the established comms-link pattern. WhatsApp goes through Bono VPS Evolution API relay (not direct). Fix whitelist is intentionally small — only safe, reversible operations.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
