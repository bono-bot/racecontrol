---
phase: 105-port-audit-scheduled-tasks-james-binary
verified: 2026-03-21T13:00:00+05:30
status: gaps_found
score: 9/10 must-haves verified
gaps:
  - truth: "DEPLOY-03 marked complete in REQUIREMENTS-v12.1.md"
    status: failed
    reason: "REQUIREMENTS-v12.1.md still shows DEPLOY-03 as '[ ] Pending' and 'Pending' in the traceability table on line 50 and line 99 — despite the rc-process-guard binary existing, building, and being substantively implemented. The binary artifact satisfies DEPLOY-03 technically but the requirements doc was not updated to reflect completion."
    artifacts:
      - path: ".planning/REQUIREMENTS-v12.1.md"
        issue: "Line 50: '- [ ] **DEPLOY-03**' should be '[x]'. Line 99 traceability row: 'Pending' should be 'Complete (105-03)'."
    missing:
      - "Update REQUIREMENTS-v12.1.md line 50: '- [ ] **DEPLOY-03**' -> '- [x] **DEPLOY-03**'"
      - "Update REQUIREMENTS-v12.1.md line 99 traceability row: 'Pending' -> 'Complete (105-03)'"
---

# Phase 105: Port Audit, Scheduled Tasks, James Binary — Verification Report

**Phase Goal:** Listening ports audited against approved list, non-whitelisted scheduled tasks flagged, and James runs standalone rc-process-guard reporting via HTTP.
**Verified:** 2026-03-21T13:00:00+05:30 (IST)
**Status:** GAPS FOUND — 1 documentation gap, all code artifacts verified
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Non-whitelisted listening port detected, logged with port + PID, emitted as ViolationType::Port | VERIFIED | `parse_netstat_listening` at line 500, `run_port_audit` at line 538, `ViolationType::Port` at line 615 in rc-agent/src/process_guard.rs |
| 2 | Port kill path: kill_process_verified + direct taskkill fallback | VERIFIED | Lines 568-630 rc-agent process_guard.rs — sysinfo start_time primary, taskkill /F /PID fallback on PID not found |
| 3 | Non-whitelisted scheduled task flagged via schtasks CSV parse; Microsoft tasks unconditionally skipped | VERIFIED | `parse_schtasks_csv` at line 635, `run_schtasks_audit` at line 685; `\\Microsoft\\` path skip at line 719 in rc-agent process_guard.rs |
| 4 | All three audits wired into audit_interval.tick() arm (run_autostart_audit + run_port_audit + run_schtasks_audit) | VERIFIED | Lines 74-77 rc-agent process_guard.rs: all three called sequentially in audit_interval.tick() arm |
| 5 | POST /api/v1/guard/report accepts ProcessViolation JSON, stores to pod_violations[machine_id] | VERIFIED | `post_guard_report_handler` at line 390 racecontrol/src/process_guard.rs; pod_violations.write().await at line 427 |
| 6 | Route registered in service_routes() with X-Guard-Token auth | VERIFIED | Line 406 routes.rs: `.route("/guard/report", post(process_guard::post_guard_report_handler))` in service_routes() |
| 7 | report_secret field in ProcessGuardConfig; None = dev-accept + warn | VERIFIED | Lines 402-418 config.rs: `pub report_secret: Option<String>` with `#[serde(default)]`, Default::default() = None |
| 8 | rc-process-guard.exe standalone binary: fetch whitelist, 60s scan loop, HTTP POST violations | VERIFIED | main.rs 486+ lines; `fetch_whitelist_with_retry`, `run_scan_cycle`, `post_violation` all present and wired in main loop |
| 9 | JAMES_CRITICAL_BINARIES = ["rc-agent.exe", "kiosk.exe"] with zero grace on James | VERIFIED | Line 22 main.rs: `const JAMES_CRITICAL_BINARIES: &[&str] = &["rc-agent.exe", "kiosk.exe"]`; zero grace confirmed in run_scan_cycle |
| 10 | DEPLOY-03 marked complete in REQUIREMENTS-v12.1.md | FAILED | Line 50: `- [ ] **DEPLOY-03**` (unchecked). Line 99 traceability table: `Pending`. Binary is implemented and builds — doc not updated. |

**Score:** 9/10 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/process_guard.rs` | run_port_audit() + run_schtasks_audit() + parse_netstat_listening() + parse_schtasks_csv() + 11 unit tests | VERIFIED | All four functions present; 11 tests in rc-agent test block (lines 871-970); all wired in audit_interval.tick() |
| `crates/racecontrol/src/process_guard.rs` | post_guard_report_handler — accepts Json<ProcessViolation>, stores to pod_violations | VERIFIED | Function present at line 390; X-Guard-Token auth; ViolationStore::push at line 430 |
| `crates/racecontrol/src/api/routes.rs` | POST /guard/report registered in service_routes() | VERIFIED | Line 406 in service_routes() block |
| `crates/racecontrol/src/config.rs` | report_secret: Option<String> in ProcessGuardConfig | VERIFIED | Lines 402-418; serde(default); Default impl sets None |
| `crates/rc-process-guard/Cargo.toml` | Standalone binary crate with rc-common, sysinfo 0.33, reqwest 0.12, tokio, walkdir, winapi | VERIFIED | All deps present; sysinfo = "0.33" confirmed; winapi 0.3 windows-only dep; no winreg (reg shell-out used) |
| `crates/rc-process-guard/src/main.rs` | main loop + fetch_whitelist + run_scan_cycle + post_violation + audit functions | VERIFIED | 486+ lines; all functions present and wired; 10 unit tests; sysinfo 0.33 API used correctly |
| `Cargo.toml` (workspace) | rc-process-guard in workspace members | VERIFIED | Line 11: `"crates/rc-process-guard"` |
| `.planning/REQUIREMENTS-v12.1.md` | DEPLOY-03 marked [x] complete | FAILED | Still shows `[ ]` Pending on line 50 and "Pending" in traceability table line 99 |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| spawn() audit_interval.tick() arm | run_port_audit() | called after run_autostart_audit | WIRED | Lines 74-77 rc-agent process_guard.rs |
| spawn() audit_interval.tick() arm | run_schtasks_audit() | called after run_port_audit | WIRED | Lines 74-77 rc-agent process_guard.rs |
| run_port_audit() | guard_violation_tx | tx.send(AgentMessage::ProcessViolation) with ViolationType::Port | WIRED | Line 615: `violation_type: ViolationType::Port` |
| POST /api/v1/guard/report | AppState::pod_violations | state.pod_violations.write().await.entry(...).push(violation) | WIRED | Lines 427-431 racecontrol/src/process_guard.rs |
| post_guard_report_handler | fleet_health | violation stored in pod_violations — fleet_health_handler reads pod_violations for violation_count_24h | WIRED | fleet_health.rs line 283: `state.pod_violations.read().await`; line 351: `vs.violation_count_24h(now)` |
| main.rs fetch_whitelist_with_retry() | GET /api/v1/guard/whitelist/james | reqwest::Client::get + format!("{}/api/v1/guard/whitelist/james") | WIRED | Line 128: `format!("{}/api/v1/guard/whitelist/james", base_url)` |
| main.rs post_violation() | POST /api/v1/guard/report | reqwest::Client::post + X-Guard-Token header + Json<ProcessViolation> | WIRED | Lines 342-346: url format + header set from config.report_secret |
| audit_interval.tick() arm in main.rs | run_port_audit_james + run_schtasks_audit_james | called in audit_interval.tick() arm | WIRED | Lines 97-101 main.rs |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| PORT-01 | 105-01 | Listening port audit against approved port list per machine | SATISFIED | parse_netstat_listening + run_port_audit in rc-agent process_guard.rs; 6 netstat unit tests |
| PORT-02 | 105-01 | Auto-kill process owning non-whitelisted listening port | SATISFIED | kill_process_verified + taskkill fallback path in run_port_audit; action_taken = "killed" |
| AUTO-03 | 105-01 | Scheduled Task audit — schtasks /query parse, flag non-whitelisted tasks | SATISFIED | parse_schtasks_csv + run_schtasks_audit in rc-agent process_guard.rs; Microsoft task skip confirmed |
| DEPLOY-03 | 105-02, 105-03 | Standalone rc-process-guard binary for James (.27), reports via Tailscale HTTP | NEEDS UPDATE | Binary implemented and builds (4.0MB per summary); HTTP POST only (no WS); REQUIREMENTS-v12.1.md not updated to [x] |

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `.planning/REQUIREMENTS-v12.1.md` | 50, 99 | DEPLOY-03 checkbox and traceability row not updated to reflect completion | Warning | Misleading requirements state; next phase or operator checking status sees incorrect "Pending" |

No code anti-patterns found. No TODOs, placeholder returns, or stub implementations detected in any of the three implementation files. All handlers have real logic; all scan functions have real shell-out + parse + emit chains.

---

## Human Verification Required

### 1. Binary Functional Test on James

**Test:** Copy `target/release/rc-process-guard.exe` to James .27 with a valid `rc-process-guard.toml`. Run the binary and observe startup output.
**Expected:** Binary connects to racecontrol, fetches whitelist for "james", starts 60s scan loop with 60s startup amnesty. Check `process-guard-james.log` for first scan cycle log entries.
**Why human:** Cannot verify network connectivity or runtime behavior from codebase inspection. No integration tests for the HTTP round-trip.

### 2. POST /guard/report Curl Round-Trip

**Test:** `curl -X POST http://192.168.31.23:8080/api/v1/guard/report -H "X-Guard-Token: rp-guard-2026" -H "Content-Type: application/json" -d '{"machine_id":"james","violation_type":"Process","name":"test.exe","exe_path":null,"action_taken":"reported","timestamp":"2026-03-21T07:30:00Z","consecutive_count":1}'`
**Expected:** HTTP 200. Then `GET /api/v1/fleet/health` shows an entry with `machine_id=james` with `violation_count_24h >= 1`.
**Why human:** Requires live server .23 with updated racecontrol binary and racecontrol.toml containing `report_secret = "rp-guard-2026"`.

### 3. rc-agent Port Violation Round-Trip on Pod

**Test:** On any pod, bind a non-whitelisted port (e.g., `nc -l -p 4444` or equivalent). Wait for the 5-minute audit interval. Check `C:\RacingPoint\process-guard.log` for `PORT_VIOLATION port=4444`.
**Expected:** Log entry with port and PID. Violation appears in `GET /api/v1/fleet/health` pod entry with `violation_count_24h >= 1`.
**Why human:** Requires live pod with updated rc-agent binary and real netstat output. 5-minute cadence impractical for automated CI.

---

## Gaps Summary

One gap found — a documentation-only issue.

The rc-process-guard binary (DEPLOY-03) was fully implemented in Plan 03: the crate exists at `crates/rc-process-guard/`, the binary builds to 4.0MB, all required functions are present and wired, 10 unit tests pass. However, REQUIREMENTS-v12.1.md was not updated when Plan 03 completed. Line 50 still shows `- [ ] **DEPLOY-03**` (unchecked) and line 99 of the traceability table still reads `Pending`. This needs a one-line fix each.

All three code truths for PORT-01, PORT-02, AUTO-03 are fully satisfied in rc-agent. All server-side DEPLOY-03 code (Plan 02: post_guard_report_handler, route registration) is verified wired and substantive. The standalone binary (Plan 03) is substantive and wired.

No blocker anti-patterns in code. The only gap is the requirements file checkbox/table not reflecting the completed implementation.

---

_Verified: 2026-03-21T13:00:00+05:30_
_Verifier: Claude (gsd-verifier)_
