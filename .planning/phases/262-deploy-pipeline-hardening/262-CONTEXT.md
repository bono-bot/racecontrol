# Phase 262: Deploy Pipeline Hardening - Context

**Gathered:** 2026-03-30
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase)

<domain>
## Phase Boundary
Harden frontend deploy pipeline: static file copy verification, NEXT_PUBLIC_ env var audit, smoke test with _next/static/ check. Must be in place before any redesigned page ships.
Requirements: DQ-01, DQ-02
</domain>

<decisions>
## Implementation Decisions
### Claude's Discretion
All choices at Claude's discretion. Key constraints:
- Standalone Next.js deploy requires `cp -r .next/static .next/standalone/.next/static`
- `outputFileTracingRoot: path.join(__dirname)` must be in both next.config.ts
- Smoke test must check `/_next/static/css/` returns 200 (not just health endpoint)
- NEXT_PUBLIC_ vars must be audited before every build
</decisions>

<code_context>
## Existing Code Insights
- deploy-staging/deploy-server.sh exists (server deploy script)
- web/next.config.ts and kiosk/next.config.ts already have outputFileTracingRoot
- Standing rule: "Frontend: standalone deploy requires .next/static copied into .next/standalone/"
</code_context>

<specifics>
No specific requirements — infrastructure phase.
</specifics>

<deferred>
None.
</deferred>
