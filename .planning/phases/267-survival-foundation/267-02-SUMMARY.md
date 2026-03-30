---
phase: 267-survival-foundation
plan: "02"
subsystem: racecontrol-server
tags: [survival, heal-lease, arbitration, SF-02, SF-03]
dependency_graph:
  requires: [267-01]
  provides: [heal-lease-api, lease-manager]
  affects: [racecontrol-server, survival-layers-1-2-3]
tech_stack:
  added: []
  patterns: [in-memory-mutex-store, axum-router-merge, service-key-auth, action-id-tracing]
key_files:
  created:
    - crates/racecontrol/src/api/survival.rs
  modified:
    - crates/racecontrol/src/api/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/state.rs
decisions:
  - "Heal lease endpoints placed in service_routes tier (X-Service-Key auth, not staff JWT) — healing layers are not human users"
  - "Used std::sync::Arc full path in state.rs to match existing codebase conventions (no use Arc import)"
  - "release_lease is unconditional (any caller can release) — holder check not required; releasing without holding is a no-op"
  - "action_id on renew logs the renew caller's action_id but the lease itself retains the original grant action_id for end-to-end tracing"
metrics:
  duration_secs: 1001
  completed_at: "2026-03-30T14:27:24Z"
  tasks_completed: 2
  files_changed: 4
  tests_added: 11
---

# Phase 267 Plan 02: Heal Lease Protocol Summary

**One-liner:** Server-arbitrated heal lease (in-memory mutex store) with grant/renew/release endpoints, X-Service-Key auth, and action_id tracing — prevents 5-healer fight per SF-02.

## What Was Built

### LeaseManager (crates/racecontrol/src/api/survival.rs)

In-memory, Mutex-protected heal lease coordinator. Maps `pod_id -> HealLease`. One active lease per pod at a time.

**Core operations:**
- `request_lease(req)` — grants if no active lease; auto-frees expired leases; denies if valid lease held by another layer. Returns `HealLeaseResponse { granted, lease, reason }`.
- `renew_lease(pod_id, layer, action_id, ttl_secs)` — extends TTL for the current holder only. Returns `Err(String)` if non-holder, no lease, or expired.
- `release_lease(pod_id)` — removes lease. Idempotent. No caller validation (any agent can release).
- `get_lease(pod_id)` — returns active lease if non-expired.

**Key design decisions:**
- Lock poisoning handled with `unwrap_or_else(|e| e.into_inner())` — service continues even after a panic in the lock guard.
- Lock is never held across `.await` — all lease operations are synchronous and return immediately.
- Expired leases are auto-freed on the next `request_lease` call (no background cleanup needed at this scale).

### Axum Endpoints

| Method | Path | Auth | Behavior |
|--------|------|------|---------|
| POST | `/api/v1/pods/:pod_id/heal-lease` | X-Service-Key | Request lease; returns 200 (granted) or 409 (denied) |
| DELETE | `/api/v1/pods/:pod_id/heal-lease` | X-Service-Key | Release lease; returns 200 always |
| POST | `/api/v1/pods/:pod_id/heal-lease/renew` | X-Service-Key | Renew lease; returns 200 (ok) or 409 (denied) |

Auth: X-Service-Key header validated against `config.pods.sentry_service_key`. Permissive when no key configured (same pattern as `fleet_alert`). Returns 401 on invalid key.

### AppState Integration

`state.rs` now has:
```rust
pub lease_manager: std::sync::Arc<LeaseManager>,
```
Initialized with `Arc::new(LeaseManager::new())` in `AppState::new()`. All three survival layers can call `state.lease_manager.request_lease(...)` directly.

### Route Registration

`survival::survival_routes()` is merged into `api_routes()` as an unnested set of routes (no tier-level middleware — auth is handled per-handler via X-Service-Key check).

## Tests (11 passing)

| Test | Covers |
|------|--------|
| test_request_lease_grants_when_no_active_lease | SF-02: basic grant |
| test_request_lease_denies_when_another_layer_holds_non_expired_lease | SF-02: contention denied |
| test_request_lease_grants_when_existing_lease_is_expired | SF-02: auto-free expired |
| test_renew_lease_extends_ttl_for_lease_holder | SF-02: renew TTL |
| test_renew_lease_rejects_different_layer_than_holder | SF-02: non-holder blocked |
| test_renew_lease_rejects_when_no_lease_exists | SF-02: no lease edge case |
| test_release_lease_removes_the_lease | SF-02: explicit release |
| test_release_lease_is_idempotent | SF-02: idempotent release |
| test_action_id_preserved_through_request_grant_cycle | SF-03: action_id tracing |
| test_action_id_preserved_after_renew | SF-03: action_id not overwritten on renew |
| test_after_release_new_grant_is_possible | SF-02: round-trip cycle |

## Verification Results

```
cargo check -p racecontrol-crate: Finished (0 errors, pre-existing warnings only)
cargo test -p racecontrol-crate -- survival: 11 passed, 0 failed
```

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None. All three endpoints are fully wired and functional. The LeaseManager is in-memory only (no persistence), which is intentional by design — leases are ephemeral coordination state, not durable records. Persistence would be added only if server restarts during active healing become a concern (deferred to Phase 272 integration review).

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 (TDD + impl) | f7f7598d | feat(267-02): implement LeaseManager and heal-lease endpoints |
| Task 2 (wiring) | ef232dc6 | feat(267-02): wire survival endpoints into routes and AppState |

## Self-Check: PASSED

- [x] crates/racecontrol/src/api/survival.rs — FOUND
- [x] .planning/phases/267-survival-foundation/267-02-SUMMARY.md — FOUND
- [x] Commit f7f7598d — FOUND
- [x] Commit ef232dc6 — FOUND
