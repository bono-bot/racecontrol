# RCA — racecontrol-main surface: Heart-V2 session router wiring

**Surface:** `racecontrol-main` (pod-state-channel foundational boundary) — `crates/racecontrol/src/main.rs` `build_router`.
**Change:** merge `api::heart_v2::heart_routes()` at the router ROOT (bare `/heart/...`, not under `/api/v1`) so the admin proxy's `RACECONTROL_HEART_URL + /heart/...` reaches the new in-process Heart-V2 surface. Additive — no existing route, nest, or handler is modified.
**Author:** bono · **Date:** 2026-05-30 · **Branch:** `feat/heart-v2-session-surface`.

> **Canonical comprehensive RCA (full 5 sections + inherited-issue catalogue):**
> `racecontrol/.planning/specs/v2/RCA-heart-v2-session-surface-20260530.md`.
> **Mechanism-trust-check (PASS 5/5):** `racecontrol/.planning/specs/v2/MECHANISM-TRUST/heart-v2-20260530.json`.
> This file is the surface-scoped RCA for `racecontrol-main` (the `pre-v2-edit-rca-check.js` gate path); the comprehensive RCA is the source of truth.

## §1 Boundary map (main.rs touch points)
- `main.rs:454` `.nest("/api/v1", api_routes(...))` — UNCHANGED; heart merges at root immediately after.
- `main.rs` `build_router` — add `.merge(api::heart_v2::heart_routes())` (one line). The router is `Router<Arc<AppState>>` at this stage (`.with_state` applied at the end), so the merge typechecks.
- `state.rs:42/251` AppState gains `heart: RwLock<HeartStore>` + `heart_stream_tx: broadcast::Sender<PodState>` (additive fields, seeded in `AppState::new`).
- No V1 route, table, or handler altered. Heart state is in-memory + V2-isolated (never touches V1 `billing_sessions`).

## §2 Inherited-issue catalogue (main.rs-relevant subset; full list in comprehensive RCA §2)
- #10 lock-across-await (HIGH): heart handlers snapshot under guard → drop → `let _ = tx.send()`. No await held across the `heart` RwLock guard.
- #13 shared tokio runtime (MED): heart handlers are non-blocking; broadcast send is sync. One 50ms `tokio::spawn` for the switch-game launcher mimic.
- #4 serde silent-drop (HIGH): heart request bodies use `#[serde(deny_unknown_fields)]`.
- #6 audit-blind-spot (verification): acceptance is behavioral (session-not-404 + SSE delta), not health-200.

## §3 Past-bug disposition
- #10 UNRESOLVED in V1 (billing.rs) → MUST-NOT-INHERIT; enforced by snapshot+drop pattern + concurrency test.
- #4 PATCHED-ONLY → APPLIED (deny_unknown_fields + a reject-unknown-field test).
- #13 INHERITED-UNVERIFIED → mitigated (non-blocking handlers); load-test deferred.

## §4 V2-alignment delta
Router stays additive; the heart becomes the V2 pod-state-channel authority (session lifecycle + PodStateSnapshot projection) while the proxy remains the wallet authority. Mounting bare `/heart/...` at root = mock-heart drop-in parity (the proven contract). Aligns with Pod=state-channel premise.

## §5 V2-framed proposal
Merge `heart_routes()` at the root of `build_router`. Verify behaviorally: `cargo test -p racecontrol-crate` (state machine + idempotency + deny-unknown-field + SSE delta) + local `cargo run` curl (launch → not-404 + SSE delta). MAOR review before push; per-PR Captain auth pre-committed at plan-approval 2026-05-30.
