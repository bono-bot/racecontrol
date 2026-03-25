# Phase 192: Intelligence Layer - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Build delta tracking (lib/delta.sh), known-issue suppression (suppress.json + lib/suppress.sh), severity scoring, and dual-format report generation (Markdown + JSON). After two consecutive audit runs, the system identifies regressions, improvements, persistent issues, and new issues — mode-aware and venue-state-aware so PASS→QUIET transitions don't flag as regressions. Known recurring issues appear as SUPPRESSED with mandatory expiry dates.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Delta categories: REGRESSION (PASS→FAIL), IMPROVEMENT (FAIL→PASS), PERSISTENT (FAIL→FAIL), NEW_ISSUE (not in previous), STABLE (PASS→PASS)
- Mode-aware comparison: PASS→QUIET when venue closes is NOT a regression
- Venue-state-aware: compare only when venue_state matches, or handle transitions gracefully
- suppress.json location: `audit/suppress.json` — array of {phase, host, reason, expires_date (ISO), added_by}
- Expired suppressions auto-ignored (compare expires_date vs current IST date)
- SUPPRESSED status: appears in report with reason, not hidden and not counted as FAIL
- Severity scoring: P1 (critical, service down), P2 (degraded, needs attention), P3 (informational) — already in emit_result schema
- Report output: `$RESULT_DIR/audit-report.md` (Markdown) + `$RESULT_DIR/audit-summary.json` (JSON)
- Previous run auto-detection: `audit/results/latest` symlink or `latest.txt` fallback → read previous JSON results
- Results storage: `audit/results/YYYY-MM-DD_HH-MM/` already established in Phase 189
- `results/index.json` tracks run history (append-only, one entry per run)
- All jq-based — no new dependencies

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `audit/audit.sh` — entry point, creates RESULT_DIR with IST timestamp, writes run-meta.json
- `audit/lib/core.sh` — emit_result writes phase-NN-host.json with {phase, tier, host, status, severity, message, timestamp, mode, venue_state}
- `audit/results/` — .gitignored runtime directory, latest symlink/latest.txt established
- All 60 phase scripts write to RESULT_DIR via emit_result

### Established Patterns
- Result JSON schema: `{phase, tier, host, status, severity, message, timestamp, mode, venue_state}`
- Status values: PASS, WARN, FAIL, QUIET
- Severity values: P1, P2, P3
- IST timestamps via `TZ=Asia/Kolkata date`
- jq for all JSON processing

### Integration Points
- audit.sh calls report generation after all phases complete (before exit code counting)
- Delta needs previous run's results (auto-find via latest symlink)
- Suppress needs to be checked during report generation, not during phase execution
- Report generation is the final step before exit

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the mechanical implementation. Delta tracking joins on phase+host key between current and previous runs. Suppression is a simple JSON lookup with date expiry. Report generation aggregates all result JSONs into a formatted Markdown table grouped by tier.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
