# Phase 202: Config Validation & Structural Fixes - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix 8 audit phase scripts that currently produce false PASSes due to unchecked config values, hardcoded assumptions, or wrong severity levels. Requirements: CV-01, CV-02, CV-03, CV-04, SF-01, SF-02, SF-03, OP-01.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. All changes are bash script edits to existing files in audit/phases/tier*/phase*.sh and one bat file edit (start-rcsentry-ai.bat).

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- http_get() in audit/lib/core.sh — curl wrapper with timeout
- safe_remote_exec() in audit/lib/core.sh — remote exec via rc-agent :8090
- emit_result() in audit/lib/results.sh — standardized result emission
- All phase scripts follow identical structure: set -u, set -o pipefail, run_phaseNN() function, export -f

### Established Patterns
- Status values: PASS, WARN, FAIL, QUIET (venue-closed suppression)
- Severity: P1 (critical), P2 (degraded), P3 (informational)
- jq for JSON parsing, grep for string matching

### Integration Points
- Phase 02: audit/phases/tier1/phase02.sh (config integrity)
- Phase 19: audit/phases/tier3/phase19.sh (display resolution)
- Phase 21: audit/phases/tier4/phase21.sh (pricing and billing)
- Phase 30: audit/phases/tier6/phase30.sh (WhatsApp alerter)
- Phase 31: audit/phases/tier6/phase31.sh (email alerts)
- Phase 53: audit/phases/tier12/phase53.sh (binary consistency)
- start-rcsentry-ai.bat: C:/RacingPoint/start-rcsentry-ai.bat (go2rtc warmup)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
