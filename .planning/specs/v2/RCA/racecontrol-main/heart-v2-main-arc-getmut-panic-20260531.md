# §S-146 RCA — heart-V2 `main()` startup panic: `Arc::get_mut` after premature `state.clone()`

**Date:** 2026-05-31 IST · **Author:** bono · **Boundary class:** FOUNDATIONAL (heart `main()` startup / pod-state-channel)
**Trigger:** modifying deployed-V2 `main()` startup that shares the V1-era `Arc<AppState>` init sequence.
**Empirical anchor:** cloud `racecontrol` (Bono VPS, `RC_IS_CLOUD=1`) crash-looped from 2026-05-31 00:20Z (~11h), exit code 101, `thread 'main' panicked at crates/racecontrol/src/main.rs:454:30: no other Arc refs yet`. Build = `b7067829` (heart-V2). This binary **never successfully booted anywhere** — module tests (33/33) + compile passed, but `main()` startup was never run until the cloud exercised it. (Closes the "NOT TESTED: binary boots" caveat from the cutover-readiness package.)
**Composes-with:** mechanism-trust-check on the `.23` deploy surface (separate, Q4=NO recorded) · CLAUDE.md "No `.unwrap()`/`.expect()` in production Rust" standing rule (this panic is an instance of that anti-pattern).

## 1. Boundary map (paths + lines)
- `crates/racecontrol/src/main.rs:154` — `let mut state = Arc::new(AppState::new(...))` (strong-count = 1).
- `crates/racecontrol/src/main.rs:159-165` — heart-V2 `load_sessions(&state.v2db)` + `state.heart.write()` (borrows only; no clone).
- `crates/racecontrol/src/main.rs:173-182` — **DELTA-A R2 green-light reconciler** (added by heart-V2 launch-bridge commit `c0a74c9f`): `let recon_state = state.clone(); tokio::spawn(async move { loop { … reconcile_heart_green_light_once(&recon_state) … } })`. This clone is **moved into a forever-looping task** → strong-count = 2 permanently, *from line 174 onward*.
- `crates/racecontrol/src/main.rs:185` — `init_telemetry(&mut state).await` → internally `Arc::get_mut(state).expect("no other Arc refs yet")` at **:454** (read) and **:461** (mutate). `Arc::get_mut` returns `Some` **only** when strong-count == 1.
- `crates/racecontrol/src/main.rs:189-252` — nine further `Arc::get_mut(&mut state).expect(...)` init calls (maintenance/business/HR/feedback/pricing tables, aggregator, rating worker, audit-hash load).
- `crates/racecontrol/src/main.rs:276/303/336` — the **correctly-placed** `state.clone()` spawns (alert checker, `spawn_all`, `build_router`) — all *after* the get_mut init phase.

## 2. Inherited-issue catalogue
- **`.expect()` in production `main()`** — pre-existing pattern used 11× (189,196,203,210,216,223,230,242,252,454,461). Directly named in CLAUDE.md Code-Quality standing rule ("No `.unwrap()` in production Rust — use `?`, `.ok()`, or match"). It "worked" only by an **implicit, undocumented invariant**: *no `state` clone may occur before the get_mut init phase completes (≤ line 252).*
- **Startup-ordering fragility class** — sibling of the V1 boot-resilience failures (silent init failure that health checks miss). Same family as "compiles ≠ runs / build_id ≠ works."

## 3. Past-bug disposition
- The `.expect()`-get_mut pattern: **PATCHED-ONLY** (fragile-by-convention; no compile-time or runtime guard enforces the "no clone before init" invariant). Open follow-up (see §5).
- The specific panic: **NEW regression**, introduced by heart-V2 launch-bridge commit `c0a74c9f` (the reconciler block was inserted at line 173 — *before* the init phase — instead of in the post-init spawn phase with the other clones). NOT a V1 bug carried forward; a V2 feature placed without respecting the V1-era init invariant.
- Secondary (non-fatal, separate): cloud DB lacks the `heart_v2_sessions` table (migration not run on the cloud DB) → `load_sessions` logs "no such table … starting with empty sessions" and continues. Handled; tracked as a follow-up (cloud DB migration parity), NOT part of this fix.

## 4. V2-alignment delta
V2 doctrine: heart-V2 background reconcilers belong in the **post-init spawn phase** alongside `spawn_alert_checker`/`spawn_all`/`build_router` (all of which clone `state` *after* exclusive-ownership init is complete). The bug is purely that the heart-V2 reconciler was wired into the wrong phase. Correct alignment = relocate it to the post-init spawn phase. No behavioral change to the reconciler itself (15s periodic loop; no-op while `heart_v2_real_launch` flag is OFF).

## 5. Proposed change (smallest reversible)
**Move the reconciler block (`main.rs:167-182`) to the post-init spawn phase**, immediately before `spawn_alert_checker` (line 276). This restores the invariant "all `Arc::get_mut` init (≤252) completes before any `state.clone()`", so every get_mut (185→252, 454, 461) runs with strong-count == 1. Behavior preserved; reconciler simply starts a few ms later in boot.
- **Rejected alternative:** refactor all 11 `get_mut` sites to interior mutability — large, touches the whole init sequence, higher blast radius. Out of scope (kaizen-minimal).
- **Follow-up trigger (hardening, deferred):** the `.expect()`-get_mut invariant remains undocumented and unguarded — a future clone-before-init would re-trip it. Candidate follow-up: a `debug_assert_eq!(Arc::strong_count(&state), 1)` at the start of the init phase, or a doc-comment marking the "exclusive-ownership init phase" boundary. Logged, not done now.
- **Verification (behavioral, the missing test):** rebuild → run the binary → confirm it boots past line 454 (no exit 101) and `GET :8080/heart/pods` returns non-404. This is the test whose absence let the panic ship.
