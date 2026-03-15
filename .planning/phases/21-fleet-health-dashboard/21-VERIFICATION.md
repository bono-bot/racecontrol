---
phase: 21-fleet-health-dashboard
verified: 2026-03-15T14:30:00Z
status: human_needed
score: 6/6 must-haves verified
re_verification: false
human_verification:
  - test: "Open http://192.168.31.23:3300/kiosk/fleet on Uday's phone"
    expected: "8 pod cards in 2-column grid, each showing WS dot, HTTP dot, version, uptime. Cards auto-refresh (timestamp changes every 5s). Offline cards are dimmed with red border. Connected pods have green border."
    why_human: "Visual layout, responsive grid, color rendering, and live polling cannot be verified programmatically"
  - test: "Connect a pod agent (Pod 8) and confirm its card flips to Healthy"
    expected: "Card border turns green, WS dot goes green, version string appears, uptime starts counting up"
    why_human: "Real-time WS event wiring requires a live agent connection to confirm the StartupReport path"
  - test: "Disconnect a pod agent and confirm its card reverts to Offline"
    expected: "Card border turns red/dimmed, both dots go red, version clears to v--"
    why_human: "The clear_on_disconnect path requires a live WS disconnection event"
---

# Phase 21: Fleet Health Dashboard Verification Report

**Phase Goal:** Provide a real-time fleet health endpoint and mobile-first dashboard so Uday can check all 8 pod statuses from his phone without running any commands.
**Verified:** 2026-03-15T14:30:00Z
**Status:** human_needed (all automated checks pass; visual approval pending)
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | GET /api/v1/fleet/health returns JSON with exactly 8 pod entries sorted by pod_number 1-8 | VERIFIED | `fleet_health_handler` iterates `for pod_number in 1u32..=8`, builds 8 entries always; handler test confirms shape |
| 2 | Each pod entry has `ws_connected` and `http_reachable` as two separate boolean fields | VERIFIED | `PodFleetStatus` struct has both fields as independent `bool`; populated from distinct sources (agent_senders vs FleetHealthStore.http_reachable) |
| 3 | Each pod entry has `version` and `uptime_secs` populated from stored StartupReport data | VERIFIED | `store_startup_report()` sets `version` + computes `agent_started_at`; handler computes live `uptime_secs = (now - started).num_seconds()` |
| 4 | Pods that have never WS-connected appear with ws_connected=false, http_reachable=false, version=null, uptime_secs=null | VERIFIED | `None` branch in handler pushes all-false/null entry; confirmed by `fleet_health_ws_connected_false_when_no_sender` test |
| 5 | The endpoint requires no authentication | VERIFIED | Route registered directly in `api_routes()` as `.route("/fleet/health", get(fleet_health::fleet_health_handler))` with no middleware wrapper; same pattern as public `/health` route |
| 6 | A background probe loop pings each registered pod's :8090/health every 15s with 3s timeout to update http_reachable | VERIFIED | `start_probe_loop()` uses `tokio::time::interval(Duration::from_secs(15))`, dedicated `reqwest::Client` with `.timeout(3s).connect_timeout(3s)`, probes `http://{ip}:8090/health`; spawned in `main.rs` after `udp_heartbeat::spawn` |

**Score:** 6/6 truths verified

---

## Required Artifacts

### Plan 21-01 (Backend)

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-core/src/fleet_health.rs` | FleetHealthStore, PodFleetStatus, store_startup_report, clear_on_disconnect, start_probe_loop, fleet_health_handler, unit tests | VERIFIED | 430 lines; all 6 public items present; 13 unit tests; no stubs or TODOs |
| `crates/rc-core/src/state.rs` | `pod_fleet_health` field on AppState | VERIFIED | Line 136: `pub pod_fleet_health: RwLock<HashMap<String, FleetHealthStore>>,`; initialized at line 182 |
| `crates/rc-core/src/ws/mod.rs` | StartupReport storage + disconnect cleanup | VERIFIED | Lines 503-505 (StartupReport), 543-545 (graceful Disconnect), 577-579 (ungraceful socket-drop) |
| `crates/rc-core/src/api/routes.rs` | GET /fleet/health route | VERIFIED | Line 33: `.route("/fleet/health", get(fleet_health::fleet_health_handler))` |

### Plan 21-02 (Frontend)

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `kiosk/src/app/fleet/page.tsx` | Fleet Health Dashboard page component | VERIFIED | 149 lines; "use client", 5s polling, 8-pod card grid, all card states, formatUptime, statusBorder, statusLabel, crash recovery badge |
| `kiosk/src/lib/types.ts` | PodFleetStatus TypeScript interface | VERIFIED | Lines 324-340: `PodFleetStatus` + `FleetHealthResponse` interfaces; all 10 fields match Rust response shape |
| `kiosk/src/lib/api.ts` | fleetHealth() API helper | VERIFIED | Line 1: `FleetHealthResponse` imported; line 24: `fleetHealth: () => fetchApi<FleetHealthResponse>("/fleet/health")` |

---

## Key Link Verification

### Plan 21-01

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/rc-core/src/ws/mod.rs` | `state.pod_fleet_health` | write on StartupReport + clear on Disconnect/socket-drop | WIRED | 3 separate write blocks confirmed at lines 503, 543, 577 |
| `crates/rc-core/src/fleet_health.rs` | `state.pods + reqwest::Client` | parallel HTTP probes to :8090/health | WIRED | `join_all(probe_futs)` with `format!("http://{}:8090/health", ip)` |
| `crates/rc-core/src/api/routes.rs` | `fleet_health::fleet_health_handler` | route registration | WIRED | `.route("/fleet/health", get(fleet_health::fleet_health_handler))` |
| `crates/rc-core/src/main.rs` | `fleet_health::start_probe_loop` | tokio::spawn background task | WIRED | Line 314: `fleet_health::start_probe_loop(state.clone())` after udp_heartbeat |

### Plan 21-02

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `kiosk/src/app/fleet/page.tsx` | `/api/v1/fleet/health` | fetchApi polling in useEffect every 5s | WIRED | `api.fleetHealth()` called on mount + `setInterval(fetchFleet, 5000)` with `clearInterval` cleanup |
| `kiosk/src/app/fleet/page.tsx` | `kiosk/src/lib/types.ts` | import PodFleetStatus type | WIRED | `import type { PodFleetStatus } from "@/lib/types"` on line 5 |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| FLEET-01 | 21-01, 21-02 | Real-time fleet health endpoint returning WS connected, HTTP reachable, version, uptime for all 8 pods | SATISFIED | `/fleet/health` handler returns all 4 fields per pod for all 8 pod slots; TypeScript page renders all fields |
| FLEET-02 | 21-01 | Background probe loop checking pod health every 15 seconds with 3s timeout | SATISFIED | `start_probe_loop()` uses 15s interval and dedicated 3s-timeout reqwest client; spawned in main.rs |
| FLEET-03 | 21-02 | Mobile-first dashboard page at /kiosk/fleet with color-coded pod cards and 5s auto-refresh | SATISFIED (automated) / PENDING HUMAN | Page exists at `kiosk/src/app/fleet/page.tsx`; `grid-cols-2 sm:grid-cols-4`; 5s setInterval; 4 border color states; visual approval pending from Uday |

---

## Anti-Patterns Found

No anti-patterns found in new or modified files.

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | No TODOs, no stubs, no empty handlers, no `any` types, no `return null` in new code | — | — |

**Warnings in pre-existing code (not introduced by Phase 21):**
- `crates/rc-core/src/api/routes.rs`: 4 pre-existing unused variable/field warnings (sid, new_balance, CouponDiscount fields, JournalQuery struct)
- `crates/rc-core/src/main.rs`: 11 unused import warnings (pre-existing — these imports are side-effect registrations, not phase 21 additions)
- `crates/rc-core/src/auth/mod.rs`: 1 unused function warning (pre-existing)

None of these were introduced by Phase 21 and none are blockers.

---

## Test Results

### Rust (rc-core)

```
cargo test -p rc-core fleet_health
running 13 tests ... test result: ok. 13 passed; 0 failed
```

All 13 fleet_health unit tests pass:
- `fleet_health_store_default_is_all_false_and_none`
- `fleet_health_store_startup_report_sets_version`
- `fleet_health_store_startup_report_computes_agent_started_at`
- `fleet_health_store_startup_report_sets_crash_recovery`
- `fleet_health_store_startup_report_does_not_clear_http_reachable`
- `fleet_health_clear_on_disconnect_clears_version_and_started_at`
- `fleet_health_clear_on_disconnect_preserves_http_reachable`
- `fleet_health_uptime_computed_live_increases_over_time`
- `fleet_health_version_from_store_is_propagated`
- `fleet_health_http_reachable_from_store_is_propagated`
- `fleet_health_ws_connected_false_when_no_sender`
- `fleet_health_ws_connected_true_when_sender_exists_and_open`
- `fleet_health_ws_connected_false_when_receiver_dropped`

```
cargo test -p rc-core
running 238 unit + 41 integration tests ... test result: ok. 279 passed; 0 failed
```

No regressions in rc-core.

### Pre-existing failure (not Phase 21)

`cargo test -p rc-agent`: `remote_ops::tests::test_exec_timeout_returns_500` FAILS — expects exit_code 124, gets 1. This test was failing before Phase 21 (the failure is in `rc-agent/src/remote_ops.rs` which was not modified in this phase). All Phase 21 commits touch only `rc-core` and `kiosk`.

### TypeScript

```
cd kiosk && npx tsc --noEmit
(no output = no errors)
```

TypeScript passes cleanly. No `any` types introduced.

---

## Human Verification Required

### 1. Mobile Grid Layout

**Test:** Open `http://192.168.31.23:3300/kiosk/fleet` on Uday's phone (portrait orientation).
**Expected:** 8 pod cards displayed in a 2-column grid, each card showing pod number, version string, status label (Healthy/WS Only/HTTP Only/Offline), two dot+label rows (WS and HTTP), and uptime in Xh Ym format. Page background is `#1A1A1A` (Asphalt Black).
**Why human:** Responsive grid rendering, font legibility on mobile, and color contrast cannot be verified programmatically.

### 2. Color-Coded Status Borders

**Test:** With at least one pod WS-connected (green border expected) and at least one offline (red/dimmed expected), compare cards visually.
**Expected:** WS+HTTP = green left border; WS only = yellow left border; HTTP only = orange left border; neither = red/dimmed with opacity-50.
**Why human:** Tailwind JIT class generation and actual color rendering require a browser.

### 3. 5-Second Auto-Refresh

**Test:** Leave the page open for 30 seconds without touching it. Watch the "Last updated:" timestamp in the header.
**Expected:** Timestamp updates approximately every 5 seconds.
**Why human:** Timer behavior in a real browser environment cannot be verified by static analysis.

### 4. Live WS State Flip

**Test:** Start rc-core + kiosk. Open /kiosk/fleet. Then connect Pod 8's rc-agent (or restart it). Watch Pod 8's card.
**Expected:** Within the next 5-second poll cycle, Pod 8's card border turns green, the WS dot turns green, version populates (e.g. "v0.5.2"), uptime starts counting.
**Why human:** Requires a live WS connection event flowing through the StartupReport handler path.

---

## Summary

Phase 21 is fully implemented and all automated checks pass:

- The backend (`fleet_health.rs`, 430 lines) is substantive and complete: FleetHealthStore state, 15s probe loop with dedicated 3s-timeout client, StartupReport wiring, disconnect cleanup in both graceful and ungraceful paths, and a public GET handler returning 8 sorted entries.
- The frontend (`kiosk/src/app/fleet/page.tsx`, 149 lines) is substantive and complete: 5s polling with immediate first fetch, error banner without data loss, 4-state color coding, mobile-first 2-col/4-col grid, crash recovery badge.
- All wiring is confirmed: route registered, probe loop spawned, WS events wired, TypeScript types matched.
- 279/279 rc-core tests pass (238 unit + 41 integration). 13/13 fleet_health tests pass. TypeScript type checks clean.
- The only remaining gate is Uday's visual approval on his phone at `http://192.168.31.23:3300/kiosk/fleet`.

---

_Verified: 2026-03-15T14:30:00Z_
_Verifier: Claude (gsd-verifier)_
