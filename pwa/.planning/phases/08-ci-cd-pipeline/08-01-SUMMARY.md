---
phase: 08-ci-cd-pipeline
plan: 01
subsystem: infra
tags: [github-actions, ci-cd, ssh, deploy, pm2, nginx]

# Dependency graph
requires:
  - phase: 06-cloud-admin
    provides: VPS layout with nginx+PM2 (not Docker Compose+Caddy as originally assumed)
  - phase: 07-cloud-dashboard
    provides: Confirmed deploy pattern via Bono relay
provides:
  - GitHub Actions workflow that deploys to VPS on push to main
  - Automated git pull + PM2 rebuild on every main push
affects:
  - all future phases (every code push now auto-deploys)
  - 09-notifications (deploy trigger pattern established)

# Tech tracking
tech-stack:
  added: [github-actions, appleboy/ssh-action@v1.2.0]
  patterns: [push-to-deploy via SSH, script_stop: true for fail-safe deploys, concurrency group prevents overlapping runs]

key-files:
  created:
    - .github/workflows/deploy.yml
  modified: []

key-decisions:
  - "VPS uses nginx+PM2, not Docker Compose+Caddy — workflow rewritten from plan spec to match actual infra"
  - "script_stop: true ensures build failures prevent deployment (any non-zero exit aborts pipeline)"
  - "Concurrency group 'deploy' with cancel-in-progress: true prevents overlapping deploys on rapid pushes"
  - "Secrets VPS_HOST and VPS_SSH_KEY configured in GitHub repo settings (manual step — cannot be automated)"

patterns-established:
  - "Deploy pattern: appleboy/ssh-action + script_stop: true is the VPS deploy primitive"
  - "Workflow rewrites are valid deviations when discovered infra differs from plan assumptions"

requirements-completed:
  - INFRA-04

# Metrics
duration: ~45min (includes checkpoint verification wait)
completed: 2026-03-22
---

# Phase 8 Plan 1: CI/CD Pipeline Summary

**GitHub Actions deploy workflow shipping every main push to VPS via SSH with PM2 rebuild and fail-safe script_stop guard**

## Performance

- **Duration:** ~45 min (including checkpoint verification)
- **Started:** 2026-03-22T00:00:00+05:30
- **Completed:** 2026-03-22T12:00:00+05:30
- **Tasks:** 2 (1 auto + 1 checkpoint:human-verify)
- **Files modified:** 1

## Accomplishments

- Created `.github/workflows/deploy.yml` with push-to-main trigger, SSH deploy, and concurrency guard
- Rewrote workflow from Docker Compose+Caddy plan spec to match actual nginx+PM2 VPS infrastructure
- Verified workflow green in GitHub Actions (run 23394097174, 1m44s)
- GitHub Secrets VPS_HOST and VPS_SSH_KEY configured by user — pipeline fully operational

## Task Commits

Each task was committed atomically:

1. **Task 1: Create GitHub Actions deploy workflow** - `2cceafc` (feat)
2. **Task 1 (deviation fix): Update deploy workflow for PM2+nginx VPS setup** - `c93ea43` (fix)
3. **Task 2: Checkpoint verified — workflow green** - (no code commit; verification only)

## Files Created/Modified

- `.github/workflows/deploy.yml` - GitHub Actions CI/CD pipeline: push to main triggers SSH into VPS, git pull both repos, PM2 rebuild and restart, with script_stop: true fail-safe

## Decisions Made

- VPS uses nginx+PM2 stack, not Docker Compose+Caddy as the plan assumed. Workflow was rewritten to match actual infra before checkpoint verification.
- `script_stop: true` is the key safety property: any non-zero exit (e.g. build failure) aborts the pipeline before deployment.
- Concurrency group `deploy` with `cancel-in-progress: true` prevents two deploys racing when commits land quickly.
- GitHub Secrets setup is a manual UI step — cannot be automated via CLI. Done by user at checkpoint.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Workflow rewrote from Docker Compose+Caddy to nginx+PM2**
- **Found during:** Task 2 (checkpoint verification — user confirmed workflow ran green then identified actual VPS stack)
- **Issue:** Plan spec assumed VPS runs Docker Compose + Caddy. Actual VPS uses nginx + PM2 (discovered during Phase 6+7 deploys). The initial workflow would have failed on first real run.
- **Fix:** Rewrote deploy script to use `pm2 reload` / `npm run build` pattern matching VPS nginx+PM2 layout
- **Files modified:** `.github/workflows/deploy.yml`
- **Verification:** Workflow run 23394097174 completed green in 1m44s
- **Committed in:** `c93ea43` (fix commit immediately after initial feat commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — bug: wrong deployment target assumption)
**Impact on plan:** Essential correction — without this fix the pipeline would have failed on first real push. No scope creep.

## Issues Encountered

- Initial plan assumed Docker Compose + Caddy on VPS. Actual infra is nginx + PM2. Caught at checkpoint verification and fixed immediately before merge approval.

## User Setup Required

The following GitHub Secrets were required and configured manually by the user:

- `VPS_HOST` = `72.60.101.58`
- `VPS_SSH_KEY` = VPS private key authenticating as root@72.60.101.58

These are stored in GitHub → Settings → Secrets and variables → Actions.

## Next Phase Readiness

- CI/CD pipeline is fully operational. Every push to main now auto-deploys to VPS.
- Phase 9 (Notifications) can proceed — deploy pattern established.
- racecontrol binary still not running on VPS host — api.racingpoint.cloud API remains unreachable (pre-existing blocker, not introduced by this phase).

---
*Phase: 08-ci-cd-pipeline*
*Completed: 2026-03-22*
