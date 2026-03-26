# Phase 203: Deep Service Verification - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Upgrade 8 audit phase scripts from shallow liveness/count checks to real service health verification. Requirements: WL-01, WL-02, WL-03, WL-04, CH-01, CH-02, CH-03, CH-04.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion -- pure infrastructure phase. All changes are bash script edits to existing files in audit/phases/tier*/phase*.sh.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- http_get in audit/lib/core.sh -- curl wrapper with timeout
- safe_remote_exec in audit/lib/core.sh -- remote exec via rc-agent :8090
- emit_result in audit/lib/results.sh -- standardized result emission

### Integration Points
- Phase 07: audit/phases/tier1/phase07.sh (process guard allowlist)
- Phase 09: audit/phases/tier1/phase09.sh (self-monitor)
- Phase 10: audit/phases/tier1/phase10.sh (AI healer watchdog)
- Phase 15: audit/phases/tier2/phase15.sh (preflight checks)
- Phase 25: audit/phases/tier4/phase25.sh (cafe menu)
- Phase 39: audit/phases/tier8/phase39.sh (feature flags)
- Phase 44: audit/phases/tier9/phase44.sh (face detection)
- Phase 56: audit/phases/tier14/phase56.sh (OpenAPI freshness)

</code_context>

<specifics>
## Specific Ideas

No specific requirements -- infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope.

</deferred>
