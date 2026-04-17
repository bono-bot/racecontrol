---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: "01"
subsystem: security/network-middleware + api/mesh-intelligence
tags: [option-z, mesh-service-key, pod-ip-gating, network-middleware, axum-route-layer]
dependency-graph:
  requires:
    - crates/racecontrol/src/network_source.rs (classify_ip, RequestSource enum)
    - crates/racecontrol/src/config/infra.rs::sentry_service_key
    - crates/racecontrol/src/state.rs::AppState
  provides:
    - GET /api/v1/pods/mesh-service-key (pod-IP-gated JSON endpoint)
    - require_pod_source middleware (fail-closed inverse of require_non_pod_source)
    - POS LAN 192.168.31.130 reclassified as Pod
    - POS Tailscale 100.95.211.1 reclassified as Pod (narrow single-IP exception)
  affects:
    - Plan 02 (MeshKeyCache rc-agent) — consumes this endpoint
    - Plan 03 (rewire 3 RCAGENT_SERVICE_KEY env-readers to cache)
    - Plan 10 (live integration test matrix)
tech-stack:
  added: []
  patterns:
    - "axum .route_layer() for per-route middleware (NOT .layer which applies to subsequent routes)"
    - "Pure-helper + handler-wrapper split for testability without AppState construction"
    - "Fail-closed middleware (rejects missing extension) — stricter than fail-open require_non_pod_source"
key-files:
  created: []
  modified:
    - crates/racecontrol/src/network_source.rs (+128 lines — 1 middleware, 1 doc update, 9 tests, 2 classify_ip edits)
    - crates/racecontrol/src/api/mesh_intelligence.rs (+56 lines — 1 handler, 1 pure helper, 3 tests)
    - crates/racecontrol/src/api/routes.rs (+9 lines — 1 import extension, 1 route registration)
decisions:
  - "Narrow POS Tailscale exception (100.95.211.1 only) — NOT a 100.x.x.x widen. Bono VPS (100.70.177.44) and server (100.125.108.37) must stay Cloud per trust-model separation."
  - "Pure helper split (render_mesh_service_key_body) for unit testability. Heavyweight AppState factory not required."
  - ".route_layer() not .layer() — pod-IP gate scoped to this route only. All other public routes continue to accept non-pod sources."
  - "Fail-closed require_pod_source (rejects missing RequestSource extension). Asymmetric with require_non_pod_source which is fail-open — rationale: pod-only endpoints are higher-trust; unclassified = untrusted."
metrics:
  duration: ~35 minutes
  completed: 2026-04-18
---

# Phase 413 Plan 01: Service key provisioning server route Summary

Pod-IP-gated server endpoint `GET /api/v1/pods/mesh-service-key` returns `pods.sentry_service_key` as JSON; POS (LAN + Tailscale) reclassified as Pod to enable LAN-outage fallback fetch.

## What Was Built

**New server route:** `GET /api/v1/pods/mesh-service-key` → JSON `{"mesh_service_key": "<key>"}`

**Auth:** pod-IP gate via `require_pod_source` middleware layered on this route only (`.route_layer`, not `.layer`). Same trust boundary as existing `/guard/whitelist/{machine_id}` and `/config/kiosk-allowlist` — LAN + pod-IP range.

**Handler signature:** `pub(crate) async fn pods_mesh_service_key(State<Arc<AppState>>) -> Response`. No headers, no query params, no body — auth is middleware-layer concern, not handler concern. Internally reads `state.config.pods.sentry_service_key.as_deref().unwrap_or("")` (degrade-open to empty string when unconfigured).

**New middleware:** `pub async fn require_pod_source(req, next) -> Response`. Fail-closed inverse of `require_non_pod_source`: rejects with 403 when `RequestSource` extension is missing OR not `Pod`.

**Network classification changes (`classify_ip`):**
- `192.168.31.130` (POS LAN): was Customer → now Pod
- `100.95.211.1` (POS Tailscale): was Cloud → now Pod (narrow single-IP exception)
- All 8 pod IPs (28/33/38/86/87/88/89/91): unchanged Pod
- Staff (20/23/27): unchanged Staff
- Bono VPS `100.70.177.44`: unchanged Cloud (regression guard)
- Server Tailscale `100.125.108.37`: unchanged Cloud (regression guard)

## Why the Tailscale Exception Is Narrow (Single IP)

Widening to `100.x.x.x` would reclassify Bono VPS (`100.70.177.44`, currently `Cloud`) and the server's own Tailscale address (`100.125.108.37`, also `Cloud`) as Pod. That breaks:
- Staff auth routing (staff reach server via LAN `.23`; Tailscale is for external admin flows that must stay out of the Pod trust zone).
- Cloud sync auth (Bono VPS talks to server via Tailscale — Cloud class is correct for that role).

The single-IP match `if octets == [100, 95, 211, 1]` is a precision tool — only the authoritative POS Tailscale IP (per CLAUDE.md Network Map) becomes Pod. Two regression-guard unit tests (`bono_vps_tailscale_stays_cloud`, `server_tailscale_stays_cloud`) enforce this constraint at build time.

## Why Task 1 Chose POS LAN as Pod (Not Staff)

POS PC runs rc-agent. Its role in Option Z is to fetch the mesh service key the same way the 8 pods do. It is an agent-level trust node for the mesh-key-fetch endpoint specifically. The Pod classification reflects "may fetch pod-level resources," which POS does. Existing staff-gated routes (which use `require_non_pod_source`) continue to block POS from staff-scoped admin operations.

## Why Fail-Closed (require_pod_source) Is Asymmetric with Fail-Open (require_non_pod_source)

`require_non_pod_source` is fail-open: if `RequestSource` extension is missing (classify middleware didn't run), it allows through. Rationale: the staff routes are already behind staff-JWT middleware — the non-pod-source check is defense-in-depth, not primary auth.

`require_pod_source` must be fail-closed because it IS the primary auth for the mesh-key endpoint. Missing `RequestSource` = classify middleware didn't run = we have no proof this is a pod = reject. Unit test `pod_guard_rejects_missing_source` enforces this behavior.

## Tasks Completed

| # | Task | Commit | Tests | Status |
|---|------|--------|-------|--------|
| 1 | require_pod_source middleware + reclassify POS LAN .130 + POS Tailscale 100.95.211.1 | `bca5eced` | 9 new + 10 existing (21 total) | Done |
| 2 | pods_mesh_service_key handler | `45d85c14` | 3 new | Done |
| 3 | Register route in public_routes with route_layer | `74a1d911` | 1 (route_uniqueness) | Done |

## Verification Results

```
cargo test -p racecontrol-crate --lib network_source::  → 21 passed, 0 failed
cargo test -p racecontrol-crate --lib mesh_intelligence → 3 passed, 0 failed
cargo test -p racecontrol-crate --lib route_uniqueness  → 1 passed, 0 failed
cargo build --release --bin racecontrol                 → Finished (1 warning, pre-existing)
```

**Grep validations:**
- `grep -c "100, 95, 211, 1" network_source.rs` = 1 (single-IP exception in classify_ip)
- `grep -c "pub async fn require_pod_source" network_source.rs` = 1
- `grep -c "pub(crate) async fn pods_mesh_service_key" mesh_intelligence.rs` = 1
- `grep -c '"/pods/mesh-service-key"' routes.rs` = 1 (exactly one registration)
- `grep -n '.route_layer' routes.rs | grep require_pod_source` = 1 (scoped, not widened)

## Deviations from Plan

None — plan executed exactly as written.

One minor note: the acceptance criterion for Task 3 stated `grep -n 'require_pod_source' routes.rs` should return exactly 1 match. Actual count is 3 (one import-line reference, one doc comment, one route_layer call). All three are intentional and required; the spirit of the criterion — "route_layer usage is singular" — is met (exactly 1 `.route_layer(...)` invocation wiring `require_pod_source`). Existing import was extended rather than duplicated (clean idiom: `use crate::network_source::{require_non_pod_source, require_pod_source};`).

## Authentication Gates

None. This plan is code-only — no deployment, no auth required.

## Ready for Plan 02

Contract locked for consumer side:
- **Path:** `GET /api/v1/pods/mesh-service-key`
- **Response:** `200 OK` with `Content-Type: application/json` and body `{"mesh_service_key": "<key>"}` (empty string when server has no `pods.sentry_service_key`)
- **Error:** `403 Forbidden` with `"Pod source required"` body when source IP is not Pod-classified
- **Caller origin:** rc-agent's `MeshKeyCache` via `rc_common::boot_resilience::spawn_periodic_refetch` at boot + every 5 minutes

## Deployment (Manifest per CLAUDE.md DMP)

- rust_binary: `racecontrol` — release build verified locally; deploy to server .23 via `deploy-server.sh` and mirror to cloud VPS (DEPLOY PARITY rule). Not deployed by this plan.
- frontend_rebuild: none
- config_change: none
- db_migration: none
- infrastructure: none
- data_files: none
- bat_file: none
- cloud_parity: `binary` (Bono VPS needs same binary update for cloud racecontrol)
- targets: `server, cloud`

Plan 11 (or orchestrator) handles live deploy + verification. This plan is code-complete only.

## Self-Check: PASSED

- [x] `crates/racecontrol/src/network_source.rs` modified (verified via grep — require_pod_source + POS IP reclassification present)
- [x] `crates/racecontrol/src/api/mesh_intelligence.rs` modified (verified via grep — pods_mesh_service_key handler present)
- [x] `crates/racecontrol/src/api/routes.rs` modified (verified via grep — `/pods/mesh-service-key` route present)
- [x] Commit `bca5eced` in git log (feat(413-01): add require_pod_source middleware + reclassify POS IPs)
- [x] Commit `45d85c14` in git log (feat(413-01): add pods_mesh_service_key handler)
- [x] Commit `74a1d911` in git log (feat(413-01): register /api/v1/pods/mesh-service-key route)
- [x] Test suite green: network_source (21), mesh_intelligence::phase413_tests (3), route_uniqueness (1)
- [x] Release binary builds clean
