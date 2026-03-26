---
phase: 210-startup-enforcement-and-fleet-audit
verified: 2026-03-26T07:15:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 210: Startup Enforcement and Fleet Audit Verification Report

**Phase Goal:** All 8 pods run bat files that match the canonical repo version, bat syntax violations are detected before deploy, and the fleet audit system includes 5 new v25.0-specific phases that permanently verify debug quality on every audit run
**Verified:** 2026-03-26T07:15:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | bat-scanner.sh produces per-pod MATCH or DRIFT report with specific line differences | VERIFIED | `bat_scan_pod()` at L186-255 computes SHA256 hashes (L221-223), prints MATCH/DRIFT, and runs `diff --color=auto` on mismatch (L239-243) with labeled output |
| 2 | Drift report shows specific missing and extra lines, not just hash mismatch | VERIFIED | `diff --label "canonical" --label "pod-$pod_num"` at L239-243 shows exact line diffs after CR stripping |
| 3 | Syntax validator catches UTF-8 BOM, parentheses in if/else, /dev/null, timeout, taskkill without restart | VERIFIED | `bat_validate_syntax()` at L61-178: BOM via xxd (L73-86), parentheses via grep -nP (L90-107), /dev/null (L110-118), timeout with /nobreak exemption (L123-135), taskkill with bloatware skip list (L142-175). Canonical bat passes with 0 violations (verified via `--validate`) |
| 4 | bat-scanner.sh works standalone and is callable as a function from audit phases | VERIFIED | Standalone via `if [[ "${BASH_SOURCE[0]}" == "${0}" ]]` guard (L472); functions `bat_scan_pod`, `bat_scan_all`, `bat_validate_syntax` available when sourced; phase61.sh sources it at L27 |
| 5 | audit.sh --mode full includes 5 new v25.0 phases (bat-drift, config-fallback, boot-resilience, sentinel-alerts, verification-chains) | VERIFIED | audit.sh L378: tier 2 dispatches run_phase61/62/63; L379: tier 3 dispatches run_phase64/65; mode-based full run at L404 (tier2) and L408 (tier3); usage updated to "65 phases" at L88 |
| 6 | bat-drift audit phase calls bat-scanner.sh and reports PASS/FAIL per pod | VERIFIED | phase61.sh sources bat-scanner.sh (L27), calls `bat_scan_pod_json` per pod (L53), parses JSON status (L55), emits per-pod results via `emit_result` (L92) and fleet summary (L111) |
| 7 | deploy-pod.sh syncs canonical bat files after binary swap and before restart | VERIFIED | Bat sync step at L145-167 (after swap at L136-143, before start at L169-172); bat files copied to BINARY_DIR at L249-250 before HTTP server starts at L255; post-deploy bat verification via bat-scanner.sh at L190-198 |
| 8 | Audit report includes v25.0 Debug Quality section with per-pod summary | VERIFIED | report.sh L363-417: generates "v25.0 Debug Quality" header, per-pod table with bat drift/config fallback/boot resilience/sentinel/verification columns from phases 61-65 results, summary line "Debug Quality: N/8 pods fully instrumented" |
| 9 | Each new audit phase follows existing pattern: emit_result, PASS/FAIL/QUIET, venue-state-aware | VERIFIED | All 5 phases use emit_result, handle venue_state=="closed" as QUIET, follow set -u/set -o pipefail/no set -e pattern, export -f their run function |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `scripts/bat-scanner.sh` | Bat drift detection + syntax validation | VERIFIED | 576 lines, valid bash syntax, standalone + sourceable, all 5 checks implemented |
| `audit/phases/tier2/phase61.sh` | bat-drift audit phase | VERIFIED | 116 lines, run_phase61, sources bat-scanner.sh, per-pod + fleet emit_result |
| `audit/phases/tier2/phase62.sh` | config-fallback audit phase | VERIFIED | 85 lines, run_phase62, checks racecontrol.toml via rc-sentry /files for OBS-02 defaults |
| `audit/phases/tier2/phase63.sh` | boot-resilience audit phase | VERIFIED | 67 lines, run_phase63, checks periodic_tasks in health via jq |
| `audit/phases/tier3/phase64.sh` | sentinel-alerts audit phase | VERIFIED | 55 lines, run_phase64, fleet-level check for active_sentinels field |
| `audit/phases/tier3/phase65.sh` | verification-chains audit phase | VERIFIED | 65 lines, run_phase65, checks server health for chain fields, uptime fallback |
| `scripts/deploy-pod.sh` | bat file sync step in deploy chain | VERIFIED | Bat sync at L145-167, bat copy to BINARY_DIR at L249-250, post-deploy verify at L190-198 |
| `audit/audit.sh` | Updated dispatch for phases 61-65 | VERIFIED | Tier 2/3 dispatch updated, mode-based full run updated, usage says 65 phases |
| `audit/lib/report.sh` | Debug Quality report section | VERIFIED | L363-417, per-pod table from phase 61-65 results, "Debug Quality: N/8" summary |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `scripts/bat-scanner.sh` | rc-sentry /files on each pod | `curl.*8091/files` | WIRED | L210-213: POST to `http://${ip}:${SENTRY_PORT}/files` with JSON payload from file |
| `scripts/bat-scanner.sh` | `scripts/deploy/start-rcagent.bat` | `CANONICAL_BAT` comparison | WIRED | L36: `CANONICAL_RCAGENT="$REPO_ROOT/scripts/deploy/start-rcagent.bat"`, used in sha256sum comparison |
| `audit/phases/tier2/phase61.sh` | `scripts/bat-scanner.sh` | source + bat_scan_pod_json | WIRED | L20-27: resolves path, sources script, calls bat_scan_pod_json at L53 |
| `audit/audit.sh` | phase61.sh | source_tier + run_phase61 dispatch | WIRED | L378: `run_phase61` in tier 2 dispatch, L404: in full run function |
| `scripts/deploy-pod.sh` | rc-sentry /exec endpoint | curl bat download command | WIRED | L149-161: `/exec` with cmd containing `curl -s -o` for bat files |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-----------|-------------|--------|----------|
| BAT-01 | 210-01 | bat-scanner.sh with drift detection via rc-sentry /files, SHA256 compare, diff output, MATCH/DRIFT per pod | SATISFIED | bat-scanner.sh L186-255: pod_ip map, /files fetch, sha256sum compare, diff on mismatch, also scans start-rcsentry.bat |
| BAT-02 | 210-01 | Syntax validator: BOM, parentheses, /dev/null, timeout, taskkill-without-restart, with line numbers and fix suggestions | SATISFIED | bat_validate_syntax L61-178: all 5 checks with line numbers and suggested fixes |
| BAT-03 | 210-02 | Bat scanner integrated into audit as Tier 2 phase, reports PASS/FAIL per pod | SATISFIED | phase61.sh: sources bat-scanner.sh, per-pod emit_result, fleet summary |
| BAT-04 | 210-02 | Deploy chain includes bat sync after binary swap, before restart, with post-deploy verification | SATISFIED | deploy-pod.sh L145-198: bat sync step, bat copy to BINARY_DIR L249-250, post-deploy bat_scan_pod verification |
| AUDIT-01 | 210-02 | All v25.0 verification features validated via audit.sh --mode full as post-milestone gate | SATISFIED | Phases 61-65 cover all 5 dimensions: bat-drift, config-fallback, boot-resilience, sentinel-alerts, verification-chains |
| AUDIT-02 | 210-02 | 5 new audit phases in audit.sh with PASS/FAIL/QUIET criteria, venue-state-aware | SATISFIED | Phases 61-63 (tier2), 64-65 (tier3) all registered in audit.sh, all follow emit_result pattern with venue-state QUIET |
| AUDIT-03 | 210-02 | Audit report "v25.0 Debug Quality" section with per-pod summary and "Debug Quality: N/8" line | SATISFIED | report.sh L363-417: per-pod table, 5 columns for phases 61-65, summary line |

No orphaned requirements found -- all 7 IDs from REQUIREMENTS-v25.md phase 210 mapping (BAT-01 through BAT-04, AUDIT-01 through AUDIT-03) are covered by the two plans.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | - | - | - | No anti-patterns found in any of the 9 modified files |

All files pass `bash -n` syntax validation. No TODO/FIXME/PLACEHOLDER comments. No empty implementations. No stub functions.

### Human Verification Required

### 1. Fleet Bat Drift Scan (Live)

**Test:** Run `bash scripts/bat-scanner.sh --all` when pods are online
**Expected:** Each pod shows MATCH or DRIFT with specific line differences for start-rcagent.bat
**Why human:** Pods are currently offline (venue closed). Cannot verify live rc-sentry /files endpoint responses.

### 2. Audit Full Run with New Phases

**Test:** Run `AUDIT_PIN=261121 bash audit/audit.sh --mode full` when venue is open
**Expected:** Phases 61-65 each produce PASS/FAIL/QUIET results and appear in the audit report under "v25.0 Debug Quality"
**Why human:** Requires live pod and server connectivity. Cannot verify audit report output without running against live infrastructure.

### 3. Deploy + Bat Sync Verification

**Test:** Deploy rc-agent to a single pod via `bash scripts/deploy-pod.sh pod-8` and verify bat sync
**Expected:** Deploy output shows "start-rcagent.bat synced" and "bat file verified (post-deploy match)"
**Why human:** Requires live deployment to a pod with HTTP server serving bat files.

### Gaps Summary

No gaps found. All 9 observable truths are verified through code inspection. All 7 requirements are satisfied. All artifacts exist, are substantive, and are properly wired. All scripts pass bash syntax validation. The bat-scanner.sh canonical bat validation returns 0 violations. The bat sync step is correctly positioned in deploy-pod.sh (after swap, before start). The audit dispatch is updated for all 5 new phases. The Debug Quality report section reads from phase 61-65 results.

The only items requiring human verification are live execution against pods (which are offline during venue-closed hours) -- these are expected operational constraints, not implementation gaps.

---

_Verified: 2026-03-26T07:15:00Z_
_Verifier: Claude (gsd-verifier)_
