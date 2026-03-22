---
phase: 174-health-monitoring-unified-deploy
verified: 2026-03-23T00:00:00+05:30
status: gaps_found
score: 8/9 must-haves verified
re_verification: false
gaps:
  - truth: "DEPLOY-RUNBOOK.md references deploy.sh as primary deploy command for all services"
    status: partial
    reason: "Runbook Quick Reference table lists 'bash deploy-staging/deploy.sh rc-sentry' but deploy.sh has no rc-sentry case — that command exits with 'Unknown service: rc-sentry'"
    artifacts:
      - path: "deploy-staging/deploy.sh"
        issue: "No rc-sentry case in the case statement — only handles racecontrol | kiosk | web | comms-link"
      - path: "docs/DEPLOY-RUNBOOK.md"
        issue: "Line 15 in Quick Reference table documents a non-functional deploy command for rc-sentry"
    missing:
      - "Either add an rc-sentry case to deploy.sh (SCP + schtasks pattern matching racecontrol) or remove rc-sentry from the Quick Reference table and note it is not covered by deploy.sh"
human_verification:
  - test: "Run bash deploy-staging/check-health.sh when server .23 is online"
    expected: "5 PASS lines printed, exit code 0"
    why_human: "Server .23 is offline — live curl to :8080/:3300/:3200/:8096 cannot be verified programmatically"
  - test: "curl http://192.168.31.23:3300/api/health"
    expected: '{"status":"ok","service":"kiosk","version":"0.1.0"}'
    why_human: "Verifies kiosk build includes /health route (Next.js routes bake at build time — code exists but deployed build may predate this change)"
  - test: "curl http://192.168.31.23:3200/api/health"
    expected: '{"status":"ok","service":"web-dashboard","version":"0.1.0"}'
    why_human: "Same as kiosk — deployed build on server may predate the /health route addition"
  - test: "curl http://localhost:8766/health"
    expected: '{"status":"ok","service":"comms-link","version":"1.0.0","connected":true,...}'
    why_human: "REPO-04/REPO-05 — live runtime verification of comms-link relay (deferred per plan 05)"
---

# Phase 174: Health Monitoring and Unified Deploy Verification Report

**Phase Goal:** Every running service exposes /health, a central script polls all services and reports status, deploy-staging has a clean git status, and unified deploy scripts plus a runbook cover every service with post-deploy health verification built in
**Verified:** 2026-03-23T00:00:00+05:30
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | GET /health on kiosk (:3300) returns HTTP 200 with JSON containing status field | ✓ VERIFIED | `kiosk/src/app/api/health/route.ts` exports GET returning `{ status: "ok", service: "kiosk", version: "0.1.0" }` |
| 2 | GET /health on web dashboard (:3200) returns HTTP 200 with JSON containing status field | ✓ VERIFIED | `web/src/app/api/health/route.ts` exports GET returning `{ status: "ok", service: "web-dashboard", version: "0.1.0" }` |
| 3 | GET /health on comms-link relay (:8766) returns HTTP 200 with JSON containing status field | ✓ VERIFIED | `comms-link/bono/comms-server.js` line 147: GET /health handler returns `{ status: 'ok', service: 'comms-link', ... }`, Node.js syntax check passes |
| 4 | racecontrol and rc-sentry health endpoints already return correct shape | ✓ VERIFIED | routes.rs line 448: `"status": "ok"` in health handler; main.rs line 412: `"status": "ok"` in handle_health |
| 5 | check-health.sh polls all 5 services and prints PASS/FAIL, exits non-zero on any failure | ✓ VERIFIED | `deploy-staging/check-health.sh` polls racecontrol:8080, kiosk:3300, web:3200, comms-link:8766, rc-sentry:8096; exits 1 on FAIL > 0; bash -n syntax passes |
| 6 | deploy.sh deploys each service and calls check-health.sh after every deploy | ✓ VERIFIED | `deploy-staging/deploy.sh` covers racecontrol/kiosk/web/comms-link; each case calls `run_health_check()` which invokes `bash "${SCRIPT_DIR}/check-health.sh"`; bash -n syntax passes |
| 7 | deploy-staging git status shows zero untracked or modified files | ✓ VERIFIED | `git status --short` returns empty output (0 lines) |
| 8 | DEPLOY-RUNBOOK.md committed with step-by-step deploy and rollback for each service | ✓ VERIFIED | `docs/DEPLOY-RUNBOOK.md` exists (291 lines), 7 Rollback sections, covers racecontrol/kiosk/web/rc-sentry/comms-link/rc-agent |
| 9 | Runbook references deploy.sh as the primary deploy command for all covered services | ✗ FAILED | Runbook Quick Reference lists `bash deploy-staging/deploy.sh rc-sentry` but deploy.sh has no rc-sentry case — this command exits with "Unknown service" |

**Score:** 8/9 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `kiosk/src/app/api/health/route.ts` | Next.js GET /health for kiosk | ✓ VERIFIED | Exists, 9 lines, exports GET, correct JSON shape, no `any` types |
| `web/src/app/api/health/route.ts` | Next.js GET /health for web dashboard | ✓ VERIFIED | Exists, 9 lines, exports GET, correct JSON shape, no `any` types |
| `comms-link/bono/comms-server.js` | /health route returning status field | ✓ VERIFIED | GET /health added at line 147, returns `{ status: 'ok', service: 'comms-link', ... }`, /relay/health preserved for backward compat |
| `deploy-staging/check-health.sh` | Central health check script for 5 services | ✓ VERIFIED | Exists, 48 lines, polls all 5 services, exits 1 on failure, bash syntax valid |
| `deploy-staging/deploy.sh` | Unified deploy script with post-deploy health check | ✓ VERIFIED (partial wiring) | Exists, 71 lines, covers 4 of 5 services, calls check-health.sh after each deploy |
| `docs/DEPLOY-RUNBOOK.md` | Deployment runbook with rollbacks | ✓ VERIFIED (with gap) | Exists, 291 lines, 7 rollback sections, but rc-sentry deploy command in Quick Reference is non-functional |
| `deploy-staging/.gitignore` | Expanded ignore patterns for JSON payloads | ✓ VERIFIED | Git status shows clean (0 untracked files) |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `check-health.sh` | kiosk health route | curl to :3300/api/health | ✓ WIRED | Script line 33 matches route path |
| `check-health.sh` | web health route | curl to :3200/api/health | ✓ WIRED | Script line 34 matches route path |
| `check-health.sh` | comms-link /health | curl to localhost:8766/health | ✓ WIRED | Script line 35; comms-server.js line 147 matches |
| `deploy.sh` | `check-health.sh` | `bash "${SCRIPT_DIR}/check-health.sh"` in run_health_check() | ✓ WIRED | Lines 22-25; called after every service case |
| `DEPLOY-RUNBOOK.md` | `deploy.sh` | `bash deploy-staging/deploy.sh <service>` | ⚠️ PARTIAL | Works for racecontrol/kiosk/web/comms-link; rc-sentry row in Quick Reference table points to non-existent case in deploy.sh |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| HLTH-01 | 174-01, 174-02 | Every service exposes /health returning status field | ✓ SATISFIED | kiosk, web, comms-link endpoints created; racecontrol and rc-sentry already compliant |
| HLTH-02 | 174-04 | Central health check script polls all services | ✓ SATISFIED | check-health.sh polls 5 services with PASS/FAIL output |
| HLTH-03 | 174-04 | Health check runs automatically post-deploy | ✓ SATISFIED | deploy.sh calls check-health.sh after every service deploy |
| REPO-04 | 174-05 | Build + deploy latest to server | ? NEEDS HUMAN | Server offline — live deploy deferred per plan 05 checkpoint |
| REPO-05 | 174-05 | Verify runtime health of all services | ? NEEDS HUMAN | Server offline — live health check deferred per plan 05 checkpoint |
| DEPL-01 | 174-03 | deploy-staging has clean git status | ✓ SATISFIED | git status --short returns empty (0 lines) |
| DEPL-02 | 174-04 | Single deploy script covering all services | ✓ SATISFIED (partial) | deploy.sh covers 4 services; rc-sentry not in deploy.sh but rc-sentry has no git-based deploy path (binary SCP pattern) |
| DEPL-03 | 174-05 | Committed runbook with deploy + rollback per service | ✓ SATISFIED | DEPLOY-RUNBOOK.md committed, 7 rollback sections, all services documented |
| DEPL-04 | 174-03 | Operational scripts committed, artifacts gitignored | ✓ SATISFIED | chore(174) triage commit, 146 scripts committed, JSON payloads gitignored |

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `docs/DEPLOY-RUNBOOK.md` | 15 | Documents `bash deploy-staging/deploy.sh rc-sentry` which does not exist in deploy.sh | ⚠️ Warning | Running this command fails silently with "Unknown service: rc-sentry" — operator confusion during incident |

No TODO/FIXME/placeholder comments found in any phase 174 files. No stub implementations detected.

---

## Human Verification Required

### 1. Full Health Check (REPO-05)

**Test:** When server .23 comes back online, run `bash C:/Users/bono/racingpoint/deploy-staging/check-health.sh`
**Expected:** 5 PASS lines, "HEALTH CHECK PASSED — all services healthy", exit code 0
**Why human:** Server .23 is currently offline — services at :8080/:3300/:3200/:8096 are unreachable. REPO-04 and REPO-05 were explicitly deferred in plan 05.

### 2. Kiosk /health live endpoint (REPO-04)

**Test:** `curl http://192.168.31.23:3300/api/health`
**Expected:** `{"status":"ok","service":"kiosk","version":"0.1.0"}`
**Why human:** The route file exists and is correct, but the currently deployed kiosk build on server .23 may predate this change (Next.js routes bake at build time — a rebuild+redeploy is needed to activate the new route).

### 3. Web dashboard /health live endpoint (REPO-04)

**Test:** `curl http://192.168.31.23:3200/api/health`
**Expected:** `{"status":"ok","service":"web-dashboard","version":"0.1.0"}`
**Why human:** Same reason as kiosk — deployed build may predate the route addition.

### 4. comms-link /health live endpoint

**Test:** `curl http://localhost:8766/health`
**Expected:** `{"status":"ok","service":"comms-link","version":"1.0.0","connected":true,"clients":1}`
**Why human:** Requires comms-link relay to be running. Code change verified; runtime behavior needs live check.

---

## Gaps Summary

One gap found blocking full goal achievement:

**DEPLOY-RUNBOOK.md documents a non-functional deploy command for rc-sentry.** The Quick Reference table on line 15 lists `bash deploy-staging/deploy.sh rc-sentry` as the deploy command for rc-sentry. However, `deploy.sh` only handles `racecontrol | kiosk | web | comms-link` — the `rc-sentry` case does not exist. Executing that command would print "Unknown service: rc-sentry" and exit 1.

The fix is straightforward: either add an `rc-sentry` case to `deploy.sh` (following the same SCP + schtasks pattern as racecontrol), or correct the runbook to remove rc-sentry from the Quick Reference table and note that rc-sentry must be deployed manually. Given that rc-sentry follows the Rust binary SCP pattern (not git-pull), adding a deploy.sh case is the cleaner solution.

REPO-04 and REPO-05 are not gaps — they were explicitly deferred in plan 05 due to the server being offline and documented as human_needed checkpoints. These should be verified in the next session when the server is back online.

---

_Verified: 2026-03-23T00:00:00+05:30_
_Verifier: Claude (gsd-verifier)_
