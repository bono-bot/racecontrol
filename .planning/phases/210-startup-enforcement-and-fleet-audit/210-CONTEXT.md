# Phase 210: Startup Enforcement and Fleet Audit - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Create bat-scanner.sh for bat file drift detection and syntax validation across all 8 pods, add 5 new v25.0-specific audit phases to audit.sh, and integrate bat file sync into the deploy chain. Ensures all pods run canonical bat files and the fleet audit permanently verifies debug quality.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure/ops phase. Key constraints from requirements:
- bat-scanner.sh compares deployed bat files against canonical repo versions via rc-sentry /files endpoint (BAT-01)
- Syntax validator checks: UTF-8 BOM, parentheses in if/else, /dev/null redirects, timeout command, taskkill without restart (BAT-02)
- 5 new audit phases: bat-drift (Tier 2), sentinel-alerts (Tier 3), config-fallback (Tier 2), boot-resilience (Tier 2), verification-chains (Tier 3) (AUDIT-02)
- Audit report gains "v25.0 Debug Quality" section with per-pod summary (AUDIT-03)
- deploy-pod.sh gains bat file sync step (BAT-04)
- All audit phases use existing infrastructure: parallel execution, delta tracking, suppress.json, dual reports

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `audit/audit.sh` — main audit runner with tier-based parallel execution
- `audit/lib/` — core.sh, parallel.sh, results.sh, delta.sh, suppress.sh, report.sh, fixes.sh, notify.sh
- `audit/phases/` — 60 existing phase scripts across 18 tier directories
- `scripts/deploy-pod.sh` — pod deployment script
- `deploy-staging/start-rcagent.bat` — canonical bat file in staging
- rc-sentry `/files` endpoint for fetching deployed bat contents
- rc-sentry `/exec` endpoint for running commands on pods

### Established Patterns
- Audit phases: bash scripts in `audit/phases/tier-N/` with PASS/FAIL/QUIET exit codes
- Parallel execution with file-based semaphore (mkdir atomic locking)
- Delta tracking with 6 categories, mode-aware
- Suppress.json for expected failures with expiry

### Integration Points
- audit.sh — register 5 new phases in tier directories
- deploy-pod.sh — add bat sync step after binary download
- New scripts/bat-scanner.sh — standalone + audit phase callable

</code_context>

<specifics>
## Specific Ideas

- bat-scanner.sh fetches deployed bat via `curl -X POST http://<pod_ip>:8091/files -d '{"path":"C:\\\\RacingPoint\\\\start-rcagent.bat"}'`
- Canonical bat path: deploy-staging/start-rcagent.bat (or embedded in repo)
- Syntax checks should use sed/grep pattern matching, not bash parsing
- Boot resilience audit phase checks rc-agent /health for periodic_tasks field
- Verification chains audit can run known-bad inputs and verify error logging

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
