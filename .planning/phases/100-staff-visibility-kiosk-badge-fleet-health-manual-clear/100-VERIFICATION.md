---
phase: 100-staff-visibility-kiosk-badge-fleet-health-manual-clear
verified: 2026-03-21T07:15:00+05:30
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 100: Staff Visibility — Kiosk Badge + Fleet Health + Manual Clear

**Phase Goal:** Staff can see at a glance which pods are in maintenance (Racing Red badge on kiosk dashboard), view failure reasons (PIN-gated), and manually clear a pod from the dashboard; maintenance pods appear as unavailable in fleet health; alert cooldown prevents notification floods
**Verified:** 2026-03-21T07:15:00 IST
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Fleet health API returns `in_maintenance=true` for pods that sent PreFlightFailed | VERIFIED | `ws/mod.rs:726` sets `store.in_maintenance = true; store.maintenance_failures = failures.clone()` in PreFlightFailed arm |
| 2 | Fleet health API returns `in_maintenance=false` after PreFlightPassed or ClearMaintenance | VERIFIED | `ws/mod.rs:715` clears in PreFlightPassed; `routes.rs:984` clears in clear_maintenance_pod; `fleet_health.rs:124` clears on disconnect |
| 3 | POST /pods/{id}/clear-maintenance sends ClearMaintenance WS message to the pod | VERIFIED | `routes.rs:972` calls `sender.send(CoreToAgentMessage::ClearMaintenance).await`; route registered at `routes.rs:196` |
| 4 | Fleet health API includes `maintenance_failures` list for maintenance pods | VERIFIED | `fleet_health.rs:282` reads from store; serialized in `PodFleetStatus` struct (line 80); both None and Some branches populated |
| 5 | Fleet page shows Racing Red Maintenance badge on pods where `in_maintenance` is true | VERIFIED | `fleet/page.tsx:146-158` conditionally renders `<button style={{ backgroundColor: "#E10600" }}>Maintenance</button>` |
| 6 | Clicking Maintenance badge prompts PIN before showing failure details | VERIFIED | `fleet/page.tsx:178-204` — `!pinVerified` branch renders 4-digit password input + Verify button; `setPinVerified(true)` on 4-digit entry |
| 7 | After PIN, Clear Maintenance button calls POST /pods/{id}/clear-maintenance | VERIFIED | `fleet/page.tsx:220` calls `api.clearMaintenance(pod.pod_id!)` which maps to `fetchApi<...>(\`/pods/${podId}/clear-maintenance\`, { method: "POST" })` in `api.ts:363-364` |
| 8 | Badge disappears on next poll cycle after maintenance is cleared | VERIFIED | 5s `setInterval(fetchFleet, 5000)` at `page.tsx:74`; `in_maintenance` cleared server-side optimistically; next poll reflects cleared state |

**Score:** 8/8 truths verified

---

## Required Artifacts

### Plan 01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/fleet_health.rs` | `in_maintenance` and `maintenance_failures` on FleetHealthStore and PodFleetStatus | VERIFIED | Lines 53-55 (FleetHealthStore fields), lines 78-80 (PodFleetStatus fields), both populated in handler — `grep -c in_maintenance` returns 10 (>= 8 threshold) |
| `crates/racecontrol/src/ws/mod.rs` | PreFlightFailed sets `in_maintenance=true`; PreFlightPassed clears it | VERIFIED | Lines 712-728 (PreFlightFailed arm sets true + failures); lines 711-717 (PreFlightPassed arm clears) |
| `crates/racecontrol/src/api/routes.rs` | POST clear-maintenance endpoint sending ClearMaintenance via WS | VERIFIED | `clear_maintenance_pod` function at line 964; route registered at line 196; `ClearMaintenance` sent at line 972; server-side optimistic clear at lines 984-985 |

### Plan 02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `kiosk/src/lib/types.ts` | PodFleetStatus includes `in_maintenance` and `maintenance_failures` | VERIFIED | Lines 378-379: `in_maintenance: boolean` and `maintenance_failures: string[]` in `PodFleetStatus` interface |
| `kiosk/src/app/fleet/page.tsx` | Maintenance badge, PIN modal, clear button | VERIFIED | Badge at lines 146-158; PIN gate at lines 178-204; clear button at lines 216-233; 12 occurrences of "Maintenance" (>= 5 threshold) |
| `kiosk/src/lib/api.ts` | `clearMaintenance` API method | VERIFIED | Lines 363-364: `clearMaintenance: (podId: string) => fetchApi<...>(\`/pods/${podId}/clear-maintenance\`, { method: "POST" })` |

---

## Key Link Verification

### Plan 01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `ws/mod.rs` | `fleet_health.rs` | PreFlightFailed arm sets `store.in_maintenance = true` | WIRED | `ws/mod.rs:726`: `store.in_maintenance = true` confirmed in PreFlightFailed match arm |
| `api/routes.rs` | `agent_senders` | `clear_maintenance_pod` sends ClearMaintenance WS message | WIRED | `routes.rs:972`: `sender.send(CoreToAgentMessage::ClearMaintenance).await` |

### Plan 02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `fleet/page.tsx` | `kiosk/src/lib/api.ts` | `clearMaintenance()` call on Clear Maintenance button | WIRED | `page.tsx:220`: `await api.clearMaintenance(pod.pod_id!)` inside async button handler |
| `kiosk/src/lib/api.ts` | `/api/v1/pods/{id}/clear-maintenance` | POST fetch | WIRED | `api.ts:364`: `fetchApi<...>(\`/pods/${podId}/clear-maintenance\`, { method: "POST" })` — `fetchApi` prepends `/api/v1` at line 10 |

---

## Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|--------------|-------------|--------|----------|
| STAFF-01 | 100-02 | Kiosk dashboard shows pre-flight status badge per pod (pass/fail/maintenance) | SATISFIED | Racing Red badge rendered conditionally in `page.tsx`; statusBorder/statusLabel/statusLabelColor helpers accept `maintenance` param and return `#E10600` / "Maintenance" / Racing Red text |
| STAFF-02 | 100-01, 100-02 | Staff can manually clear MaintenanceRequired state from kiosk dashboard | SATISFIED | PIN-gated modal shows "Clear Maintenance" button calling `api.clearMaintenance(pod.pod_id!)` which POSTs to `/pods/{id}/clear-maintenance`; server sends ClearMaintenance WS msg and clears store |
| STAFF-03 | 100-01 | Pod marked unavailable in fleet health while in MaintenanceRequired state | SATISFIED | `in_maintenance: true` in fleet health JSON per pod; kiosk fleet page overrides status label/border to Racing Red when `in_maintenance`; maintenance pods visually distinct at full opacity |
| STAFF-04 | NOT claimed by Phase 100 plans | Pre-flight failure alerts rate-limited (no flood on repeated failures) | SATISFIED IN PHASE 99 | REQUIREMENTS.md maps STAFF-04 to Phase 99 (Complete); Phase 99 Plan 02 implements 60s cooldown in `ws_handler.rs`; Phase 100 plans do not claim this requirement |

**Note on STAFF-04:** The ROADMAP `Requirements` field for Phase 100 lists STAFF-04, but neither Phase 100 plan's `requirements:` frontmatter claims it. REQUIREMENTS.md and Phase 99 Plan 02 confirm STAFF-04 was delivered in Phase 99. No gap — already complete.

---

## Build and Test Verification

| Check | Result |
|-------|--------|
| `cargo build --bin racecontrol` | Finished with 0 errors, 1 harmless unused import warning |
| `cargo test -p racecontrol-crate fleet_health` | 16 passed, 0 failed — includes 2 new Phase 100 tests: `fleet_health_store_default_not_in_maintenance` and `fleet_health_clear_on_disconnect_clears_maintenance` |
| `npx tsc --noEmit` (kiosk) | 0 errors, 0 output |

---

## Anti-Patterns Scan

No anti-patterns found in modified files:

- `fleet_health.rs` — no TODOs, no placeholders, no `return {}` stubs
- `ws/mod.rs` — both PreFlightFailed and PreFlightPassed arms fully implemented with real state mutations
- `routes.rs` — `clear_maintenance_pod` function sends actual WS message AND clears server state; not a stub
- `kiosk/src/lib/types.ts` — type fields are real, not optional stubs
- `kiosk/src/lib/api.ts` — `clearMaintenance` does a real POST fetch
- `kiosk/src/app/fleet/page.tsx` — badge renders conditionally on real data; PIN modal has real state machine; clear button calls real API; no `console.log`-only handlers

One `console.error("Failed to clear maintenance:", err)` at `page.tsx:223` is in an error handler — expected and correct, not a stub.

---

## Human Verification Required

### 1. Badge Appearance on Venue TV

**Test:** With a pod in MaintenanceRequired state, open the kiosk fleet page on the venue TV (port 3300).
**Expected:** Pod card shows a Racing Red (#E10600) "Maintenance" badge button; status label reads "Maintenance" in Racing Red text; card border is Racing Red; card renders at full opacity (not dimmed).
**Why human:** Visual rendering and color accuracy cannot be verified by code inspection alone.

### 2. PIN Gate UX

**Test:** Click a Maintenance badge; verify the PIN input is the only element visible before entering 4 digits; enter any 4 digits and press Verify.
**Expected:** PIN input appears focused; entering fewer than 4 digits keeps Verify button disabled (opacity-40); after 4 digits Verify enables; pressing Verify shows the failure details list.
**Why human:** Client-side PIN state machine with UI interactions cannot be end-to-end tested by grep.

### 3. Clear Maintenance Round-Trip

**Test:** With Pod X in MaintenanceRequired, open the kiosk fleet page, click the Maintenance badge, enter PIN, click "Clear Maintenance".
**Expected:** "Clearing..." button label appears; modal closes on success; within 5 seconds the Maintenance badge disappears from Pod X's card and the border/label return to Healthy/WS Only/etc.
**Why human:** Requires a live connected pod agent and network to verify the ClearMaintenance WS round-trip and UI poll update.

---

## Gaps Summary

No gaps. All 8 must-haves are verified, all artifacts are substantive and wired, all key links confirmed, all three claimed requirements (STAFF-01, STAFF-02, STAFF-03) are satisfied. STAFF-04 was delivered in Phase 99 and is complete independently of this phase.

---

_Verified: 2026-03-21T07:15:00 IST_
_Verifier: Claude (gsd-verifier)_
