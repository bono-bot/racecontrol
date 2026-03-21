---
phase: 104-server-guard-module-alerts
verified: 2026-03-21T12:30:00+05:30
status: human_needed
score: 11/12 must-haves verified
re_verification: false
human_verification:
  - test: "Load kiosk fleet grid at http://localhost:3300/fleet (or kiosk port). Temporarily set violation_count_24h to a non-zero value in the API or mock data (e.g. inject violation for Pod 8). Confirm the Racing Red badge appears with correct count text ('3 violations'). Confirm badge is absent when count=0."
    expected: "Red badge (bg #E10600) appears below Uptime row on affected pod card. Shows correct singular/plural text. Absent for clean pods. Existing Maintenance button and status colors are unaffected."
    why_human: "Badge rendering and visual positioning cannot be verified programmatically — JSX renders to DOM at runtime."
  - test: "Confirm REQUIREMENTS-v12.1.md ALERT-02 checkbox updated to [x] (stale Pending marker)"
    expected: "Line 41 reads: [x] **ALERT-02**: Staff kiosk notification badge for active violations"
    why_human: "File edit required to reflect completed work — code is implemented but requirement file was not updated by Plan 03."
---

# Phase 104: Server Guard Module + Alerts Verification Report

**Phase Goal:** The racecontrol server receives all pod violations, displays an active-violation badge on the staff kiosk, escalates repeat offenders to email, and surfaces violation counts in the fleet health endpoint.
**Verified:** 2026-03-21T12:30:00+05:30 (IST)
**Status:** human_needed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | GET /api/v1/fleet/health includes `violation_count_24h` and `last_violation_at` per pod | VERIFIED | `fleet_health.rs` lines 349-352: `violations.get(pod_id)` read lock, `violation_count_24h(now)` and `last_violation_at()` populated in both `Some(info)` and `None` branches of `PodFleetStatus` |
| 2 | `ProcessViolation` WS messages from pods are stored in `pod_violations` in-memory store | VERIFIED | `ws/mod.rs` lines 762-768: write lock on `state.pod_violations`, `vmap.entry(pod_key).or_default()`, `store.push(violation.clone())` |
| 3 | Three kills of the same process within 5 minutes triggers email to Uday | VERIFIED | `ws/mod.rs` lines 765, 771-784: `repeat_offender_check()` returns true when >= 2 prior "killed" entries in 300s window; `state.email_alerter.write().await` then `alerter.send_alert()` called when `should_escalate` |
| 4 | racecontrol server runs a background process scan loop every 60 seconds | VERIFIED | `process_guard.rs` lines 149+: `spawn_server_guard()` spawns tokio task with `tokio::time::interval(Duration::from_secs(config.poll_interval_secs))`; wired in `main.rs` line 565 after `fleet_health::start_probe_loop` |
| 5 | `rc-agent.exe` detected on server produces a CRITICAL log entry with zero grace period | VERIFIED | `process_guard.rs` lines 108-113: `SERVER_CRITICAL_BINARIES = ["rc-agent.exe"]`, `is_server_critical()` case-insensitive; CRITICAL skips grace check (lines 224, 293: `if !is_critical { ... grace ... continue }`) |
| 6 | Server guard violations logged to `C:\RacingPoint\process-guard.log` with 512KB rotation | VERIFIED | `process_guard.rs` lines 116-139: `log_server_guard_event()` with `MAX_LOG_BYTES = 512 * 1024`, truncate-then-append rotation |
| 7 | Server guard pushes `ProcessViolation` to `state.pod_violations["server"]` | VERIFIED | `process_guard.rs` lines 318, 380-387: `state.pod_violations.write().await`, `vmap.entry("server".to_string()).or_default().push(violation)` |
| 8 | Fleet grid pod card shows a red badge when `violation_count_24h > 0` | VERIFIED (code) | `kiosk/src/app/fleet/page.tsx` lines 142-150: `{(pod.violation_count_24h ?? 0) > 0 && <div style={{ backgroundColor: '#E10600' }}>...}` with correct Racing Red color |
| 9 | Badge shows correct violation count with singular/plural text | VERIFIED (code) | `fleet/page.tsx` line 148: `{pod.violation_count_24h} {pod.violation_count_24h === 1 ? 'violation' : 'violations'}` |
| 10 | Badge absent when `violation_count_24h == 0` or null | VERIFIED (code) | `fleet/page.tsx` line 142: `(pod.violation_count_24h ?? 0) > 0` null-safe guard |
| 11 | `PodFleetStatus` TypeScript type includes `violation_count_24h` and `last_violation_at` | VERIFIED | `kiosk/src/lib/types.ts` lines 388-389: `violation_count_24h: number` and `last_violation_at: string | null` in `PodFleetStatus` interface |
| 12 | Visual badge renders correctly in browser at correct position | NEEDS HUMAN | Cannot verify DOM layout and visual appearance programmatically |

**Score:** 11/12 truths verified (1 deferred to human)

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/fleet_health.rs` | ViolationStore + FleetHealthStore fields + PodFleetStatus fields + fleet_health_handler reads pod_violations | VERIFIED | ViolationStore (lines 64-114) with push/violation_count_24h/last_violation_at/repeat_offender_check; FleetHealthStore gains `violation_count_24h: u32` (line 59) and `violation_count_last_at: Option<String>` (line 61); PodFleetStatus gains `violation_count_24h: u32` (line 140) and `last_violation_at: Option<String>` (line 142); handler reads violations read lock (line 283) |
| `crates/racecontrol/src/state.rs` | `AppState.pod_violations: RwLock<HashMap<String, ViolationStore>>` | VERIFIED | Field declaration at line 173; initialized `RwLock::new(HashMap::new())` at line 231 in `AppState::new()` |
| `crates/racecontrol/src/ws/mod.rs` | `AgentMessage::ProcessViolation` handler arm replacing wildcard | VERIFIED | Lines 742-784: explicit `ProcessViolation` arm with store+escalate; line 786: explicit `ProcessGuardStatus` arm; line 792: `_ => {}` catch-all. Wildcard fully replaced with typed arms |
| `crates/racecontrol/src/process_guard.rs` | `spawn_server_guard()` + `SERVER_CRITICAL_BINARIES` + `is_server_critical()` + `log_server_guard_event()` | VERIFIED | All four present at lines 108, 111, 118, 149 respectively; TDD test `test_is_server_critical_rc_agent` at line 570 |
| `crates/racecontrol/src/main.rs` | `spawn_server_guard(state.clone())` called after `fleet_health::start_probe_loop` | VERIFIED | Line 565: `process_guard::spawn_server_guard(state.clone())` immediately after `fleet_health::start_probe_loop` (line 562) |
| `kiosk/src/lib/types.ts` | `PodFleetStatus` interface extended with violation fields | VERIFIED | Lines 388-389: `violation_count_24h: number` and `last_violation_at: string | null` |
| `kiosk/src/app/fleet/page.tsx` | Violation badge conditional render using `#E10600` | VERIFIED | Lines 142-150: conditional badge with `style={{ backgroundColor: '#E10600' }}`, null-safe guard, singular/plural text |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `ws/mod.rs AgentMessage::ProcessViolation` handler | `state.pod_violations` | write lock insert into VecDeque | WIRED | `state.pod_violations.write().await` at line 763; `vmap.entry(pod_key).or_default(); store.push(violation.clone())` at lines 764-766 |
| `fleet_health_handler` | `state.pod_violations` | read lock violation_count_24h / last_violation_at | WIRED | `state.pod_violations.read().await` at line 283; `vstore.map(|vs| vs.violation_count_24h(now))` at line 351; `vstore.and_then(|vs| vs.last_violation_at()).map(String::from)` at line 352 |
| `repeat_offender_check` | `email_alerter.send_alert` | 3 kills same process within 300s window | WIRED | Lines 762-783: `store.repeat_offender_check(violation, now)` → `should_escalate` → `state.email_alerter.write().await` → `alerter.send_alert(&pod_key, &subject, &body).await` |
| `process_guard::spawn_server_guard` | `state.pod_violations` | write lock push ViolationStore for "server" | WIRED | `process_guard.rs` lines 382-387: `state.pod_violations.write().await`, `vmap.entry("server".to_string()).or_default().push(violation)` |
| `spawn_server_guard` in `main.rs` | log file at `C:\RacingPoint\process-guard.log` | `log_server_guard_event()` with 512KB rotation | WIRED | `log_server_guard_event` called at lines 249, 318; constant `GUARD_LOG = r"C:\RacingPoint\process-guard.log"` at line 120 |
| `fleet/page.tsx fetchFleet()` | `api.fleetHealth()` | existing API call | WIRED | `fleet/page.tsx` line 64: `const data = await api.fleetHealth()`; `kiosk/src/lib/api.ts` line 24: `fleetHealth: () => fetchApi<FleetHealthResponse>("/fleet/health")` |
| pod card JSX | `pod.violation_count_24h` | conditional render | WIRED | `fleet/page.tsx` line 142: `{(pod.violation_count_24h ?? 0) > 0 && <div ...>` |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| ALERT-02 | 104-03 | Staff kiosk notification badge for active violations | SATISFIED (code verified) | `kiosk/src/app/fleet/page.tsx` lines 142-150: Racing Red badge conditional render; `types.ts` lines 388-389: type extended. **Note:** REQUIREMENTS-v12.1.md checkbox `[ ]` at line 41 still shows Pending — documentation not updated after Plan 03 |
| ALERT-03 | 104-01 | Email escalation on repeat offenders (N kills in time window) | SATISFIED | `ws/mod.rs` lines 762-784: `repeat_offender_check()` → `send_alert()` for 3 kills in 300s; `ViolationStore.repeat_offender_check()` in `fleet_health.rs` lines 100-113 |
| ALERT-05 | 104-01 | Fleet-wide violation summary in GET /api/v1/fleet/health (violation_count_24h, last_violation_at) | SATISFIED | `fleet_health.rs` lines 283, 349-352: `pod_violations` read lock; both fields in `PodFleetStatus` struct (lines 140, 142) and populated for all 8 pods |
| DEPLOY-02 | 104-02 | Process guard module in racecontrol (server .23) | SATISFIED | `process_guard.rs`: `spawn_server_guard()` at line 149; `main.rs` line 565: wired. sysinfo = "0.33" in `Cargo.toml` line 63 |

**Orphaned requirements:** None — all phase-mapped requirements (ALERT-02, ALERT-03, ALERT-05, DEPLOY-02) appear in plan frontmatter.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `kiosk/src/app/fleet/page.tsx` | 203 | `placeholder="----"` | Info | HTML form input placeholder attribute — legitimate use in PIN entry field, not a code stub |

No blocker or warning-level anti-patterns found in modified files.

---

### Human Verification Required

#### 1. Kiosk Violation Badge Visual Check

**Test:** Start the kiosk (`cd kiosk && npm run dev`). Navigate to the fleet page. Temporarily modify `api.fleetHealth()` response or mock data so one pod returns `violation_count_24h: 3`. Observe the pod card.
**Expected:** A Racing Red (`#E10600`) badge reading "3 violations" appears below the Uptime row and above the Crash Recovered indicator. For pods with `violation_count_24h: 0`, no badge appears. The Maintenance button and status border colors are unaffected.
**Why human:** DOM layout and visual rendering cannot be verified by static analysis. The conditional logic and color are confirmed in source, but pixel-level positioning and visual hierarchy require a running browser.

#### 2. REQUIREMENTS-v12.1.md ALERT-02 checkbox update

**Test:** Open `.planning/REQUIREMENTS-v12.1.md` line 41.
**Expected:** `- [x] **ALERT-02**: Staff kiosk notification badge for active violations`
**Why human:** The code for ALERT-02 is fully implemented (verified above), but the requirements file still shows `[ ]` (Pending). This is a documentation discrepancy. Updating the checkbox requires a deliberate edit and commit.

---

### Gaps Summary

No implementation gaps found. All four requirements (ALERT-02, ALERT-03, ALERT-05, DEPLOY-02) have complete, substantive, wired implementations confirmed against the actual codebase. All commit hashes from summaries (d37f083, 42ebcb6, c8f8324, 9506d1d) verified in git history.

The only open items are:
1. Human visual verification of the kiosk badge (cannot automate DOM/visual checks)
2. Stale `[ ]` checkbox for ALERT-02 in REQUIREMENTS-v12.1.md — the traceability table at line 93 already reflects Phase 104 ownership but the checkbox at line 41 was not updated after Plan 03 completed

---

_Verified: 2026-03-21T12:30:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
