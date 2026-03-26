---
phase: 202-config-validation-structural-fixes
verified: 2026-03-26T05:15:00+05:30
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 202: Config Validation & Structural Fixes Verification Report

**Phase Goal:** Audit phase scripts that currently produce false PASSes due to unchecked config values, hardcoded assumptions, or wrong severity levels are fixed to detect real misconfigurations
**Verified:** 2026-03-26T05:15:00+05:30
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Phase 02 audit emits WARN when ws_connect_timeout < 600ms in racecontrol.toml | VERIFIED | phase02.sh lines 43-56: safe_remote_exec findstr ws_connect_timeout, parses numeric value, emits WARN P2 when < 600, PASS P3 when >= 600 |
| 2 | Phase 02 audit emits WARN when app_health monitoring URLs have wrong ports | VERIFIED | phase02.sh lines 58-75: safe_remote_exec findstr app_health, checks for :3201 and :3300, emits WARN P2 when either missing |
| 3 | Phase 21 audit emits WARN (not PASS) when billing endpoint is unreachable during venue hours | VERIFIED | phase21.sh lines 46-53 (active) and 60-66 (history): empty response + venue_state != "closed" = WARN P2, venue closed = QUIET P3. Pricing also venue-aware (lines 24-27) |
| 4 | Phase 53 audit emits WARN when server PowerShell count is 0 (watchdog dead) | VERIFIED | phase53.sh lines 72-73: ps_count -eq 0 = WARN P2 "watchdog may be dead"; line 74-75: ps_count -eq 1 = PASS P3 "singleton healthy" |
| 5 | Phase 30 WhatsApp check tests Evolution API live connection state (not just config presence) | VERIFIED | phase30.sh lines 28-54: extracts evo_url from TOML, queries /api/instance/connectionState/racingpoint, checks state=open (PASS), other state (WARN), unreachable (FAIL P1) |
| 6 | Phase 31 email check verifies OAuth token expiry date proactively | VERIFIED | phase31.sh lines 40-93: tries 3 token filenames, parses expiry_date (epoch ms, epoch s, ISO), calculates days_left, FAIL if expired, WARN if <= 7 days, PASS if > 7 days |
| 7 | Phase 19 display resolution queries actual resolution from pod via rc-agent exec (not hardcoded) | VERIFIED | phase19.sh lines 30-52: safe_remote_exec with Get-CimInstance Win32_VideoController, parses horiz/vert from stdout. Hardcoded `horiz="1920"; vert="1080"` confirmed ABSENT. Fallback to health check emits WARN if query fails. |
| 8 | start-rcsentry-ai.bat exists in repo with go2rtc warmup step before rc-sentry-ai launch | VERIFIED | scripts/deploy/start-rcsentry-ai.bat: 24 lines, CRLF line endings (DOS batch), go2rtc warmup loop over 13 streams (line 17), rc-sentry-ai.exe launch (line 23) |

**Score:** 8/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `audit/phases/tier1/phase02.sh` | Config value validation for ws_connect_timeout and app_health URLs | VERIFIED | 113 lines, contains ws_connect_timeout check (lines 43-56), app_health URL check (lines 58-75), bash -n PASS |
| `audit/phases/tier4/phase21.sh` | Billing endpoint unreachable WARN during venue hours | VERIFIED | 72 lines, venue-state-aware checks for pricing, active sessions, and history endpoints, bash -n PASS |
| `audit/phases/tier12/phase53.sh` | Watchdog dead detection (ps_count=0) | VERIFIED | 86 lines, ps_count=0 -> WARN, ps_count=1 -> PASS, ps_count>1 -> WARN, bash -n PASS |
| `audit/phases/tier6/phase30.sh` | Evolution API live connection state check | VERIFIED | 73 lines, extracts evo_url from TOML, queries connectionState endpoint, bash -n PASS |
| `audit/phases/tier6/phase31.sh` | OAuth token expiry proactive verification | VERIFIED | 112 lines, tries 3 token filenames, parses epoch ms/s/ISO, calculates days_left, bash -n PASS |
| `audit/phases/tier3/phase19.sh` | Real resolution query via rc-agent exec | VERIFIED | 74 lines, Get-CimInstance Win32_VideoController, fallback to health check, no hardcoded values, bash -n PASS |
| `scripts/deploy/start-rcsentry-ai.bat` | go2rtc warmup before rc-sentry-ai start | VERIFIED | 24 lines, DOS batch file with CRLF, log rotation, 13-stream warmup loop, rc-sentry-ai.exe launch |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| phase02.sh | racecontrol.toml on server .23 | safe_remote_exec findstr | WIRED | Lines 44-46: `safe_remote_exec "192.168.31.23" "8090" 'findstr /C:"ws_connect_timeout" C:\RacingPoint\racecontrol.toml'` |
| phase21.sh | billing endpoints on server .23 | curl HTTP code check | WIRED | Lines 43-45: curl to /api/v1/billing/sessions/active, lines 57-59: curl to /api/v1/billing/sessions?limit=3, venue_state check on empty response |
| phase30.sh | Evolution API on Bono VPS | http_get connection state endpoint | WIRED | Line 41: `http_get "${evo_url}/api/instance/connectionState/racingpoint" 10`, line 43: jq parse `.instance.state // .state` |
| phase19.sh | rc-agent /exec on each pod | safe_remote_exec PowerShell resolution query | WIRED | Lines 33-35: `safe_remote_exec "$ip" "8090" 'powershell -Command "(Get-CimInstance Win32_VideoController..."'` |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CV-01 | 202-01 | Phase 02 validates ws_connect_timeout >= 600ms | SATISFIED | phase02.sh lines 43-56, emit_result host="server-23-ws-timeout" |
| CV-02 | 202-01 | Phase 02 validates app_health URL ports | SATISFIED | phase02.sh lines 58-75, emit_result host="server-23-app-health-urls" |
| CV-03 | 202-02 | Phase 30 tests Evolution API live connection state | SATISFIED | phase30.sh lines 28-54, emit_result host="server-23-wa-connection" |
| CV-04 | 202-02 | Phase 31 checks OAuth token expiry proactively | SATISFIED | phase31.sh lines 40-93, emit_result host="server-23-email-oauth-expiry" |
| SF-01 | 202-02 | Phase 19 queries real resolution via rc-agent exec | SATISFIED | phase19.sh lines 30-52, Get-CimInstance Win32_VideoController, no hardcoded values |
| SF-02 | 202-01 | Phase 21 billing unreachable returns WARN during venue hours | SATISFIED | phase21.sh lines 49-50 and 62-63, WARN P2 when venue_state != "closed" |
| SF-03 | 202-01 | Phase 53 treats ps_count=0 as WARN (watchdog dead) | SATISFIED | phase53.sh lines 72-73, WARN P2 with "watchdog may be dead" message |
| OP-01 | 202-02 | go2rtc warmup in start-rcsentry-ai.bat | SATISFIED | scripts/deploy/start-rcsentry-ai.bat lines 14-20, 13 streams warmed before launch |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No TODO/FIXME/PLACEHOLDER/HACK found in any modified file |

### Human Verification Required

### 1. Live Audit Run

**Test:** Run `AUDIT_PIN=261121 bash audit/audit.sh --mode quick` during venue open hours with at least one pod online
**Expected:** Phase 02 emits PASS for ws_connect_timeout (assuming server has >= 600ms), Phase 19 emits actual resolution values (not hardcoded 1920x1080), Phase 21 emits PASS or WARN based on actual endpoint state, Phase 30 shows Evolution API connection status, Phase 31 shows OAuth token status
**Why human:** Requires live server, pods, and Evolution API to be running -- cannot verify end-to-end behavior from code alone

### 2. Venue-Closed Behavior

**Test:** Run audit during closed hours (or set VENUE_STATE=closed manually)
**Expected:** Phase 19 emits QUIET for all pods, Phase 21 emits QUIET for unreachable billing endpoints (not WARN), Phase 53 emits correctly regardless of venue state
**Why human:** Venue state logic paths need live testing to confirm correct branching

### Gaps Summary

No gaps found. All 8 requirements are satisfied with substantive implementations. All scripts pass bash syntax validation. No anti-patterns detected. All key links are wired with actual remote exec calls, HTTP queries, and proper response parsing.

---

_Verified: 2026-03-26T05:15:00+05:30_
_Verifier: Claude (gsd-verifier)_
