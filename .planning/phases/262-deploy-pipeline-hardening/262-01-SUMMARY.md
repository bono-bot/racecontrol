---
phase: 262-deploy-pipeline-hardening
plan: 01
subsystem: deploy-pipeline
tags: [deploy, nextjs, env-audit, static-files, hardening]
dependency_graph:
  requires: []
  provides: [check-frontend-env.sh, deploy-nextjs-hardened]
  affects: [deploy-nextjs.sh]
tech_stack:
  added: []
  patterns: [pre-build-audit, static-smoke-test]
key_files:
  created:
    - scripts/deploy/check-frontend-env.sh
  modified:
    - scripts/deploy/deploy-nextjs.sh
decisions:
  - Static smoke test reports "degraded" instead of aborting deploy (app is serving HTML but CSS may be broken)
  - Env audit scans src/ for actual NEXT_PUBLIC_ references rather than using a hardcoded list
metrics:
  duration: 5min
  completed: "2026-03-30T10:38:00Z"
---

# Phase 262 Plan 01: Deploy Pipeline Hardening Summary

Pre-build NEXT_PUBLIC_ env var audit script + static file smoke test in deploy-nextjs.sh

## What Was Done

### Task 1: check-frontend-env.sh (new script)
Created `scripts/deploy/check-frontend-env.sh` that audits NEXT_PUBLIC_ env vars before any Next.js build.

**What it checks:**
1. Scans `<app>/src/` for all `NEXT_PUBLIC_*` variable references in `.ts`/`.tsx` files
2. Verifies each found var exists in `<app>/.env.production.local`
3. Verifies no var value contains `localhost` or `127.0.0.1`
4. Exits 0 on pass (all vars present with LAN IPs), exits 1 on failure

**Commit:** `e9a2c8a8`

### Task 2: deploy-nextjs.sh hardening
Modified `scripts/deploy/deploy-nextjs.sh` with two changes:

1. **Step [1/9] env audit injection:** Calls `check-frontend-env.sh` BEFORE `npm run build`. With `set -euo pipefail`, a non-zero exit aborts the entire deploy before any build runs.

2. **Step [9/9] static file smoke test:** After health check passes, curls `_next/static/css/<first-css>.css` and verifies HTTP 200. If static files return 404, deploy is marked "degraded" with manual fix instructions (does not abort since HTML is serving).

3. **Step renumbering:** All steps updated from /8 to /9.

**Commit:** `f58c3fc6`

## Deviations from Plan

None - plan executed exactly as written.

## Verification

- Both `.env.production.local` files verified to contain 3 NEXT_PUBLIC_ vars with LAN IP `192.168.31.23`
- `grep -n "check-frontend-env"` returns hit in deploy-nextjs.sh line 160
- `grep -n "_next/static/css"` returns hit in deploy-nextjs.sh line 353
- `grep -n "9/9"` returns hits in deploy-nextjs.sh lines 17 and 336+

## Known Stubs

None - both scripts are fully functional with no placeholder logic.

## Self-Check: PASSED
- scripts/deploy/check-frontend-env.sh: FOUND
- scripts/deploy/deploy-nextjs.sh: FOUND (modified)
- Commit e9a2c8a8: FOUND
- Commit f58c3fc6: FOUND
