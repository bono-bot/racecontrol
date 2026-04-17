---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: 02
subsystem: rc-agent/mesh-key-bootstrap
tags: [option-z, mesh-service-key, rc-agent, cache, boot-resilience, w5-observability]
dependency-graph:
  requires:
    - rc_common::boot_resilience::spawn_periodic_refetch (pattern — not yet wired here)
    - reqwest 0.12 (dep, default features for rc-agent via http-client feature flag)
  provides:
    - crate::mesh_key_cache::MeshKeyCache (Arc<RwLock<Option<String>>>)
    - crate::mesh_key_cache::new_cache() -> MeshKeyCache
    - crate::mesh_key_cache::fetch_from_server(&reqwest::Client, &str, &MeshKeyCache) -> Result<(), reqwest::Error>
    - crate::mesh_key_cache::get_key_or_env(&MeshKeyCache) -> Option<String>
  affects:
    - crates/rc-agent/src/main.rs (mod declaration)
    - crates/rc-agent/Cargo.toml (wiremock dev-dep)
    - Cargo.lock (wiremock transitive deps)
tech-stack:
  added:
    - wiremock 0.6 (dev-dependency, HTTP mock for unit tests)
  patterns:
    - Arc<RwLock<Option<String>>> shared cache (mirrors FeatureFlags pattern)
    - error_for_status() propagates 4xx/5xx to preserve last-known-good via caller
    - serde_json::Value for loose JSON parsing (forward-compat with server schema additions)
    - #[serial_test::serial] for env-var tests to avoid cross-test race
key-files:
  created:
    - crates/rc-agent/src/mesh_key_cache.rs (329 lines, 10 unit tests)
  modified:
    - crates/rc-agent/Cargo.toml (+2 lines — wiremock dev-dep)
    - crates/rc-agent/src/main.rs (+2 lines — mod declaration, feature-gated)
    - Cargo.lock (wiremock transitive deps, +80 lines)
decisions:
  - "Module gated on http-client feature (default on) to match reqwest availability"
  - "mod (not pub mod) in main.rs — rc-agent is binary-only, no visibility difference"
  - "Empty server response treated as authoritative → cache overwrites existing key with None; network error treated as transient → cache preserved"
  - "403/FORBIDDEN gets a distinct warn! log line; other non-2xx log at debug. Cache behavior identical (Err preserves last-known-good via error_for_status)"
  - "get_key_or_env returns Option<String>, None-when-both-empty; matches idiom .unwrap_or_default().is_empty() that existing consumers use"
metrics:
  duration_seconds: 718
  duration_human: "~12m"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 3
  tests_added: 10
  tests_passing: 10
  completed_date: 2026-04-17
---

# Phase 413 Plan 02: rc-agent MeshKeyCache (Option Z data layer) Summary

Created `crates/rc-agent/src/mesh_key_cache.rs` — the Arc<RwLock<Option<String>>> shared cache + `fetch_from_server` HTTP client + `get_key_or_env` helper that Plan 03 will drive via `spawn_periodic_refetch` and Plan 04 will plug into the 3 `std::env::var("RCAGENT_SERVICE_KEY")` consumer sites. Ten unit tests cover all contract paths including a dedicated W5 test for 403-preserves-last-known-good.

## What Shipped

### Module: `crates/rc-agent/src/mesh_key_cache.rs` (329 lines)

**Type:** `pub type MeshKeyCache = Arc<RwLock<Option<String>>>`

**fetch_from_server contract:**

| Server response | Cache behavior | Return |
|-----------------|----------------|--------|
| 200 + `{"mesh_service_key":"abc"}` | Cache becomes `Some("abc")` | `Ok(())` |
| 200 + `{"mesh_service_key":""}` | Cache becomes `None` (overwrites existing) | `Ok(())` |
| 200 + missing field | Cache becomes `None` | `Ok(())` |
| 403 FORBIDDEN | Cache UNCHANGED + `tracing::warn!` | `Err(reqwest::Error)` |
| Other 4xx/5xx | Cache UNCHANGED + `tracing::debug!` | `Err(reqwest::Error)` |
| Network error (connection refused, timeout) | Cache UNCHANGED | `Err(reqwest::Error)` |

URL: `{http_base}/pods/mesh-service-key` (trailing slash on `http_base` tolerated via `trim_end_matches('/')`).

**get_key_or_env semantics:**
- Reads cache first. If `Some(non_empty)` → return that.
- Otherwise reads `std::env::var("RCAGENT_SERVICE_KEY")`. If `Ok(non_empty)` → return that.
- Otherwise → `None`.
- Cache `Some("")` treated as None (caller bails through).

Rationale: minimizes diff in Plan 04. Existing call sites use `.unwrap_or_default().is_empty()`; those will become `.as_deref().unwrap_or("").is_empty()` or cleaner `.is_none()`, preserving the "empty-string == no-key" semantic that consumers already assume.

### W5 Observability Fix

A `tracing::warn!(target: "mesh_key_cache", status = 403, url = %url, …)` fires on 403 responses distinct from the `tracing::debug!` used for other non-2xx. Rationale (from plan): "silent 403 = pod IP removed from allowlist" was the risk in CONTEXT.md. Now that path emits a warn line that operators can grep for in `rc-agent.log` after a pod IP reassignment. Cache behavior unchanged — last-known-good preserved via `error_for_status()?` propagating the Err up to `spawn_periodic_refetch`, which logs `"periodic_refetch failed"` without mutating the cache.

### Test Coverage Matrix (10 tests, all passing)

| # | Test | Covers |
|---|------|--------|
| 1 | `fetch_populates_cache` | 200 + non-empty key → `Some(key)` |
| 2 | `fetch_preserves_last_known_good_on_500` | 500 → Err, cache unchanged |
| 3 | `fetch_403_logs_warn_and_preserves_cache` | **W5** — 403 → Err, cache unchanged, warn fires |
| 4 | `fetch_preserves_last_known_good_on_network_failure` | Connection refused → Err, cache unchanged |
| 5 | `fetch_empty_response_sets_cache_to_none` | 200 + empty on fresh cache → None |
| 6 | `fetch_empty_response_overwrites_existing_key` | 200 + empty on populated cache → None (explicit overwrite) |
| 7 | `get_key_or_env_prefers_cache` | Cache wins over env when both set |
| 8 | `get_key_or_env_falls_back_to_env_when_cache_none` | Env used when cache is None |
| 9 | `get_key_or_env_returns_none_when_both_empty` | Both empty → None |
| 10 | `get_key_or_env_returns_none_when_cache_empty_string` | Cache `Some("")` counted as None |

Tests 7-10 are annotated `#[serial_test::serial]` to avoid env-var race across parallel tokio tests. `unsafe { std::env::set_var(...) }` + `unsafe { std::env::remove_var(...) }` per edition 2024 safety requirement (matches existing pattern in `remote_ops.rs`).

Test HTTP is via `wiremock = "0.6"` dev-dependency (newly added). Chosen over `mockito` to match the plan and because wiremock's async API aligns with tokio-based unit tests.

## Commits

| Task | Commit | Scope |
|------|--------|-------|
| 1 (module + deps) | `45d85c14` | `crates/rc-agent/src/mesh_key_cache.rs` (new, 329 lines), `crates/rc-agent/Cargo.toml` (+wiremock), `Cargo.lock` (transitive deps) |
| 2 (mod declaration) | `85b1968e` | `crates/rc-agent/src/main.rs` (+`mod mesh_key_cache;` gated on `http-client` feature) |

**Commit `45d85c14` collision note:** A parallel 413-01 executor agent committed at approximately the same instant this agent was committing Task 1. The parallel agent's `git commit` absorbed my staged files (`crates/rc-agent/src/mesh_key_cache.rs`, `crates/rc-agent/Cargo.toml`, `Cargo.lock`) along with its own `crates/racecontrol/src/api/mesh_intelligence.rs`. The commit message is branded "feat(413-01): add pods_mesh_service_key handler" but the commit diff also contains 413-02 Task 1 work. No code lost; both plans' contributions are present in the HEAD tree. This is a known hazard of parallel executor agents; documented as Rule 3 deviation below so the ROADMAP/traceability remains honest.

## Verification

```
cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache  →  10 passed; 0 failed
cargo build --release --bin rc-agent                         →  clean (0 errors, 103 pre-existing warnings)
```

Acceptance-criteria spot checks (run against HEAD):

| Check | Expected | Actual |
|-------|----------|--------|
| `wc -l crates/rc-agent/src/mesh_key_cache.rs` | ≥ 120 | 329 |
| `grep -c "fn fetch_from_server"` | 1 | 1 |
| `grep -cE "pub async fn get_key_or_env"` | 1 | 1 |
| `grep -c "pub type MeshKeyCache"` | 1 | 1 |
| `grep -cE "warn!.*(403\|FORBIDDEN\|forbidden)"` | ≥ 1 | 1 |
| `grep -c "mod mesh_key_cache"` in main.rs | 1 | 1 |
| `.unwrap()` count in production code | 0 | 0 |

## Deviations from Plan

### Rule 3 — Blocking issue: rc-agent has no `lib.rs`

**Found during:** Task 2 (add `pub mod mesh_key_cache;` to `crates/rc-agent/src/lib.rs`)
**Issue:** The plan's Task 2 action and frontmatter `files_modified` reference `crates/rc-agent/src/lib.rs`. Running `cargo metadata` confirms `rc-agent-crate` exposes only `bin` + `custom-build` targets — no `lib` target, no `lib.rs` file on disk. All ~65 sibling modules are declared in `main.rs` with plain `mod foo;` (not `pub mod`).
**Fix:** Added `mod mesh_key_cache;` to `crates/rc-agent/src/main.rs` (feature-gated on `http-client` to match reqwest's availability), placed after `mod mesh_gossip;` for alphabetical neighborhood. `pub mod` has no external-visibility effect on a binary-only crate, so the `pub` was dropped to match existing convention. Plan's phrase "`pub mod mesh_key_cache;`" was interpreted as the intent (module publicly reachable from sibling modules via `crate::mesh_key_cache::*`, which it is with either `mod` or `pub mod` inside a binary crate).
**Files modified:** `crates/rc-agent/src/main.rs` (instead of `crates/rc-agent/src/lib.rs`)
**Commit:** `85b1968e` — the commit message documents this deviation explicitly.
**Impact on downstream plans:** Plans 03 and 04 should use `crate::mesh_key_cache::{MeshKeyCache, fetch_from_server, get_key_or_env}` exactly as planned; the module path is identical.

### Rule 3 — Plan's verify commands assume lib target

**Found during:** Task 1 + Task 2 verification
**Issue:** Plan specifies `cargo test -p rc-agent-crate --lib mesh_key_cache` and `cargo build -p rc-agent-crate --lib`. `--lib` flag is only valid for crates with a `lib` target; rc-agent-crate has none.
**Fix:** Ran `cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache` (same filter, correct target) and `cargo build --release --bin rc-agent` (covers the library-like build as there's nothing else to build in rc-agent-crate). Results equivalent to the plan's intent.
**No code change required** — purely a verify-command substitution.

### Rule 3 — Parallel-executor commit collision

**Found during:** Task 1 commit
**Issue:** A parallel 413-01 executor agent's `git commit` staged and committed my Task 1 files alongside its own, producing commit `45d85c14` with mixed provenance. My separate `git commit` for Task 1 succeeded but ended up as an empty/no-op because the files were already committed.
**Fix:** Accepted the merged commit; all Task 1 code is present in HEAD. Documented in commit-table above so traceability is honest. No code lost, no re-commit needed.
**Impact:** ROADMAP plan-completion tracking still works (Task 1's code is in the tree, HEAD contains everything), but anyone reading `git log` for 413-02 will see only one commit (`85b1968e`) authored by this plan; the Task 1 bulk-of-work is in `45d85c14` which is branded 413-01.

### No other deviations

The fetch contract, W5 warn log, test matrix, and semantics of `get_key_or_env` were all implemented exactly as specified in the plan action block.

## Known Stubs

None. This is a pure data-layer plan — all functions have real implementations tested against a mock HTTP server. No placeholder data flows to any UI. The module becomes "live" in Plan 03 (wire-up) and Plan 04 (consumer rewire); until then, it is declared but not yet invoked by the binary. This is intentional per the plan's objective ("no boot-time wire-up yet (Plan 03 does that)").

## What's Next

**Plan 413-03** will:
1. Instantiate `MeshKeyCache` once in `main.rs` (near where `FeatureFlags` is instantiated around line 1595-1617).
2. Pass a clone into `rc_common::boot_resilience::spawn_periodic_refetch` with a 5-minute interval and a closure that calls `mesh_key_cache::fetch_from_server`.
3. Pass another clone into whichever struct aggregates state for the 3 consumer modules.

**Plan 413-04** will rewire the 3 consumer sites:
- `crates/rc-agent/src/ai_debugger.rs:779` — replace `std::env::var("RCAGENT_SERVICE_KEY").unwrap_or_default()` with `mesh_key_cache::get_key_or_env(&cache).await.unwrap_or_default()`.
- `crates/rc-agent/src/remote_ops.rs:165` — same transform for the middleware.
- `crates/rc-agent/src/ws_handler.rs:431` — same transform for the csv_lap_fallback push.

Production env `RCAGENT_SERVICE_KEY` will be unset after Plan 04; Gap 4 (pod HKLM key ≠ server TOML key) becomes unreachable because there's only one source of truth (server TOML → HTTP → cache).

## Self-Check: PASSED

- [x] `crates/rc-agent/src/mesh_key_cache.rs` exists (329 lines, matches the committed blob hash in `45d85c14`)
- [x] Commit `45d85c14` contains `crates/rc-agent/src/mesh_key_cache.rs` (verified via `git show 45d85c14 -- crates/rc-agent/src/mesh_key_cache.rs`)
- [x] Commit `85b1968e` contains `mod mesh_key_cache;` in `main.rs` (verified via `git show 85b1968e`)
- [x] HEAD `cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache` → 10 passed / 0 failed
- [x] HEAD `cargo build --release --bin rc-agent` → clean
- [x] Plan frontmatter `must_haves.truths` W5 requirement satisfied (warn line on 403, debug on other non-2xx, cache preserved)
- [x] Plan `key_links` satisfied: reqwest GET → `/pods/mesh-service-key`; get_key_or_env falls back to `std::env::var("RCAGENT_SERVICE_KEY")`
