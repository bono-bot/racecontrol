# Phase 174: Health Monitoring & Unified Deploy - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Add /health endpoints to all running services, create central health check script, clean deploy-staging (714 dirty files), create unified deploy scripts with post-deploy health checks, and write deployment runbook. Build and deploy latest code to all environments, verify running at runtime.

</domain>

<decisions>
## Implementation Decisions

### Health Endpoints
- Every service needs GET /health returning JSON { status: "ok", service: "name", version: "x.y.z" }
- racecontrol already has /health — verify it returns correct JSON format
- kiosk, web dashboard, comms-link relay need /health added or verified
- rc-sentry already has /health — verify

### Central Health Check
- Bash script check-health.sh in deploy-staging
- Polls all services and prints pass/fail per service
- Exits non-zero if any service is down
- Services to check: racecontrol :8080, kiosk :3300, web dashboard :3200, comms-link relay :8766, rc-sentry :8096

### Deploy-Staging Cleanup
- 714 dirty files need triage: keep, delete, or .gitignore
- Most are likely build artifacts, old binaries, temp files
- Goal: git status clean after

### Unified Deploy Script
- deploy.sh in deploy-staging — deploys each service by name
- Runs health check after each deploy
- Covers: racecontrol, rc-agent, kiosk, web dashboard, comms-link

### Deployment Runbook
- docs/DEPLOY-RUNBOOK.md in racecontrol repo
- Step-by-step for each service with one-command rollback
- Incorporates existing standing rules about deploy

### Build & Runtime Verification (REPO-04, REPO-05)
- Server and pods are OFFLINE — cannot verify live deployment
- Can verify: builds compile, kiosk builds, scripts are syntactically valid
- Live verification deferred (human_needed)

### Claude's Discretion
- deploy-staging file triage decisions (which files to keep/delete/ignore)
- Health check script implementation details
- Deploy script structure and service ordering
- Runbook format and level of detail

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- racecontrol /health endpoint exists
- rc-sentry /health endpoint exists
- deploy-staging already has ops.bat, deploy scripts, start-*.bat files
- Standing rules document deploy procedures for each service

### Established Patterns
- .bat files for Windows services
- Bash scripts for James-side automation
- Standing rule: schtasks for server deploy (survives SSH disconnect)
- Standing rule: RCAGENT_SELF_RESTART sentinel for pod deploy

### Integration Points
- Server .23 (offline)
- All 8 pods (offline)
- James .27 (local)
- Bono VPS (relay available)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

- Live deployment verification (needs server/pods online)
- Automated deploy pipeline (CI/CD — future milestone)

</deferred>
